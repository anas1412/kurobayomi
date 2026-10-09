use std::path::{Path, PathBuf};

use kuro_platform::paths::Paths;
use rusqlite::Connection;

pub use crate::schema::{BASELINE_SQL, MANGA_SCHEMA, SCHEMA_VERSION as BASELINE_VERSION};
use crate::flags::library_sort;
use crate::open::{DbResult, open_and_migrate};
use crate::statements_manga::{chapters, mangas};

/// File name of the manga database inside the data directory.
pub const MANGA_DB_FILE: &str = "manga.db";

/// Kurobayomi migrations after the baseline: `(target version, SQL)`, ascending and contiguous.
/// Empty until the first post-33 change (migration 34 is T-03a-024).
pub const MIGRATIONS: &[(i64, &str)] = &[];

/// `<data_dir>/manga.db`.
pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MANGA_DB_FILE)
}

/// Opens `<data_dir>/manga.db` for the app's platform paths. See [`open`].
pub fn open_in_data_dir(paths: &dyn Paths) -> DbResult<Connection> {
    open(&db_path(&paths.data_dir()))
}

/// Opens the manga database at `path`: creates the baseline in an empty file or migrates it, then
/// clears the `is_syncing` flags left by a run that did not finish (T-03a-030). A file this app must
/// not touch is refused before any write. A file created by this call gets the system category's
/// sort set to Alphabetical Ascending (Q28; the baseline seeds Aniyomi's 0); an existing file keeps it.
pub fn open(path: &Path) -> DbResult<Connection> {
    let fresh = !std::fs::metadata(path).is_ok_and(|m| m.len() > 0);
    let c = open_and_migrate(path, &MANGA_SCHEMA)?;
    if fresh {
        c.execute("UPDATE categories SET flags = ?1 WHERE _id = 0", [library_sort::DEFAULT])?;
    }
    mangas::reset_is_syncing(&c)?;
    chapters::reset_is_syncing(&c)?;
    Ok(c)
}

/// In-memory database with the baseline and foreign keys on, for the statement tests.
#[cfg(test)]
pub(crate) fn test_db() -> rusqlite::Connection {
    let c = rusqlite::Connection::open_in_memory().expect("in-memory database");
    c.pragma_update(None, "foreign_keys", "ON").expect("foreign keys");
    crate::open::register_functions(&c).expect("lower_unicode");
    c.execute_batch(BASELINE_SQL).expect("baseline schema");
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::statements_manga::fixtures::{chapter, manga};

    #[test]
    fn open_in_data_dir_creates_manga_db_in_the_data_dir() {
        let paths = kuro_testkit::TempPaths::new().unwrap();
        drop(open_in_data_dir(&paths).unwrap());
        assert!(db_path(&paths.data_dir()).is_file());
    }

    #[test]
    fn q28_sets_the_system_flags_on_a_new_file_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manga.db");
        let flags = |c: &Connection| c.query_row("SELECT flags FROM categories WHERE _id = 0", [], |r| r.get::<_, i64>(0)).unwrap();
        let c = open(&path).unwrap();
        assert_eq!(flags(&c), 64, "a new file starts Alphabetical Ascending");
        c.execute("UPDATE categories SET flags = 0 WHERE _id = 0", []).unwrap();
        drop(c);
        assert_eq!(flags(&open(&path).unwrap()), 0, "an existing file keeps its flags");
    }

    #[test]
    fn reopening_clears_is_syncing_and_leaves_the_version_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manga.db");
        let version_before = {
            let c = open(&path).unwrap();
            let id = mangas::insert(&c, &manga("/a")).unwrap();
            chapters::insert(&c, &chapter(id, "/a/1")).unwrap();
            c.execute("UPDATE mangas SET is_syncing = 1", []).unwrap();
            c.execute("UPDATE chapters SET is_syncing = 1", []).unwrap();
            c.query_row("SELECT version FROM mangas", [], |r| r.get::<_, i64>(0)).unwrap()
        };
        let c = open(&path).unwrap();
        let syncing: i64 = c
            .query_row(
                "SELECT (SELECT count(*) FROM mangas WHERE is_syncing = 1) + (SELECT count(*) FROM chapters WHERE is_syncing = 1)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(syncing, 0);
        let version: i64 = c.query_row("SELECT version FROM mangas", [], |r| r.get(0)).unwrap();
        assert_eq!(version, version_before);
        assert_eq!(c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 33);
    }
}
