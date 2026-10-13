//! Test support for this crate: temp-file databases, entry and item builders, fixture helpers and an
//! injectable clock. Compiled only for tests.

use kuro_platform::kind::Kind;
use std::sync::Arc;

use kuro_db::handle::Db;
use tempfile::TempDir;

use crate::entry::{Entry, EntryRepo};
use crate::item::{Item, ItemRepo};
use crate::time::Clock;

/// A migrated unified file in a fresh directory, behind a `Db`. Keep the `TempDir` alive.
pub(crate) fn test_db() -> (TempDir, Arc<Db>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = kuro_db::schema_unified::db_path(dir.path());
    let writer = kuro_db::schema_unified::open_file(&path).expect("unified schema");
    (dir, Arc::new(Db::new(&path, writer).expect("handle")))
}

/// A clock that always reads the same instant, for tests of time-dependent code.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_millis(&self) -> i64 {
        self.0
    }
}

/// A favourite or non-favourite entry with the given url and title, every other field default.
pub(crate) fn entry(kind: Kind, url: &str, title: &str, favorite: bool) -> Entry {
    Entry { url: url.to_owned(), title: title.to_owned(), favorite, source: 1, status: 1, ..Entry::create(kind) }
}

/// An item of `entry_id` with the given url (also its name) and scanlator, every other field default.
pub(crate) fn item(kind: Kind, entry_id: i64, url: &str, scanlator: Option<&str>) -> Item {
    Item { entry_id, url: url.to_owned(), name: url.to_owned(), scanlator: scanlator.map(str::to_owned), ..Item::create(kind) }
}

/// Inserts one entry of `kind` and returns its id.
pub(crate) fn insert_entry(db: &Arc<Db>, kind: Kind, url: &str, favorite: bool) -> i64 {
    EntryRepo::new(Arc::clone(db)).insert(&entry(kind, url, url, favorite)).expect("insert entry")
}

/// Inserts one item of `entry_id` and returns its id.
pub(crate) fn insert_item(db: &Arc<Db>, kind: Kind, entry_id: i64, url: &str) -> i64 {
    ItemRepo::new(Arc::clone(db)).add_all(&[item(kind, entry_id, url, None)]).expect("insert item")[0].id
}

pub(crate) fn insert_manga(db: &Arc<Db>, url: &str, favorite: bool) -> i64 {
    insert_entry(db, Kind::Manga, url, favorite)
}

pub(crate) fn insert_anime(db: &Arc<Db>, url: &str, favorite: bool) -> i64 {
    insert_entry(db, Kind::Anime, url, favorite)
}

pub(crate) fn insert_chapter(db: &Arc<Db>, manga_id: i64, url: &str) -> i64 {
    insert_item(db, Kind::Manga, manga_id, url)
}

pub(crate) fn insert_episode(db: &Arc<Db>, anime_id: i64, url: &str) -> i64 {
    insert_item(db, Kind::Anime, anime_id, url)
}

/// Inserts one category of the given kind and returns its id.
pub(crate) fn insert_category(db: &Arc<Db>, kind: Kind, name: &str, order: i64) -> i64 {
    db.write(|tx| Ok(kuro_db::tables::categories::insert(tx, kind, name, order, 0)?)).expect("insert category")
}
