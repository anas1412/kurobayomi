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
    let manga_writer = kuro_db::schema_manga::open(&manga_path).map_err(|e| e.to_string())?;
    let anime_writer = kuro_db::schema_anime::open(&anime_path).map_err(|e| e.to_string())?;
    let manga_db = Arc::new(Db::new(&manga_path, manga_writer).map_err(|e| e.to_string())?);
    let anime_db = Arc::new(Db::new(&anime_path, anime_writer).map_err(|e| e.to_string())?);
    Ok(Opened { prefs_store, manga_db, anime_db })
}

/// Builds the shared state on the GTK thread: the startup cleanup, then the first-launch texture limit (probed in a
/// short-lived GL context, stored as `pref_hardware_bitmap_threshold` when unset).
/// Does nothing when the state is already installed.
pub fn install(opened: Opened) -> Result<(), String> {
    if APP.with(|cell| cell.get().is_some()) {
        return Ok(());
    }
    let Opened { prefs_store, manga_db, anime_db } = opened;
    let prefs = Arc::new(LibraryPrefs::new(prefs_store.clone()));
    let known_manga = known_ids(&CategoryRepo::new(Arc::clone(&manga_db), EntryKind::Manga))?;
    let known_anime = known_ids(&CategoryRepo::new(Arc::clone(&anime_db), EntryKind::Anime))?;
    startup_cleanup(&prefs_store, &prefs, &known_manga, &known_anime);

    let base = BasePrefs::new(prefs_store.clone());
    crate::startup::probe_texture_limit();
    if let Err(e) = base.seed_hardware_bitmap_threshold(kuro_platform::device::device_texture_limit()) {
        tracing::warn!("could not store the hardware bitmap threshold: {e}");
    }
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
