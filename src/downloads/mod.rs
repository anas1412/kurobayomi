//! The download managers, started by the shell once the data has opened (lane D): the manga manager and the anime queue.
//! The manga queue lives in the manga database file and the anime queue in the anime file. Each has its own connection,
//! and both stores run their idempotent DDL when they open. The GApplication hold is not wired yet, so a run stops when
//! the process exits.

pub mod anime_io;
pub mod anime_queue;
pub mod directory;
pub mod live;
pub mod manga_io;
pub mod platform;
pub mod queue_page;

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

use kuro_db::handle::Db;
use kuro_db::schema_unified;
use kuro_platform::kind::Kind;
use kuro_domain::download::prefs::DownloadPrefs;
use kuro_domain::storage::StorageManager;
use kuro_downloads::anime::downloader::EpisodeEngine;
use kuro_downloads::anime::layout::AnimeLayout;
use kuro_downloads::anime::manager::AnimeQueue;
use kuro_downloads::downloader::{Connectivity, DownloadSettings, NoHold};
use kuro_downloads::index::DownloadIndex;
use kuro_downloads::layout::DownloadLayout;
use kuro_downloads::manager::{DownloadManager, ManagerParams};
use kuro_downloads::manga::runner::ChapterPipeline;
use kuro_downloads::notify::{NotifierSink, spawn_notifier};
use kuro_downloads::pending::MangaPendingStore;
use kuro_downloads::store::{QueueRow, QueueStore};
use kuro_platform::notifier::Notifier;
use kuro_platform::paths::Paths;
use kuro_prefs::network::NetworkPrefs;
use kuro_prefs::store::PrefStore;
use kuro_source::MangaSource;
use tokio::sync::{mpsc, watch};

use kuro_downloads::next::gate::Connectivity as NextConnectivity;
use kuro_downloads::next::index::{DownloadIndex as NextIndex, IndexRoots};
use kuro_downloads::next::layout::Layout;
use kuro_downloads::next::notify::spawn_notifier as spawn_next_notifier;
use kuro_downloads::next::pending::PendingStore;
use kuro_downloads::next::queue::{Layouts, Queue, QueueParams};
use kuro_downloads::next::runner::UnitRunner;
use kuro_downloads::next::service::Downloads;
use kuro_downloads::next::store::QueueStore as NextQueueStore;
use kuro_downloads::next::watch::RootWatch;

use self::anime_io::{HttpVideoFetch, LiveHosters};
use self::directory::AppDirectory;
use self::manga_io::{Waiting, restore_rows};
use self::platform::NetworkFeed;

/// The queues the page and the shell reach. The anime queue is `None` when it did not start.
#[derive(Clone)]
pub struct Queues {
    pub manga: Arc<DownloadManager>,
    pub anime: Option<AnimeQueue>,
    /// Saved manga rows waiting for their source, as the queue page lists them.
    pub waiting: Arc<RwLock<Vec<Waiting>>>,
    /// Cancels a waiting row, by its chapter id.
    pub cancel_waiting: mpsc::UnboundedSender<i64>,
}

/// What [`start`] keeps alive. The connectivity sender stays here so the watch stays open while the managers run.
pub struct Started {
    pub queues: Queues,
    _connectivity: watch::Sender<Connectivity>,
}

/// The platform free-space query the managers use for the low-storage check.
pub fn free_space(path: &Path) -> Option<u64> {
    kuro_platform::linux::storage::free_bytes(path)
}

/// Starts the managers on the current runtime. The manga manager is required; a failure of the anime queue is logged
/// and the app runs with no anime queue. `index` is the shell's download index (shared, not reopened, so the snapshot
/// file has one writer). Errors are text for the log.
pub fn start(
    paths: &dyn Paths,
    index: DownloadIndex,
    db: Arc<Db>,
    prefs: PrefStore,
    notifier: Arc<dyn Notifier>,
) -> Result<Started, String> {
    let waiting = Arc::new(RwLock::new(Vec::new()));
    let (cancel_waiting, cancels) = mpsc::unbounded_channel();
    let storage = crate::settings_ui::storage::manager(&prefs).ok_or("the storage folder could not be resolved")?;
    let (manga, connectivity) = start_manga(paths, &storage, index, notifier.clone(), db.clone(), waiting.clone(), cancels)?;
    let anime = match anime_queue::start(paths, &storage, db, prefs, notifier) {
        Ok(queue) => Some(queue),
        Err(e) => {
            tracing::error!("the anime download queue did not start: {e}");
            None
        }
    };
    Ok(Started { queues: Queues { manga, anime, waiting, cancel_waiting }, _connectivity: connectivity })
}

/// The saved manga queue rows, in saved order. A table that cannot be read gives no rows, and the store then refuses to
/// write.
fn saved_rows(store: &QueueStore) -> Vec<QueueRow> {
    store.load().unwrap_or_else(|e| {
        tracing::error!("the saved manga queue could not be read, it is left as it is: {e}");
        Vec::new()
    })
}

/// Restores the saved manga rows. Once the sources have loaded, and again each time they change, every saved row whose
/// source is installed is queued (QUEUE, nothing starts, as the anime queue restores). The rest stay saved and are listed
/// as waiting. A cancel from the queue page removes a waiting row. All changes go through this task, so they stay in order.
fn spawn_restore(
    manga: Arc<DownloadManager>,
    db: Arc<Db>,
    saved: Vec<QueueRow>,
    shown: Arc<RwLock<Vec<Waiting>>>,
    mut cancels: mpsc::UnboundedReceiver<i64>,
) {
    tokio::spawn(async move {
        let mut ready = crate::recipe_sources::ready();
        let mut changes = crate::recipe_sources::changes();
        let mut waiting: Vec<Waiting> = saved.into_iter().map(|row| Waiting { row, title: String::new(), chapter: String::new() }).collect();
        loop {
            let loaded = *ready.borrow();
            if loaded && !waiting.is_empty() {
                let rows: Vec<QueueRow> = waiting.iter().map(|w| w.row.clone()).collect();
                let pass = restore_rows(&rows, &db, &|id: i64| -> Option<Arc<dyn MangaSource>> {
                    crate::browse::directory::current(crate::browse::route::BrowseKind::Manga).catalogue(id)?.manga()
                });
                let rest = pass.waiting;
                match manga.restore(pass.ready, rest.iter().map(|w| w.row.clone()).collect()).await {
                    Ok(()) => {
                        waiting = rest;
                        publish(&shown, &waiting);
                    }
                    Err(e) => tracing::error!("the saved manga queue could not be restored: {e}"),
                }
            }
            tokio::select! {
                changed = ready.changed() => if changed.is_err() { break },
                changed = changes.changed() => if changed.is_err() { break },
                Some(chapter_id) = cancels.recv() => {
                    waiting.retain(|w| w.row.chapter_id != chapter_id);
                    let rows: Vec<QueueRow> = waiting.iter().map(|w| w.row.clone()).collect();
                    if let Err(e) = manga.restore(Vec::new(), rows).await {
                        tracing::error!("the cancelled manga download could not be removed from the saved queue: {e}");
                    }
                    publish(&shown, &waiting);
                }
            }
        }
    });
}

/// Shows the waiting rows on the queue page.
fn publish(shown: &RwLock<Vec<Waiting>>, waiting: &[Waiting]) {
    *shown.write().unwrap_or_else(|e| e.into_inner()) = waiting.to_vec();
}

/// Starts the manga manager. Every saved row is kept until the first restore pass ([`spawn_restore`]) decides it.
fn start_manga(
    paths: &dyn Paths,
    storage: &StorageManager,
    index: DownloadIndex,
    notifier: Arc<dyn Notifier>,
    db: Arc<Db>,
    shown: Arc<RwLock<Vec<Waiting>>>,
    cancels: mpsc::UnboundedReceiver<i64>,
) -> Result<(Arc<DownloadManager>, watch::Sender<Connectivity>), String> {
    let root = storage.downloads_dir(Kind::Manga).map_err(|e| format!("the manga downloads folder is not available: {e}"))?;
    let store = QueueStore::open(schema_unified::open_in_data_dir(paths).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let pending = MangaPendingStore::open(schema_unified::open_in_data_dir(paths).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let saved = saved_rows(&store);
    // The Wi-Fi-only rule and the speed limit are off (the settings diet cut both switches), and the connectivity watch
    // starts as online and unmetered.
    let (connectivity_tx, connectivity) = watch::channel(Connectivity { online: true, metered: false });
    let manga = Arc::new(DownloadManager::spawn(ManagerParams {
        layout: Arc::new(DownloadLayout::new(&root)),
        index: Arc::new(index),
        store: Arc::new(store),
        free_space,
        settings: DownloadSettings::default(),
        pending: Arc::new(pending),
        page_cache: None,
        connectivity,
        hold: Arc::new(NoHold),
        restored: Vec::new(),
        kept_rows: saved.clone(),
    }));
    spawn_notifier(manga.events(), Arc::new(NotifierSink::new(notifier)), Arc::new(AtomicBool::new(false)));
    spawn_restore(manga.clone(), db, saved, shown, cancels);
    Ok((manga, connectivity_tx))
}

/// The one download service, both kinds, beside the old managers until the callers have moved (phase C of
/// docs/design/unified-downloads.md). What [`start_service`] keeps alive: the service, the folder watch and the network feed.
pub struct Service {
    pub downloads: Downloads,
    _watch: Option<RootWatch>,
    _network: NetworkFeed,
    /// The database the queue is saved in while `persist` is off. Deleted with the service.
    _scratch: Option<tempfile::TempDir>,
}

/// How often the index looks at whether a folder change made it stale.
const INDEX_CHECK: std::time::Duration = std::time::Duration::from_secs(10);

/// Starts the one `Downloads` on the current runtime. `persist` says whether the queue is saved in `download_queue`: the
/// old managers still save there until the flip, so it is off until then (a second writer would erase their rows). With it
/// off the queue is saved in a scratch database that goes with the service. `db` is the app database.
pub fn start_service(
    paths: &dyn Paths,
    db: Arc<Db>,
    prefs: PrefStore,
    notifier: Arc<dyn Notifier>,
    persist: bool,
) -> Result<Service, String> {
    let storage = crate::settings_ui::storage::manager(&prefs).ok_or("the storage folder could not be resolved")?;
    let root = |kind| storage.downloads_dir(kind).map_err(|e| format!("the {} downloads folder is not available: {e}", kind.as_str()));
    let (manga_root, anime_root) = (root(Kind::Manga)?, root(Kind::Anime)?);
    let (scratch, store_db) = if persist {
        (None, db.clone())
    } else {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let path = schema_unified::db_path(dir.path());
        let writer = schema_unified::open_file(&path).map_err(|e| e.to_string())?;
        let store_db = Arc::new(Db::new(&path, writer).map_err(|e| e.to_string())?);
        (Some(dir), store_db)
    };
    let directory = Arc::new(AppDirectory::new(db.clone()));
    let client = crate::net_client::build(&NetworkPrefs::new(prefs.clone()), paths).map_err(|e| e.to_string())?;
    let anime_layout = AnimeLayout::new(Some(anime_root.clone()));
    let runners: Vec<Arc<dyn UnitRunner>> = vec![
        Arc::new(ChapterPipeline::new(directory.clone())),
        Arc::new(EpisodeEngine::new(anime_layout, Arc::new(LiveHosters)).with_fetch(Arc::new(HttpVideoFetch::new(client)))),
    ];
    let index = NextIndex::open(
        IndexRoots { manga: storage.downloads_path(Kind::Manga), anime: storage.downloads_path(Kind::Anime) },
        &paths.download_index_cache_dir(),
    );
    // Read at every scheduling pass, so a change in Settings > Downloads applies to the running queue.
    let numbers = DownloadPrefs::new(&prefs);
    let (connectivity_tx, connectivity) = watch::channel(NextConnectivity { online: true, metered: false });
    let layouts = || Layouts { manga: Layout::new(Kind::Manga, manga_root.clone()), anime: Layout::new(Kind::Anime, anime_root.clone()) };
    let queue = Queue::spawn(QueueParams {
        db: db.clone(),
        store: Arc::new(NextQueueStore::new(store_db)),
        runners,
        directory: directory.clone(),
        layouts: layouts(),
        index: index.clone(),
        free_space,
        max_active: Arc::new(move || numbers.number_of_downloads().max(1) as usize),
        connectivity,
        only_over_wifi: false,
    });
    let downloads = Downloads::new(queue, index.clone(), layouts(), Arc::new(PendingStore::new(db.clone())), directory, db);
    spawn_next_notifier(downloads.events(), Arc::new(NotifierSink::new(notifier)), Arc::new(AtomicBool::new(false)));
    follow_sources(&downloads);
    Ok(Service { _watch: index.watch(), downloads, _network: NetworkFeed::start(connectivity_tx), _scratch: scratch })
}

/// Keeps the index and the waiting units in step with the sources: after each load of the sources both kinds are scanned
/// again and the waiting units are asked about their source, and a folder change that made the index stale is rescanned.
fn follow_sources(downloads: &Downloads) {
    let (index, queue) = (downloads.index().clone(), downloads.clone());
    crate::recipe_sources::follow_loads(move || {
        let (index, downloads) = (index.clone(), queue.clone());
        async move {
            for kind in Kind::ALL {
                index.rescan(kind, crate::recipe_sources::ready(), || crate::recipe_sources::folder_names(kind)).await;
            }
            downloads.sources_changed();
        }
    });
    let index = downloads.index().clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(INDEX_CHECK);
        loop {
            tick.tick().await;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
            for kind in Kind::ALL {
                index.renew_if_stale(kind, now, crate::recipe_sources::ready(), || crate::recipe_sources::folder_names(kind)).await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_saved_manga_rows_are_read_back_for_the_manager_to_keep() {
        let store = QueueStore::open_in_memory().unwrap();
        let rows = vec![
            QueueRow { chapter_id: 10, manga_id: 1, source_id: 7, position: 0, status: 1, enqueued_at: 5 },
            QueueRow { chapter_id: 11, manga_id: 1, source_id: 7, position: 1, status: 1, enqueued_at: 5 },
        ];
        store.replace_all(&rows).unwrap();
        assert_eq!(saved_rows(&store), rows);
    }

    #[test]
    fn free_space_reports_the_filesystem_of_an_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free_space(dir.path()).is_some_and(|bytes| bytes > 0));
    }
}
