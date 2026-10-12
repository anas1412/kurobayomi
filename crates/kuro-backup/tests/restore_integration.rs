//! Restore into real databases built from the area 03 and 04 schemas (area 22, T-22a-042, p1-edge-cases-and-tests.md
//! section 8, X-06, X-07, X-100 to X-102, X-108). Fixtures come from `fixtures_gen`, so nothing depends on Kotlin files.

mod fixtures_gen;

use std::cell::Cell;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use chrono::{Local, TimeZone};
use kuro_backup::format::decode_file;
use kuro_backup::restore::driver::{Targets, counts, restore_backup};
use kuro_backup::restore::options::RestoreOptions;
use kuro_backup::restore::prefs::IntervalScheduler;
use kuro_backup::restore::sections::Run;
use kuro_db::statements_anime;
use kuro_db::statements_manga::manga_sync;
use kuro_db::{schema_anime, schema_manga};
use kuro_prefs::store::PrefStore;
use rusqlite::Connection;

struct Dbs {
    manga: Connection,
    anime: Connection,
    app: PrefStore,
}

/// In-memory SQLite built from the area 03 and 04 schemas (T-22a-042).
fn fresh() -> Dbs {
    // The manga schema has no public in-memory opener, so the baseline is applied here with the same pragmas.
    let manga = Connection::open_in_memory().unwrap();
    manga.pragma_update(None, "foreign_keys", "ON").unwrap();
    kuro_db::open::register_functions(&manga).unwrap();
    manga.execute_batch(schema_manga::BASELINE_SQL).unwrap();
    Dbs { manga, anime: schema_anime::open_in_memory().unwrap(), app: PrefStore::in_memory() }
}

fn restore(dbs: &mut Dbs, path: &Path, opts: RestoreOptions, cancel: &dyn Fn() -> bool) -> Run {
    let backup = decode_file(path).unwrap();
    let library = |_: Option<u32>| {};
    let sched = IntervalScheduler { store: &dbs.app, library_timer: &library, backup_timer: &library };
    let open = |_: &str| PrefStore::in_memory();
    let now = Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).single().unwrap();
    let targets = Targets { manga: &mut dbs.manga, anime: &mut dbs.anime, app_prefs: &dbs.app, source_prefs: &open, scheduler: &sched };
    restore_backup(backup, opts, now, targets, cancel, &mut |_| {})
}

fn all_on() -> RestoreOptions {
    RestoreOptions { extensions: false, ..RestoreOptions::from_bits([true; 7]) }
}

fn never() -> bool {
    false
}

fn count(c: &Connection, table: &str) -> usize {
    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get::<_, i64>(0)).unwrap() as usize
}

fn expected(fixtures: &[fixtures_gen::Fixture], index: usize, key: &str) -> usize {
    fixtures[index].expected[key].as_u64().unwrap_or_else(|| panic!("no {key}")) as usize
}

fn write_fixtures(dir: &Path) -> Vec<fixtures_gen::Fixture> {
    fixtures_gen::write(dir);
    fixtures_gen::all()
}

#[test]
fn x100_mihon_file_restores_into_an_empty_database() {
    let dir = tempfile::tempdir().unwrap();
    let _ = write_fixtures(dir.path());
    let mut dbs = fresh();
    let seeded_categories = count(&dbs.manga, "categories");
    let run = restore(&mut dbs, &dir.path().join("mihon_1x_small.tachibk"), all_on(), &never);
    assert!(run.errors.is_empty(), "{:?}", run.errors);
    assert_eq!(count(&dbs.manga, "mangas"), 3);
    assert_eq!(count(&dbs.manga, "chapters"), 6);
    assert_eq!(count(&dbs.manga, "categories") - seeded_categories, 1);
    assert_eq!(count(&dbs.manga, "history"), 1);
    assert_eq!(manga_sync::get_tracks(&dbs.manga).unwrap().len(), 1);
}

#[test]
fn x101_current_file_restores_seasons_buttons_and_stores() {
    let dir = tempfile::tempdir().unwrap();
    let fixtures = write_fixtures(dir.path());
    let path = dir.path().join("aniyomi_current.tachibk");
    let backup = decode_file(&path).unwrap();
    let mut dbs = fresh();
    // An empty library: the schema's default button is a favourite, which would block the file's favourites (D10).
    dbs.anime.execute("DELETE FROM custom_buttons", []).unwrap();
    let run = restore(&mut dbs, &path, all_on(), &never);
    assert!(run.errors.is_empty(), "{:?}", run.errors);
    assert_eq!(counts(&backup).anime_entries as usize, expected(&fixtures, 2, "anime"));
    assert_eq!(count(&dbs.anime, "animes"), expected(&fixtures, 2, "anime"));
    assert_eq!(count(&dbs.manga, "mangas"), expected(&fixtures, 2, "manga"));
    // The file carries stores and buttons, but the restore writes neither: stores become links, buttons are not restored.
    assert_eq!(counts(&backup).anime_stores as usize, expected(&fixtures, 2, "extension_stores"));
    assert_eq!(backup.custom_buttons.len(), expected(&fixtures, 2, "custom_buttons"));
    assert_eq!(statements_anime::extension_store_count(&dbs.anime).unwrap(), 0);
    assert!(statements_anime::custom_buttons(&dbs.anime).unwrap().is_empty());

    // Seasons point at the database id of their root.
    let roots = backup.anime.iter().filter(|a| a.parent_id.is_none()).count();
    assert_eq!(roots + expected(&fixtures, 2, "seasons"), expected(&fixtures, 2, "anime"));
    let seasons: Vec<i64> = dbs
        .anime
        .prepare("SELECT parent_id FROM animes WHERE parent_id IS NOT NULL")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(seasons.len(), expected(&fixtures, 2, "seasons"));
    assert!(seasons.iter().all(|p| *p > 0));
}

#[test]
fn x06_legacy_file_restores_with_the_expected_counts() {
    let dir = tempfile::tempdir().unwrap();
    let fixtures = write_fixtures(dir.path());
    let mut dbs = fresh();
    let run = restore(&mut dbs, &dir.path().join("aniyomi_legacy_0x.tachibk"), all_on(), &never);
    assert!(run.errors.is_empty(), "{:?}", run.errors);
    assert_eq!(count(&dbs.anime, "animes"), expected(&fixtures, 1, "anime"));
    assert_eq!(count(&dbs.manga, "mangas"), expected(&fixtures, 1, "manga"));
}

#[test]
fn x102_second_run_of_the_same_file_adds_no_entries() {
    let dir = tempfile::tempdir().unwrap();
    let _ = write_fixtures(dir.path());
    let path = dir.path().join("aniyomi_current.tachibk");
    let mut dbs = fresh();
    restore(&mut dbs, &path, all_on(), &never);
    let manga_rows = count(&dbs.manga, "mangas");
    let anime_rows = count(&dbs.anime, "animes");
    let manga_before = dbs.manga.total_changes();
    let anime_before = dbs.anime.total_changes();
    let run = restore(&mut dbs, &path, all_on(), &never);
    assert!(run.errors.is_empty(), "{:?}", run.errors);
    assert_eq!(count(&dbs.manga, "mangas"), manga_rows);
    assert_eq!(count(&dbs.anime, "animes"), anime_rows);
    assert_eq!(dbs.manga.total_changes() - manga_before, 0, "no manga write on the second run");
    assert_eq!(dbs.anime.total_changes() - anime_before, 0, "no anime write on the second run");
}

#[test]
fn x07_path_with_spaces_and_invalid_utf8_restores() {
    let dir = tempfile::tempdir().unwrap();
    let _ = write_fixtures(dir.path());
    let source = std::fs::read(dir.path().join("mihon_1x_small.tachibk")).unwrap();
    let odd = dir.path().join(std::ffi::OsStr::from_bytes(b"my backup \xff caf\xc3\xa9.tachibk"));
    std::fs::write(&odd, source).unwrap();
    let mut dbs = fresh();
    let run = restore(&mut dbs, &odd, all_on(), &never);
    assert!(run.errors.is_empty());
    assert_eq!(count(&dbs.manga, "mangas"), 3);
}

#[test]
fn content_is_sniffed_not_the_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let _ = write_fixtures(dir.path());
    let renamed = dir.path().join("random.txt");
    std::fs::copy(dir.path().join("aniyomi_current.tachibk"), &renamed).unwrap();
    let mut dbs = fresh();
    let run = restore(&mut dbs, &renamed, all_on(), &never);
    assert!(run.errors.is_empty());
    assert!(count(&dbs.manga, "mangas") > 0);
}

#[test]
fn cancel_before_the_first_entry_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let _ = write_fixtures(dir.path());
    let mut dbs = fresh();
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        true
    };
    let run = restore(&mut dbs, &dir.path().join("mihon_1x_small.tachibk"), all_on(), &cancel);
    assert!(run.stop.is_some());
    assert_eq!(count(&dbs.manga, "mangas"), 0);
}

// ---- X-103 to X-113: the fixtures restored into a database (area 22, p1-edge-cases-and-tests.md section 8) ----

use kuro_backup::schema::{Backup, BackupAnime, BackupChapter, BackupManga};
use kuro_backup::wire::Message;
use kuro_db::statements_manga::{chapters, mangas};
use kuro_prefs::values::PrefValue as Stored;

fn restore_fixture(dbs: &mut Dbs, dir: &Path, name: &str) -> Run {
    let run = restore(dbs, &dir.join(name), all_on(), &never);
    assert!(run.errors.is_empty(), "{name}: {:?}", run.errors);
    run
}

fn restore_value(dbs: &mut Dbs, dir: &Path, backup: &Backup) -> Run {
    let path = dir.join("value.tachibk");
    std::fs::write(&path, backup.to_bytes().unwrap()).unwrap();
    let run = restore(dbs, &path, all_on(), &never);
    assert!(run.errors.is_empty(), "{:?}", run.errors);
    run
}

fn fixtures_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fixtures_gen::write(dir.path());
    dir
}

#[test]
fn x103_duplicate_category_names_make_one_row_and_both_links_point_at_it() {
    let dir = fixtures_dir();
    let mut dbs = fresh();
    let seeded = count(&dbs.manga, "categories");
    restore_fixture(&mut dbs, dir.path(), "dup_categories.tachibk");
    assert_eq!(count(&dbs.manga, "categories") - seeded, 1, "E-16: merged by name");
    assert_eq!(count(&dbs.manga, "mangas_categories"), 1, "both links map to the one row");
}

fn season_state(c: &Connection) -> Vec<(String, Option<String>)> {
    c.prepare("SELECT a.url, p.url FROM animes a LEFT JOIN animes p ON a.parent_id = p._id ORDER BY a.url")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn x104_seasons_in_either_file_order_give_the_same_state() {
    let dir = fixtures_dir();
    let mut states = Vec::new();
    for name in ["seasons_parent_first.tachibk", "seasons_season_first.tachibk"] {
        let mut dbs = fresh();
        dbs.anime.execute("DELETE FROM custom_buttons", []).unwrap();
        restore_fixture(&mut dbs, dir.path(), name);
        states.push(season_state(&dbs.anime));
    }
    assert_eq!(states[0], states[1]);
    assert_eq!(states[0], vec![("/a/1".to_string(), None), ("/a/2".to_string(), Some("/a/1".to_string()))]);
}

#[test]
fn x105_float_chapter_numbers_and_scores_are_stored() {
    let dir = fixtures_dir();
    let mut dbs = fresh();
    restore_fixture(&mut dbs, dir.path(), "float_numbers.tachibk");
    let id = mangas::get_by_url_and_source(&dbs.manga, "/m/1", 7).unwrap().unwrap().id;
    let mut numbers: Vec<f64> = chapters::get_by_manga_id(&dbs.manga, id, false).unwrap().iter().map(|c| c.chapter_number).collect();
    numbers.sort_by(f64::total_cmp);
    assert_eq!(numbers, vec![1.1, 2.5, 10.0]);
    let mut scores: Vec<f64> = manga_sync::get_tracks(&dbs.manga).unwrap().iter().map(|t| t.score).collect();
    scores.sort_by(f64::total_cmp);
    assert_eq!(scores, vec![f64::from(7.3_f32), 8.5]);
}

#[test]
fn x106_preferences_skip_app_state_and_a_type_mismatch() {
    let dir = fixtures_dir();
    let mut dbs = fresh();
    dbs.app.set_raw("pref_theme_mode_key", Stored::Str("SYSTEM".into()));
    restore_fixture(&mut dbs, dir.path(), "prefs_mixed.tachibk");
    assert_eq!(dbs.app.get_raw("__APP_STATE_last_auto_backup_timestamp"), None);
    assert_eq!(dbs.app.get_raw("pref_theme_mode_key"), Some(Stored::Str("SYSTEM".into())), "an Int never replaces a String");
}

#[test]
fn x107_repo_rows_are_not_written_so_there_are_no_conflicts_and_their_urls_become_links() {
    let dir = fixtures_dir();
    let mut dbs = fresh();
    let path = dir.path().join("repos_dup.tachibk");
    let run = restore(&mut dbs, &path, all_on(), &never);
    assert!(run.errors.is_empty(), "{:?}", run.errors);
    assert_eq!(count(&dbs.manga, "extension_repos"), 0);
    let backup = decode_file(&path).unwrap();
    let urls = kuro_backup::restore::extensions::legacy_repo_urls(&backup);
    assert!(backup.manga_extension_repos.iter().all(|r| urls.contains(&r.base_url)));
}

#[test]
fn x108_custom_buttons_in_the_file_are_not_restored() {
    let dir = fixtures_dir();
    let fixtures = fixtures_gen::all();
    let mut dbs = fresh();
    dbs.anime.execute("DELETE FROM custom_buttons", []).unwrap();
    let path = dir.path().join("buttons_two_fav.tachibk");
    // The file still carries the two favourite buttons; the restore no longer writes custom buttons at all.
    assert_eq!(decode_file(&path).unwrap().custom_buttons.len(), expected(&fixtures, 15, "custom_buttons"));
    restore_fixture(&mut dbs, dir.path(), "buttons_two_fav.tachibk");
    assert!(statements_anime::custom_buttons(&dbs.anime).unwrap().is_empty());
}

#[test]
fn x109_history_stays_on_the_manga_it_belongs_to() {
    let dir = fixtures_dir();
    let mut dbs = fresh();
    restore_fixture(&mut dbs, dir.path(), "history_collision.tachibk");
    let owners: Vec<String> = dbs
        .manga
        .prepare("SELECT m.url FROM history h JOIN chapters c ON c._id = h.chapter_id JOIN mangas m ON m._id = c.manga_id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(owners, vec!["/b".to_string()]);
}

#[test]
fn x110_a_manga_without_the_favorite_field_is_restored_as_a_favourite() {
    let dir = fixtures_dir();
    let mut dbs = fresh();
    restore_fixture(&mut dbs, dir.path(), "no_favorite_field.tachibk");
    assert!(mangas::get_by_url_and_source(&dbs.manga, "/m/9", 7).unwrap().unwrap().favorite);
}

#[test]
fn x111_apk_extensions_are_skipped_without_errors_or_rows() {
    let dir = fixtures_dir();
    let mut dbs = fresh();
    let all_with_extensions = RestoreOptions::from_bits([true; 7]);
    let run = restore(&mut dbs, &dir.path().join("apk_ext.tachibk"), all_with_extensions, &never);
    assert!(run.errors.is_empty(), "{:?}", run.errors);
    assert_eq!(run.stop, None);
    assert_eq!(count(&dbs.manga, "mangas"), 0);
    assert_eq!(count(&dbs.anime, "animes"), 0);
}

fn versioned_manga(version: i64, title: &str, author: &str, read: bool) -> BackupManga {
    BackupManga {
        source: 7,
        url: "/m/v".into(),
        title: title.into(),
        author: Some(author.into()),
        version,
        chapters: vec![BackupChapter { url: "/m/v/1".into(), name: "c1".into(), read, chapter_number: 1.0, version, ..Default::default() }],
        ..Default::default()
    }
}

fn versioned_anime(version: i64, title: &str, author: &str) -> BackupAnime {
    BackupAnime { source: 8, url: "/a/v".into(), title: title.into(), author: Some(author.into()), version, ..Default::default() }
}

#[test]
fn x113_restore_over_rows_that_are_newer_or_older_than_the_file() {
    let dir = tempfile::tempdir().unwrap();
    // (backup version, expected author, expected title, expected stored version): the database row is version 5.
    let cases = [(9, "backup", "db title", 9), (2, "db", "backup title", 5), (5, "db", "backup title", 5)];
    for (version, author, title, stored) in cases {
        let mut dbs = fresh();
        dbs.anime.execute("DELETE FROM custom_buttons", []).unwrap();
        let seed = Backup {
            manga: vec![versioned_manga(5, "db title", "db", true)],
            anime: vec![versioned_anime(5, "db title", "db")],
            ..Default::default()
        };
        restore_value(&mut dbs, dir.path(), &seed);
        let file = Backup {
            manga: vec![versioned_manga(version, "backup title", "backup", false)],
            anime: vec![versioned_anime(version, "backup title", "backup")],
            ..Default::default()
        };
        restore_value(&mut dbs, dir.path(), &file);

        let m = mangas::get_by_url_and_source(&dbs.manga, "/m/v", 7).unwrap().unwrap();
        assert_eq!((m.author.as_deref(), m.title.as_str(), m.version), (Some(author), title, stored), "manga, backup version {version}");
        let a = statements_anime::anime_by_url_and_source(&dbs.anime, "/a/v", 8).unwrap().unwrap();
        assert_eq!((a.author.as_deref(), a.title.as_str(), a.version), (Some(author), title, stored), "anime, backup version {version}");
        // Read progress never goes backwards, whichever side is newer.
        let ch = chapters::get_by_manga_id(&dbs.manga, m.id, false).unwrap();
        assert!(ch[0].read, "backup version {version}: a read chapter stays read");
        assert_eq!(count(&dbs.manga, "mangas"), 1);
        assert_eq!(count(&dbs.anime, "animes"), 1);
    }
}
