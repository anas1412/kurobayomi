//! The download state of an update row (T-42a-029). Pure: the queue snapshot and the disk check are the caller's; this
//! file decides the precedence, keeps the disk-check cache, and applies status and progress events to the one row
//! they name. The enum is the download module's `DownloadState` (ordinals 0 to 4, as stored).

use std::collections::HashMap;

pub use kuro_downloads::model::DownloadState;

/// The state and progress a row shows. Progress is the queue's value, or 0 when there is no queue entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowDownload {
    pub state: DownloadState,
    pub progress: i64,
}

/// The queue entry of one unit: its status and progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueEntry {
    pub status: DownloadState,
    pub progress: i64,
}

/// The precedence of T-42a-029: the active queue status, else DOWNLOADED when the disk check passes, else
/// NOT_DOWNLOADED.
pub fn derive(queue: Option<QueueEntry>, on_disk: bool) -> RowDownload {
    match queue {
        Some(entry) => RowDownload { state: entry.status, progress: entry.progress },
        None if on_disk => RowDownload { state: DownloadState::Downloaded, progress: 0 },
        None => RowDownload { state: DownloadState::NotDownloaded, progress: 0 },
    }
}

/// The key of a disk check: the source, the entry title, the unit name and the scanlator (the Kotlin lookup).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DiskKey {
    pub source_id: i64,
    pub entry_title: String,
    pub unit_name: String,
    pub scanlator: Option<String>,
}

/// Disk checks cached per key. The cache is dropped when the download cache signals a change (rust design 10,
/// pitfall 8), so a check is never older than the last signal.
#[derive(Debug, Default)]
pub struct DiskCache {
    checks: HashMap<DiskKey, bool>,
}

impl DiskCache {
    /// The cached answer for `key`, or the answer of `check` (run once and remembered).
    pub fn is_downloaded(&mut self, key: DiskKey, check: impl FnOnce() -> bool) -> bool {
        *self.checks.entry(key).or_insert_with(check)
    }

    /// Forgets every answer. Call it on each download-cache signal.
    pub fn invalidate(&mut self) {
        self.checks.clear();
    }

    pub fn len(&self) -> usize {
        self.checks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }
}

/// One row of the feed as the download state sees it: the unit it belongs to and what it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitDownload {
    pub unit_id: i64,
    pub download: RowDownload,
}

/// A download event for one unit: a new status, or a new progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Status(DownloadState),
    Progress(i64),
}

/// Applies `change` to the row with `unit_id` only. Returns whether a row was found; unknown ids are ignored.
pub fn apply(rows: &mut [UnitDownload], unit_id: i64, change: Change) -> bool {
    match rows.iter_mut().find(|r| r.unit_id == unit_id) {
        Some(row) => {
            match change {
                Change::Status(state) => row.download.state = state,
                Change::Progress(progress) => row.download.progress = progress,
            }
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> DiskKey {
        DiskKey { source_id: 1, entry_title: "Title".into(), unit_name: name.into(), scanlator: None }
    }

    #[test]
    fn download_state_precedence_queue_then_disk_then_none() {
        let queued = QueueEntry { status: DownloadState::Downloading, progress: 40 };
        assert_eq!(derive(Some(queued), true), RowDownload { state: DownloadState::Downloading, progress: 40 });
        assert_eq!(derive(None, true), RowDownload { state: DownloadState::Downloaded, progress: 0 });
        assert_eq!(derive(None, false), RowDownload { state: DownloadState::NotDownloaded, progress: 0 });
    }

    #[test]
    fn download_state_ordinals_are_preserved() {
        assert_eq!(DownloadState::NotDownloaded as u8, 0);
        assert_eq!(DownloadState::Queue as u8, 1);
        assert_eq!(DownloadState::Downloading as u8, 2);
        assert_eq!(DownloadState::Downloaded as u8, 3);
        assert_eq!(DownloadState::Error as u8, 4);
    }

    #[test]
    fn download_disk_check_is_cached_per_key_and_invalidated() {
        let mut cache = DiskCache::default();
        let mut calls = 0;
        assert!(cache.is_downloaded(key("1"), || { calls += 1; true }));
        assert!(cache.is_downloaded(key("1"), || { calls += 1; false }));
        assert_eq!(calls, 1, "the second lookup is served from the cache");
        assert!(!cache.is_downloaded(key("2"), || false));
        assert_eq!(cache.len(), 2);
        cache.invalidate();
        assert!(cache.is_empty());
        assert!(cache.is_downloaded(key("1"), || true));
    }

    #[test]
    fn download_events_update_only_the_named_row() {
        let mut rows = vec![
            UnitDownload { unit_id: 10, download: RowDownload { state: DownloadState::Queue, progress: 0 } },
            UnitDownload { unit_id: 11, download: RowDownload { state: DownloadState::NotDownloaded, progress: 0 } },
        ];
        assert!(apply(&mut rows, 11, Change::Status(DownloadState::Downloading)));
        assert!(apply(&mut rows, 11, Change::Progress(55)));
        assert_eq!(rows[0].download, RowDownload { state: DownloadState::Queue, progress: 0 });
        assert_eq!(rows[1].download, RowDownload { state: DownloadState::Downloading, progress: 55 });
        assert!(!apply(&mut rows, 99, Change::Progress(100)), "an unknown id is ignored");
        assert_eq!(rows[1].download.progress, 55);
    }
}
