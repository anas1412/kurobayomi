//! Torrent file and info models, and the magnet link builder (`torrentutils/model`; T-16a-039).

use url::form_urlencoded::byte_serialize;

/// One file inside a torrent. `trackers` is the list the magnet link is built from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TorrentFile {
    pub path: String,
    pub index_file: i32,
    pub size: i64,
    pub torrent_hash: String,
    pub trackers: Vec<String>,
}

impl TorrentFile {
    /// `magnet:?xt=urn:btih:<hash>`, one `&tr=` per tracker (form-encoded, as Kotlin `URLEncoder` writes it), then
    /// `&index=<index>`.
    pub fn magnet_uri(&self) -> String {
        let mut uri = format!("magnet:?xt=urn:btih:{}", self.torrent_hash);
        for tracker in &self.trackers {
            uri.push_str("&tr=");
            uri.extend(byte_serialize(tracker.as_bytes()));
        }
        uri.push_str(&format!("&index={}", self.index_file));
        uri
    }
}

/// The metadata of one torrent: its title, its files, and the trackers of the whole torrent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TorrentInfo {
    pub title: String,
    pub files: Vec<TorrentFile>,
    pub hash: String,
    pub size: i64,
    pub trackers: Vec<String>,
}

impl TorrentInfo {
    /// A copy of this torrent with `trackers` in place of the old list. The receiver is unchanged.
    pub fn set_trackers(&self, trackers: Vec<String>) -> Self {
        Self { trackers, ..self.clone() }
    }
}

/// Failures of a torrent lookup (`torrentutils/TorrentUtils.kt`, E-16 to E-18). `Dead` is the Kotlin
/// `DeadTorrentException` (the server timed out). `ServerNotRunning` is the `DisabledTorrServerException` text, shown
/// only when the server cannot be reached. Every other failure keeps its own message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TorrentError {
    #[error("the torrent is dead")]
    Dead,
    #[error("TorrServer is not running")]
    ServerNotRunning,
    #[error("HTTP error {0}")]
    Http(u16),
    #[error("{0}")]
    Transport(String),
    #[error("parse error: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(hash: &str, trackers: &[&str], index: i32) -> TorrentFile {
        TorrentFile {
            path: "a.mkv".into(),
            index_file: index,
            size: 1,
            torrent_hash: hash.into(),
            trackers: trackers.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn the_magnet_encodes_the_tracker_and_the_index() {
        let magnet = file("abc", &["udp://tracker.example:80/announce?x=a b"], 2).magnet_uri();
        assert_eq!(magnet, "magnet:?xt=urn:btih:abc&tr=udp%3A%2F%2Ftracker.example%3A80%2Fannounce%3Fx%3Da+b&index=2");
    }

    #[test]
    fn a_magnet_without_trackers_has_no_tr_parameter() {
        assert_eq!(file("abc", &[], 0).magnet_uri(), "magnet:?xt=urn:btih:abc&index=0");
    }

    #[test]
    fn every_tracker_gets_its_own_tr_parameter_in_order() {
        let magnet = file("h", &["udp://a.example/x", "http://b.example/"], 1).magnet_uri();
        assert_eq!(magnet, "magnet:?xt=urn:btih:h&tr=udp%3A%2F%2Fa.example%2Fx&tr=http%3A%2F%2Fb.example%2F&index=1");
    }

    #[test]
    fn set_trackers_returns_a_copy_with_the_new_list() {
        let info = TorrentInfo { title: "T".into(), hash: "h".into(), trackers: vec!["old".into()], ..Default::default() };
        let updated = info.set_trackers(vec!["new".into()]);
        assert_eq!(updated.trackers, vec!["new".to_string()]);
        assert_eq!((updated.title.as_str(), updated.hash.as_str()), ("T", "h"));
        assert_eq!(info.trackers, vec!["old".to_string()], "the receiver is unchanged");
    }

    #[test]
    fn a_dead_torrent_is_an_error_variant_with_a_text() {
        let err: Box<dyn std::error::Error> = Box::new(TorrentError::Dead);
        assert_eq!(err.to_string(), "the torrent is dead");
    }
}
