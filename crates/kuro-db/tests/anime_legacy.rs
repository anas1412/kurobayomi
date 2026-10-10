#![cfg(feature = "legacy-import")]
//! The anime legacy chain 113 to 138 and its equivalence with the fresh schema (T-04a-025, T-04a-026).

use std::collections::BTreeSet;
use std::path::Path;

use kuro_db::DbError;
use kuro_db::anime_legacy::{self, FIRST_STEP, LAST_STEP, upgrade_legacy};
use kuro_db::schema_anime::{ANIME_SCHEMA, ANIME_SCHEMA_VERSION};
use rusqlite::Connection;

const BASELINE: &str = include_str!("fixtures/anime_v113_baseline.sql");

/// A version-113 file with the seed rows, or any later version by running the chain up to it.
fn legacy_file(path: &Path, version: i64) {
    let c = Connection::open(path).unwrap();
    c.pragma_update(None, "foreign_keys", "OFF").unwrap();
    c.execute_batch(BASELINE).unwrap();
    c.pragma_update(None, "user_version", FIRST_STEP).unwrap();
    anime_legacy::apply_steps(&c, FIRST_STEP, version.min(LAST_STEP + 1)).unwrap();
    c.pragma_update(None, "user_version", version).unwrap();
}

fn one(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn fresh() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(ANIME_SCHEMA.baseline_sql).unwrap();
    c
}

fn objects(c: &Connection, kind: &str) -> BTreeSet<String> {
    let mut st = c.prepare("SELECT name FROM sqlite_master WHERE type = ?1 AND name NOT LIKE 'sqlite_%'").unwrap();
    st.query_map([kind], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

/// `table.column:type:notnull:default` of every column of every table.
fn columns(c: &Connection) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for table in objects(c, "table") {
        let mut st = c.prepare("SELECT name, type, \"notnull\", dflt_value FROM pragma_table_info(?1)").unwrap();
        let rows = st.query_map([&table], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, Option<String>>(3)?))).unwrap();
        for row in rows {
            let (name, ty, notnull, default) = row.unwrap();
            out.insert(format!("{table}.{name}:{ty}:{notnull}:{}", default.unwrap_or_default()));
        }
    }
    out
}

fn diff(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Vec<String> {
    a.difference(b).cloned().collect()
}

#[test]
fn the_seed_upgrades_to_139_with_the_documented_row_results() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("anime.db");
    legacy_file(&path, FIRST_STEP);
    let result = upgrade_legacy(&path).unwrap().expect("a 113 file is upgraded");
    assert_eq!(result.from, 113);
    assert_eq!(result.backup.file_name().unwrap(), "anime.db.bak-v113");
    assert_eq!(one(&Connection::open(&result.backup).unwrap(), "PRAGMA user_version"), 113, "the copy is the original");

    let c = Connection::open(&path).unwrap();
    assert_eq!(one(&c, "PRAGMA user_version"), ANIME_SCHEMA_VERSION);
    assert_eq!(one(&c, "PRAGMA application_id"), kuro_db::schema_anime::ANIME_APPLICATION_ID);
    assert_eq!(one(&c, "SELECT count(*) FROM animes WHERE _id IN (1, 2) AND fetch_type = 1"), 2, "step 135: every entry is Episodes-type");
    assert_eq!((one(&c, "SELECT date_added FROM animes WHERE _id = 1"), one(&c, "SELECT date_added FROM animes WHERE _id = 2")), (1000, 0));
    assert_eq!((one(&c, "SELECT date_upload FROM episodes WHERE _id = 1"), one(&c, "SELECT date_upload FROM episodes WHERE _id = 2")), (50, 60), "step 113");
    assert_eq!(one(&c, "SELECT count(*) FROM episodes WHERE fillermark = 0 AND memo = x'7b7d'"), 2);
    assert_eq!(one(&c, "SELECT count(*) FROM animes WHERE memo = x'7b7d'"), 2);
    let history: Vec<(i64, i64, i64)> = c
        .prepare("SELECT _id, episode_id, last_seen FROM animehistory ORDER BY _id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(history, [(1, 1, 0), (2, 2, 9000)], "NULL last_seen became 0");
    assert_eq!(one(&c, "SELECT count(*) FROM pragma_table_info('animehistory') WHERE name = 'time_seen'"), 0, "time_seen is lost");
    assert_eq!(one(&c, "SELECT count(*) FROM sqlite_master WHERE name = 'animehistory_temp'"), 0, "the leftover table is dropped");
    let cats: Vec<(i64, String, i64, i64, i64)> = c
        .prepare("SELECT _id, name, sort, flags, hidden FROM categories ORDER BY _id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(cats, [(0, String::new(), -1, 0, 0), (1, "cat".into(), 0, 64, 0)]);
    assert_eq!(one(&c, "SELECT private FROM anime_sync"), 0);
    let lib: (i64, i64, i64, i64) = c
        .query_row("SELECT totalCount, seenCount, lastSeen, category FROM animelibView WHERE _id = 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap();
    assert_eq!(lib, (2, 1, 9000, 1));
    assert_eq!(one(&c, "SELECT count(*) FROM pragma_foreign_key_check"), 0);
    assert_eq!(objects(&c, "view"), objects(&fresh(), "view"), "the eight views come from views.sql");
}

#[test]
fn t_04a_026_fresh_and_upgraded_differ_only_where_the_plan_lists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("anime.db");
    legacy_file(&path, FIRST_STEP);
    upgrade_legacy(&path).unwrap();
    let (up, fr) = (Connection::open(&path).unwrap(), fresh());

    let set = |items: &[&str]| -> Vec<String> { items.iter().map(|s| (*s).to_owned()).collect() };
    // rows 1 to 3 (names) and 4 (the extra trigger); row 5 is dropped by the upgrade, so no table differs
    assert_eq!(diff(&objects(&up, "index"), &objects(&fr, "index")), set(&["library_favorite_index"]));
    assert_eq!(diff(&objects(&fr, "index"), &objects(&up, "index")), set(&["animelib_favorite_index"]));
    assert_eq!(
        diff(&objects(&up, "trigger"), &objects(&fr, "trigger")),
        set(&["system_anime_category_delete_trigger", "update_last_favorited_at_animes", "update_last_modified_at_animes_categories"])
    );
    assert_eq!(
        diff(&objects(&fr, "trigger"), &objects(&up, "trigger")),
        set(&["system_category_delete_trigger", "update_last_favorite_at_animes"])
    );
    assert_eq!(objects(&up, "table"), objects(&fr, "table"), "animehistory_temp is gone");
    assert_eq!(objects(&up, "view"), objects(&fr, "view"));
    // rows 4 and 6 to 9 (columns, notnull, defaults)
    let (cu, cf) = (columns(&up), columns(&fr));
    let up_only = diff(&cu, &cf);
    let fresh_only = diff(&cf, &cu);
    let allowed_up = [
        "animes_categories.last_modified_at:INTEGER:1:0",
        "animehistory.last_seen:INTEGER:1:",
        "animes.season_flags:INTEGER:1:0",
        "animes.season_number:REAL:1:-1.0",
        "animes.season_source_order:INTEGER:1:0",
        "animes.background_last_modified:INTEGER:1:0",
        "episodes.fillermark:INTEGER:1:0",
        "animes.memo:BLOB:1:x'7b7d'",
        "episodes.memo:BLOB:1:x'7b7d'",
        "extension_store.resolved_index_url:TEXT:1:''",
    ];
    let allowed_fresh = [
        "animehistory.last_seen:INTEGER:0:",
        "animes.season_flags:INTEGER:1:",
        "animes.season_number:REAL:1:",
        "animes.season_source_order:INTEGER:1:",
        "animes.background_last_modified:INTEGER:1:",
        "episodes.fillermark:INTEGER:1:",
        "animes.memo:BLOB:1:'{}'",
        "episodes.memo:BLOB:1:'{}'",
        "extension_store.resolved_index_url:TEXT:1:",
    ];
    for column in &up_only {
        assert!(allowed_up.contains(&column.as_str()), "unexpected upgraded column {column}");
    }
    for column in &fresh_only {
        assert!(allowed_fresh.contains(&column.as_str()), "unexpected fresh column {column}");
    }
}

#[test]
fn resuming_from_every_version_reaches_the_same_result_as_one_run() {
    let dir = tempfile::tempdir().unwrap();
    let whole = dir.path().join("whole.db");
    legacy_file(&whole, FIRST_STEP);
    upgrade_legacy(&whole).unwrap();
    let dump = |path: &Path| -> Vec<String> {
        let c = Connection::open(path).unwrap();
        let mut out = columns(&c).into_iter().collect::<Vec<_>>();
        for table in ["animes", "episodes", "animehistory", "categories", "anime_sync", "extension_store"] {
            let cols: Vec<String> = c
                .prepare(&format!("SELECT name FROM pragma_table_info('{table}') WHERE name NOT IN ('last_modified_at', 'favorite_modified_at') ORDER BY cid"))
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            let select = cols.iter().map(|c| format!("quote({c})")).collect::<Vec<_>>().join(" || ',' || ");
            let rows: Vec<String> = c.prepare(&format!("SELECT {select} FROM {table} ORDER BY 1")).unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
            out.extend(rows.into_iter().map(|r| format!("{table}: {r}")));
        }
        out
    };
    let expected = dump(&whole);
    for version in FIRST_STEP..=LAST_STEP {
        let path = dir.path().join(format!("v{version}.db"));
        legacy_file(&path, version);
        upgrade_legacy(&path).unwrap();
        assert_eq!(dump(&path), expected, "resuming at {version}");
    }
}

#[test]
fn version_gates_refuse_too_old_and_too_new_and_leave_139_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("anime.db");
    let set = |version: i64| {
        let c = Connection::open(&path).unwrap();
        c.execute_batch("CREATE TABLE IF NOT EXISTS t(x)").unwrap();
        c.pragma_update(None, "user_version", version).unwrap();
    };
    set(112);
    assert!(matches!(upgrade_legacy(&path), Err(DbError::TooOldForImport { found: 112 })));
    assert!(!dir.path().join("anime.db.bak-v112").exists(), "refused before any copy");
    set(141);
    assert!(matches!(upgrade_legacy(&path), Err(DbError::TooNew { found: 141, supported: 140 })));
    set(140);
    let before = std::fs::read(&path).unwrap();
    assert_eq!(upgrade_legacy(&path).unwrap(), None);
    assert_eq!(std::fs::read(&path).unwrap(), before, "a 140 file is untouched");
    legacy_file(&path, 139);
    assert!(upgrade_legacy(&path).unwrap().is_some(), "139 runs the native step to 140");
    let c = Connection::open(&path).unwrap();
    assert_eq!(one(&c, "PRAGMA user_version"), 140);
    let indexes: i64 = c
        .query_row("SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name LIKE 'animes_categories_%_index'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(indexes, 2);
}

#[test]
fn a_failing_step_keeps_the_backup_and_the_version_of_the_step_before() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("anime.db");
    legacy_file(&path, 120);
    // step 121 adds a column that already exists: it fails, and the earlier stamp stays
    Connection::open(&path).unwrap().execute_batch("ALTER TABLE animes ADD COLUMN update_strategy INTEGER NOT NULL DEFAULT 0").unwrap();
    Connection::open(&path).unwrap().pragma_update(None, "user_version", 121).unwrap();
    assert!(upgrade_legacy(&path).is_err());
    assert_eq!(one(&Connection::open(&path).unwrap(), "PRAGMA user_version"), 121);
    assert!(dir.path().join("anime.db.bak-v121").exists());
}

#[test]
fn a_legacy_repo_becomes_a_store_that_resolves_to_its_own_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("anime.db");
    legacy_file(&path, 129);
    Connection::open(&path)
        .unwrap()
        .execute_batch("INSERT INTO extension_repos VALUES ('https://repo.test', 'Repo', NULL, 'https://repo.test/site', 'abc')")
        .unwrap();
    upgrade_legacy(&path).unwrap();
    let c = Connection::open(&path).unwrap();
    let row: (String, String, String, i64) = c
        .query_row("SELECT index_url, badge_label, resolved_index_url, is_legacy FROM extension_store", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap();
    assert_eq!(row, ("https://repo.test/repo.json".into(), "Repo".into(), "https://repo.test/repo.json".into(), 1));
}
