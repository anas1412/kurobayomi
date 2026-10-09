//! Torrent helpers: the strict bencode parser and writer, `.torrent` metadata extraction, the TorrServer
//! JSON models and play-link routing (always built), and the TorrServer HTTP client and process supervisor
//! (behind the `torrent` feature, which pulls in `reqwest`).

pub mod bencode;
pub mod meta;
pub mod model;
pub mod route;

#[cfg(feature = "torrent")]
pub mod client;
#[cfg(feature = "torrent")]
pub mod supervisor;

/// Errors of the torrent area. User-facing text comes from message keys (see [`TorrentError::message_key`]),
/// not from `Display`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TorrentError {
    #[error("invalid torrent: {0}")]
    InvalidTorrent(String),
    #[error("bencode: {0}")]
    Bencode(String),
    #[error("torrent server is not running")]
    ServerUnavailable,
    #[error("torrent server timed out")]
    ServerTimeout,
    #[error("torrent server rejected request: {0}")]
    ServerRejected(u16),
    #[error("invalid port {0}")]
    InvalidPort(String),
    #[error("invalid proxy url: {0}")]
    InvalidProxy(String),
    #[error("torrent server failed: {0}")]
    Failed(String),
    #[error("http: {0}")]
    Http(String),
}

impl TorrentError {
    /// The message key for the playback-error toast: `torrserver_disabled` for `ServerUnavailable` (the Kotlin
    /// `DisabledTorrServerException`). Other errors have no catalogue key and are shown from their `Display`.
    pub fn message_key(&self) -> Option<&'static str> {
        match self {
            TorrentError::ServerUnavailable => Some("torrserver_disabled"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_unavailable_maps_to_the_torrserver_disabled_key() {
        assert_eq!(TorrentError::ServerUnavailable.message_key(), Some("torrserver_disabled"));
        assert_eq!(TorrentError::InvalidTorrent("x".into()).message_key(), None);
    }
}
