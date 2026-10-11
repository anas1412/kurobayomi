//! The manga download manager, started by the shell once the data has opened (lane D, basic downloads).
//!
//! The queue lives in the manga database file: its tables are created by `QueueStore::open` and `MangaPendingStore::open`
//! (idempotent DDL). Rows saved by an earlier run are not restored (no chapter lookup yet); the first save rewrites the table
//! from the live queue. The Wi-Fi-only rule is off, the speed
//! limit is read from `download_speed_limit` at each chapter start, and the connectivity watch starts as online and
//! unmetered. The GApplication hold is not wired yet, so a run stops when the process exits.

use std::path::Path;
use std::sync::Arc;

use kuro_db::schema_manga;
use kuro_downloads::download_prefs::DownloadPrefs;
use kuro_downloads::downloader::{Connectivity, DownloadSettings, NoHold, SpeedLimitFn};
use kuro_downloads::index::DownloadIndex;
use kuro_downloads::layout::DownloadLayout;
use kuro_downloads::manager::{DownloadManager, ManagerParams};
use kuro_downloads::pending::MangaPendingStore;
use kuro_downloads::store::QueueStore;
use kuro_platform::paths::Paths;
use tokio::sync::watch;

/// The running manga download manager. The connectivity sender lives here so the watch stays open while the manager runs.
pub struct Started {
    pub manga: DownloadManager,
    _connectivity: watch::Sender<Connectivity>,
}

/// The platform free-space query the managers use for the low-storage check.
pub fn free_space(path: &Path) -> Option<u64> {
    kuro_platform::linux::storage::free_bytes(path)
}

/// Starts the manga manager on the current runtime. `index` is the shell's download index (shared, not reopened, so
/// the snapshot file has one writer). Errors are text for the log: the app runs without downloads when they fail.
pub fn start_manga(paths: &dyn Paths, index: DownloadIndex, prefs: DownloadPrefs) -> Result<Started, String> {
    let root = paths.default_downloads_dir();
    std::fs::create_dir_all(&root).map_err(|e| format!("the downloads folder was not created: {e}"))?;
    let store = QueueStore::open(schema_manga::open_in_data_dir(paths).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let pending = MangaPendingStore::open(schema_manga::open_in_data_dir(paths).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    // Saved rows are not restored yet. `kept_rows` is for SourceMissing rows only: passing every row would make Clear
    // and cancel come back on each launch and let a re-enqueued chapter insert its id twice. The next save rewrites the table.
    let speed: SpeedLimitFn = Arc::new(move || prefs.download_speed_limit().get());
    let (connectivity_tx, connectivity) = watch::channel(Connectivity { online: true, metered: false });
    let manga = DownloadManager::spawn(ManagerParams {
        layout: Arc::new(DownloadLayout::new(&root)),
        index: Arc::new(index),
        store: Arc::new(store),
        free_space,
        settings: DownloadSettings { only_over_wifi: false, ..DownloadSettings::default() },
        speed_limit_kib_s: Some(speed),
        pending: Arc::new(pending),
        page_cache: None,
        connectivity,
        hold: Arc::new(NoHold),
        restored: Vec::new(),
        kept_rows: Vec::new(),
    });
    Ok(Started { manga, _connectivity: connectivity_tx })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_space_reports_the_filesystem_of_an_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free_space(dir.path()).is_some_and(|bytes| bytes > 0));
    }
}
