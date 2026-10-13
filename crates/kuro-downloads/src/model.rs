//! The state of a download, shared by both kinds (`MangaDownload.State`).

/// `MangaDownload.State`. The numeric values are stored in the queue table, and "active" means `<= Downloading`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DownloadState {
    NotDownloaded = 0,
    Queue = 1,
    Downloading = 2,
    Downloaded = 3,
    Error = 4,
}

impl DownloadState {
    /// Kotlin `status.value <= DOWNLOADING.value`: states 0, 1 and 2.
    pub fn is_active(self) -> bool {
        (self as u8) <= (Self::Downloading as u8)
    }

    /// Whether the status diagram of the anime download (behaviour 1.1, T-24a-001) has an edge from `self` to `to`.
    /// Staying where it is is always allowed. Removal ends in NOT_DOWNLOADED only from QUEUE and DOWNLOADING (an
    /// ERROR item keeps its status when removed), and DOWNLOADED is reached only from DOWNLOADING.
    pub fn can_become(self, to: Self) -> bool {
        use DownloadState::{Downloaded, Downloading, Error, NotDownloaded, Queue};
        self == to
            || matches!(
                (self, to),
                (Queue, Downloading)
                    | (Downloading, Downloaded)
                    | (Downloading, Error)
                    | (Error, Queue)
                    | (Downloading, Queue)
                    | (Queue, NotDownloaded)
                    | (Downloading, NotDownloaded)
            )
    }
}

#[cfg(test)]
mod state_tests {
    use super::DownloadState::{self, *};

    const ALL: [DownloadState; 5] = [NotDownloaded, Queue, Downloading, Downloaded, Error];

    #[test]
    fn the_numeric_values_and_is_active_follow_the_spec() {
        assert_eq!(ALL.map(|s| s as u8), [0, 1, 2, 3, 4]);
        assert_eq!(ALL.map(DownloadState::is_active), [true, true, true, false, false], "T-24a-001: active is 0..=2");
    }

    #[test]
    fn exactly_the_edges_of_the_state_diagram_are_allowed() {
        let edges = [
            (Queue, Downloading),
            (Downloading, Downloaded),
            (Downloading, Error),
            (Error, Queue),
            (Downloading, Queue),
            (Queue, NotDownloaded),
            (Downloading, NotDownloaded),
        ];
        for from in ALL {
            for to in ALL {
                let expected = from == to || edges.contains(&(from, to));
                assert_eq!(from.can_become(to), expected, "{from:?} -> {to:?}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_state_values_are_stable() {
        assert_eq!(DownloadState::NotDownloaded as u8, 0);
        assert_eq!(DownloadState::Queue as u8, 1);
        assert_eq!(DownloadState::Downloading as u8, 2);
        assert_eq!(DownloadState::Downloaded as u8, 3);
        assert_eq!(DownloadState::Error as u8, 4);
    }

    #[test]
    fn only_states_zero_to_two_are_active() {
        let active: Vec<u8> = [
            DownloadState::NotDownloaded,
            DownloadState::Queue,
            DownloadState::Downloading,
            DownloadState::Downloaded,
            DownloadState::Error,
        ]
        .into_iter()
        .filter(|s| s.is_active())
        .map(|s| s as u8)
        .collect();
        assert_eq!(active, vec![0, 1, 2]);
    }
}
