//! Integration tests (area 21, T-21a-040 section 4.11): a fake library, real files in a temp folder, and the decoder.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::{Duration as ChronoDuration, Local, TimeZone};
use kuro_backup::auto::{AutoConfig, AutoHost, PowerState, run_loop};
use kuro_backup::creator::{AnimeEntry, CreateInputs, LibraryReader, MangaEntry, PrefStore, SourcePrefStore, SourceRegistry, build_backup};
use kuro_backup::error::BackupError;
use kuro_backup::file::{MAX_AUTO_BACKUPS, Target, create_file, write_backup};
use kuro_backup::format::decode_file;
use kuro_backup::job::JobSlot;
use kuro_backup::options::BackupOptions;
use kuro_backup::schema::PrefValue;
use kuro_data::category::Category;
use kuro_data::chapter::Chapter;
use kuro_data::manga::Manga;
use kuro_db::statements_anime::{CustomButton, ExtensionStore};
use kuro_data::extension_repo::ExtensionRepo;
use tokio::sync::watch;

struct Library {
    manga: Vec<MangaEntry>,
}

impl LibraryReader for Library {
    fn manga_favorites(&self) -> Result<Vec<MangaEntry>, BackupError> {
        Ok(self.manga.iter().map(copy).collect())
    }
    fn manga_read_not_in_library(&self) -> Result<Vec<MangaEntry>, BackupError> {
        Ok(Vec::new())
    }
    fn anime_favorites(&self) -> Result<Vec<AnimeEntry>, BackupError> {
        Ok(Vec::new())
    }
    fn anime_watched_not_in_library(&self) -> Result<Vec<AnimeEntry>, BackupError> {
        Ok(Vec::new())
    }
    fn manga_categories(&self) -> Result<Vec<Category>, BackupError> {
        Ok(vec![Category { id: 4, name: "Reading".into(), order: 1, flags: 0, hidden: false }])
    }
    fn anime_categories(&self) -> Result<Vec<Category>, BackupError> {
        Ok(Vec::new())
    }
    fn manga_extension_repos(&self) -> Result<Vec<ExtensionRepo>, BackupError> {
        Ok(Vec::new())
    }
    fn anime_extension_stores(&self) -> Result<Vec<ExtensionStore>, BackupError> {
        Ok(Vec::new())
    }
    fn custom_buttons(&self) -> Result<Vec<CustomButton>, BackupError> {
        Ok(Vec::new())
    }
}

fn copy(e: &MangaEntry) -> MangaEntry {
    MangaEntry {
        manga: e.manga.clone(),
        chapters: e.chapters.clone(),
        excluded_scanlators: e.excluded_scanlators.clone(),
        categories: e.categories.clone(),
        tracks: e.tracks.clone(),
        history: e.history.clone(),
    }
}

struct NoSources;
impl SourceRegistry for NoSources {
    fn is_installed(&self, _: i64) -> bool {
        false
    }
    fn name_of(&self, id: i64) -> String {
        format!("source {id}")
    }
    fn stub_label(&self, id: i64) -> String {
        id.to_string()
    }
}

struct NoPrefs;
impl PrefStore for NoPrefs {
    fn all(&self) -> Vec<(String, PrefValue)> {
        Vec::new()
    }
}
impl SourcePrefStore for NoPrefs {
    fn configurable(&self, _: bool) -> Vec<(String, Vec<(String, PrefValue)>)> {
        Vec::new()
    }
}

fn one_manga(chapters: usize) -> MangaEntry {
    let mut m = Manga::create();
    m.id = 1;
    m.source = 7;
    m.url = "/m/1".into();
    m.title = "Title".into();
    m.favorite = true;
    let chapters = (0..chapters)
        .map(|i| {
            let mut c = Chapter::create();
            c.id = 10 + i as i64;
            c.manga_id = 1;
            c.url = format!("/c/{i}");
            c.name = format!("Chapter {i}");
            c
        })
        .collect();
    MangaEntry {
        manga: m,
        chapters,
        excluded_scanlators: Default::default(),
        categories: vec![Category { id: 4, name: "Reading".into(), order: 1, flags: 0, hidden: false }],
        tracks: Vec::new(),
        history: Vec::new(),
    }
}

fn inputs<'a>(lib: &'a Library, sources: &'a NoSources, prefs: &'a NoPrefs) -> CreateInputs<'a> {
    CreateInputs { library: lib, sources, prefs, source_prefs: prefs, extensions: None }
}

#[test]
fn manual_backup_round_trips_through_the_decoder() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library { manga: vec![one_manga(2)] };
    let target = dir.path().join("manual.tachibk");
    let written = create_file(&inputs(&lib, &NoSources, &NoPrefs), &BackupOptions::default(), &Target::Manual(target.clone())).unwrap();
    assert_eq!(written, target);
    let decoded = decode_file(&target).unwrap();
    assert_eq!(decoded.manga.len(), 1);
    assert_eq!(decoded.manga[0].chapters.len(), 2);
    assert_eq!(decoded.manga[0].categories, vec![1]);
    assert_eq!(decoded.categories.len(), 1);
    assert!(!decoded.is_legacy);
    let names: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["manual.tachibk".to_string()], "no .part file is left behind");
}

#[test]
fn automatic_runs_keep_the_newest_four() {
    let dir = tempfile::tempdir().unwrap();
    let auto_dir = dir.path().join("autobackup");
    let lib = Library { manga: vec![one_manga(1)] };
    let backup = build_backup(&inputs(&lib, &NoSources, &NoPrefs), &BackupOptions::default()).unwrap();
    let start = Local.with_ymd_and_hms(2026, 10, 9, 8, 0, 0).unwrap();
    for i in 0..6 {
        let now = start + ChronoDuration::minutes(i);
        write_backup(&backup, &Target::Automatic { dir: auto_dir.clone(), now }).unwrap();
    }
    let mut names: Vec<String> = std::fs::read_dir(&auto_dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names.len(), MAX_AUTO_BACKUPS);
    assert_eq!(names.first().map(String::as_str), Some("kurobayomi_2026-10-09_08-02-00.tachibk"));
    assert_eq!(names.last().map(String::as_str), Some("kurobayomi_2026-10-09_08-05-00.tachibk"));
}

struct Silent;
impl kuro_platform::notifier::Notifier for Silent {
    fn show(&self, _: &str, _: &str, _: Option<&str>) {}
    fn withdraw(&self, _: &str) {}
}

struct CountingHost {
    runs: AtomicU32,
    restore: bool,
    last: Mutex<i64>,
    dir: PathBuf,
}

impl AutoHost for CountingHost {
    fn now_ms(&self) -> i64 {
        1_000_000_000_000
    }
    fn last_auto_ms(&self) -> i64 {
        *self.last.lock().unwrap()
    }
    fn restore_running(&self) -> bool {
        self.restore
    }
    fn power(&self) -> PowerState {
        PowerState::default()
    }
    fn run_backup(&self) -> Result<PathBuf, BackupError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let lib = Library { manga: vec![one_manga(0)] };
        let backup = build_backup(&inputs(&lib, &NoSources, &NoPrefs), &BackupOptions::default())?;
        let now = Local::now();
        write_backup(&backup, &Target::Automatic { dir: self.dir.clone(), now })
    }
    fn save_last_auto_ms(&self, ms: i64) {
        *self.last.lock().unwrap() = ms;
    }
    fn report_failure(&self, _streak: u32) {}
}

#[tokio::test(start_paused = true)]
async fn restore_running_defers_the_automatic_run_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let host = Arc::new(CountingHost { runs: AtomicU32::new(0), restore: true, last: Mutex::new(0), dir: dir.path().join("autobackup") });
    let (tx, rx) = watch::channel(AutoConfig { interval_hours: 6, last_auto_epoch_ms: 0, enabled_at_epoch_ms: 1, min_battery_percent: 15 });
    let task = tokio::spawn(run_loop(rx, JobSlot::new(), Arc::clone(&host), Arc::new(Silent)));
    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    drop(tx);
    task.await.unwrap();
    assert_eq!(host.runs.load(Ordering::SeqCst), 0);
    assert!(!host.dir.exists() || std::fs::read_dir(&host.dir).unwrap().next().is_none());
}

struct Prefs;
impl PrefStore for Prefs {
    fn all(&self) -> Vec<(String, PrefValue)> {
        vec![
            ("pref_theme_mode_key".into(), PrefValue::String("SYSTEM".into())),
            ("__PRIVATE_track_token_2".into(), PrefValue::String("secret".into())),
            ("__APP_STATE_last_auto_backup_timestamp".into(), PrefValue::Long(5)),
        ]
    }
}
impl SourcePrefStore for Prefs {
    fn configurable(&self, anime: bool) -> Vec<(String, Vec<(String, PrefValue)>)> {
        if anime {
            Vec::new()
        } else {
            vec![("source_7".into(), vec![("quality".into(), PrefValue::Int(2))])]
        }
    }
}

/// T-21a-044: default options write a backup that carries the app and source preferences; `__PRIVATE_` keys appear
/// only when the sensitive-settings option is on; `__APP_STATE_` keys never appear.
#[test]
fn default_options_write_preferences_and_private_keys_follow_the_option() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library { manga: vec![one_manga(1)] };
    let inputs = CreateInputs { library: &lib, sources: &NoSources, prefs: &Prefs, source_prefs: &Prefs, extensions: None };

    let plain = dir.path().join("plain.tachibk");
    create_file(&inputs, &BackupOptions::default(), &Target::Manual(plain.clone())).unwrap();
    let decoded = decode_file(&plain).unwrap();
    let keys: Vec<&str> = decoded.preferences.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(keys, vec!["pref_theme_mode_key"]);
    assert_eq!(decoded.preferences[0].value, PrefValue::String("SYSTEM".into()));
    assert_eq!(decoded.source_preferences.len(), 1);
    assert_eq!(decoded.source_preferences[0].prefs[0].value, PrefValue::Int(2));

    let sensitive = dir.path().join("sensitive.tachibk");
    let opts = BackupOptions { private_settings: true, ..BackupOptions::default() };
    create_file(&inputs, &opts, &Target::Manual(sensitive.clone())).unwrap();
    let decoded = decode_file(&sensitive).unwrap();
    let keys: Vec<&str> = decoded.preferences.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(keys, vec!["pref_theme_mode_key", "__PRIVATE_track_token_2"]);
}
