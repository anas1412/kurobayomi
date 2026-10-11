//! The download managers, started by the shell once the data has opened (lane D): the manga manager and the anime queue.
//! The manga queue lives in the manga database file and the anime queue in the anime file. Each has its own connection,
//! and both stores run their idempotent DDL when they open. The GApplication hold is not wired yet, so a run stops when
//! the process exits.

pub mod anime_io;
pub mod anime_queue;
pub mod queue_page;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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
use tokio::sync::watch;

/// The queues the page and the shell reach. The anime queue is `None` when it did not start.
#[derive(Clone)]
pub struct Queues {
    pub manga: Arc<DownloadManager>,
    pub anime: Option<AnimeQueue>,
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
    anime_db: Arc<kuro_db::handle::Db>,
    prefs: PrefStore,
    notifier: Arc<dyn Notifier>,
) -> Result<Started, String> {
    let (manga, connectivity) = start_manga(paths, index, notifier.clone())?;
    let anime = match anime_queue::start(paths, anime_db, prefs, notifier) {
        Ok(queue) => Some(queue),
        Err(e) => {
            tracing::error!("the anime download queue did not start: {e}");
            None
        }
    };
    Ok(Started { queues: Queues { manga, anime }, _connectivity: connectivity })
}

/// The saved manga queue rows, in saved order. Nothing in the app builds a manga `ChapterSource` yet, so no row can
/// come back as a queued download: every row is a SourceMissing row (the manager's `kept_rows`), written back unchanged
/// by each save instead of being erased. A table that cannot be read gives no rows, and the store then refuses to write.
fn saved_rows(store: &QueueStore) -> Vec<QueueRow> {
    store.load().unwrap_or_else(|e| {
        tracing::error!("the saved manga queue could not be read, it is left as it is: {e}");
        Vec::new()
    })
}

/// Starts the manga manager. Saved rows stay in `kept_rows` ([`saved_rows`]) until a manga source can resolve them.
fn start_manga(
    paths: &dyn Paths,
    index: DownloadIndex,
    notifier: Arc<dyn Notifier>,
) -> Result<(Arc<DownloadManager>, watch::Sender<Connectivity>), String> {
    let root = paths.default_downloads_dir();
    std::fs::create_dir_all(&root).map_err(|e| format!("the downloads folder was not created: {e}"))?;
    let store = QueueStore::open(schema_manga::open_in_data_dir(paths).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let pending = MangaPendingStore::open(schema_manga::open_in_data_dir(paths).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let kept_rows = saved_rows(&store);
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
        kept_rows,
    }));
    spawn_notifier(manga.events(), Arc::new(NotifierSink::new(notifier)), Arc::new(AtomicBool::new(false)));
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
