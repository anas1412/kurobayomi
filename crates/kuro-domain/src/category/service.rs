//! Category writes for one kind: create, rename, hide, partial update, reorder, delete, entry membership and
//! the sort-mode setters. Every write runs on a detached blocking task, so a closed screen cannot cancel a
//! write that already started (`withNonCancellableContext`). Validation runs inside the same write
//! transaction as the change, so two creates cannot both pass the duplicate check.

use kuro_platform::kind::Kind;
use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::open::DbResult;
use kuro_db::tables::CategoryPatch;
use kuro_db::tables::categories as table;
use kuro_prefs::category::{forget_category, set_random_sort_seed, write_sort_prefs};
use kuro_prefs::library::LibraryPrefs;

use super::model::{Category, CategoryUpdate};
use super::repo::CategoryRepo;
use crate::SYSTEM_CATEGORY_ID;
use crate::error::DomainError;
use crate::library::display_mode::LibraryDisplayMode;
use crate::library::sort::LibrarySort;

/// A validation outcome inside a write transaction. `Err` is a domain rule, `Ok` carries the value.
type Checked<T> = Result<T, DomainError>;

/// Every category of the kind in display order, read inside the write transaction.
fn all_in(tx: &kuro_db::Transaction<'_>, kind: Kind) -> DbResult<Vec<Category>> {
    Ok(table::all(tx, kind)?.into_iter().map(Category::from).collect())
}

/// Trims a name and rejects an empty result.
fn validate_name(name: &str) -> Checked<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() { Err(DomainError::InvalidName) } else { Ok(trimmed.to_owned()) }
}

/// Another category (system and `except` excluded, hidden ones included) has this exact trimmed name.
fn name_taken(all: &[Category], name: &str, except: Option<i64>) -> bool {
    all.iter().any(|c| !c.is_system() && Some(c.id) != except && c.name.trim() == name)
}

/// The writes and the sort-mode setters of one kind.
pub struct CategoryService {
    repo: CategoryRepo,
    prefs: Arc<LibraryPrefs>,
    /// Serialises reorders and deletes of this kind (`ReorderXCategory`'s `Mutex`). Visible to the tests.
    pub(super) reorder_lock: Arc<tokio::sync::Mutex<()>>,
}

impl CategoryService {
    pub fn new(db: Arc<Db>, kind: Kind, prefs: Arc<LibraryPrefs>) -> Self {
        Self { repo: CategoryRepo::new(db, kind), prefs, reorder_lock: Arc::new(tokio::sync::Mutex::new(())) }
    }

    /// The read side (lists, lookups, live queries).
    pub fn repo(&self) -> &CategoryRepo {
        &self.repo
    }

    fn kind(&self) -> Kind {
        self.repo.kind()
    }

    /// The global sort preference of this kind.
    pub fn global_sort(&self) -> LibrarySort {
        let stored = match self.kind() {
            Kind::Manga => self.prefs.manga_sorting_mode.get().0,
            Kind::Anime => self.prefs.anime_sorting_mode.get().0,
        };
        LibrarySort::deserialize(self.kind(), &stored)
    }

    /// `global.type + global.direction`: the flags a new category takes (`initialFlags`).
    pub fn initial_flags(&self) -> i64 {
        self.global_sort().flag()
    }

    /// Runs `f` on a blocking task that is detached from the caller: dropping the returned future does not
    /// stop the write.
    async fn detached<T, F>(&self, f: F) -> Result<T, DomainError>
    where
        F: FnOnce(&Db) -> Result<T, DomainError> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.repo.db().clone();
        let out = tokio::task::spawn_blocking(move || f(&db)).await.map_err(DomainError::Join)?;
        if let Err(e) = &out {
            tracing::error!(error = ?e, "category write failed");
        }
        out
    }

    /// Creates a category at the end of the order. The name is trimmed; it must be non-empty and unused.
    /// Flags are the global sort (`initial_flags`).
    pub async fn create(&self, name: &str) -> Result<Category, DomainError> {
        let name = validate_name(name)?;
        let kind = self.kind();
        let flags = self.initial_flags();
        self.detached(move |db| {
            let out = db.write(|tx| -> DbResult<Checked<Category>> {
                let all = all_in(tx, kind)?;
                if name_taken(&all, &name, None) {
                    return Ok(Err(DomainError::DuplicateName));
                }
                let order = all.iter().map(|c| c.order).max().map_or(0, |max| max + 1);
                let id = table::insert(tx, kind, &name, order, flags)?;
                Ok(Ok(Category { id, name: name.clone(), order, flags, hidden: false }))
            })?;
            out
        })
        .await
    }

    /// Renames a user category. Renaming to the current name is a successful no-op.
    pub async fn rename(&self, id: i64, name: &str) -> Result<(), DomainError> {
        let name = validate_name(name)?;
        self.update_checked(CategoryUpdate { id, name: Some(name), ..Default::default() }).await
    }

    /// Flips `hidden` on a user category.
    pub async fn toggle_hidden(&self, id: i64) -> Result<(), DomainError> {
        let kind = self.kind();
        self.detached(move |db| {
            let out = db.write(|tx| -> DbResult<Checked<()>> {
                let current = table::get(tx, kind, id)?.map(Category::from);
                let Some(current) = current else {
                    return Ok(Err(DomainError::NotFound));
                };
                if current.is_system() {
                    return Ok(Err(DomainError::SystemCategory));
                }
                let hidden = !current.hidden;
                table::update(tx, kind, &CategoryPatch { id, hidden: Some(hidden), ..Default::default() })?;
                Ok(Ok(()))
            })?;
            out
        })
        .await
    }

    /// Generic partial update (`UpdateXCategory`). A name is trimmed and checked like `create`.
    pub async fn update_partial(&self, update: CategoryUpdate) -> Result<(), DomainError> {
        let update = match update.name.as_deref() {
            Some(name) => CategoryUpdate { name: Some(validate_name(name)?), ..update },
            None => update,
        };
        self.update_checked(update).await
    }

    /// Applies several partial updates in one transaction.
    pub async fn update_partial_many(&self, updates: Vec<CategoryUpdate>) -> Result<(), DomainError> {
        let kind = self.kind();
        let mut checked = Vec::with_capacity(updates.len());
        for u in updates {
            checked.push(match u.name.as_deref() {
                Some(name) => CategoryUpdate { name: Some(validate_name(name)?), ..u },
                None => u,
            });
        }
        self.detached(move |db| {
            let out = db.write(|tx| -> DbResult<Checked<()>> {
                for u in &checked {
                    if let Err(e) = apply_update(tx, kind, u)? {
                        return Ok(Err(e));
                    }
                }
                Ok(Ok(()))
            })?;
            out
        })
        .await
    }

    /// One update with the name rule and the system-category rule, in one transaction.
    async fn update_checked(&self, u: CategoryUpdate) -> Result<(), DomainError> {
        let kind = self.kind();
        self.detached(move |db| {
            let out = db.write(|tx| -> DbResult<Checked<()>> { Ok(apply_update(tx, kind, &u)?) })?;
            out
        })
        .await
    }

    /// Moves a user category before `before` (`None` moves it to the end). The moved id and the `before` id
    /// must both be user categories; the result renumbers all user categories `0..n-1`. The system category's
    /// `sort` is never written.
    pub async fn move_before(&self, id: i64, before: Option<i64>) -> Result<(), DomainError> {
        let kind = self.kind();
        let guard = self.reorder_lock.clone().lock_owned().await;
        self.detached(move |db| {
            let _guard = guard;
            let out = db.write(|tx| -> DbResult<Checked<()>> {
                let all = all_in(tx, kind)?;
                let mut ids: Vec<i64> = all.iter().filter(|c| !c.is_system()).map(|c| c.id).collect();
                let Some(from) = ids.iter().position(|&c| c == id) else {
                    return Ok(Err(DomainError::NotFound));
                };
                if before == Some(id) {
                    return Ok(Ok(()));
                }
                let moved = ids.remove(from);
                match before {
                    None => ids.push(moved),
                    Some(target) => {
                        let Some(pos) = ids.iter().position(|&c| c == target) else {
                            return Ok(Err(DomainError::NotFound));
                        };
                        ids.insert(pos, moved);
                    }
                }
                for (order, category) in ids.iter().enumerate() {
                    set_order(tx, kind, *category, order as i64)?;
                }
                Ok(Ok(()))
            })?;
            out
        })
        .await
    }

    /// Deletes a user category. Its links cascade; then every category, system included, is renumbered to its
    /// index in display order in the same transaction (T-08a-010); then the preference sets forget the id.
    /// Takes the reorder lock, so a delete never renumbers under a move in flight (T-08a-009). The system
    /// category cannot be deleted.
    pub async fn delete(&self, id: i64) -> Result<(), DomainError> {
        if id == SYSTEM_CATEGORY_ID {
            return Err(DomainError::SystemCategory);
        }
        let kind = self.kind();
        let guard = self.reorder_lock.clone().lock_owned().await;
        self.detached(move |db| {
            let _guard = guard;
            let out = db.write(|tx| -> DbResult<Checked<()>> {
                table::delete(tx, kind, id)?;
                table::renumber_all(tx, kind)?;
                Ok(Ok(()))
            })?;
            out
        })
        .await?;
        forget_category(self.prefs.store(), &self.prefs, self.kind(), id);
        Ok(())
    }

    /// Replaces the categories of one entry. The Default category (id 0) and ids that do not exist are dropped,
    /// duplicates are removed, and an empty list means "Default". Errors are returned; callers may ignore them.
    pub async fn set_entry_categories(&self, entry_id: i64, ids: &[i64]) -> Result<(), DomainError> {
        let existing: Vec<i64> = self.repo.all()?.into_iter().map(|c| c.id).collect();
        let mut wanted: Vec<i64> = ids
            .iter()
            .copied()
            .filter(|id| *id != SYSTEM_CATEGORY_ID && existing.contains(id))
            .collect();
        wanted.sort_unstable();
        wanted.dedup();
        let db = self.repo.db().clone();
        tokio::task::spawn_blocking(move || kuro_data::entry::EntryRepo::new(db).set_categories(entry_id, &wanted))
            .await
            .map_err(DomainError::Join)?
            .map_err(DomainError::from)
            .inspect_err(|e| tracing::error!(error = ?e, entry_id, "set entry categories failed"))
    }

    /// `SetSortModeForXCategory`. The global sort is written and every category takes the same flags (the settings
    /// diet cut the per-category sort, `categorized_display`). `category` only supplies the flag bits the new sort is
    /// applied to. `draw_seed` is called once, and only for the Random sort; its value becomes the new random seed,
    /// kept in memory (`kuro_prefs::category::random_sort_seed`).
    pub async fn set_sort_mode(
        &self,
        category: Option<i64>,
        sort: LibrarySort,
        draw_seed: impl FnOnce() -> i32,
    ) -> Result<(), DomainError> {
        let kind = self.kind();
        let category = match category {
            Some(id) => self.repo.get(id)?,
            None => None,
        };
        let flags = sort.apply_to(category.as_ref().map_or(0, |c| c.flags));
        if sort.ty == crate::library::sort::SortType::Random {
            set_random_sort_seed(self.kind(), draw_seed());
        }
        let serialized = sort.serialize(kind);
        write_sort_prefs(&self.prefs, self.kind(), &serialized, true);
        let db = self.repo.db().clone();
        tokio::task::spawn_blocking(move || write_all_flags(&db, kind, flags)).await.map_err(DomainError::Join)??;
        Ok(())
    }

    /// `ResetXCategoryFlags`: every category takes the global `type + direction`.
    pub async fn reset_category_flags(&self) -> Result<(), DomainError> {
        let flags = self.global_sort().flag();
        let db = self.repo.db().clone();
        let kind = self.kind();
        tokio::task::spawn_blocking(move || write_all_flags(&db, kind, flags)).await.map_err(DomainError::Join)?
    }
}

/// `SetXDisplayMode`: one global key for both kinds.
pub fn set_display_mode(prefs: &LibraryPrefs, mode: LibraryDisplayMode) {
    prefs.display_mode.set(mode);
}

/// Applies one update inside a write transaction. A `None` field keeps the stored value.
fn apply_update(tx: &kuro_db::Transaction<'_>, kind: Kind, u: &CategoryUpdate) -> DbResult<Checked<()>> {
    let all = all_in(tx, kind)?;
    let Some(current) = all.iter().find(|c| c.id == u.id) else {
        return Ok(Err(DomainError::NotFound));
    };
    if current.is_system() && (u.name.is_some() || u.hidden.is_some()) {
        return Ok(Err(DomainError::SystemCategory));
    }
    if let Some(name) = u.name.as_deref() {
        if name_taken(&all, name, Some(u.id)) {
            return Ok(Err(DomainError::DuplicateName));
        }
    }
    let patch = CategoryPatch { id: u.id, name: u.name.clone(), order: u.order, flags: u.flags, hidden: u.hidden };
    table::update(tx, kind, &patch)?;
    Ok(Ok(()))
}

fn set_order(tx: &kuro_db::Transaction<'_>, kind: Kind, id: i64, order: i64) -> DbResult<()> {
    table::update(tx, kind, &CategoryPatch { id, order: Some(order), ..Default::default() })?;
    Ok(())
}

fn write_all_flags(db: &Db, kind: Kind, flags: i64) -> Result<(), DomainError> {
    db.write(|tx| {
        table::update_all_flags(tx, kind, Some(flags))?;
        Ok(())
    })
    .map_err(DomainError::from)
}
