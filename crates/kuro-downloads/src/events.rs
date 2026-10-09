//! `DownloadEvent`, replacing the Kotlin StateFlows. Delivered on a broadcast channel. A slow subscriber gets
//! `Lagged` and resynchronises from `DownloadManager::snapshot()`.

use crate::model::DownloadState;

/// Capacity of the broadcast channel (T-23a-030).
pub const EVENT_CAPACITY: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub enum DownloadEvent {
    /// A chapter changed state. Emitted for every transition the queue makes.
    Status { chapter_id: i64, state: DownloadState },
    /// Page progress for one chapter. Coalesced to at most one event per 50 ms per chapter.
    Progress { chapter_id: i64, manga_title: String, chapter_name: String, downloaded: usize, total: usize },
    /// The queue started running.
    Started,
    /// The user paused, with chapters still queued.
    Paused,
    /// The run ended with nothing left to do.
    Complete,
    /// The queue was cleared: remove the progress notification.
    Dismiss,
    /// A non-fatal notice. `timeout_secs` and `help_url` are optional.
    Warning { text: String, timeout_secs: Option<u32>, help_url: Option<String> },
    /// A failure. Names are present when the failing chapter is known.
    Error { chapter_id: Option<i64>, manga_id: Option<i64>, manga_title: Option<String>, chapter_name: Option<String>, message: String },
    /// A chapter was downloaded and removed from the queue.
    Finished { chapter_id: i64 },
    /// Membership or order changed. The view model rebuilds from `snapshot()`.
    QueueChanged,
}
