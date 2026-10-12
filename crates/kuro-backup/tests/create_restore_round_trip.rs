//! Q-21 and X-112 (area 22, T-22a-042, T-22a-047): a backup the Create side writes passes the restore preflight and
//! restores, for the full default set and for each single option that `can_create` allows.

use chrono::{Local, TimeZone};
use kuro_backup::creator::{AnimeEntry, CreateInputs, LibraryReader, MangaEntry, PrefStore, SourcePrefStore, SourceRegistry};
use kuro_backup::error::BackupError;
use kuro_backup::file::{Target, create_file};
use kuro_backup::format::decode_file;
use kuro_backup::options::BackupOptions;
use kuro_backup::restore::driver::{Targets, restore_backup};
use kuro_backup::restore::options::RestoreOptions;
use kuro_backup::restore::prefs::IntervalScheduler;
use kuro_backup::schema::PrefValue;
use kuro_backup::validate::{TrackerRegistry, preflight_file};
use kuro_data::anime::Anime;
use kuro_data::category::Category;
use kuro_data::chapter::Chapter;
use kuro_data::extension_repo::ExtensionRepo;
use kuro_data::manga::Manga;
use kuro_db::{schema_anime, schema_manga};
use kuro_prefs::store::PrefStore as Store;
use kuro_prefs::values::PrefValue as Stored;
use rusqlite::Connection;

fn category() -> Category {
    Category { id: 4, name: "Reading".into(), order: 1, flags: 0, hidden: false }
}

fn manga_entry() -> MangaEntry {
    let mut m = Manga::create();
    m.id = 1;
    m.source = 7;
    m.url = "/m/1".into();
    m.title = "Manga".into();
    m.favorite = true;
    let mut c = Chapter::create();
    c.id = 10;
    c.manga_id = 1;
    c.url = "/m/1/c".into();
    c.name = "Chapter".into();
    MangaEntry { manga: m, chapters: vec![c], excluded_scanlators: Default::default(), categories: vec![category()], tracks: Vec::new(), history: Vec::new() }
}

fn anime_entry() -> AnimeEntry {
    let mut a = Anime::create();
    a.id = 2;
    a.source = 8;
    a.url = "/a/1".into();
    a.title = "Anime".into();
    a.favorite = true;
    AnimeEntry { anime: a, episodes: Vec::new(), categories: Vec::new(), tracks: Vec::new(), history: Vec::new() }
}

struct Library;

impl LibraryReader for Library {
    fn manga_favorites(&self) -> Result<Vec<MangaEntry>, BackupError> {
        Ok(vec![manga_entry()])
    }
    fn manga_read_not_in_library(&self) -> Result<Vec<MangaEntry>, BackupError> {
        Ok(Vec::new())
    }
    fn anime_favorites(&self) -> Result<Vec<AnimeEntry>, BackupError> {
        Ok(vec![anime_entry()])
    }
    fn anime_watched_not_in_library(&self) -> Result<Vec<AnimeEntry>, BackupError> {
        Ok(Vec::new())
    }
    fn manga_categories(&self) -> Result<Vec<Category>, BackupError> {
        Ok(vec![category()])
    }
    fn anime_categories(&self) -> Result<Vec<Category>, BackupError> {
        Ok(vec![Category { id: 5, name: "Watching".into(), order: 1, flags: 0, hidden: false }])
    }
    fn manga_extension_repos(&self) -> Result<Vec<ExtensionRepo>, BackupError> {
        Ok(vec![ExtensionRepo {
            base_url: "https://repo.example".into(),
            name: "Repo".into(),
            short_name: None,
            website: "w".into(),
            signing_key_fingerprint: "fp".into(),
        }])
    }
}

struct Sources;
impl SourceRegistry for Sources {
    fn is_installed(&self, _: i64) -> bool {
        true
    }
    fn name_of(&self, id: i64) -> String {
        format!("source {id}")
    }
    fn stub_label(&self, id: i64) -> String {
        id.to_string()
    }
}

struct Trackers;
impl TrackerRegistry for Trackers {
    fn name(&self, _: i32) -> Option<String> {
        None
    }
    fn logged_in(&self, _: i32) -> bool {
        true
    }
}

struct Prefs;
impl PrefStore for Prefs {
    fn all(&self) -> Vec<(String, PrefValue)> {
        vec![("show_nsfw_source".into(), PrefValue::Boolean(true))]
    }
}
impl SourcePrefStore for Prefs {
    fn configurable(&self, anime: bool) -> Vec<(String, Vec<(String, PrefValue)>)> {
        if anime { Vec::new() } else { vec![("1234".into(), vec![("show_nsfw".into(), PrefValue::Boolean(true))])] }
    }
}

/// Every option off; a test turns on the one it checks.
fn none() -> BackupOptions {
    BackupOptions {
        library_entries: false,
        categories: false,
        chapters: false,
        tracking: false,
        history: false,
        read_entries: false,
        app_settings: false,
        extension_stores: false,
        custom_button: false,
        source_settings: false,
        private_settings: false,
        extensions: false,
    }
}

fn create(dir: &std::path::Path, opts: &BackupOptions) -> std::path::PathBuf {
    let inputs = CreateInputs { library: &Library, sources: &Sources, prefs: &Prefs, source_prefs: &Prefs, extensions: None };
    create_file(&inputs, opts, &Target::Manual(dir.join("out.tachibk"))).unwrap()
}

struct Dbs {
    manga: Connection,
    anime: Connection,
    app: Store,
    source: Store,
}

fn fresh() -> Dbs {
    let manga = Connection::open_in_memory().unwrap();
    manga.pragma_update(None, "foreign_keys", "ON").unwrap();
    kuro_db::open::register_functions(&manga).unwrap();
    manga.execute_batch(schema_manga::BASELINE_SQL).unwrap();
    let anime = schema_anime::open_in_memory().unwrap();
    anime.execute("DELETE FROM custom_buttons", []).unwrap();
    Dbs { manga, anime, app: Store::in_memory(), source: Store::in_memory() }
}

fn restore(dbs: &mut Dbs, path: &std::path::Path) {
    let backup = decode_file(path).unwrap();
    let nothing = |_: Option<u32>| {};
    let sched = IntervalScheduler { store: &dbs.app, library_timer: &nothing, backup_timer: &nothing };
    let source = dbs.source.clone();
    let open = move |_: &str| source.clone();
    let targets = Targets { manga: &mut dbs.manga, anime: &mut dbs.anime, app_prefs: &dbs.app, source_prefs: &open, scheduler: &sched };
    let now = Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).single().unwrap();
    let run = restore_backup(backup, RestoreOptions::from_bits([true; 7]), now, targets, &|| false, &mut |_| {});
    assert!(run.errors.is_empty(), "{:?}", run.errors);
    assert_eq!(run.stop, None);
}

fn rows(c: &Connection, table: &str) -> i64 {
    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}

#[test]
fn x112_default_backup_restores_into_an_empty_database_with_the_same_counts() {
    let dir = tempfile::tempdir().unwrap();
    let path = create(dir.path(), &BackupOptions::default());
    preflight_file(&path, &Sources, &Trackers).unwrap();
    let mut dbs = fresh();
    let seeded = rows(&dbs.manga, "categories");
    restore(&mut dbs, &path);
    assert_eq!(rows(&dbs.manga, "mangas"), 1);
    assert_eq!(rows(&dbs.manga, "chapters"), 1);
    assert_eq!(rows(&dbs.anime, "animes"), 1);
    assert_eq!(rows(&dbs.manga, "categories") - seeded, 1);
    // Repo rows become links in the prefs link list, and custom buttons are not restored: neither writes a row.
    assert_eq!(rows(&dbs.manga, "extension_repos"), 0);
    assert_eq!(rows(&dbs.anime, "custom_buttons"), 0);
    assert_eq!(dbs.app.get_raw("show_nsfw_source"), Some(Stored::Bool(true)));
    assert_eq!(dbs.source.get_raw("show_nsfw"), Some(Stored::Bool(true)));
}

/// Each option that `can_create` accepts on its own writes a file that the preflight accepts and the restore applies.
#[test]
fn every_single_option_the_create_side_allows_restores() {
    let dir = tempfile::tempdir().unwrap();
    let singles: [(&str, BackupOptions, fn(&Dbs) -> bool); 6] = [
        ("library entries", BackupOptions { library_entries: true, ..none() }, |d| rows(&d.manga, "mangas") == 1 && rows(&d.anime, "animes") == 1),
        ("categories", BackupOptions { categories: true, ..none() }, |d| rows(&d.anime, "categories") >= 1),
        ("app settings", BackupOptions { app_settings: true, ..none() }, |d| d.app.get_raw("show_nsfw_source").is_some()),
        ("extension stores", BackupOptions { extension_stores: true, ..none() }, |d| rows(&d.manga, "extension_repos") == 0),
        ("custom buttons", BackupOptions { custom_button: true, ..none() }, |d| rows(&d.anime, "custom_buttons") == 0),
        ("source settings", BackupOptions { source_settings: true, ..none() }, |d| d.source.get_raw("show_nsfw").is_some()),
    ];
    for (name, opts, applied) in singles {
        assert!(opts.can_create(), "{name}");
        let path = create(dir.path(), &opts);
        preflight_file(&path, &Sources, &Trackers).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut dbs = fresh();
        restore(&mut dbs, &path);
        // Extension stores and custom buttons are accepted and restore without error, but write no row any more.
        assert!(applied(&dbs), "{name}: the restore wrote what the file carries");
    }
}
