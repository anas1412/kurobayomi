//! The app's databases and preference store, opened once on the GTK thread. The screens read them through [`app`].
//! The startup cleanup of the category-id preference sets (T-08a-035) runs here, after both databases open.

use std::cell::OnceCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_domain::category::backup::startup_cleanup;
use kuro_domain::category::repo::CategoryRepo;
use kuro_domain::category::service::CategoryService;
use kuro_domain::kind::EntryKind;
use kuro_platform::linux::LinuxPaths;
use kuro_platform::paths::Paths as _;
use kuro_prefs::base::BasePrefs;
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::security::SecurityPrefs;
use kuro_prefs::store::PrefStore;

/// The shared state of the library screens.
pub struct App {
    pub manga_db: Arc<Db>,
    pub anime_db: Arc<Db>,
    pub manga_categories: CategoryService,
    pub anime_categories: CategoryService,
    /// App-state switches (incognito, downloaded only) over the shared store.
    pub base: BasePrefs,
    /// Lock and notification privacy switches.
    pub security: SecurityPrefs,
    /// The `[library]` preferences, for the startup work that reads them (cache clear).
    pub library: Arc<LibraryPrefs>,
}

impl App {
    /// The category service of one kind.
    pub fn categories(&self, kind: EntryKind) -> &CategoryService {
        match kind {
            EntryKind::Manga => &self.manga_categories,
            EntryKind::Anime => &self.anime_categories,
        }
    }

    /// The database of one kind.
    pub fn db(&self, kind: EntryKind) -> Arc<Db> {
        match kind {
            EntryKind::Manga => Arc::clone(&self.manga_db),
            EntryKind::Anime => Arc::clone(&self.anime_db),
        }
    }
}

thread_local! {
    static APP: OnceCell<Rc<App>> = const { OnceCell::new() };
}

/// The stores opened by [`open_blocking`], ready to move to the GTK thread for [`install`].
pub struct Opened {
    prefs_store: PrefStore,
    manga_db: Arc<Db>,
    anime_db: Arc<Db>,
}

/// Opens the preference store and both databases, running the raw-key and schema migrations. This is the slow part of
/// startup, so it runs off the GTK thread while the splash timer ticks (T-01a-043).
pub fn open_blocking() -> Result<Opened, String> {
    let paths = LinuxPaths::system().map_err(|e| e.to_string())?;
    let prefs_store = kuro_prefs::settings_load::open(&paths).map_err(|e| e.to_string())?;
    // raw-key migrations run before any typed section is built over the store
    kuro_prefs::migrate::migrate(&prefs_store, kuro_core::app_info::APP_INFO.version_code as i32).map_err(|e| e.to_string())?;
    let data_dir = paths.data_dir();
    let manga_path = kuro_db::schema_manga::db_path(&data_dir);
    let anime_path = kuro_db::schema_anime::db_path(&data_dir);
    let manga_writer = open_or_restore(&manga_path, kuro_db::schema_manga::open)?;
    let anime_writer = open_or_restore(&anime_path, kuro_db::schema_anime::open)?;
    let manga_db = Arc::new(Db::new(&manga_path, manga_writer).map_err(|e| e.to_string())?);
    let anime_db = Arc::new(Db::new(&anime_path, anime_writer).map_err(|e| e.to_string())?);
    Ok(Opened { prefs_store, manga_db, anime_db })
}

/// Opens one database file. A damaged file is moved aside by the open (`<file>.corrupt-<UTC>`, which returns
/// `DbError::Corrupt`); this puts the newest pre-migration backup in its place and opens again. With no backup the
/// second open starts an empty database, and the damaged file stays beside it.
fn open_or_restore(
    path: &std::path::Path,
    open: fn(&std::path::Path) -> kuro_db::DbResult<rusqlite::Connection>,
) -> Result<rusqlite::Connection, String> {
    match open(path) {
        Err(kuro_db::DbError::Corrupt(why)) if !path.exists() => {
            tracing::error!("{}: {why}", path.display());
            match kuro_db::open::restore_newest_backup(path) {
                Ok(backup) => tracing::warn!("restored {} from {}", path.display(), backup.display()),
                Err(kuro_db::DbError::NotFound) => tracing::warn!("no backup for {}; starting empty", path.display()),
                Err(e) => return Err(e.to_string()),
            }
            open(path).map_err(|e| e.to_string())
        }
        other => other.map_err(|e| e.to_string()),
    }
}

/// Shows one `settings_save_failed` toast each time the store reports a streak of failed saves (T-02a-038). The store
/// sends one notice per streak and keeps the values readable from memory meanwhile.
fn watch_save_failures(store: &PrefStore) {
    let notices = store.save_failures();
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        if notices.try_iter().count() > 0 {
            crate::toast::notify(kuro_i18n::tr(kuro_i18n::MsgKey::settings_save_failed));
        }
        gtk::glib::ControlFlow::Continue
    });
}

/// Writes the preferences to disk at shutdown, waiting at most `SHUTDOWN_FLUSH_TIMEOUT` (T-02a-038). True when every
/// value reached the disk.
pub fn flush_on_shutdown(store: &PrefStore) -> bool {
    match store.flush_within(kuro_prefs::store::SHUTDOWN_FLUSH_TIMEOUT) {
        Some(Ok(())) => true,
        Some(Err(e)) => {
            tracing::warn!("the preferences were not saved at shutdown: {e}");
            false
        }
        None => {
            tracing::warn!("the preferences were still saving when the shutdown wait ended");
            false
        }
    }
}

/// Builds the shared state on the GTK thread: the startup cleanup, then the texture limit probe (a short-lived GL
/// context; the limit is not stored, T-13a-030).
/// Does nothing when the state is already installed.
pub fn install(opened: Opened) -> Result<(), String> {
    if APP.with(|cell| cell.get().is_some()) {
        return Ok(());
    }
    let Opened { prefs_store, manga_db, anime_db } = opened;
    watch_save_failures(&prefs_store);
    let prefs = Arc::new(LibraryPrefs::new(prefs_store.clone()));
    let known_manga = known_ids(&CategoryRepo::new(Arc::clone(&manga_db), EntryKind::Manga))?;
    let known_anime = known_ids(&CategoryRepo::new(Arc::clone(&anime_db), EntryKind::Anime))?;
    startup_cleanup(&prefs_store, &prefs, &known_manga, &known_anime);

    let base = BasePrefs::new(prefs_store.clone());
    crate::startup::probe_texture_limit();
    let app = App {
        manga_categories: CategoryService::new(Arc::clone(&manga_db), EntryKind::Manga, Arc::clone(&prefs)),
        anime_categories: CategoryService::new(Arc::clone(&anime_db), EntryKind::Anime, Arc::clone(&prefs)),
        manga_db,
        anime_db,
        base,
        security: SecurityPrefs::new(prefs_store),
        library: prefs,
    };
    APP.with(|cell| cell.set(Rc::new(app))).map_err(|_| "the app data is already open".to_owned())
}

/// True once [`install`] has succeeded.
pub fn is_installed() -> bool {
    APP.with(|cell| cell.get().is_some())
}

/// The opened app data. Panics before [`init`] succeeds, which `main` rules out by stopping at the error.
pub fn app() -> Rc<App> {
    APP.with(|cell| cell.get().cloned()).expect("data::install runs before any screen is built")
}

fn known_ids(repo: &CategoryRepo) -> Result<BTreeSet<i64>, String> {
    Ok(repo.all().map_err(|e| e.to_string())?.iter().map(|c| c.id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_flush_reports_a_finished_save() {
        assert!(flush_on_shutdown(&PrefStore::in_memory()));
    }

    #[test]
    fn a_corrupt_database_is_replaced_by_the_newest_backup_or_a_fresh_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = kuro_db::schema_manga::db_path(dir.path());
        std::fs::write(&path, b"not a database ".repeat(300)).unwrap();
        // no backup: the damaged file is kept beside a new empty database
        drop(open_or_restore(&path, kuro_db::schema_manga::open).unwrap());
        let kept = std::fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().contains(".corrupt-"));
        assert!(kept);
        assert!(path.exists());
    }
}
