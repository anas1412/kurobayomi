//! Category rules at the data level (T-05a-025): create order, delete renumbering, reorder and the database
//! half of a sort-mode change, for the manga and anime kinds. Each rule runs in one write transaction, so
//! the writer lock also serialises the read-modify-write of a reorder (the Kotlin `Mutex`). The preference
//! half (exclude sets, default category, sort string, seed) is `kuro-prefs::category`. The interactor
//! wrappers are not here (area 08).
//!
//! Renumbering follows area 08 (T-08a-010): a delete renumbers every category, system included, to its
//! index in display order (system row 0); a reorder renumbers only the user categories `0, 1, 2, ...`
//! and never writes the system row.

use std::sync::Arc;

use kuro_db::flags::library_sort;
use kuro_db::handle::Db;
use kuro_db::open::{DbError, DbResult};
use kuro_db::statements_anime as anime;
use kuro_db::statements_manga::{CategoryPatch, categories as manga};
use kuro_db::subscribe::Subscription;
use rusqlite::Connection;

use crate::{DataResult, ENTRY_DEBOUNCE};

/// Result of a reorder, as `ReorderMangaCategory.Result` reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reorder {
    Success,
    /// The id is not a user category, so nothing was written.
    Unchanged,
}

/// `max(order) + 1` over every category, system row included. The system row has order -1, so the
/// first user category gets 0. An empty table also gives 0.
fn next_order(orders: impl Iterator<Item = i64>) -> i64 {
    orders.max().map_or(0, |max| max + 1)
}

/// The position check of a reorder. `Unchanged` when `id` is not a user category; an error when
/// `new_index` is past the end. Returns the user ids with `id` moved to `new_index`.
fn move_to_index(mut user_ids: Vec<i64>, id: i64, new_index: usize) -> Result<Option<Vec<i64>>, DbError> {
    let Some(from) = user_ids.iter().position(|&c| c == id) else {
        return Ok(None);
    };
    if new_index >= user_ids.len() {
        return Err(DbError::Constraint(format!(
            "category index {new_index} is out of range 0..{}",
            user_ids.len()
        )));
    }
    let item = user_ids.remove(from);
    user_ids.insert(new_index, item);
    Ok(Some(user_ids))
}

/// Creates a manga category at the end of the order and returns its id. `flags` are the sort flags of the
/// global preference, `library_sort::from_serialized(&lib.manga_sorting_mode.get())`, composed by the caller.
pub fn create_manga_category(db: &Db, name: &str, flags: i64) -> DataResult<i64> {
    db.write(|tx| {
        let order = next_order(manga::get_categories(tx)?.iter().map(|c| c.order));
        Ok(manga::insert(tx, name, order, flags)?)
    })
}

/// Deletes a manga category (its links go with it) and renumbers every category, system included. The
/// system category cannot be deleted (EC-23): the trigger aborts the transaction.
pub fn delete_manga_category(db: &Db, id: i64) -> DataResult<()> {
    db.write(|tx| {
        manga::delete(tx, id)?;
        manga::renumber_all(tx)?;
        Ok(())
    })
}

/// Moves a manga user category to `new_index` among the user categories and renumbers them from 0.
pub fn reorder_manga_category(db: &Db, id: i64, new_index: usize) -> DataResult<Reorder> {
    db.write(|tx| {
        let user_ids: Vec<i64> = manga::get_categories(tx)?.iter().map(|c| c.id).filter(|&c| c != 0).collect();
        let Some(ids) = move_to_index(user_ids, id, new_index)? else {
            return Ok(Reorder::Unchanged);
        };
        for (order, category) in ids.iter().enumerate() {
            manga::update(tx, &CategoryPatch { id: *category, order: Some(order as i64), ..Default::default() })?;
        }
        Ok(Reorder::Success)
    })
}

/// Creates an anime category at the end of the order and returns its id.
pub fn create_anime_category(db: &Db, name: &str, flags: i64) -> DataResult<i64> {
    db.write(|tx| {
        let order = next_order(anime::categories(tx)?.iter().map(|c| c.order));
        Ok(anime::insert_category(tx, name, order, flags)?)
    })
}

/// Deletes an anime category (its links go with it) and renumbers every category, system included.
pub fn delete_anime_category(db: &Db, id: i64) -> DataResult<()> {
    db.write(|tx| {
        anime::delete_category(tx, id)?;
        let ids: Vec<i64> = anime::categories(tx)?.iter().map(|c| c.id).collect();
        for (order, category) in ids.iter().enumerate() {
            set_anime_order(tx, *category, order as i64)?;
        }
        Ok(())
    })
}

/// Moves an anime user category to `new_index` among the user categories and renumbers them from 0.
pub fn reorder_anime_category(db: &Db, id: i64, new_index: usize) -> DataResult<Reorder> {
    db.write(|tx| {
        let user_ids: Vec<i64> = anime::categories(tx)?.iter().map(|c| c.id).filter(|&c| c != 0).collect();
        let Some(ids) = move_to_index(user_ids, id, new_index)? else {
            return Ok(Reorder::Unchanged);
        };
        for (order, category) in ids.iter().enumerate() {
            set_anime_order(tx, *category, order as i64)?;
        }
        Ok(Reorder::Success)
    })
}

/// `SetSortModeForMangaCategory`, database half. With `categorized` display on and a `category`, only that
/// category changes. Otherwise every category takes the flags of `category` (or of nothing), with the new
/// sort. Only the sort bits move (`library_sort::with_sort`).
pub fn set_manga_sort(db: &Db, category: Option<i64>, categorized: bool, sort_type: i64, ascending: bool) -> DataResult<()> {
    db.write(|tx| {
        let rows = manga::get_categories(tx)?;
        let base = category.and_then(|id| rows.iter().find(|c| c.id == id)).map_or(0, |c| c.flags);
        let flags = library_sort::with_sort(base, sort_type, ascending);
        match category {
            Some(id) if categorized => {
                manga::update(tx, &CategoryPatch { id, flags: Some(flags), ..Default::default() })?;
            }
            _ => {
                manga::update_all_flags(tx, Some(flags))?;
            }
        }
        Ok(())
    })
}

/// `SetSortModeForAnimeCategory`, database half. The rule is the same as [`set_manga_sort`].
pub fn set_anime_sort(db: &Db, category: Option<i64>, categorized: bool, sort_type: i64, ascending: bool) -> DataResult<()> {
    db.write(|tx| {
        let rows = anime::categories(tx)?;
        let base = category.and_then(|id| rows.iter().find(|c| c.id == id)).map_or(0, |c| c.flags);
        let flags = library_sort::with_sort(base, sort_type, ascending);
        match category {
            Some(id) if categorized => {
                anime::update_category(tx, id, None, None, Some(flags), None)?;
            }
            _ => {
                anime::update_all_category_flags(tx, Some(flags))?;
            }
        }
        Ok(())
    })
}

fn set_anime_order(tx: &rusqlite::Transaction<'_>, id: i64, order: i64) -> rusqlite::Result<()> {
    tx.execute("UPDATE categories SET sort = ?1 WHERE _id = ?2", (order, id))?;
    Ok(())
}

/// Which category table a [`CategoryRepo`] reads and writes: the manga or the anime categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CategoryKind {
    Manga,
    Anime,
}

/// One `categories` row. `order` is the `sort` column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub order: i64,
    pub flags: i64,
    pub hidden: bool,
}

/// Partial category update. `None` keeps the stored value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CategoryUpdate {
    pub id: i64,
    pub name: Option<String>,
    pub order: Option<i64>,
    pub flags: Option<i64>,
    pub hidden: Option<bool>,
}

/// Which rows a category read selects.
#[derive(Clone, Copy)]
enum Scope {
    All,
    Visible,
    ByEntry(i64),
    VisibleByEntry(i64),
}

/// Category access for one kind (`MangaCategoryRepositoryImpl.kt`, `AnimeCategoryRepositoryImpl.kt`).
#[derive(Clone)]
pub struct CategoryRepo {
    db: Arc<Db>,
    kind: CategoryKind,
}

/// Tables a category list reads, per kind.
fn tables(kind: CategoryKind) -> &'static [&'static str] {
    match kind {
        CategoryKind::Manga => &["categories", "mangas_categories"],
        CategoryKind::Anime => &["categories", "animes_categories"],
    }
}

/// The column list of every category read. `hidden = 1` makes only the value 1 hidden (EC-30); a row with any
/// other value reads as visible, and never fails the query.
const CATEGORY_COLUMNS: &str = "C._id, C.name, C.sort, C.flags, C.hidden = 1";

/// The SQL of one category read for `kind` and `scope`. The by-entry scopes join the link table; the order
/// is `sort, _id` for every scope, which is what the upstream lists use.
fn read_sql(kind: CategoryKind, scope: Scope) -> String {
    let (link, entry_col) = match kind {
        CategoryKind::Manga => ("mangas_categories", "manga_id"),
        CategoryKind::Anime => ("animes_categories", "anime_id"),
    };
    let visible = "C.hidden != 1";
    let (join, filter) = match scope {
        Scope::All => (String::new(), String::new()),
        Scope::Visible => (String::new(), format!(" WHERE {visible}")),
        Scope::ByEntry(_) => (
            format!(" JOIN {link} L ON C._id = L.category_id"),
            format!(" WHERE L.{entry_col} = :entry"),
        ),
        Scope::VisibleByEntry(_) => (
            format!(" JOIN {link} L ON C._id = L.category_id"),
            format!(" WHERE L.{entry_col} = :entry AND {visible}"),
        ),
    };
    format!("SELECT {CATEGORY_COLUMNS} FROM categories C{join}{filter} ORDER BY C.sort, C._id")
}

fn category_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category { id: r.get(0)?, name: r.get(1)?, order: r.get(2)?, flags: r.get(3)?, hidden: r.get(4)? })
}

fn list(c: &Connection, kind: CategoryKind, scope: Scope) -> DbResult<Vec<Category>> {
    let sql = read_sql(kind, scope);
    let mut st = c.prepare_cached(&sql)?;
    let rows = match scope {
        Scope::ByEntry(entry) | Scope::VisibleByEntry(entry) => {
            st.query_map(rusqlite::named_params! {":entry": entry}, category_from_row)?
        }
        Scope::All | Scope::Visible => st.query_map([], category_from_row)?,
    };
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

impl CategoryRepo {
    pub fn new(db: Arc<Db>, kind: CategoryKind) -> Self {
        Self { db, kind }
    }

    /// The category with `id`, or `None`.
    pub fn get(&self, id: i64) -> DataResult<Option<Category>> {
        let sql = format!(
            "SELECT {CATEGORY_COLUMNS} FROM categories C WHERE C._id = :id"
        );
        let found = self.db.read(|c| {
            let mut st = c.prepare_cached(&sql)?;
            Ok(st.query_map(rusqlite::named_params! {":id": id}, category_from_row)?.next().transpose()?)
        })?;
        Ok(found)
    }

    /// Every category, the system row (id 0, name "", order -1) first. Ordered `sort, _id` (EC-10).
    pub fn all(&self) -> DataResult<Vec<Category>> {
        self.read(Scope::All)
    }

    /// Every category except the hidden ones. Only `hidden = 1` hides a row (EC-30).
    pub fn all_visible(&self) -> DataResult<Vec<Category>> {
        self.read(Scope::Visible)
    }

    pub fn subscribe_all(&self) -> Subscription<Vec<Category>> {
        self.subscribe(Scope::All)
    }

    pub fn subscribe_all_visible(&self) -> Subscription<Vec<Category>> {
        self.subscribe(Scope::Visible)
    }

    /// The categories an entry belongs to, in display order.
    pub fn by_entry(&self, entry_id: i64) -> DataResult<Vec<Category>> {
        self.read(Scope::ByEntry(entry_id))
    }

    pub fn visible_by_entry(&self, entry_id: i64) -> DataResult<Vec<Category>> {
        self.read(Scope::VisibleByEntry(entry_id))
    }

    pub fn subscribe_by_entry(&self, entry_id: i64) -> Subscription<Vec<Category>> {
        self.subscribe(Scope::ByEntry(entry_id))
    }

    pub fn subscribe_visible_by_entry(&self, entry_id: i64) -> Subscription<Vec<Category>> {
        self.subscribe(Scope::VisibleByEntry(entry_id))
    }

    /// Creates a category and returns its id. The passed `id` and `hidden` are ignored.
    pub fn insert(&self, cat: &Category) -> DataResult<i64> {
        let kind = self.kind;
        self.db.write(|tx| {
            Ok(match kind {
                CategoryKind::Manga => manga::insert(tx, &cat.name, cat.order, cat.flags)?,
                CategoryKind::Anime => anime::insert_category(tx, &cat.name, cat.order, cat.flags)?,
            })
        })
    }

    /// Coalesce update of one category.
    pub fn update_partial(&self, u: &CategoryUpdate) -> DataResult<()> {
        self.update_partials(std::slice::from_ref(u))
    }

    /// Every update in one transaction: one failure rolls back all of them.
    pub fn update_partials(&self, updates: &[CategoryUpdate]) -> DataResult<()> {
        let kind = self.kind;
        self.db.write(|tx| {
            for u in updates {
                match kind {
                    CategoryKind::Manga => {
                        let patch = CategoryPatch {
                            id: u.id,
                            name: u.name.clone(),
                            order: u.order,
                            flags: u.flags,
                            hidden: u.hidden,
                        };
                        manga::update(tx, &patch)?;
                    }
                    CategoryKind::Anime => {
                        anime::update_category(tx, u.id, u.name.as_deref(), u.order, u.flags, u.hidden)?;
                    }
                }
            }
            Ok(())
        })
    }

    /// Sets `flags` on every category, the system row included. `None` changes nothing.
    pub fn update_all_flags(&self, flags: Option<i64>) -> DataResult<()> {
        let kind = self.kind;
        self.db.write(|tx| {
            match kind {
                CategoryKind::Manga => manga::update_all_flags(tx, flags)?,
                CategoryKind::Anime => anime::update_all_category_flags(tx, flags)?,
            };
            Ok(())
        })
    }

    /// Deletes a category; its link rows cascade. The system category (0) cannot be deleted (EC-23).
    pub fn delete(&self, id: i64) -> DataResult<()> {
        let kind = self.kind;
        self.db.write(|tx| {
            match kind {
                CategoryKind::Manga => manga::delete(tx, id)?,
                CategoryKind::Anime => anime::delete_category(tx, id)?,
            };
            Ok(())
        })
    }

    fn read(&self, scope: Scope) -> DataResult<Vec<Category>> {
        let kind = self.kind;
        self.db.read(|c| list(c, kind, scope))
    }

    fn subscribe(&self, scope: Scope) -> Subscription<Vec<Category>> {
        let kind = self.kind;
        self.db.subscribe(tables(kind), ENTRY_DEBOUNCE, move |c| list(c, kind, scope))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{anime_db, manga_db};

    fn manga_orders(db: &Db) -> Vec<(i64, String, i64)> {
        db.read(|c| Ok(manga::get_categories(c)?.into_iter().map(|c| (c.id, c.name, c.order)).collect()))
            .unwrap()
    }

    fn anime_orders(db: &Db) -> Vec<(i64, String, i64)> {
        db.read(|c| Ok(anime::categories(c)?.into_iter().map(|c| (c.id, c.name, c.order)).collect()))
            .unwrap()
    }

    #[test]
    fn create_gives_the_next_order_and_the_first_user_category_zero() {
        let (_d, db) = manga_db();
        let a = create_manga_category(&db, "A", 0).unwrap();
        let b = create_manga_category(&db, "B", 0).unwrap();
        let orders: Vec<(i64, i64)> = manga_orders(&db).into_iter().map(|(id, _, o)| (id, o)).collect();
        assert_eq!(orders, vec![(0, -1), (a, 0), (b, 1)]);
    }

    #[test]
    fn delete_renumbers_every_category_including_the_system_row() {
        let (_d, db) = manga_db();
        let a = create_manga_category(&db, "A", 0).unwrap();
        let b = create_manga_category(&db, "B", 0).unwrap();
        let c = create_manga_category(&db, "C", 0).unwrap();
        delete_manga_category(&db, a).unwrap();
        let orders = manga_orders(&db);
        assert_eq!(orders.iter().map(|(id, ..)| *id).collect::<Vec<_>>(), vec![0, b, c]);
        assert_eq!(orders.iter().map(|(.., o)| *o).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert!(delete_manga_category(&db, 0).is_err(), "the system category cannot be deleted");
    }

    #[test]
    fn delete_removes_the_category_links_of_the_deleted_category() {
        let (_d, db) = manga_db();
        let repo = crate::manga::MangaRepo::new(db.clone());
        let cat = create_manga_category(&db, "A", 0).unwrap();
        let id = repo
            .insert(&crate::manga::Manga { favorite: true, source: 1, url: "u".into(), title: "T".into(), ..crate::manga::Manga::create() })
            .unwrap();
        repo.set_categories(id, &[cat]).unwrap();
        delete_manga_category(&db, cat).unwrap();
        let links: i64 = db
            .read(|c| Ok(c.query_row("SELECT count(*) FROM mangas_categories WHERE category_id = ?1", [cat], |r| r.get(0))?))
            .unwrap();
        assert_eq!(links, 0);
    }

    #[test]
    fn reorder_moves_renumbers_ignores_the_system_row_and_rejects_bad_indexes() {
        let (_d, db) = manga_db();
        let a = create_manga_category(&db, "A", 0).unwrap();
        let b = create_manga_category(&db, "B", 0).unwrap();
        let c = create_manga_category(&db, "C", 0).unwrap();
        assert_eq!(reorder_manga_category(&db, c, 0).unwrap(), Reorder::Success);
        assert_eq!(manga_orders(&db).iter().map(|(id, ..)| *id).collect::<Vec<_>>(), vec![0, c, a, b]);
        assert_eq!(reorder_manga_category(&db, 9_999, 0).unwrap(), Reorder::Unchanged);
        assert_eq!(reorder_manga_category(&db, 0, 0).unwrap(), Reorder::Unchanged);
        assert!(reorder_manga_category(&db, a, 3).is_err());
    }

    #[test]
    fn anime_categories_follow_the_same_create_delete_and_reorder_rules() {
        let (_d, db) = anime_db();
        let a = create_anime_category(&db, "A", 0).unwrap();
        let b = create_anime_category(&db, "B", 0).unwrap();
        let c = create_anime_category(&db, "C", 0).unwrap();
        assert_eq!(reorder_anime_category(&db, c, 0).unwrap(), Reorder::Success);
        assert_eq!(anime_orders(&db).iter().map(|(id, ..)| *id).collect::<Vec<_>>(), vec![0, c, a, b]);
        delete_anime_category(&db, c).unwrap();
        assert_eq!(anime_orders(&db).iter().map(|(.., o)| *o).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert!(delete_anime_category(&db, 0).is_err());
    }

    #[test]
    fn ec10_a_reorder_after_a_delete_keeps_the_system_row_first() {
        let (_d, db) = manga_db();
        let a = create_manga_category(&db, "A", 0).unwrap();
        let b = create_manga_category(&db, "B", 0).unwrap();
        let c = create_manga_category(&db, "C", 0).unwrap();
        delete_manga_category(&db, b).unwrap();
        assert_eq!(manga_orders(&db), vec![(0, String::new(), 0), (a, "A".into(), 1), (c, "C".into(), 2)]);

        assert_eq!(reorder_manga_category(&db, c, 0).unwrap(), Reorder::Success);
        assert_eq!(manga_orders(&db).iter().map(|(id, ..)| *id).collect::<Vec<_>>(), vec![0, c, a]);
    }

    #[test]
    fn created_categories_take_the_sort_flags_of_the_global_preference() {
        let (_d, db) = manga_db();
        let flags = library_sort::from_serialized("LAST_MANGA_UPDATE,DESCENDING");
        let id = create_manga_category(&db, "A", flags).unwrap();
        let stored = manga_flags(&db, id);
        assert_eq!(stored, library_sort::encode(library_sort::LAST_UPDATE, false));
    }

    fn manga_flags(db: &Db, id: i64) -> i64 {
        db.read(|c| Ok(manga::get_categories(c)?.into_iter().find(|r| r.id == id).unwrap().flags)).unwrap()
    }

    #[test]
    fn a_manga_sort_change_with_categorized_display_touches_only_that_category() {
        let (_d, db) = manga_db();
        let a = create_manga_category(&db, "A", library_sort::encode(library_sort::TRACKER_MEAN, true) | 0b1).unwrap();
        let b = create_manga_category(&db, "B", 0).unwrap();
        set_manga_sort(&db, Some(a), true, library_sort::RANDOM, false).unwrap();
        assert_eq!(manga_flags(&db, a), library_sort::encode(library_sort::RANDOM, false) | 0b1, "other bits kept");
        assert_eq!(manga_flags(&db, b), 0);
    }

    #[test]
    fn a_manga_sort_change_without_categorized_display_sets_every_category() {
        let (_d, db) = manga_db();
        let a = create_manga_category(&db, "A", 0).unwrap();
        let b = create_manga_category(&db, "B", 0).unwrap();
        set_manga_sort(&db, None, false, library_sort::DATE_ADDED, true).unwrap();
        let expected = library_sort::encode(library_sort::DATE_ADDED, true);
        assert_eq!((manga_flags(&db, a), manga_flags(&db, b)), (expected, expected));
        assert_eq!(manga_flags(&db, 0), expected, "the system row takes the sort too");
    }

    #[test]
    fn an_anime_sort_change_follows_the_same_rules() {
        let (_d, db) = anime_db();
        let a = create_anime_category(&db, "A", 0).unwrap();
        let b = create_anime_category(&db, "B", 0).unwrap();
        set_anime_sort(&db, Some(a), true, library_sort::UNREAD_COUNT, true).unwrap();
        let flags_of = |id: i64| anime_orders_flags(&db).into_iter().find(|(i, _)| *i == id).unwrap().1;
        assert_eq!(flags_of(a), library_sort::encode(library_sort::UNREAD_COUNT, true));
        assert_eq!(flags_of(b), 0);
        set_anime_sort(&db, None, false, library_sort::RANDOM, false).unwrap();
        assert!(anime_orders_flags(&db).iter().all(|(_, f)| *f == library_sort::encode(library_sort::RANDOM, false)));
    }

    fn anime_orders_flags(db: &Db) -> Vec<(i64, i64)> {
        db.read(|c| Ok(anime::categories(c)?.into_iter().map(|c| (c.id, c.flags)).collect())).unwrap()
    }
}

#[cfg(test)]
mod repo_tests {
    use super::*;
    use crate::support::{anime_db, insert_anime, insert_category, insert_manga, manga_db};

    fn names(list: &[Category]) -> Vec<String> {
        list.iter().map(|c| c.name.clone()).collect()
    }

    #[test]
    fn all_starts_with_the_system_row_and_orders_by_sort_then_id() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Manga);
        insert_category(&db, CategoryKind::Manga, "B", 1);
        insert_category(&db, CategoryKind::Manga, "A", 0);
        let all = repo.all().unwrap();
        assert_eq!(names(&all), ["", "A", "B"]);
        assert_eq!((all[0].id, all[0].order), (0, -1));
    }

    #[test]
    fn the_anime_kind_lists_its_system_row_first() {
        let (_d, db) = anime_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Anime);
        insert_category(&db, CategoryKind::Anime, "Watching", 0);
        let all = repo.all().unwrap();
        assert_eq!((all[0].id, all[0].name.as_str(), all[0].order), (0, "", -1));
        assert_eq!(names(&all), ["", "Watching"]);
    }

    #[test]
    fn hidden_categories_are_excluded_from_all_visible_and_only_one_hides() {
        let (_d, db) = anime_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Anime);
        let shown = insert_category(&db, CategoryKind::Anime, "Shown", 0);
        let hidden = insert_category(&db, CategoryKind::Anime, "Hidden", 1);
        repo.update_partial(&CategoryUpdate { hidden: Some(true), ..CategoryUpdate { id: hidden, ..Default::default() } })
            .unwrap();
        assert!(names(&repo.all_visible().unwrap()).iter().all(|n| n != "Hidden"));
        assert!(repo.all().unwrap().iter().any(|c| c.id == hidden && c.hidden));
        assert!(repo.get(shown).unwrap().is_some());
    }

    #[test]
    fn hidden_values_other_than_one_read_as_visible() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Manga);
        let id = insert_category(&db, CategoryKind::Manga, "Odd", 0);
        db.write(|tx| Ok(tx.execute("UPDATE categories SET hidden = 2 WHERE _id = ?1", [id]).map(drop)?)).unwrap();
        assert!(repo.all_visible().unwrap().iter().any(|c| c.id == id), "hidden = 2 is visible (EC-30)");
    }

    #[test]
    fn insert_ignores_the_passed_id_and_hidden_and_returns_the_new_id() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Manga);
        let id = repo.insert(&Category { id: 77, name: "New".into(), order: 3, flags: 9, hidden: true }).unwrap();
        assert_ne!(id, 77);
        let got = repo.get(id).unwrap().unwrap();
        assert_eq!((got.name.as_str(), got.order, got.flags, got.hidden), ("New", 3, 9, false));
    }

    #[test]
    fn update_partial_coalesces_and_writes_hidden_as_a_flag() {
        let (_d, db) = anime_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Anime);
        let id = insert_category(&db, CategoryKind::Anime, "Keep", 0);
        repo.update_partial(&CategoryUpdate { id, flags: Some(5), hidden: Some(true), ..Default::default() }).unwrap();
        let got = repo.get(id).unwrap().unwrap();
        assert_eq!((got.name.as_str(), got.flags, got.hidden), ("Keep", 5, true));
    }

    #[test]
    fn update_partials_is_atomic() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Manga);
        let a = insert_category(&db, CategoryKind::Manga, "A", 0);
        db.write(|tx| {
            Ok(tx.execute_batch(
                "CREATE TRIGGER refuse_boom BEFORE UPDATE ON categories WHEN new.name = 'boom' \
                 BEGIN SELECT RAISE(ABORT, 'boom'); END;",
            )?)
        })
        .unwrap();
        let failing = repo.update_partials(&[
            CategoryUpdate { id: a, name: Some("Renamed".into()), ..Default::default() },
            CategoryUpdate { id: a, name: Some("boom".into()), ..Default::default() },
        ]);
        assert!(failing.is_err());
        assert_eq!(repo.get(a).unwrap().unwrap().name, "A", "the first update rolled back with the second");
        repo.update_partials(&[CategoryUpdate { id: a, name: Some("Ok".into()), ..Default::default() }]).unwrap();
        assert_eq!(repo.get(a).unwrap().unwrap().name, "Ok");
    }

    #[test]
    fn update_all_flags_with_none_changes_nothing() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Manga);
        let a = insert_category(&db, CategoryKind::Manga, "A", 0);
        repo.update_partial(&CategoryUpdate { id: a, flags: Some(4), ..Default::default() }).unwrap();
        repo.update_all_flags(None).unwrap();
        assert_eq!(repo.get(a).unwrap().unwrap().flags, 4);
        repo.update_all_flags(Some(8)).unwrap();
        assert_eq!(repo.get(a).unwrap().unwrap().flags, 8);
    }

    #[test]
    fn deleting_the_system_category_fails_and_deleting_another_cascades_links() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Manga);
        let err = repo.delete(0).unwrap_err();
        assert!(err.to_string().contains("System category can't be deleted"), "{err}");

        let m = insert_manga(&db, "m", true);
        let cat = insert_category(&db, CategoryKind::Manga, "Tag", 0);
        db.write(|tx| Ok(tx.execute("INSERT INTO mangas_categories(manga_id, category_id) VALUES (?1, ?2)", [m, cat]).map(drop)?))
            .unwrap();
        assert_eq!(names(&repo.by_entry(m).unwrap()), ["Tag"]);
        repo.delete(cat).unwrap();
        assert!(repo.by_entry(m).unwrap().is_empty());
    }

    #[test]
    fn by_entry_lists_the_entry_categories_in_display_order() {
        let (_d, db) = anime_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Anime);
        let a = insert_anime(&db, "a", true);
        let late = insert_category(&db, CategoryKind::Anime, "Late", 5);
        let early = insert_category(&db, CategoryKind::Anime, "Early", 2);
        let _other = insert_category(&db, CategoryKind::Anime, "Other", 0);
        db.write(|tx| {
            for cat in [late, early] {
                tx.execute("INSERT INTO animes_categories(anime_id, category_id) VALUES (?1, ?2)", [a, cat])?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(names(&repo.by_entry(a).unwrap()), ["Early", "Late"]);
    }

    #[test]
    fn a_subscription_re_emits_after_an_insert() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), CategoryKind::Manga);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut sub = repo.subscribe_all();
            assert_eq!(sub.changed().await.unwrap().unwrap().len(), 1, "the system row only");
            insert_category(&db, CategoryKind::Manga, "New", 0);
            assert_eq!(sub.changed().await.unwrap().unwrap().len(), 2);
        });
    }
}
