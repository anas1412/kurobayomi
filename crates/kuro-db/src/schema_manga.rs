use std::path::{Path, PathBuf};

use kuro_platform::paths::Paths;
use rusqlite::Connection;

pub use crate::schema::{BASELINE_SQL, BASELINE_VERSION, MANGA_SCHEMA, SCHEMA_VERSION};
use crate::flags::library_sort;
use crate::open::{DbResult, open_and_migrate};
use crate::statements_manga::{chapters, mangas};

/// File name of the manga database inside the data directory.
pub const MANGA_DB_FILE: &str = "manga.db";

/// Kurobayomi migrations after the baseline: `(target version, SQL)`, ascending and contiguous.
pub const MIGRATIONS: &[(i64, &str)] = crate::schema::MANGA_MIGRATIONS;

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
    crate::open::register_functions(&c).expect("kuro_lower");
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
        assert_eq!(c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn migration_34_adds_the_url_index_and_keeps_library_rows_identical() {
        let c = test_db();
        let a = mangas::insert(&c, &manga("/a")).unwrap();
        let b = mangas::insert(&c, &manga("/b")).unwrap();
        let hidden = mangas::insert(&c, &manga("/c")).unwrap();
        c.execute("UPDATE mangas SET favorite = 0 WHERE _id = ?1", [hidden]).unwrap();
        for (id, url) in [(a, "/a/1"), (a, "/a/2"), (b, "/b/1"), (hidden, "/c/1")] {
            chapters::insert(&c, &chapter(id, url)).unwrap();
        }
        let rows = |c: &Connection| -> Vec<(i64, i64, i64, i64)> {
            let mut st = c
                .prepare("SELECT _id, totalCount, readCount, category FROM libraryView ORDER BY _id")
                .unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        let before = rows(&c);
        assert_eq!(before.len(), 2, "only the favourites are in the library view");
        c.execute_batch(crate::schema::MANGA_MIGRATIONS[0].1).unwrap();
        assert_eq!(rows(&c), before, "migration 34 keeps the library view rows");
        let url_index: i64 = c
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = 'chapters_url_index'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(url_index, 1);
    }

    #[test]
    fn the_latest_manga_version_is_34_and_fresh_files_get_it() {
        assert_eq!(SCHEMA_VERSION, 34);
        assert_eq!(BASELINE_VERSION, 33);
        assert_eq!(MANGA_SCHEMA.migrations.len(), 1);
        assert_eq!(MANGA_SCHEMA.migrations[0].0, 34);
        let dir = tempfile::tempdir().unwrap();
        let c = open(&dir.path().join("manga.db")).unwrap();
        assert_eq!(c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 34);
    }

    #[test]
    fn duplicate_title_match_folds_unicode_case_on_both_sides() {
        let c = test_db();
        let stored = mangas::insert(&c, &manga("/a")).unwrap();
        let adding = mangas::insert(&c, &manga("/b")).unwrap();
        c.execute("UPDATE mangas SET title = 'Ärger' WHERE _id = ?1", [stored]).unwrap();
        for input in ["ärger", "ÄRGER", "Ärger"] {
            let found = mangas::get_duplicate_library_manga(&c, input, adding).unwrap();
            assert_eq!(found.len(), 1, "input {input}");
        }
        c.execute("UPDATE mangas SET title = 'Naruto' WHERE _id = ?1", [stored]).unwrap();
        assert_eq!(mangas::get_duplicate_library_manga(&c, "NARUTO", adding).unwrap().len(), 1);
        assert!(mangas::get_duplicate_library_manga(&c, "naruto", stored).unwrap().is_empty(), "the same row is excluded");
    }
}
