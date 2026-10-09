//! In-memory-style test databases: each helper opens a migrated file in a temporary directory. Keep the
//! returned `TempDir` alive for the length of the test.

use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;
use kuro_db::handle::Db;
use tempfile::TempDir;

/// A migrated manga database handle.
pub fn manga_db() -> (TempDir, Arc<Db>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("tachiyomi.db");
    let writer = kuro_db::schema_manga::open(&path).expect("manga schema");
    (dir, Arc::new(Db::new(&path, writer).expect("manga handle")))
}

/// A migrated anime database handle.
pub fn anime_db() -> (TempDir, Arc<Db>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("tachiyomi.animedb");
    let writer = kuro_db::schema_anime::open(&path).expect("anime schema");
    (dir, Arc::new(Db::new(&path, writer).expect("anime handle")))
}

/// A migrated manga database behind a repository.
pub fn manga_repo() -> (TempDir, MangaRepo) {
    let (dir, db) = manga_db();
    (dir, MangaRepo::new(db))
}

/// A migrated anime database behind a repository.
pub fn anime_repo() -> (TempDir, AnimeRepo) {
    let (dir, db) = anime_db();
    (dir, AnimeRepo::new(db))
}
