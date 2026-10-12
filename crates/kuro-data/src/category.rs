//! Category data access: create order for the manga and anime kinds (a write transaction each) and the
//! [`CategoryRepo`] reads and updates. The preference half (exclude sets, default category, sort string,
//! seed) is `kuro-prefs::category`.

use kuro_platform::kind::Kind;
use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::open::DbResult;
use kuro_db::statements_anime as anime;
use kuro_db::statements_manga::{CategoryPatch, categories as manga};
use kuro_db::subscribe::Subscription;
use rusqlite::Connection;

use crate::{DataResult, ENTRY_DEBOUNCE};

/// `max(order) + 1` over every category, system row included. The system row has order -1, so the
/// first user category gets 0. An empty table also gives 0.
fn next_order(orders: impl Iterator<Item = i64>) -> i64 {
    orders.max().map_or(0, |max| max + 1)
}

/// Creates a manga category at the end of the order and returns its id. `flags` are the sort flags of the
/// global preference, `library_sort::from_serialized(&lib.manga_sorting_mode.get())`, composed by the caller.
pub fn create_manga_category(db: &Db, name: &str, flags: i64) -> DataResult<i64> {
    db.write(|tx| {
        let order = next_order(manga::get_categories(tx)?.iter().map(|c| c.order));
        Ok(manga::insert(tx, name, order, flags)?)
    })
}

/// Creates an anime category at the end of the order and returns its id.
pub fn create_anime_category(db: &Db, name: &str, flags: i64) -> DataResult<i64> {
    db.write(|tx| {
        let order = next_order(anime::categories(tx)?.iter().map(|c| c.order));
        Ok(anime::insert_category(tx, name, order, flags)?)
    })
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
    kind: Kind,
}

/// Tables a category list reads, per kind.
fn tables(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Manga => &["categories", "mangas_categories"],
        Kind::Anime => &["categories", "animes_categories"],
    }
}

/// The column list of every category read. `hidden = 1` makes only the value 1 hidden (EC-30); a row with any
/// other value reads as visible, and never fails the query.
const CATEGORY_COLUMNS: &str = "C._id, C.name, C.sort, C.flags, C.hidden = 1";

/// The SQL of one category read for `kind` and `scope`. The by-entry scopes join the link table; the order
/// is `sort, _id` for every scope, which is what the upstream lists use.
fn read_sql(kind: Kind, scope: Scope) -> String {
    let (link, entry_col) = match kind {
        Kind::Manga => ("mangas_categories", "manga_id"),
        Kind::Anime => ("animes_categories", "anime_id"),
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

fn list(c: &Connection, kind: Kind, scope: Scope) -> DbResult<Vec<Category>> {
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
    pub fn new(db: Arc<Db>, kind: Kind) -> Self {
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
                Kind::Manga => manga::insert(tx, &cat.name, cat.order, cat.flags)?,
                Kind::Anime => anime::insert_category(tx, &cat.name, cat.order, cat.flags)?,
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
                    Kind::Manga => {
                        let patch = CategoryPatch {
                            id: u.id,
                            name: u.name.clone(),
                            order: u.order,
                            flags: u.flags,
                            hidden: u.hidden,
                        };
                        manga::update(tx, &patch)?;
                    }
                    Kind::Anime => {
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
                Kind::Manga => manga::update_all_flags(tx, flags)?,
                Kind::Anime => anime::update_all_category_flags(tx, flags)?,
            };
            Ok(())
        })
    }

    /// Deletes a category; its link rows cascade. The system category (0) cannot be deleted (EC-23).
    pub fn delete(&self, id: i64) -> DataResult<()> {
        let kind = self.kind;
        self.db.write(|tx| {
            match kind {
                Kind::Manga => manga::delete(tx, id)?,
                Kind::Anime => anime::delete_category(tx, id)?,
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
    use crate::tests::manga_db;
    use kuro_db::flags::library_sort;

    fn manga_orders(db: &Db) -> Vec<(i64, String, i64)> {
        db.read(|c| Ok(manga::get_categories(c)?.into_iter().map(|c| (c.id, c.name, c.order)).collect()))
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
        let repo = CategoryRepo::new(db.clone(), Kind::Manga);
        insert_category(&db, Kind::Manga, "B", 1);
        insert_category(&db, Kind::Manga, "A", 0);
        let all = repo.all().unwrap();
        assert_eq!(names(&all), ["", "A", "B"]);
        assert_eq!((all[0].id, all[0].order), (0, -1));
    }

    #[test]
    fn the_anime_kind_lists_its_system_row_first() {
        let (_d, db) = anime_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Anime);
        insert_category(&db, Kind::Anime, "Watching", 0);
        let all = repo.all().unwrap();
        assert_eq!((all[0].id, all[0].name.as_str(), all[0].order), (0, "", -1));
        assert_eq!(names(&all), ["", "Watching"]);
    }

    #[test]
    fn hidden_categories_are_excluded_from_all_visible_and_only_one_hides() {
        let (_d, db) = anime_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Anime);
        let shown = insert_category(&db, Kind::Anime, "Shown", 0);
        let hidden = insert_category(&db, Kind::Anime, "Hidden", 1);
        repo.update_partial(&CategoryUpdate { hidden: Some(true), ..CategoryUpdate { id: hidden, ..Default::default() } })
            .unwrap();
        assert!(names(&repo.all_visible().unwrap()).iter().all(|n| n != "Hidden"));
        assert!(repo.all().unwrap().iter().any(|c| c.id == hidden && c.hidden));
        assert!(repo.get(shown).unwrap().is_some());
    }

    #[test]
    fn hidden_values_other_than_one_read_as_visible() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Manga);
        let id = insert_category(&db, Kind::Manga, "Odd", 0);
        db.write(|tx| Ok(tx.execute("UPDATE categories SET hidden = 2 WHERE _id = ?1", [id]).map(drop)?)).unwrap();
        assert!(repo.all_visible().unwrap().iter().any(|c| c.id == id), "hidden = 2 is visible (EC-30)");
    }

    #[test]
    fn insert_ignores_the_passed_id_and_hidden_and_returns_the_new_id() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Manga);
        let id = repo.insert(&Category { id: 77, name: "New".into(), order: 3, flags: 9, hidden: true }).unwrap();
        assert_ne!(id, 77);
        let got = repo.get(id).unwrap().unwrap();
        assert_eq!((got.name.as_str(), got.order, got.flags, got.hidden), ("New", 3, 9, false));
    }

    #[test]
    fn update_partial_coalesces_and_writes_hidden_as_a_flag() {
        let (_d, db) = anime_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Anime);
        let id = insert_category(&db, Kind::Anime, "Keep", 0);
        repo.update_partial(&CategoryUpdate { id, flags: Some(5), hidden: Some(true), ..Default::default() }).unwrap();
        let got = repo.get(id).unwrap().unwrap();
        assert_eq!((got.name.as_str(), got.flags, got.hidden), ("Keep", 5, true));
    }

    #[test]
    fn update_partials_is_atomic() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Manga);
        let a = insert_category(&db, Kind::Manga, "A", 0);
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
        let repo = CategoryRepo::new(db.clone(), Kind::Manga);
        let a = insert_category(&db, Kind::Manga, "A", 0);
        repo.update_partial(&CategoryUpdate { id: a, flags: Some(4), ..Default::default() }).unwrap();
        repo.update_all_flags(None).unwrap();
        assert_eq!(repo.get(a).unwrap().unwrap().flags, 4);
        repo.update_all_flags(Some(8)).unwrap();
        assert_eq!(repo.get(a).unwrap().unwrap().flags, 8);
    }

    #[test]
    fn deleting_the_system_category_fails_and_deleting_another_cascades_links() {
        let (_d, db) = manga_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Manga);
        let err = repo.delete(0).unwrap_err();
        assert!(err.to_string().contains("System category can't be deleted"), "{err}");

        let m = insert_manga(&db, "m", true);
        let cat = insert_category(&db, Kind::Manga, "Tag", 0);
        db.write(|tx| Ok(tx.execute("INSERT INTO mangas_categories(manga_id, category_id) VALUES (?1, ?2)", [m, cat]).map(drop)?))
            .unwrap();
        assert_eq!(names(&repo.by_entry(m).unwrap()), ["Tag"]);
        repo.delete(cat).unwrap();
        assert!(repo.by_entry(m).unwrap().is_empty());
    }

    #[test]
    fn by_entry_lists_the_entry_categories_in_display_order() {
        let (_d, db) = anime_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Anime);
        let a = insert_anime(&db, "a", true);
        let late = insert_category(&db, Kind::Anime, "Late", 5);
        let early = insert_category(&db, Kind::Anime, "Early", 2);
        let _other = insert_category(&db, Kind::Anime, "Other", 0);
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
        let repo = CategoryRepo::new(db.clone(), Kind::Manga);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut sub = repo.subscribe_all();
            assert_eq!(sub.changed().await.unwrap().unwrap().len(), 1, "the system row only");
            insert_category(&db, Kind::Manga, "New", 0);
            assert_eq!(sub.changed().await.unwrap().unwrap().len(), 2);
        });
    }
}
