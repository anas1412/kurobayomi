//! Test database: a migrated file in a temporary directory. Keep the returned `TempDir` alive for the length of the
//! test.

use std::sync::Arc;

use kuro_data::entry::EntryRepo;
use kuro_db::handle::Db;
use tempfile::TempDir;

/// A migrated unified database handle.
pub fn test_db() -> (TempDir, Arc<Db>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = kuro_db::schema_unified::db_path(dir.path());
    let writer = kuro_db::schema_unified::open_file(&path).expect("unified schema");
    (dir, Arc::new(Db::new(&path, writer).expect("handle")))
}

/// The database behind an entry repository.
pub fn entry_repo() -> (TempDir, EntryRepo) {
    let (dir, db) = test_db();
    (dir, EntryRepo::new(db))
}
