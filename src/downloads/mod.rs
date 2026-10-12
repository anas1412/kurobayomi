//! The download managers, started by the shell once the data has opened (lane D): the manga manager and the anime queue.
//! The manga queue lives in the manga database file and the anime queue in the anime file. Each has its own connection,
//! and both stores run their idempotent DDL when they open. The GApplication hold is not wired yet, so a run stops when
//! the process exits.

pub mod anime_io;
pub mod anime_queue;
pub mod manga_io;
pub mod queue_page;

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

use kuro_db::handle::Db;
use kuro_db::schema_manga;
use kuro_downloads::anime::manager::AnimeQueue;
use kuro_downloads::downloader::{Connectivity, DownloadSettings, NoHold};
use kuro_downloads::index::DownloadIndex;
use kuro_downloads::layout::DownloadLayout;
use kuro_downloads::manager::{DownloadManager, ManagerParams};
use kuro_downloads::notify::{NotifierSink, spawn_notifier};
use kuro_downloads::pending::MangaPendingStore;
use kuro_downloads::store::{QueueRow, QueueStore};
use kuro_platform::notifier::Notifier;
use kuro_platform::paths::Paths;
use kuro_prefs::store::PrefStore;
use kuro_source::MangaSource;
use tokio::sync::{mpsc, watch};

use self::manga_io::{Waiting, restore_rows};

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
    manga_db: Arc<Db>,
    anime_db: Arc<kuro_db::handle::Db>,
    prefs: PrefStore,
    notifier: Arc<dyn Notifier>,
) -> Result<Started, String> {
    let waiting = Arc::new(RwLock::new(Vec::new()));
    let (cancel_waiting, cancels) = mpsc::unbounded_channel();
    let (manga, connectivity) = start_manga(paths, index, notifier.clone(), manga_db, waiting.clone(), cancels)?;
    let anime = match anime_queue::start(paths, anime_db, prefs, notifier) {
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
                    crate::browse::directory::current()?.manga_source(id)
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
    index: DownloadIndex,
    notifier: Arc<dyn Notifier>,
    manga_db: Arc<Db>,
    shown: Arc<RwLock<Vec<Waiting>>>,
    cancels: mpsc::UnboundedReceiver<i64>,
) -> Result<(Arc<DownloadManager>, watch::Sender<Connectivity>), String> {
    let root = paths.default_downloads_dir();
    std::fs::create_dir_all(&root).map_err(|e| format!("the downloads folder was not created: {e}"))?;
    let store = QueueStore::open(schema_manga::open_in_data_dir(paths).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let pending = MangaPendingStore::open(schema_manga::open_in_data_dir(paths).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
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
    spawn_restore(manga.clone(), manga_db, saved, shown, cancels);
    Ok((manga, connectivity_tx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_saved_manga_rows_are_read_back_for_the_manager_to_keep() {
        // the queue tables point at the library tables, so the stand-ins exist before the store opens
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE mangas(_id INTEGER PRIMARY KEY); CREATE TABLE chapters(_id INTEGER PRIMARY KEY);
             INSERT INTO mangas(_id) VALUES (1); INSERT INTO chapters(_id) VALUES (10), (11);",
        )
        .unwrap();
        let store = QueueStore::open(conn).unwrap();
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
