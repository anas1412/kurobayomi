//! Category reads over `kuro_db::tables::categories`: one read-only [`CategoryRepo`] per kind. The writes are the
//! `CategoryService` of `kuro-domain`; the two `create_*_category` helpers below stay for the callers that still
//! build fixtures with them (phase E deletes them). The preference half (exclude sets, default category, sort string,
//! seed) is `kuro-prefs::category`.

use std::sync::Arc;
use std::time::Duration;

use kuro_db::handle::Db;
use kuro_db::open::DbResult;
use kuro_db::subscribe::Subscription;
use kuro_db::tables::categories as t;
use kuro_db::tables::CategoryRow;
use kuro_platform::kind::Kind;
use rusqlite::Connection;

use crate::DataResult;

/// Quiet time before a category subscription re-queries (the category service's value).
const CATEGORY_DEBOUNCE: Duration = Duration::from_millis(20);

/// `max(order) + 1` over every category, system row included. The system row has order -1, so the
/// first user category gets 0. An empty table also gives 0.
fn next_order(orders: impl Iterator<Item = i64>) -> i64 {
    orders.max().map_or(0, |max| max + 1)
}

/// Creates a category of `kind` at the end of the order and returns its id. `flags` are the sort flags of the global
/// preference, composed by the caller.
fn create_category(db: &Db, kind: Kind, name: &str, flags: i64) -> DataResult<i64> {
    db.write(|tx| {
        let order = next_order(t::all(tx, kind)?.iter().map(|c| c.order));
        Ok(t::insert(tx, kind, name, order, flags)?)
    })
}

/// Creates a manga category at the end of the order and returns its id.
pub fn create_manga_category(db: &Db, name: &str, flags: i64) -> DataResult<i64> {
    create_category(db, Kind::Manga, name, flags)
}

/// Creates an anime category at the end of the order and returns its id.
pub fn create_anime_category(db: &Db, name: &str, flags: i64) -> DataResult<i64> {
    create_category(db, Kind::Anime, name, flags)
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

impl From<CategoryRow> for Category {
    fn from(r: CategoryRow) -> Self {
        Self { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden }
    }
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

/// Category reads for one kind, ordered `sort, _id` in every scope.
#[derive(Clone)]
pub struct CategoryRepo {
    db: Arc<Db>,
    kind: Kind,
}

/// Tables a category list reads.
const CATEGORY_TABLES: &[&str] = &["categories", "entry_categories"];

fn list(c: &Connection, kind: Kind, scope: Scope) -> DbResult<Vec<Category>> {
    let rows = match scope {
        Scope::All => t::all(c, kind),
        Scope::Visible => t::visible(c, kind),
        Scope::ByEntry(entry) => t::by_entry(c, kind, entry),
        Scope::VisibleByEntry(entry) => t::visible_by_entry(c, kind, entry),
    }?;
    Ok(rows.into_iter().map(Category::from).collect())
}

impl CategoryRepo {
    pub fn new(db: Arc<Db>, kind: Kind) -> Self {
        Self { db, kind }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The category with `id`, or `None`.
    pub fn get(&self, id: i64) -> DataResult<Option<Category>> {
        let kind = self.kind;
        Ok(self.db.read(|c| Ok(t::get(c, kind, id)?))?.map(Category::from))
    }

    /// Every category, the system row (id 0, name "", order -1) first. Ordered `sort, _id`.
    pub fn all(&self) -> DataResult<Vec<Category>> {
        self.read(Scope::All)
    }

    /// Every category except the hidden ones.
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

    /// Creates a category and returns its id. The passed `id` and `hidden` are ignored. The writes are the
    /// `CategoryService`'s; this stays only for the callers that build fixtures with it (phase E deletes it).
    pub fn insert(&self, cat: &Category) -> DataResult<i64> {
        let kind = self.kind;
        self.db.write(|tx| Ok(t::insert(tx, kind, &cat.name, cat.order, cat.flags)?))
    }

    fn read(&self, scope: Scope) -> DataResult<Vec<Category>> {
        let kind = self.kind;
        self.db.read(|c| list(c, kind, scope))
    }

    fn subscribe(&self, scope: Scope) -> Subscription<Vec<Category>> {
        let kind = self.kind;
        self.db.subscribe(CATEGORY_TABLES, CATEGORY_DEBOUNCE, move |c| list(c, kind, scope))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_db;
    use kuro_db::flags::library_sort;

    fn manga_orders(db: &Db) -> Vec<(i64, String, i64)> {
        db.read(|c| Ok(t::all(c, Kind::Manga)?.into_iter().map(|c| (c.id, c.name, c.order)).collect()))
            .unwrap()
    }

    #[test]
    fn create_gives_the_next_order_and_the_first_user_category_zero() {
        let (_d, db) = test_db();
        let a = create_manga_category(&db, "A", 0).unwrap();
        let b = create_manga_category(&db, "B", 0).unwrap();
        let orders: Vec<(i64, i64)> = manga_orders(&db).into_iter().map(|(id, _, o)| (id, o)).collect();
        assert_eq!(orders, vec![(0, -1), (a, 0), (b, 1)]);
    }

    #[test]
    fn created_categories_take_the_sort_flags_of_the_global_preference() {
        let (_d, db) = test_db();
        let flags = library_sort::from_serialized("LAST_MANGA_UPDATE,DESCENDING");
        let id = create_manga_category(&db, "A", flags).unwrap();
        let stored = manga_flags(&db, id);
        assert_eq!(stored, library_sort::encode(library_sort::LAST_UPDATE, false));
    }

    fn manga_flags(db: &Db, id: i64) -> i64 {
        db.read(|c| Ok(t::all(c, Kind::Manga)?.into_iter().find(|r| r.id == id).unwrap().flags)).unwrap()
    }
}

#[cfg(test)]
mod repo_tests {
    use super::*;
    use crate::support::{test_db, insert_anime, insert_category, insert_manga};

    fn names(list: &[Category]) -> Vec<String> {
        list.iter().map(|c| c.name.clone()).collect()
    }

    #[test]
    fn all_starts_with_the_system_row_and_orders_by_sort_then_id() {
        let (_d, db) = test_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Manga);
        insert_category(&db, Kind::Manga, "B", 1);
        insert_category(&db, Kind::Manga, "A", 0);
        let all = repo.all().unwrap();
        assert_eq!(names(&all), ["", "A", "B"]);
        assert_eq!((all[0].id, all[0].order), (0, -1));
    }

    #[test]
    fn the_anime_kind_lists_its_system_row_first() {
        let (_d, db) = test_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Anime);
        insert_category(&db, Kind::Anime, "Watching", 0);
        let all = repo.all().unwrap();
        assert_eq!((all[0].id, all[0].name.as_str(), all[0].order), (0, "", -1));
        assert_eq!(names(&all), ["", "Watching"]);
    }

    #[test]
    fn hidden_categories_are_excluded_from_all_visible_and_only_one_hides() {
        let (_d, db) = test_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Anime);
        let shown = insert_category(&db, Kind::Anime, "Shown", 0);
        let hidden = insert_category(&db, Kind::Anime, "Hidden", 1);
        db.write(|tx| Ok(tx.execute("UPDATE categories SET hidden = 1 WHERE kind = 'anime' AND _id = ?1", [hidden]).map(drop)?)).unwrap();
        assert!(names(&repo.all_visible().unwrap()).iter().all(|n| n != "Hidden"));
        assert!(repo.all().unwrap().iter().any(|c| c.id == hidden && c.hidden));
        assert!(repo.get(shown).unwrap().is_some());
    }

    #[test]
    fn deleting_a_category_cascades_its_links() {
        let (_d, db) = test_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Manga);
        let m = insert_manga(&db, "m", true);
        let cat = insert_category(&db, Kind::Manga, "Tag", 0);
        db.write(|tx| Ok(tx.execute("INSERT INTO entry_categories(entry_id, kind, category_id) VALUES (?1, 'manga', ?2)", [m, cat]).map(drop)?))
            .unwrap();
        assert_eq!(names(&repo.by_entry(m).unwrap()), ["Tag"]);
        db.write(|tx| Ok(t::delete(tx, Kind::Manga, cat).map(drop)?)).unwrap();
        assert!(repo.by_entry(m).unwrap().is_empty());
    }

    #[test]
    fn by_entry_lists_the_entry_categories_in_display_order() {
        let (_d, db) = test_db();
        let repo = CategoryRepo::new(db.clone(), Kind::Anime);
        let a = insert_anime(&db, "a", true);
        let late = insert_category(&db, Kind::Anime, "Late", 5);
        let early = insert_category(&db, Kind::Anime, "Early", 2);
        let _other = insert_category(&db, Kind::Anime, "Other", 0);
        db.write(|tx| {
            for cat in [late, early] {
                tx.execute("INSERT INTO entry_categories(entry_id, kind, category_id) VALUES (?1, 'anime', ?2)", [a, cat])?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(names(&repo.by_entry(a).unwrap()), ["Early", "Late"]);
    }

    #[test]
    fn a_subscription_re_emits_after_an_insert() {
        let (_d, db) = test_db();
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
