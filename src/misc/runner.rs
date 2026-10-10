//! The update runner of the app (T-26a-102, T-26a-104, T-26a-131): one `PassRunner` for both kinds over the app's
//! databases, the settings of a pass read from the preferences, the library's refresh button and the schedule loops.
//!
//! Decisions:
//! - The runtime's `Handle` is taken at startup and the schedule loops are spawned on it. The runtime is built by
//!   `kuro_core::runtime::build` (2 to 8 workers by core count); the 4 workers of T-26a-131 and the "keep running in the
//!   background" check at shutdown belong to that builder and `main`, which this lane does not own.
//! - The app has no installed-source bridge yet, so every entry of a pass ends as "source not installed" (the stub
//!   slot of `SourceRegistry`), exactly as the backup host's registry does. The pass, the lock, the counters and the
//!   notifications are real; a source bridge replaces [`NothingInstalled`] when it lands.
//! - The "only on Wi-Fi" restriction reads `kuro_platform::device::read()`. That reader cannot tell "wired" from
//!   "NetworkManager unreadable", so [`device_facts`] treats a machine with no sign of NetworkManager as unknown
//!   (SC-07). A `network_manager_present` flag on the platform's `DeviceState` would remove the guess.

use std::cell::{OnceCell, RefCell};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_domain::items::source_sync::MangaSyncLocks;
use kuro_domain::kind::EntryKind;
use kuro_downloads::download_prefs::DownloadPrefs;
use kuro_prefs::Prefs;
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;
use tokio::runtime::Handle;
use tokio::sync::watch;

use crate::library_update::constraints::{self, DeviceFacts};
use crate::library_update::counters::PrefsHost;
use crate::library_update::download::DownloadPolicy;
use crate::library_update::entry::Deps;
use crate::library_update::error::UpdateError;
use crate::library_update::error_log;
use crate::library_update::metadata::{DbMetadataStore, MetadataDeps, MetadataRunner};
use crate::library_update::pass::{EntryUpdate, Failure, Host, PassRunner, PassSettings, PassStatus, Trigger};
use crate::library_update::queue::QueueRestrictions;
use crate::library_update::renames::FolderRenames;
use crate::library_update::schedule::run_schedule;
use crate::library_update::source::{Fetched, RemoteFlags, Source, SourceDetails, SourceRegistry};
use crate::library_update::store::SqliteLibraryStore;
use crate::library_update::sync::DbItemSync;
use crate::library_update::text;
use crate::misc::notify;
use crate::updates::download_host;

/// A source that cannot exist: the registry below never returns one.
pub enum NoSource {}

impl Source for NoSource {
    fn is_local(&self) -> bool {
        match *self {}
    }

    fn is_unmetered(&self) -> bool {
        match *self {}
    }

    async fn fetch(&self, _kind: EntryKind, _url: &str, _with_details: bool) -> Result<Fetched, UpdateError> {
        match *self {}
    }

    async fn refresh(&self, _kind: EntryKind, _url: &str, _flags: RemoteFlags) -> Result<Option<SourceDetails>, UpdateError> {
        match *self {}
    }
}

/// The registry while the app has no installed-source bridge.
pub struct NothingInstalled;

impl SourceRegistry for NothingInstalled {
    type Source = NoSource;

    fn get(&self, _source_id: i64) -> Option<NoSource> {
        None
    }
}

/// The notifications and downloads of a pass: a toast for the summary, and desktop notices (`notify`) for each entry,
/// for failures and for the large-queue warning, which fall back to a toast when no notifier is attached. New items go
/// to the downloads backend (`download_host::current`) paused, and one start at the end of the pass. Progress and
/// counters have no surface here: the counters are written by `PrefsHost`.
///
/// The host runs on the runtime's workers, where the app's thread-bound handles are out of reach, so it keeps the two
/// database handles and the external-downloader preference it needs.
pub struct ToastHost {
    manga_db: Arc<Db>,
    anime_db: Arc<Db>,
    external_downloader: Pref<bool>,
}

impl ToastHost {
    pub fn new(manga_db: Arc<Db>, anime_db: Arc<Db>, store: &PrefStore) -> Self {
        Self { manga_db, anime_db, external_downloader: DownloadPrefs::new(store.clone()).use_external_downloader() }
    }

    fn db(&self, kind: EntryKind) -> &Arc<Db> {
        match kind {
            EntryKind::Manga => &self.manga_db,
            EntryKind::Anime => &self.anime_db,
        }
    }
}

impl Host for ToastHost {
    fn set_last_update_ms(&self, _now_ms: i64) {}

    fn add_new_updates(&self, _kind: EntryKind, _count: usize) {}

    fn show_progress(&self, _kind: EntryKind, _title: &str, _body: Option<&str>) {}

    fn cancel_progress(&self, _kind: EntryKind) {}

    fn show_summary(&self, kind: EntryKind, updates: &[EntryUpdate], hide_content: bool) {
        if updates.is_empty() {
            return;
        }
        let titles: Vec<String> = updates.iter().map(|u| u.title.clone()).collect();
        let summary = text::summary(kind, &titles, hide_content);
        crate::toast::notify(format!("{}: {}", summary.title, summary.text));
        for update in updates {
            if let Some(post) = notify::entry_post(kind, update, hide_content) {
                notify::post(post);
            }
        }
    }

    fn enqueue_downloads(&self, kind: EntryKind, entry_id: i64, items: &[crate::library_update::source::SourceItem]) {
        let urls: Vec<String> = items.iter().map(|item| item.url.clone()).collect();
        let ids = crate::library::actions::item_ids_by_url(self.db(kind), kind, entry_id, &urls);
        if ids.is_empty() {
            return;
        }
        let external = kind == EntryKind::Anime && self.external_downloader.get();
        if !download_host::current().enqueue(kind, entry_id, &ids, external) {
            tracing::debug!(entry_id, items = ids.len(), "the downloads backend queued none of the new items");
        }
    }

    fn start_downloads(&self, kind: EntryKind) {
        download_host::current().start_queue(kind);
    }

    fn write_error_log(&self, kind: EntryKind, failures: &[Failure]) -> Option<PathBuf> {
        let paths = kuro_platform::linux::LinuxPaths::system().ok()?;
        // No installed-source bridge yet, so a source shows as its id, as an uninstalled source does in Kotlin.
        match error_log::write(&error_log::path(&paths), kind, failures, &|id| id.to_string()) {
            Ok(file) => file,
            Err(error) => {
                tracing::warn!(%error, "the update error log was not written");
                None
            }
        }
    }

    fn show_error(&self, kind: EntryKind, failed: usize, log: Option<&Path>) {
        if let Some(post) = notify::error_post(kind, failed, log) {
            let toast = post.title.clone();
            if !notify::post(post) {
                crate::toast::notify(toast);
            }
        }
    }

    fn show_queue_warning(&self, kind: EntryKind) {
        if !notify::post(notify::warning_post(kind)) {
            crate::toast::notify(format!("{}: {}", text::QUEUE_WARNING_TITLE, text::QUEUE_WARNING_TEXT));
        }
    }
}

/// The facts a pass checks, from the platform's two readers. The connection comes from the conditions reader (it falls
/// back to the network monitor when NetworkManager is missing). Wi-Fi comes from the device reader, which reports
/// `false` both for "wired" and for "NetworkManager unreadable"; a machine that shows no sign of NetworkManager gets
/// `None`, so Wi-Fi-only never blocks it (SC-07). Any positive reading proves NetworkManager answered.
fn device_facts(
    conditions: kuro_platform::power_network::Conditions,
    device: kuro_platform::device::DeviceState,
    network_manager_seen: bool,
) -> DeviceFacts {
    let answered = device.on_wifi || device.network_not_metered || network_manager_seen;
    DeviceFacts {
        connected: Some(conditions.connected),
        wifi: answered.then_some(device.on_wifi),
        metered: Some(conditions.metered),
        on_ac: Some(conditions.on_ac),
    }
}

/// Whether the device restrictions the user chose hold right now. A blocked scheduled pass retries later.
async fn device_allows(inputs: &Inputs) -> bool {
    let restrictions = inputs.library.auto_update_device_restrictions.get();
    let conditions = kuro_platform::power_network::read_conditions().await;
    let device = kuro_platform::device::read().await;
    let seen = std::path::Path::new("/run/NetworkManager").exists();
    constraints::satisfied(&restrictions, device_facts(conditions, device, seen))
}

type Runner = PassRunner<NothingInstalled, SqliteLibraryStore, DbItemSync, PrefsHost<ToastHost>>;

/// The preferences a pass reads each time it starts.
pub struct Inputs {
    pub library: LibraryPrefs,
    pub download: DownloadPrefs,
    pub hide_content: Pref<bool>,
}

fn ids(set: BTreeSet<String>) -> Vec<i64> {
    set.iter().filter_map(|s| s.trim().parse().ok()).collect()
}

/// The download policy of a pass (T-26a-090, T-26a-091): the toggle, the unread-only switch and the category lists of
/// the chapter keys for manga and of the episode keys for anime. The toggle is off by default.
pub fn download_policy(kind: EntryKind, download: &DownloadPrefs) -> DownloadPolicy {
    match kind {
        EntryKind::Manga => DownloadPolicy {
            enabled: download.download_new_chapters().get(),
            unread_only: download.download_new_unread_chapters_only().get(),
            include_categories: ids(download.download_new_chapter_categories().get()),
            exclude_categories: ids(download.download_new_chapter_categories_exclude().get()),
        },
        EntryKind::Anime => DownloadPolicy {
            enabled: download.download_new_episodes().get(),
            unread_only: download.download_new_unseen_episodes_only().get(),
            include_categories: ids(download.download_new_episode_categories().get()),
            exclude_categories: ids(download.download_new_episode_categories_exclude().get()),
        },
    }
}

/// The settings of one pass of `kind`. A manual pass of one category passes `category`; the include and exclude lists
/// are then ignored by the queue.
pub fn pass_settings(kind: EntryKind, category: Option<i64>, inputs: &Inputs) -> PassSettings {
    let lib = &inputs.library;
    let (include, exclude) = match kind {
        EntryKind::Manga => (lib.manga_update_categories.get(), lib.manga_update_categories_exclude.get()),
        EntryKind::Anime => (lib.anime_update_categories.get(), lib.anime_update_categories_exclude.get()),
    };
    let tokens: Vec<String> = lib.auto_update_item_restrictions.get().into_iter().collect();
    PassSettings {
        download: download_policy(kind, &inputs.download),
        category,
        include_categories: ids(include),
        exclude_categories: ids(exclude),
        restrictions: QueueRestrictions::from_pref(Some(&tokens)),
        auto_update_metadata: lib.auto_update_metadata.get(),
        hide_content: inputs.hide_content.get(),
        ..PassSettings::new(kind)
    }
}

/// The folder renames of the item sync: the shell's downloads folder and cache, and no source names while the app has
/// no installed-source bridge (a source the lookup does not know keeps its folders).
fn folder_renames() -> Arc<FolderRenames> {
    let shell = crate::shell::host::Shell::get();
    let root = shell
        .as_ref()
        .map(|s| s.downloads().root().to_path_buf())
        .or_else(|| {
            kuro_platform::linux::LinuxPaths::system().ok().map(|p| kuro_platform::paths::Paths::default_downloads_dir(&p))
        })
        .unwrap_or_default();
    Arc::new(FolderRenames::new(root, Arc::new(|_| None), shell.map(|s| s.downloads().clone())))
}

fn runner() -> Arc<Runner> {
    let app = crate::data::app();
    let lib = LibraryPrefs::new(app.base.store().clone());
    let renames = folder_renames();
    let sync = DbItemSync::new(
        app.manga_db.clone(),
        app.anime_db.clone(),
        Arc::new(MangaSyncLocks::default()),
        Arc::new(MangaSyncLocks::default()),
        lib.mark_duplicate_read_chapter_as_read.get().contains(kuro_prefs::library::MARK_DUPLICATE_CHAPTER_READ_NEW),
        kuro_domain::items::episode_sync::mark_duplicate_as_seen_from_pref(&lib.mark_duplicate_seen_episode_as_seen.get()),
    )
    .with_renames(renames.clone(), renames);
    let prefs = Prefs::with_store(app.base.store().clone(), Arc::new(kuro_prefs::memory::MemorySecrets::default()));
    let host = PrefsHost::new(ToastHost::new(app.manga_db.clone(), app.anime_db.clone(), app.base.store()), &prefs);
    Arc::new(PassRunner::new(Deps {
        registry: NothingInstalled,
        store: SqliteLibraryStore::new(app.manga_db.clone(), app.anime_db.clone()),
        sync,
        host,
    }))
}

/// Hands the runner's two closures to the library's refresh button and to the Updates page's refresh button, so both
/// start the same runner and share one single-flight lock (T-26a-102, T-26a-103).
fn register_buttons(start: std::rc::Rc<dyn Fn(EntryKind, Option<i64>) -> bool>, running: std::rc::Rc<dyn Fn(EntryKind) -> bool>) {
    crate::library::mount::on_update(crate::library::mount::UpdateHost { start: start.clone(), running: running.clone() });
    crate::updates::library_trigger::install_shared(start, running);
}

type MetadataPasses = MetadataRunner<NothingInstalled, SqliteLibraryStore, DbMetadataStore>;

thread_local! {
    static STARTED: OnceCell<()> = const { OnceCell::new() };
    static METADATA: RefCell<Option<Arc<MetadataPasses>>> = const { RefCell::new(None) };
}

fn metadata_passes() -> Arc<MetadataPasses> {
    let app = crate::data::app();
    Arc::new(MetadataRunner::new(MetadataDeps {
        registry: NothingInstalled,
        store: SqliteLibraryStore::new(app.manga_db.clone(), app.anime_db.clone()),
        writer: DbMetadataStore::new(app.manga_db.clone(), app.anime_db.clone()),
    }))
}

/// The manual "Refresh library covers" action (T-26a-050): starts the manga and the anime metadata pass. The passes have
/// no schedule, and each is refused only by a metadata pass of its own kind (T-26a-051). Returns, per kind, whether its
/// pass started. Call from the GTK main thread inside the app's runtime, after [`start`]; before that nothing starts.
pub fn refresh_library_covers() -> [(EntryKind, bool); 2] {
    let runner = METADATA.with(|m| m.borrow().clone());
    let now_ms = chrono::Local::now().timestamp_millis();
    [EntryKind::Manga, EntryKind::Anime].map(|kind| (kind, runner.as_ref().is_some_and(|r| r.start_now(kind, now_ms))))
}

/// Builds the runner, hands it to the library's refresh button and starts one schedule loop per kind on the
/// runtime's handle. Runs once; call from the GTK main thread inside the app's runtime.
pub fn start() {
    if STARTED.with(|s| s.set(()).is_err()) || !crate::data::is_installed() {
        return;
    }
    let handle = Handle::current();
    let runner = runner();
    let app = crate::data::app();
    let inputs = Arc::new(Inputs {
        library: LibraryPrefs::new(app.base.store().clone()),
        download: DownloadPrefs::new(app.base.store().clone()),
        hide_content: app.security.hide_notification_content.clone(),
    });
    METADATA.with(|m| *m.borrow_mut() = Some(metadata_passes()));

    let (start_runner, start_inputs) = (runner.clone(), inputs.clone());
    let running_runner = runner.clone();
    let start: std::rc::Rc<dyn Fn(EntryKind, Option<i64>) -> bool> =
        std::rc::Rc::new(move |kind, category| start_runner.start_now(pass_settings(kind, category, &start_inputs)));
    let running: std::rc::Rc<dyn Fn(EntryKind) -> bool> =
        std::rc::Rc::new(move |kind| running_runner.locks().holder(kind).is_some());
    register_buttons(start, running);

    let mut senders = Vec::new();
    for kind in [EntryKind::Manga, EntryKind::Anime] {
        let (tx, rx) = watch::channel(0_u32);
        senders.push(tx);
        let (runner, inputs) = (runner.clone(), inputs.clone());
        handle.spawn(run_schedule(rx, move || {
            let (runner, inputs) = (runner.clone(), inputs.clone());
            async move {
                if !device_allows(&inputs).await {
                    return PassStatus::Retry;
                }
                runner.run(pass_settings(kind, None, &inputs), Trigger::Auto).await.status
            }
        }));
    }
    let pref = LibraryPrefs::new(app.base.store().clone()).auto_update_interval;
    crate::misc::interval::pump(&pref, senders);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    fn inputs() -> (PrefStore, Inputs) {
        let store = PrefStore::in_memory();
        let hide = Pref::new(store.clone(), "hide_notification_content", false);
        (store.clone(), Inputs { library: LibraryPrefs::new(store.clone()), download: DownloadPrefs::new(store), hide_content: hide })
    }

    #[test]
    fn a_pass_reads_its_category_lists_per_kind() {
        let (_store, inputs) = inputs();
        inputs.library.manga_update_categories.set(["3".to_owned(), "x".to_owned()].into()).unwrap();
        inputs.library.anime_update_categories_exclude.set(["7".to_owned()].into()).unwrap();
        let manga = pass_settings(EntryKind::Manga, None, &inputs);
        assert_eq!(manga.include_categories, vec![3], "a text that is not an id is dropped");
        assert!(manga.exclude_categories.is_empty());
        let anime = pass_settings(EntryKind::Anime, None, &inputs);
        assert_eq!(anime.exclude_categories, vec![7]);
        assert!(anime.include_categories.is_empty());
    }

    #[test]
    fn a_manual_pass_names_its_category_and_follows_the_switches() {
        let (_store, inputs) = inputs();
        inputs.library.auto_update_metadata.set(true).unwrap();
        inputs.hide_content.set(true).unwrap();
        let settings = pass_settings(EntryKind::Manga, Some(5), &inputs);
        assert_eq!(settings.category, Some(5));
        assert!(settings.auto_update_metadata && settings.hide_content);
        assert_eq!(settings.kind, EntryKind::Manga);
    }

    #[test]
    fn the_download_policy_reads_the_chapter_keys_for_manga_and_the_episode_keys_for_anime() {
        // T-26a-090, T-26a-091
        let (_store, inputs) = inputs();
        let off = pass_settings(EntryKind::Manga, None, &inputs).download;
        assert_eq!(off, DownloadPolicy::default(), "download_new is off by default");
        assert!(!pass_settings(EntryKind::Anime, None, &inputs).download.enabled);
        let d = &inputs.download;
        d.download_new_chapters().set(true).unwrap();
        d.download_new_unread_chapters_only().set(true).unwrap();
        d.download_new_chapter_categories().set(["2".to_owned(), "x".to_owned()].into()).unwrap();
        d.download_new_chapter_categories_exclude().set(["0".to_owned()].into()).unwrap();
        let manga = pass_settings(EntryKind::Manga, None, &inputs).download;
        assert_eq!(
            manga,
            DownloadPolicy { enabled: true, unread_only: true, include_categories: vec![2], exclude_categories: vec![0] }
        );
        assert!(!pass_settings(EntryKind::Anime, None, &inputs).download.enabled, "the chapter toggle does not turn anime on");
        d.download_new_episodes().set(true).unwrap();
        d.download_new_unseen_episodes_only().set(true).unwrap();
        d.download_new_episode_categories().set(["5".to_owned()].into()).unwrap();
        d.download_new_episode_categories_exclude().set(["6".to_owned()].into()).unwrap();
        let anime = pass_settings(EntryKind::Anime, None, &inputs).download;
        assert_eq!(
            anime,
            DownloadPolicy { enabled: true, unread_only: true, include_categories: vec![5], exclude_categories: vec![6] }
        );
    }

    #[tokio::test]
    async fn a_pass_built_from_the_prefs_queues_new_items_of_a_favourite_and_not_of_a_non_favourite() {
        // T-26a-090: the toggle reaches the pass, so the enqueue happens for the favourite only.
        use crate::library_update::fake::{FakeHost, FakeStore, FakeSync, entry, item};
        use crate::library_update::queue::LibraryEntry;
        use crate::library_update::source::fake::{FakeRegistry, FakeSource};
        let (_store, inputs) = inputs();
        let library = vec![entry(1, 1, "Favourite"), LibraryEntry { favorite: false, ..entry(2, 1, "Not a favourite") }];
        let run = |settings: PassSettings| {
            let mut registry = FakeRegistry::default();
            registry.sources.insert(1, FakeSource::ok(vec![item(1.0, 0)]));
            let deps = Deps { registry, store: FakeStore::with(library.clone()), sync: FakeSync::default(), host: FakeHost::default() };
            let runner = Arc::new(PassRunner::new(deps));
            async move { runner.run(settings, Trigger::Manual).await }
        };
        let mut off = pass_settings(EntryKind::Manga, None, &inputs);
        off.restrictions = QueueRestrictions::default();
        assert_eq!(run(off).await.queued_downloads, 0, "download_new is off");
        inputs.download.download_new_chapters().set(true).unwrap();
        let mut on = pass_settings(EntryKind::Manga, None, &inputs);
        on.restrictions = QueueRestrictions::default();
        let outcome = run(on).await;
        assert_eq!(outcome.queued_downloads, 1, "one enqueue: the favourite's, not the other entry's");
        assert_eq!(outcome.updates.len(), 2, "both entries still update");
    }

    #[test]
    fn t_26a_103_the_updates_button_reaches_the_registered_runner_and_shares_its_single_flight() {
        use crate::updates::library_trigger::{self, Outcome};
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;
        assert_eq!(library_trigger::trigger(EntryKind::Anime), Outcome::Refused, "no runner, no start");
        let (busy, calls) = (Rc::new(Cell::new(false)), Rc::new(RefCell::new(Vec::new())));
        let (start_busy, start_calls, running_busy) = (busy.clone(), calls.clone(), busy.clone());
        register_buttons(
            Rc::new(move |kind, category| {
                start_calls.borrow_mut().push((kind, category));
                start_busy.set(true);
                true
            }),
            Rc::new(move |_| running_busy.get()),
        );
        assert_eq!(library_trigger::trigger(EntryKind::Anime), Outcome::Started, "the page starts a pass of the visible kind");
        assert_eq!(*calls.borrow(), vec![(EntryKind::Anime, None)], "a whole-library pass, no category");
        assert_eq!(library_trigger::trigger(EntryKind::Anime), Outcome::AlreadyRunning, "a pass is already running");
        assert_eq!(calls.borrow().len(), 1);
        assert_eq!(library_trigger::message(Outcome::Started), "Updating library");
        assert_eq!(library_trigger::message(Outcome::AlreadyRunning), "An update is already running");
    }

    #[test]
    fn the_metadata_passes_do_not_start_before_the_runner_exists() {
        // T-26a-050: the only way to start one is the manual call, and it needs the runner.
        assert!(refresh_library_covers().iter().all(|(_, started)| !started));
    }

    fn conditions(connected: bool) -> kuro_platform::power_network::Conditions {
        kuro_platform::power_network::Conditions { connected, metered: false, on_ac: true, battery_low: false }
    }

    fn wifi_only() -> BTreeSet<String> {
        [kuro_prefs::library::DEVICE_ONLY_ON_WIFI.to_owned()].into()
    }

    #[test]
    fn wifi_only_blocks_a_wired_machine_and_lets_wifi_through() {
        // SC-04
        let wired = kuro_platform::device::DeviceState { on_wifi: false, network_not_metered: true, charging: true };
        assert!(!constraints::satisfied(&wifi_only(), device_facts(conditions(true), wired, true)));
        let wifi = kuro_platform::device::DeviceState { on_wifi: true, ..wired };
        assert!(constraints::satisfied(&wifi_only(), device_facts(conditions(true), wifi, true)));
    }

    #[test]
    fn a_machine_without_network_manager_satisfies_wifi_only() {
        // SC-07
        let nothing = kuro_platform::device::DeviceState::default();
        let facts = device_facts(conditions(true), nothing, false);
        assert_eq!(facts.wifi, None);
        assert!(constraints::satisfied(&wifi_only(), facts));
        assert!(!constraints::satisfied(&wifi_only(), device_facts(conditions(true), nothing, true)), "seen but wired");
    }

    #[test]
    fn no_connection_blocks_whatever_the_restrictions() {
        let nothing = kuro_platform::device::DeviceState::default();
        assert!(!constraints::satisfied(&BTreeSet::new(), device_facts(conditions(false), nothing, false)));
    }

    #[tokio::test]
    async fn nothing_installed_gives_the_stub_slot_and_a_missing_source_error() {
        use crate::library_update::source::SourceSlot;
        let registry = NothingInstalled;
        let slot = registry.get_or_stub(9);
        assert!(matches!(slot, SourceSlot::Missing));
        assert_eq!(slot.fetch(EntryKind::Manga, "/m", false).await, Err(UpdateError::SourceNotInstalled));
    }
}
