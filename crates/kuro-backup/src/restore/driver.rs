//! The restore run against real stores (area 22, T-22a-007, T-22a-011, T-22a-023). `DbRestorer` plugs the section
//! writers into `run_sections`: one transaction per entry, one `(source, url)` snapshot per entry section, one run
//! clock, and the category maps handed from the categories section to the entries and the preferences.

use std::collections::HashMap;

use chrono::{DateTime, Local};
use kuro_db::{DbResult, Transaction};
use kuro_prefs::import::CategoryMaps;
use kuro_prefs::store::PrefStore;
use rusqlite::Connection;

use super::RestoreError;
use super::anime::{AnimeOutcome, existing_pairs as anime_pairs, plan_anime, restore_anime_entry, sort_anime};
use super::categories::{CategoryRestore, Kind, restore_categories};
use super::extensions::{restore_anime_store, restore_custom_buttons, restore_manga_repo};
use super::manga::{existing_pairs as manga_pairs, restore_manga_entry, sort_manga};
use super::merge::RunClock;
use super::options::RestoreOptions;
use super::prefs::{Scheduler, category_maps, restore_app_prefs, restore_source_prefs};
use super::progress::{Progress, ProgressEvent, amount};
use super::sections::{Counts, Item, Restorer, Run, Section, plan, run_sections};
use crate::text;
use crate::schema::{Backup, BackupAnime, BackupAnimeSource, BackupManga, BackupSource};

/// Where a restore writes. The caller owns the connections and the stores.
pub struct Targets<'a> {
    pub manga: &'a mut Connection,
    pub anime: &'a mut Connection,
    pub app_prefs: &'a PrefStore,
    /// Returns the store for a name such as `source_1234`.
    pub source_prefs: &'a dyn Fn(&str) -> PrefStore,
    pub scheduler: &'a dyn Scheduler,
}

/// Item counts of a decoded backup. Seasons are rows of `anime`, so they are already in the entry count.
pub fn counts(b: &Backup) -> Counts {
    Counts {
        anime_entries: len(&b.anime),
        manga_entries: len(&b.manga),
        anime_stores: len(&b.anime_extension_stores),
        manga_repos: len(&b.manga_extension_repos),
    }
}

fn len<T>(v: &[T]) -> u32 {
    u32::try_from(v.len()).unwrap_or(u32::MAX)
}

/// Runs the whole restore for `opts`. Progress reaches the amount for every option set (X-72, X-73).
pub fn restore_backup(
    backup: Backup,
    opts: RestoreOptions,
    now: DateTime<Local>,
    targets: Targets<'_>,
    cancel: &dyn Fn() -> bool,
    emit: &mut dyn FnMut(ProgressEvent),
) -> Run {
    let c = counts(&backup);
    let mut progress = Progress::new(amount(opts, c));
    let mut restorer = DbRestorer {
        backup,
        t: targets,
        skipped_settings: 0,
        clock: RunClock::new(now),
        cancel,
        manga_by_order: HashMap::new(),
        anime_by_order: HashMap::new(),
        display: None,
        manga_rows: None,
        anime_rows: None,
        parent: None,
    };
    run_sections(&plan(opts, c), &mut restorer, cancel, &mut progress, emit)
}

struct DbRestorer<'a> {
    backup: Backup,
    t: Targets<'a>,
    /// Keys the app-preference allowlist skipped (T-22a-052).
    skipped_settings: usize,
    clock: RunClock,
    /// Checked again just before an entry commits (F-01).
    cancel: &'a dyn Fn() -> bool,
    manga_by_order: HashMap<i64, i64>,
    anime_by_order: HashMap<i64, i64>,
    display: Option<bool>,
    /// Sorted once, when the section starts: the snapshot of the table is read then and not again.
    manga_rows: Option<Vec<BackupManga>>,
    /// Sorted rows, each root followed by its seasons. The flag is true for a root.
    anime_rows: Option<Vec<(BackupAnime, bool)>>,
    /// Database id of the root whose seasons are running, `None` when that root failed.
    parent: Option<i64>,
}

fn in_tx<T>(c: &mut Connection, f: impl FnOnce(&Transaction<'_>) -> DbResult<T>) -> DbResult<T> {
    let tx = c.transaction()?;
    let value = f(&tx)?;
    tx.commit()?;
    Ok(value)
}

/// Like `in_tx`, but a run canceled by the time the writes are done drops the transaction unchanged: the entry is
/// rolled back and `Ok(None)` tells the caller to stop without an error line (F-01, D4 fix).
fn in_entry_tx<T>(
    c: &mut Connection,
    cancel: &dyn Fn() -> bool,
    f: impl FnOnce(&Transaction<'_>) -> DbResult<T>,
) -> DbResult<Option<T>> {
    let tx = c.transaction()?;
    let value = f(&tx)?;
    if cancel() {
        return Ok(None);
    }
    tx.commit()?;
    Ok(Some(value))
}

/// An item that failed rolls back by itself (its transaction is dropped) and becomes a log line `prefix: message`.
fn done_as(title: String, prefix: String, result: Result<(), String>) -> Result<Item, RestoreError> {
    Ok(Item { error: result.err().map(|e| format!("{prefix}: {e}")), title })
}

fn done(title: String, result: Result<(), String>) -> Result<Item, RestoreError> {
    done_as(title.clone(), title, result)
}

/// The source name in an entry error, as the backup names it (D1 fix: anime use the anime list). An id the backup
/// does not name is shown as the number.
fn manga_source_name(sources: &[BackupSource], id: i64) -> String {
    sources.iter().find(|s| s.source_id == id).map_or_else(|| id.to_string(), |s| s.name.clone())
}

fn anime_source_name(sources: &[BackupAnimeSource], id: i64) -> String {
    sources.iter().find(|s| s.source_id == id).map_or_else(|| id.to_string(), |s| s.name.clone())
}

impl DbRestorer<'_> {
    fn categories(&mut self, index: u32) -> Result<Item, RestoreError> {
        let (kind, title) = if index == 0 { (Kind::Anime, text::PROGRESS_ANIME_CATEGORIES.english) } else { (Kind::Manga, text::PROGRESS_MANGA_CATEGORIES.english) };
        let (conn, list) = match kind {
            Kind::Anime => (&mut *self.t.anime, &self.backup.anime_categories),
            Kind::Manga => (&mut *self.t.manga, &self.backup.categories),
        };
        let result = in_tx(conn, |tx| restore_categories(tx, kind, list)).map_err(|e| e.to_string());
        done(title.into(), result.map(|CategoryRestore { by_order, categorized_display }| {
            self.display = match (self.display, categorized_display) {
                (Some(a), Some(b)) => Some(a || b),
                (a, b) => a.or(b),
            };
            match kind {
                Kind::Anime => self.anime_by_order = by_order,
                Kind::Manga => self.manga_by_order = by_order,
            }
        }))
    }

    fn app_settings(&mut self) -> Result<Item, RestoreError> {
        // Anime preference keys name anime category ids, so the anime half of the maps comes from the anime list.
        let maps = category_maps(self.t.manga, self.t.anime, &self.backup.categories).and_then(|m| {
            category_maps(self.t.manga, self.t.anime, &self.backup.anime_categories)
                .map(|a| CategoryMaps { manga: m.manga, anime: a.anime })
        });
        let result = maps.map_err(|e| e.to_string()).map(|maps| {
            let outcome = restore_app_prefs(self.t.app_prefs, &self.backup.preferences, &maps, self.display, self.t.scheduler);
            self.skipped_settings = outcome.skipped;
        });
        done(text::APP_SETTINGS.english.into(), result)
    }

    fn source_settings(&mut self) -> Result<Item, RestoreError> {
        restore_source_prefs(self.t.source_prefs, &self.backup.source_preferences);
        done(text::SOURCE_SETTINGS.english.into(), Ok(()))
    }

    fn manga_entry(&mut self, index: u32) -> Result<Item, RestoreError> {
        if self.manga_rows.is_none() {
            // The snapshot only orders the rows. If it cannot be read, the backup order stands.
            let existing = in_tx(self.t.manga, |tx| manga_pairs(tx)).unwrap_or_default();
            self.manga_rows = Some(sort_manga(std::mem::take(&mut self.backup.manga), &existing));
        }
        let entry = std::mem::take(&mut self.manga_rows.as_mut().expect("set above")[index as usize]);
        let prefix = format!("{} [{}]", entry.title, manga_source_name(&self.backup.sources, entry.source));
        let result = in_entry_tx(self.t.manga, self.cancel, |tx| restore_manga_entry(tx, &self.clock, &entry, &self.manga_by_order));
        match result {
            Ok(Some(_)) => done_as(entry.title, prefix, Ok(())),
            Ok(None) => Err(RestoreError::Cancelled),
            Err(e) => done_as(entry.title, prefix, Err(e.to_string())),
        }
    }

    fn anime_entry(&mut self, index: u32) -> Result<Item, RestoreError> {
        if self.anime_rows.is_none() {
            let existing = in_tx(self.t.anime, |tx| anime_pairs(tx)).unwrap_or_default();
            let sorted = sort_anime(std::mem::take(&mut self.backup.anime), &existing);
            self.anime_rows = Some(
                plan_anime(sorted)
                    .into_iter()
                    .flat_map(|r| std::iter::once((r.entry, true)).chain(r.seasons.into_iter().map(|s| (s, false))))
                    .collect(),
            );
        }
        let (entry, is_root) = std::mem::take(&mut self.anime_rows.as_mut().expect("set above")[index as usize]);
        let parent = if is_root { None } else { self.parent };
        if !is_root && parent.is_none() {
            return done(entry.title, Err("its parent entry was not restored".into()));
        }
        let prefix = format!("{} [{}]", entry.title, anime_source_name(&self.backup.anime_sources, entry.source));
        let mut outcome = AnimeOutcome::default();
        let result = in_entry_tx(self.t.anime, self.cancel, |tx| {
            restore_anime_entry(tx, &self.clock, &entry, parent, &self.anime_by_order, &mut outcome)
        });
        match result {
            Ok(Some(id)) => {
                if is_root {
                    self.parent = Some(id);
                }
                done_as(entry.title, prefix, Ok(()))
            }
            Ok(None) => Err(RestoreError::Cancelled),
            Err(e) => {
                if is_root {
                    self.parent = None;
                }
                done_as(entry.title, prefix, Err(e.to_string()))
            }
        }
    }

    fn custom_buttons(&mut self) -> Result<Item, RestoreError> {
        let title = text::CUSTOM_BUTTON_SETTINGS.english;
        match in_tx(self.t.anime, |tx| restore_custom_buttons(tx, &self.backup.custom_buttons)) {
            Ok(inserted) => done(format!("{title} ({inserted})"), Ok(())),
            Err(e) => done(title.into(), Err(e.to_string())),
        }
    }
}

impl Restorer for DbRestorer<'_> {
    fn skipped_settings(&self) -> usize {
        self.skipped_settings
    }

    fn item(&mut self, section: Section, index: u32) -> Result<Item, RestoreError> {
        let i = index as usize;
        match section {
            Section::Categories => self.categories(index),
            Section::AppSettings => self.app_settings(),
            Section::SourceSettings => self.source_settings(),
            Section::AnimeEntries => self.anime_entry(index),
            Section::MangaEntries => self.manga_entry(index),
            Section::AnimeStores => {
                let s = &self.backup.anime_extension_stores[i];
                done(s.name.clone(), restore_anime_store(self.t.anime, s).map_err(|e| e.to_string()))
            }
            Section::MangaRepos => {
                let r = &self.backup.manga_extension_repos[i];
                // The message already carries the `Error Adding Manga Repo` text.
                Ok(Item { title: r.name.clone(), error: restore_manga_repo(self.t.manga, r).err() })
            }
            Section::CustomButtons => self.custom_buttons(),
            // No installer exists yet: the step counts and writes nothing.
            Section::Extensions => done(text::LABEL_EXTENSIONS.english.into(), Ok(())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use chrono::TimeZone;
    use kuro_db::statements_anime::{anime_by_url_and_source, categories_by_anime_id};
    use kuro_db::statements_manga::{categories, chapters, mangas};
    use kuro_db::{schema_anime, schema_manga};
    use kuro_prefs::values::PrefValue as Value;

    use super::*;
    use super::super::prefs::IntervalScheduler;
    use crate::schema::{
        BackupCategory, BackupChapter, BackupExtensionRepos, BackupExtensionStore, BackupPreference, BackupSourcePreferences,
        PrefValue,
    };

    struct Env {
        _dir: tempfile::TempDir,
        manga: Connection,
        anime: Connection,
        app: PrefStore,
        sources: RefCell<HashMap<String, PrefStore>>,
        timers: RefCell<Vec<(&'static str, Option<u32>)>>,
    }

    fn env() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let manga = schema_manga::open(&dir.path().join("m.db")).unwrap();
        let anime = schema_anime::open(&dir.path().join("a.db")).unwrap();
        Env { _dir: dir, manga, anime, app: PrefStore::in_memory(), sources: RefCell::default(), timers: RefCell::default() }
    }

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).single().unwrap()
    }

    fn cat(name: &str, order: i64, id: i64) -> BackupCategory {
        BackupCategory { name: name.into(), order, id, flags: 0 }
    }

    fn manga(url: &str, categories: Vec<i64>) -> BackupManga {
        BackupManga {
            source: 7,
            url: url.into(),
            title: format!("M {url}"),
            version: 4,
            categories,
            chapters: vec![BackupChapter { url: format!("{url}/c1"), name: "c1".into(), chapter_number: 1.0, ..Default::default() }],
            ..Default::default()
        }
    }

    fn anime(url: &str, id: i64, parent: Option<i64>, categories: Vec<i64>) -> BackupAnime {
        BackupAnime { source: 3, url: url.into(), title: format!("A {url}"), id: Some(id), parent_id: parent, version: 6, categories, ..Default::default() }
    }

    fn pref(key: &str, value: PrefValue) -> BackupPreference {
        BackupPreference { key: key.into(), value }
    }

    fn fixture() -> Backup {
        Backup {
            categories: vec![cat("Reading", 0, 10), cat("Later", 1, 11)],
            anime_categories: vec![cat("Watching", 0, 20)],
            manga: vec![manga("old", vec![0]), manga("fresh", vec![0, 1])],
            anime: vec![anime("season", 2, Some(1), vec![0]), anime("root", 1, None, vec![0])],
            preferences: vec![
                pref("default_category", PrefValue::Int(11)),
                pref("backup_interval", PrefValue::Int(0)),
                pref("pref_library_update_interval_key", PrefValue::Int(24)),
            ],
            source_preferences: vec![BackupSourcePreferences {
                source_key: "1234".into(),
                prefs: vec![pref("show_nsfw_source", PrefValue::Boolean(true))],
            }],
            anime_extension_stores: vec![BackupExtensionStore {
                index_url: "https://s/index.json".into(),
                name: "Store".into(),
                contact_website: "w".into(),
                signing_key: "k".into(),
                ..Default::default()
            }],
            manga_extension_repos: vec![BackupExtensionRepos {
                base_url: "https://r/one".into(),
                name: "Repo".into(),
                short_name: None,
                website: "w".into(),
                signing_key_fingerprint: "fp".into(),
            }],
            ..Default::default()
        }
    }

    fn run(e: &mut Env, backup: Backup, opts: RestoreOptions, cancel: &dyn Fn() -> bool) -> (Run, Vec<ProgressEvent>) {
        let library = |h: Option<u32>| e.timers.borrow_mut().push(("library", h));
        let backup_timer = |h: Option<u32>| e.timers.borrow_mut().push(("backup", h));
        let sched = IntervalScheduler { store: &e.app, library_timer: &library, backup_timer: &backup_timer };
        let open = |name: &str| e.sources.borrow_mut().entry(name.to_owned()).or_insert_with(PrefStore::in_memory).clone();
        let targets = Targets { manga: &mut e.manga, anime: &mut e.anime, app_prefs: &e.app, source_prefs: &open, scheduler: &sched };
        let mut events = Vec::new();
        let result = restore_backup(backup, opts, now(), targets, cancel, &mut |ev| events.push(ev));
        (result, events)
    }

    fn all_but_extensions() -> RestoreOptions {
        RestoreOptions::default()
    }

    #[test]
    fn a_fixture_runs_through_every_section_and_progress_reaches_the_amount() {
        let mut e = env();
        let backup = fixture();
        let total = amount(all_but_extensions(), counts(&backup));
        let (run, events) = run(&mut e, backup, all_but_extensions(), &|| false);
        assert!(run.errors.is_empty());
        assert_eq!(run.stop, None);
        assert_eq!(events.len() as u32, total, "one event per row, seasons included (X-73)");
        assert_eq!((events.last().unwrap().done, events.last().unwrap().total), (total, total));

        // categories first, entries mapped, and the stored version is the backup version (T-22a-014)
        let reading = categories::get_categories(&e.manga).unwrap().into_iter().find(|c| c.name == "Reading").unwrap().id;
        let fresh = mangas::get_by_url_and_source(&e.manga, "fresh", 7).unwrap().unwrap();
        assert_eq!(fresh.version, 4);
        let member: Vec<i64> = categories::get_categories_by_manga_id(&e.manga, fresh.id).unwrap().into_iter().map(|r| r.id).collect();
        assert!(member.contains(&reading) && member.len() == 2);
        assert_eq!(chapters::get_by_manga_id(&e.manga, fresh.id, false).unwrap().len(), 1);
        let root = anime_by_url_and_source(&e.anime, "root", 3).unwrap().unwrap();
        let season = anime_by_url_and_source(&e.anime, "season", 3).unwrap().unwrap();
        assert_eq!((root.version, season.version), (6, 6));
        assert_eq!(season.parent_id, Some(root.id));
        assert_eq!(categories_by_anime_id(&e.anime, season.id).unwrap().len(), 1);

        // preferences: the default category is remapped, the schedulers saw the restored intervals once each
        let later = categories::get_categories(&e.manga).unwrap().into_iter().find(|c| c.name == "Later").unwrap().id;
        assert_eq!(e.app.get_raw("default_category"), Some(Value::Int(later as i32)));
        assert_eq!(e.timers.borrow().as_slice(), [("library", Some(24)), ("backup", None)]);
        assert_eq!(e.sources.borrow()["source_1234"].get_raw("show_nsfw_source"), Some(Value::Bool(true)));
    }

    #[test]
    fn rows_not_in_the_database_run_first_from_one_snapshot() {
        let mut e = env();
        let (_, _) = run(&mut e, Backup { manga: vec![manga("old", vec![])], ..Default::default() }, all_but_extensions(), &|| false);
        let backup = Backup { manga: vec![manga("old", vec![]), manga("fresh", vec![])], ..Default::default() };
        let (_, events) = run(&mut e, backup, all_but_extensions(), &|| false);
        let titles: Vec<&str> = events.iter().map(|ev| ev.title.as_str()).collect();
        let pos = |t: &str| titles.iter().position(|x| *x == t).unwrap();
        assert!(pos("M fresh") < pos("M old"));
    }

    #[test]
    fn existing_anime_row_keeps_its_version_when_memberships_are_written() {
        let mut e = env();
        let first = Backup { anime: vec![anime("root", 1, None, vec![])], ..Default::default() };
        run(&mut e, first, all_but_extensions(), &|| false);
        let second = Backup {
            anime_categories: vec![cat("Watching", 0, 20)],
            anime: vec![anime("root", 1, None, vec![0])],
            ..Default::default()
        };
        run(&mut e, second, all_but_extensions(), &|| false);
        assert_eq!(anime_by_url_and_source(&e.anime, "root", 3).unwrap().unwrap().version, 6);
    }

    /// F-02: the flag is read once per section (nine, in plan order), and for an entry before it and before its commit.
    /// With only the library on and three manga, checks 1 to 5 are the four earlier sections and the manga section,
    /// 6 and 7 are the first entry, 8 and 9 the second, 10 and 11 the third, 12 to 15 the later sections. A flag that
    /// turns true at check `n` leaves exactly the entries finished before it, and never lets the next one start.
    #[test]
    fn cancel_stops_at_the_exact_entry_through_the_driver() {
        for n in 1..=16u32 {
            let mut e = env();
            let seen = Cell::new(0);
            let backup = Backup { manga: vec![manga("a", vec![]), manga("b", vec![]), manga("c", vec![])], ..Default::default() };
            let opts = RestoreOptions::from_bits([true, false, false, false, false, false, false]);
            let mut events = Vec::new();
            let library = |_: Option<u32>| {};
            let sched = IntervalScheduler { store: &e.app, library_timer: &library, backup_timer: &library };
            let open = |_: &str| PrefStore::in_memory();
            let targets = Targets { manga: &mut e.manga, anime: &mut e.anime, app_prefs: &e.app, source_prefs: &open, scheduler: &sched };
            let cancel = || {
                seen.set(seen.get() + 1);
                seen.get() >= n
            };
            let run = restore_backup(backup, opts, now(), targets, &cancel, &mut |ev| events.push(ev));
            let finished = if n <= 7 { 0 } else { ((n as usize - 6) / 2).min(3) };
            assert_eq!(run.stop, (n <= 15).then_some(RestoreError::Cancelled), "flag at check {n}");
            assert!(run.errors.is_empty(), "a cancel writes no error line");
            assert_eq!(events.len(), finished, "flag at check {n}");
            let stored = mangas::get_all_source_and_url(&e.manga).unwrap().len();
            assert_eq!(stored, finished, "flag at check {n}: each finished entry is committed, nothing else");
        }
    }

    #[test]
    fn a_failing_repo_is_logged_and_the_run_goes_on() {
        let mut e = env();
        let mut backup = fixture();
        backup.manga_extension_repos.push(BackupExtensionRepos {
            base_url: "https://r/two".into(),
            name: "Clash".into(),
            short_name: None,
            website: "w".into(),
            signing_key_fingerprint: "fp".into(),
        });
        let total = amount(all_but_extensions(), counts(&backup));
        let (run, events) = run(&mut e, backup, all_but_extensions(), &|| false);
        assert_eq!(run.errors.len(), 1);
        assert!(run.errors[0].message.starts_with("Error Adding Manga Repo: Clash"));
        assert_eq!(events.len() as u32, total);
    }

    #[test]
    fn a_season_whose_root_failed_is_reported_not_restored() {
        let mut e = env();
        // A trigger aborts the root's insert, so the root fails and its season has no parent.
        e.anime.execute_batch("CREATE TRIGGER fail_root BEFORE INSERT ON animes WHEN new.url = 'root' BEGIN SELECT RAISE(ABORT, 'no'); END;").unwrap();
        let backup = Backup { anime: vec![anime("root", 1, None, vec![]), anime("season", 2, Some(1), vec![])], ..Default::default() };
        let only_library = RestoreOptions::from_bits([true, false, false, false, false, false, false]);
        let (run, events) = run(&mut e, backup, only_library, &|| false);
        assert_eq!(run.errors.len(), 2);
        assert_eq!(events.len(), 2);
        assert!(anime_by_url_and_source(&e.anime, "season", 3).unwrap().is_none());
    }

    #[test]
    fn cancel_between_entry_writes_rolls_the_entry_back() {
        let mut e = env();
        let items_done = Cell::new(0);
        let checks_after_first = Cell::new(0);
        // After the first entry commits, the next checks are: the check before the second entry, then the check before
        // its commit. The second one reports the cancel, so the second entry is rolled back.
        let cancel = || {
            if items_done.get() == 0 {
                return false;
            }
            checks_after_first.set(checks_after_first.get() + 1);
            checks_after_first.get() == 2
        };
        let backup = Backup { manga: vec![manga("a", vec![]), manga("b", vec![])], ..Default::default() };
        let opts = RestoreOptions { categories: false, app_settings: false, source_settings: false, ..all_but_extensions() };
        let library = |_: Option<u32>| {};
        let sched = IntervalScheduler { store: &e.app, library_timer: &library, backup_timer: &library };
        let open = |_: &str| PrefStore::in_memory();
        let targets = Targets { manga: &mut e.manga, anime: &mut e.anime, app_prefs: &e.app, source_prefs: &open, scheduler: &sched };
        let run = restore_backup(backup, opts, now(), targets, &cancel, &mut |_| items_done.set(items_done.get() + 1));
        assert_eq!(run.stop, Some(RestoreError::Cancelled));
        assert!(run.errors.is_empty(), "a cancel is not an error line");
        let stored: Vec<String> = mangas::get_all_source_and_url(&e.manga).unwrap().into_iter().map(|(_, url)| url).collect();
        assert_eq!(stored, vec!["a".to_string()]);
    }

    #[test]
    fn counts_include_seasons_and_every_list() {
        let c = counts(&fixture());
        assert_eq!((c.anime_entries, c.manga_entries, c.anime_stores, c.manga_repos), (2, 2, 1, 1));
    }
}
