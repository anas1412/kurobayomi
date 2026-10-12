//! Test database: a migrated file in a temporary directory. Keep the returned `TempDir` alive for the length of the
//! test.

use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;
use kuro_db::handle::Db;
use tempfile::TempDir;

/// A migrated unified database handle.
pub fn test_db() -> (TempDir, Arc<Db>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = kuro_db::schema_unified::db_path(dir.path());
    let writer = kuro_db::schema_unified::open_file(&path).expect("unified schema");
    (dir, Arc::new(Db::new(&path, writer).expect("handle")))
}

/// The database behind a manga repository.
pub fn manga_repo() -> (TempDir, MangaRepo) {
    let (dir, db) = test_db();
    (dir, MangaRepo::new(db))
}

/// The database behind an anime repository.
pub fn anime_repo() -> (TempDir, AnimeRepo) {
    let (dir, db) = test_db();
    (dir, AnimeRepo::new(db))
}
