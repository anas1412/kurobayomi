//! Test support for this crate: temp-file databases, entry and item builders, fixture helpers and an
//! injectable clock. Compiled only for tests.

use kuro_platform::kind::Kind;
use std::sync::Arc;

use kuro_db::handle::Db;
use tempfile::TempDir;

use crate::anime::{Anime, AnimeRepo};
use crate::category::{Category, CategoryRepo};
use crate::chapter::ChapterRepo;
use crate::episode::EpisodeRepo;
use crate::manga::{Manga, MangaRepo};
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

/// A favourite or non-favourite manga with the given url and title, every other field default.
pub(crate) fn manga(url: &str, title: &str, favorite: bool) -> Manga {
    Manga { url: url.to_owned(), title: title.to_owned(), favorite, source: 1, status: 1, ..Manga::create() }
}

/// An anime with the given url and title, every other field default.
pub(crate) fn anime(url: &str, title: &str, favorite: bool) -> Anime {
    Anime { url: url.to_owned(), title: title.to_owned(), favorite, source: 1, status: 1, ..Anime::create() }
}

/// A user category with the given name and order, every other field default.
pub(crate) fn category(name: &str, order: i64) -> Category {
    Category { id: -1, name: name.to_owned(), order, flags: 0, hidden: false }
}

/// Inserts one manga and returns its id.
pub(crate) fn insert_manga(db: &Arc<Db>, url: &str, favorite: bool) -> i64 {
    MangaRepo::new(Arc::clone(db)).insert(&manga(url, url, favorite)).expect("insert manga")
}

/// Inserts one anime and returns its id.
pub(crate) fn insert_anime(db: &Arc<Db>, url: &str, favorite: bool) -> i64 {
    AnimeRepo::new(Arc::clone(db)).insert(&anime(url, url, favorite)).expect("insert anime")
}

/// Inserts one chapter of `manga_id` and returns its id.
pub(crate) fn insert_chapter(db: &Arc<Db>, manga_id: i64, url: &str) -> i64 {
    let mut chapter = crate::chapter::Chapter::create();
    chapter.manga_id = manga_id;
    chapter.url = url.to_owned();
    chapter.name = url.to_owned();
    ChapterRepo::new(Arc::clone(db)).add_all(&[chapter]).expect("insert chapter")[0].id
}

/// Inserts one episode of `anime_id` and returns its id.
pub(crate) fn insert_episode(db: &Arc<Db>, anime_id: i64, url: &str) -> i64 {
    let mut episode = crate::episode::Episode::create();
    episode.anime_id = anime_id;
    episode.url = url.to_owned();
    episode.name = url.to_owned();
    EpisodeRepo::new(Arc::clone(db)).add_all(&[episode]).expect("insert episode")[0].id
}

/// Inserts one category of the given kind and returns its id.
pub(crate) fn insert_category(db: &Arc<Db>, kind: Kind, name: &str, order: i64) -> i64 {
    CategoryRepo::new(Arc::clone(db), kind).insert(&category(name, order)).expect("insert category")
}
