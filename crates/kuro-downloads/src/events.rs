//! `DownloadEvent` for both kinds, delivered on a broadcast channel. A slow subscriber gets `Lagged` and resynchronises
//! from the queue snapshot (docs/design/unified-downloads.md 2.12).

use kuro_platform::kind::Kind;

use crate::model::DownloadState;

/// Capacity of the broadcast channel.
pub const EVENT_CAPACITY: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub enum DownloadEvent {
    /// A unit changed state. Emitted for every transition the queue makes.
    Status { kind: Kind, unit_id: i64, state: DownloadState },
    /// Progress of one unit. `total` is 0 when the page count is not known (anime). At most one event per 100 ms per unit.
    Progress {
        kind: Kind,
        unit_id: i64,
        entry_id: i64,
        entry_title: String,
        unit_name: String,
        done: u32,
        total: u32,
        percent: u8,
    },
    /// The lane started running.
    Started { kind: Kind },
    /// The user paused, with units still queued.
    Paused { kind: Kind },
    /// The run ended with nothing left to do.
    Complete { kind: Kind },
    /// The lane was cleared: remove the progress notification.
    Dismiss { kind: Kind },
    /// A non-fatal notice. `timeout_secs` and `help_url` are optional.
    Warning { kind: Kind, text: String, timeout_secs: Option<u32>, help_url: Option<String> },
    /// A failure. The ids and names are present when the failing unit is known.
    Error {
        kind: Kind,
        unit_id: Option<i64>,
        entry_id: Option<i64>,
        entry_title: Option<String>,
        unit_name: Option<String>,
        message: String,
    },
    /// A unit was downloaded and removed from the queue.
    Finished { kind: Kind, unit_id: i64, entry_id: i64 },
    /// Membership or order changed. The view rebuilds from the snapshot.
    QueueChanged { kind: Kind },
}

impl DownloadEvent {
    /// The lane the event belongs to.
    pub fn kind(&self) -> Kind {
        match self {
            Self::Status { kind, .. }
            | Self::Progress { kind, .. }
            | Self::Started { kind }
            | Self::Paused { kind }
            | Self::Complete { kind }
            | Self::Dismiss { kind }
            | Self::Warning { kind, .. }
            | Self::Error { kind, .. }
            | Self::Finished { kind, .. }
            | Self::QueueChanged { kind } => *kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_event_names_its_lane() {
        let events = [
            DownloadEvent::Status { kind: Kind::Anime, unit_id: 1, state: DownloadState::Queue },
            DownloadEvent::Started { kind: Kind::Anime },
            DownloadEvent::Warning { kind: Kind::Anime, text: String::new(), timeout_secs: None, help_url: None },
            DownloadEvent::Finished { kind: Kind::Anime, unit_id: 1, entry_id: 2 },
            DownloadEvent::QueueChanged { kind: Kind::Anime },
        ];
        assert!(events.iter().all(|e| e.kind() == Kind::Anime));
        assert_eq!(DownloadEvent::Complete { kind: Kind::Manga }.kind(), Kind::Manga);
    }
}
