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
//! - The "only on Wi-Fi" device restriction is not applied: the platform layer has no network-type query.

use std::cell::OnceCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use kuro_domain::items::source_sync::MangaSyncLocks;
use kuro_domain::kind::EntryKind;
use kuro_prefs::Prefs;
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::pref::Pref;
use tokio::runtime::Handle;
use tokio::sync::watch;

use crate::library_update::constraints::{self, DeviceFacts};
use crate::library_update::counters::PrefsHost;
use crate::library_update::entry::Deps;
use crate::library_update::error::UpdateError;
use crate::library_update::notice;
use crate::library_update::pass::{EntryUpdate, Host, PassRunner, PassSettings, PassStatus, Trigger};
use crate::library_update::queue::QueueRestrictions;
use crate::library_update::schedule::run_schedule;
use crate::library_update::source::{Fetched, Source, SourceRegistry};
use crate::library_update::store::SqliteLibraryStore;
use crate::library_update::sync::DbItemSync;
use crate::library_update::text;

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
}

/// The registry while the app has no installed-source bridge.
pub struct NothingInstalled;

impl SourceRegistry for NothingInstalled {
    type Source = NoSource;

    fn get(&self, _source_id: i64) -> Option<NoSource> {
        None
    }
}

/// The notifications of a pass: a toast for the summary and for failures. Progress, counters and downloads have no
/// surface here: the counters are written by `PrefsHost`, and no download manager is registered with the pass yet.
pub struct ToastHost;

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
    }

    fn enqueue_downloads(&self, _kind: EntryKind, entry_id: i64, items: &[crate::library_update::source::SourceItem]) {
        tracing::debug!(entry_id, items = items.len(), "no download manager takes the new items of an update pass");
    }

    fn start_downloads(&self, _kind: EntryKind) {}

    fn show_error(&self, _kind: EntryKind, failed: usize) {
        // No error log file is written yet, so the notice has no tap action.
        if let Some(notice) = notice::error_notice(failed, false) {
            crate::toast::notify(format!("{}: {}", notice.title, notice.text));
        }
    }

    // Toasts are short; the 30 s warning and its help link need a notification surface (not wired yet).
    fn show_queue_warning(&self, _kind: EntryKind) {
        crate::toast::notify(format!("{}: {}", text::QUEUE_WARNING_TITLE, text::QUEUE_WARNING_TEXT));
    }
}

/// Whether the device restrictions the user chose hold right now. Only the facts the platform reports count: the
/// connection is not checked here yet (the platform reports "not connected" when it cannot read NetworkManager), and
/// Wi-Fi is unknown until the platform reports the primary connection type. A blocked scheduled pass retries later.
async fn device_allows(inputs: &Inputs) -> bool {
    let restrictions = inputs.library.auto_update_device_restrictions.get();
    let now = kuro_platform::power_network::read_conditions().await;
    constraints::satisfied(
        &restrictions,
        DeviceFacts { connected: None, wifi: None, metered: Some(now.metered), on_ac: Some(now.on_ac) },
    )
}

type Runner = PassRunner<NothingInstalled, SqliteLibraryStore, DbItemSync, PrefsHost<ToastHost>>;

/// The preferences a pass reads each time it starts.
pub struct Inputs {
    pub library: LibraryPrefs,
    pub hide_content: Pref<bool>,
}

fn ids(set: BTreeSet<String>) -> Vec<i64> {
    set.iter().filter_map(|s| s.trim().parse().ok()).collect()
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
        category,
        include_categories: ids(include),
        exclude_categories: ids(exclude),
        restrictions: QueueRestrictions::from_pref(Some(&tokens)),
        auto_update_metadata: lib.auto_update_metadata.get(),
        hide_content: inputs.hide_content.get(),
        ..PassSettings::new(kind)
    }
}

fn runner() -> Arc<Runner> {
    let app = crate::data::app();
    let lib = LibraryPrefs::new(app.base.store().clone());
    let sync = DbItemSync::new(
        app.manga_db.clone(),
        app.anime_db.clone(),
        Arc::new(MangaSyncLocks::default()),
        Arc::new(MangaSyncLocks::default()),
        lib.mark_duplicate_read_chapter_as_read.get().contains(kuro_prefs::library::MARK_DUPLICATE_CHAPTER_READ_NEW),
        kuro_domain::items::episode_sync::mark_duplicate_as_seen_from_pref(&lib.mark_duplicate_seen_episode_as_seen.get()),
    );
    let prefs = Prefs::with_store(app.base.store().clone(), Arc::new(kuro_prefs::memory::MemorySecrets::default()));
    let host = PrefsHost::new(ToastHost, &prefs);
    Arc::new(PassRunner::new(Deps {
        registry: NothingInstalled,
        store: SqliteLibraryStore::new(app.manga_db.clone(), app.anime_db.clone()),
        sync,
        host,
    }))
}

thread_local! {
    static STARTED: OnceCell<()> = const { OnceCell::new() };
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
        hide_content: app.security.hide_notification_content.clone(),
    });

    let (start_runner, start_inputs) = (runner.clone(), inputs.clone());
    let running_runner = runner.clone();
    crate::library::mount::on_update(crate::library::mount::UpdateHost {
        start: std::rc::Rc::new(move |kind, category| start_runner.start_now(pass_settings(kind, category, &start_inputs))),
        running: std::rc::Rc::new(move |kind| running_runner.locks().holder(kind).is_some()),
    });

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
        (store.clone(), Inputs { library: LibraryPrefs::new(store), hide_content: hide })
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

    #[tokio::test]
    async fn nothing_installed_gives_the_stub_slot_and_a_missing_source_error() {
        use crate::library_update::source::SourceSlot;
        let registry = NothingInstalled;
        let slot = registry.get_or_stub(9);
        assert!(matches!(slot, SourceSlot::Missing));
        assert_eq!(slot.fetch(EntryKind::Manga, "/m", false).await, Err(UpdateError::SourceNotInstalled));
    }
}
