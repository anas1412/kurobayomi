#![cfg(feature = "legacy-import")]
//! The Aniyomi importer and the legacy migration replay (T-03a-022, T-03a-023; TC-I02 to TC-I10).

use std::path::{Path, PathBuf};

use kuro_db::import::{ImportError, ImportReport, Progress, Quirk, import_aniyomi_db, install_imported, utc_stamp};
use kuro_db::legacy;
use kuro_db::schema_manga;
use rusqlite::Connection;

const FIXTURE: &str = include_str!("fixtures/legacy_v13.sql");

/// A file at `path` holding the fixture with `PRAGMA user_version = version`.
fn aniyomi_file(path: &Path, version: i64) {
    let c = Connection::open(path).unwrap();
    c.pragma_update(None, "foreign_keys", "OFF").unwrap();
    c.execute_batch(FIXTURE).unwrap();
    if version > 14 {
        c.pragma_update(None, "user_version", 14).unwrap();
        legacy::replay(&c, 14, false).unwrap();
        c.pragma_update(None, "user_version", version).unwrap();
    } else {
        c.pragma_update(None, "user_version", version).unwrap();
    }
}

fn run(src: &Path, dest: &Path) -> Result<ImportReport, ImportError> {
    import_aniyomi_db(src, dest, &mut |_| {})
}

fn schema_of(c: &Connection) -> Vec<(String, String, Option<String>)> {
    let mut st = c.prepare("SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name").unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect()
}

fn one(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn tc_i02_the_replayed_schema_differs_from_the_baseline_only_where_the_plan_says() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.db");
    aniyomi_file(&old, 33);
    let upgraded = Connection::open(&old).unwrap();
    let fresh = Connection::open_in_memory().unwrap();
    fresh.execute_batch(kuro_db::schema::BASELINE_SQL).unwrap();
    let names = |c: &Connection, kind: &str| -> Vec<String> {
        let mut st = c.prepare("SELECT name FROM sqlite_master WHERE type = ?1 AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap();
        st.query_map([kind], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
    };
    let only_in = |a: Vec<String>, b: &[String]| -> Vec<String> { a.into_iter().filter(|n| !b.contains(n)).collect() };
    let (up_t, fr_t) = (names(&upgraded, "trigger"), names(&fresh, "trigger"));
    assert_eq!(only_in(up_t.clone(), &fr_t), ["update_last_favorited_at_mangas", "update_last_modified_at_mangas_categories"]);
    assert_eq!(only_in(fr_t, &up_t), ["update_last_favorite_at_mangas"]);
    assert_eq!(only_in(names(&upgraded, "table"), &names(&fresh, "table")), ["history_temp"]);
    assert!(only_in(names(&fresh, "table"), &names(&upgraded, "table")).is_empty());
    let has = |c: &Connection, table: &str, column: &str| one(c, &format!("SELECT count(*) FROM pragma_table_info('{table}') WHERE name = '{column}'")) == 1;
    assert!(has(&upgraded, "mangas_categories", "last_modified_at") && !has(&fresh, "mangas_categories", "last_modified_at"));
    assert_eq!(one(&upgraded, "SELECT \"notnull\" FROM pragma_table_info('history') WHERE name = 'last_read'"), 1);
}

#[test]
fn tc_i04_migration_10_gives_every_manga_the_same_date_added() {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(
        "CREATE TABLE mangas(_id INTEGER PRIMARY KEY, title TEXT);
         CREATE TABLE chapters(_id INTEGER PRIMARY KEY, manga_id INTEGER, date_fetch INTEGER);
         INSERT INTO mangas(_id, title) VALUES (1, 'A'), (2, 'B');
         INSERT INTO chapters(manga_id, date_fetch) VALUES (1, 500), (1, 400), (2, 100), (2, 900);",
    )
    .unwrap();
    c.execute_batch(legacy::migration(10).unwrap()).unwrap();
    assert_eq!(one(&c, "SELECT count(DISTINCT date_added) FROM mangas"), 1, "the bug: one shared value");
    assert_eq!(one(&c, "SELECT date_added FROM mangas WHERE _id = 1"), one(&c, "SELECT date_added FROM mangas WHERE _id = 2"));
}

#[test]
fn tc_i09_and_tc_i10_migrations_28_and_29_clean_scanlators_and_scores() {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(
        "CREATE TABLE chapters(_id INTEGER PRIMARY KEY, scanlator TEXT);
         CREATE TABLE manga_sync(_id INTEGER PRIMARY KEY, sync_id INTEGER, score REAL);
         INSERT INTO chapters VALUES (1, '  Group '), (2, NULL);
         INSERT INTO manga_sync VALUES (1, 7, -1), (2, 7, 4), (3, 1, -1);",
    )
    .unwrap();
    c.execute_batch(legacy::migration(28).unwrap()).unwrap();
    c.execute_batch(legacy::migration(29).unwrap()).unwrap();
    assert_eq!(c.query_row("SELECT scanlator FROM chapters WHERE _id = 1", [], |r| r.get::<_, String>(0)).unwrap(), "Group");
    assert_eq!(c.query_row("SELECT scanlator FROM chapters WHERE _id = 2", [], |r| r.get::<_, Option<String>>(0)).unwrap(), None);
    let scores: Vec<f64> = c.prepare("SELECT score FROM manga_sync ORDER BY _id").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(scores, [0.0, 4.0, -1.0], "only MangaUpdates (sync_id 7) is clamped");
}

#[test]
fn tc_i03_the_import_matches_the_fresh_schema_and_keeps_the_rows_as_they_were() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("tachiyomi.db"), dir.path().join("new.db"));
    aniyomi_file(&src, 33);
    // values that a trigger would overwrite if the rows went through them
    {
        let c = Connection::open(&src).unwrap();
        let triggers: Vec<String> = c.prepare("SELECT name FROM sqlite_master WHERE type = 'trigger'").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        for name in triggers {
            c.execute_batch(&format!("DROP TRIGGER {name}")).unwrap();
        }
        c.execute_batch("UPDATE mangas SET version = 7, last_modified_at = 1234, favorite_modified_at = 99 WHERE _id = 1;
                         UPDATE chapters SET version = 3, last_modified_at = 555 WHERE _id = 2;").unwrap();
    }
    let mut steps = Vec::new();
    let report = import_aniyomi_db(&src, &dest, &mut |p| steps.push(p)).unwrap();
    assert_eq!(report.source_version, 33);
    assert!(steps.contains(&Progress::CopyingSource) && steps.contains(&Progress::CopyingTable("chapters")));

    let new = Connection::open(&dest).unwrap();
    let fresh = Connection::open(dir.path().join("fresh.db")).unwrap();
    fresh.execute_batch(kuro_db::schema::BASELINE_SQL).unwrap();
    for (_, sql) in kuro_db::schema::MANGA_MIGRATIONS {
        fresh.execute_batch(sql).unwrap();
    }
    assert_eq!(schema_of(&new), schema_of(&fresh), "sqlite_master equals the fresh schema");
    assert_eq!(one(&new, "SELECT count(*) FROM sqlite_master WHERE name = 'history_temp'"), 0);
    assert_eq!(one(&new, "PRAGMA user_version"), 34);
    assert_eq!(one(&new, "PRAGMA application_id"), kuro_db::open::APPLICATION_ID);
    // rows and ids
    assert_eq!((one(&new, "SELECT count(*) FROM mangas"), one(&new, "SELECT count(*) FROM chapters")), (3, 3));
    assert_eq!(one(&new, "SELECT count(*) FROM mangas WHERE _id IN (1, 2, 3)"), 3);
    // no trigger side effects
    assert_eq!(
        new.query_row("SELECT version, last_modified_at, favorite_modified_at FROM mangas WHERE _id = 1", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))).unwrap(),
        (7, 1234, 99)
    );
    assert_eq!(
        new.query_row("SELECT version, last_modified_at FROM chapters WHERE _id = 2", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).unwrap(),
        (3, 555)
    );
    // the triggers work again afterwards
    new.execute("UPDATE chapters SET read = 1 WHERE _id = 3", []).unwrap();
    assert_eq!(one(&new, "SELECT version FROM chapters WHERE _id = 3"), 1);
    // history keeps its rows, with a null last-read turned into 0
    assert_eq!(one(&new, "SELECT last_read FROM history WHERE chapter_id = 2"), 0);
    // migrations 28 and 29 ran on the way
    assert_eq!(new.query_row("SELECT scanlator FROM chapters WHERE _id = 1", [], |r| r.get::<_, String>(0)).unwrap(), "Group");
    assert_eq!(one(&new, "SELECT cast(score AS INTEGER) FROM manga_sync WHERE sync_id = 7"), 0);
    // the quirks
    assert!(report.quirks.contains(&Quirk::HistoryTempDropped));
    assert!(report.quirks.contains(&Quirk::MangasCategoriesTimestampDropped));
    assert!(!report.quirks.contains(&Quirk::DateAddedBackfillBug), "the fixture dates differ");
    // the source is untouched
    assert_eq!(one(&Connection::open(&src).unwrap(), "PRAGMA user_version"), 33);
    assert!(!dir.path().join(".new.db.import-work").exists(), "the work directory is gone");
}

#[test]
fn a_shared_date_added_is_reported_as_the_migration_10_bug() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("a.db"), dir.path().join("b.db"));
    aniyomi_file(&src, 33);
    Connection::open(&src).unwrap().execute("UPDATE mangas SET date_added = 400", []).unwrap();
    assert!(run(&src, &dest).unwrap().quirks.contains(&Quirk::DateAddedBackfillBug));
}

#[test]
fn extension_repos_are_not_copied_and_the_table_stays_empty() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("a.db"), dir.path().join("b.db"));
    aniyomi_file(&src, 33);
    Connection::open(&src)
        .unwrap()
        .execute(
            "INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES ('https://r.test', 'R', 'R', 'https://r.test', 'fp')",
            [],
        )
        .unwrap();
    let report = run(&src, &dest).unwrap();
    assert!(!report.copied.contains_key("extension_repos"));
    assert_eq!(one(&Connection::open(&dest).unwrap(), "SELECT count(*) FROM extension_repos"), 0);
}

#[test]
fn tc_i05_orphan_rows_are_deleted_and_counted() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("a.db"), dir.path().join("b.db"));
    aniyomi_file(&src, 33);
    let report = run(&src, &dest).unwrap();
    assert_eq!(report.orphans_removed.get("chapters"), Some(&1), "chapter 4 points at manga 99");
    assert_eq!(report.orphans_removed.get("history"), Some(&1), "history row for chapter 98");
    assert_eq!(report.orphans_total(), 2);
    let new = Connection::open(&dest).unwrap();
    assert_eq!(one(&new, "SELECT count(*) FROM pragma_foreign_key_check"), 0);
    assert_eq!(report.copied.get("chapters"), Some(&3));
}

#[test]
fn tc_i06_versions_13_and_34_are_refused_and_14_and_33_are_accepted() {
    let dir = tempfile::tempdir().unwrap();
    for (version, accepted) in [(13, false), (14, true), (33, true), (34, false)] {
        let (src, dest) = (dir.path().join(format!("s{version}.db")), dir.path().join(format!("d{version}.db")));
        aniyomi_file(&src, version);
        let result = run(&src, &dest);
        match (accepted, &result) {
            (true, Ok(report)) => assert_eq!(report.source_version, version),
            (false, Err(ImportError::TooOld { found })) => assert_eq!((version, *found), (13, 13)),
            (false, Err(ImportError::TooNew { found })) => assert_eq!((version, *found), (34, 34)),
            _ => panic!("version {version}: {result:?}"),
        }
        assert_eq!(dest.exists(), accepted, "a refused file leaves nothing at the destination");
    }
    let messages = (ImportError::TooNew { found: 34 }.to_string(), ImportError::TooOld { found: 13 }.to_string());
    assert_eq!(messages.0, "This database comes from a newer Aniyomi version");
    assert_eq!(messages.1, "Database too old; open it once in Aniyomi/Mihon first");
}

#[test]
fn a_version_14_file_runs_every_step_and_ends_like_a_version_33_file() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("v14.db"), dir.path().join("out.db"));
    let c = Connection::open(&src).unwrap();
    c.pragma_update(None, "foreign_keys", "OFF").unwrap();
    c.execute_batch(FIXTURE).unwrap();
    c.pragma_update(None, "user_version", 14).unwrap();
    drop(c);
    let mut steps = Vec::new();
    let report = import_aniyomi_db(&src, &dest, &mut |p| steps.push(p)).unwrap();
    assert_eq!(report.source_version, 14);
    assert!(steps.contains(&Progress::Migrating { from: 14, to: 15 }) && steps.contains(&Progress::Migrating { from: 32, to: 33 }));
    assert_eq!(report.copied.get("mangas"), Some(&3));
}

#[test]
fn tc_i07_the_content_of_the_wal_is_included() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("wal.db"), dir.path().join("out.db"));
    aniyomi_file(&src, 33);
    let live = Connection::open(&src).unwrap();
    live.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get::<_, String>(0)).unwrap();
    live.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    live.execute("INSERT INTO mangas(_id, source, url, title, status, favorite, initialized, viewer, chapter_flags, cover_last_modified, date_added) VALUES (50, 1, '/wal', 'Only in the WAL', 0, 0, 0, 0, 0, 0, 0)", []).unwrap();
    assert!(src.with_file_name("wal.db-wal").metadata().unwrap().len() > 0, "the row sits in the WAL");
    let report = run(&src, &dest).unwrap();
    drop(live);
    assert_eq!(report.copied.get("mangas"), Some(&4));
    let new = Connection::open(&dest).unwrap();
    assert_eq!(one(&new, "SELECT count(*) FROM mangas WHERE title = 'Only in the WAL'"), 1);
}

#[test]
fn tc_i08_the_swap_renames_the_live_file_and_a_failed_import_leaves_it_alone() {
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("manga.db");
    std::fs::write(&live, b"live data").unwrap();
    // a failed import: the live file is untouched and nothing appears at the destination
    let junk = dir.path().join("junk.db");
    std::fs::write(&junk, b"this is not a database").unwrap();
    let dest = dir.path().join("imported.db");
    assert!(run(&junk, &dest).is_err());
    assert!(!dest.exists());
    assert_eq!(std::fs::read(&live).unwrap(), b"live data");
    // a good import, then the swap
    let src = dir.path().join("src.db");
    aniyomi_file(&src, 33);
    run(&src, &dest).unwrap();
    let backup = install_imported(&live, &dest).unwrap().expect("the live file existed");
    let name = backup.file_name().unwrap().to_str().unwrap().to_owned();
    assert!(name.starts_with("manga.db.bak-") && name.len() == "manga.db.bak-".len() + 14, "{name}");
    assert_eq!(std::fs::read(&backup).unwrap(), b"live data", "the old file is renamed, never deleted");
    assert!(!dest.exists());
    assert_eq!(one(&Connection::open(&live).unwrap(), "SELECT count(*) FROM mangas"), 3);
    // without a live file there is nothing to back up
    let again = dir.path().join("again.db");
    run(&src, &again).unwrap();
    let fresh_live: PathBuf = dir.path().join("none").join("manga.db");
    std::fs::create_dir_all(fresh_live.parent().unwrap()).unwrap();
    assert_eq!(install_imported(&fresh_live, &again).unwrap(), None);
}

#[test]
fn a_destination_that_exists_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("a.db"), dir.path().join("b.db"));
    aniyomi_file(&src, 33);
    std::fs::write(&dest, b"keep").unwrap();
    assert!(matches!(run(&src, &dest), Err(ImportError::DestinationExists(_))));
    assert_eq!(std::fs::read(&dest).unwrap(), b"keep");
}

#[test]
fn a_kurobayomi_file_is_not_an_aniyomi_database() {
    let dir = tempfile::tempdir().unwrap();
    let (src, dest) = (dir.path().join("manga.db"), dir.path().join("out.db"));
    drop(schema_manga::open(&src).unwrap());
    assert!(matches!(run(&src, &dest), Err(ImportError::NotAnAniyomiDatabase)));
}

#[test]
fn the_utc_stamp_has_fourteen_digits() {
    assert_eq!(utc_stamp(0), "19700101000000");
    assert_eq!(utc_stamp(1_700_000_000), "20231114221320");
    assert_eq!(utc_stamp(951_782_400), "20000229000000");
}

/// Manual test with a real exported database (T-03a-023). Pull `tachiyomi.db` (with its `-wal` and `-shm`) from an
/// Aniyomi or Mihon install, then run
/// `KURO_TEST_ANIYOMI_DB=/path/to/tachiyomi.db cargo test -p kuro-db --test legacy_import -- --ignored real_database`.
/// The report is printed; the source file is only read.
#[test]
#[ignore = "needs a real database in KURO_TEST_ANIYOMI_DB"]
fn real_database_imports_through_kuro_test_aniyomi_db() {
    let Ok(src) = std::env::var("KURO_TEST_ANIYOMI_DB") else {
        panic!("set KURO_TEST_ANIYOMI_DB to the path of an exported tachiyomi.db");
    };
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("manga.db");
    let report = run(Path::new(&src), &dest).unwrap();
    println!("{report:#?}");
    let new = schema_manga::open(&dest).unwrap();
    assert_eq!(one(&new, "SELECT count(*) FROM pragma_foreign_key_check"), 0);
    assert_eq!(one(&new, "SELECT count(*) FROM mangas") as u64, report.copied["mangas"]);
}
