//! Torrent links of anime episodes (T-24a-023; behaviour 5.5). A link is a torrent when it is a magnet link, a `.torrent`
//! file, or a link under the TorrServer host. Such a link needs the torrent server, which is built only with the
//! `torrent-server` feature. Without the feature a torrent link fails with `TorrentUnavailable` (E-X04).

use crate::error::DownloadError;

/// The TorrServer address when the user has not set one. TorrServer listens on port 8090 by default.
pub const TORRENT_SERVER_HOST: &str = "http://127.0.0.1:8090";

/// `isTorrent`: a magnet link, a `.torrent` file, or a link under the TorrServer host. An empty host matches nothing.
pub fn is_torrent(url: &str, server_host: &str) -> bool {
    url.starts_with("magnet") || url.ends_with(".torrent") || (!server_host.is_empty() && url.starts_with(server_host))
}

/// A TorrServer link `…?link=<hash>&index=<n>` as a magnet link `magnet:?xt=urn:btih:<hash>&index=<n>`. `None` when either
/// part is missing or the index is not a number.
pub fn host_link_to_magnet(url: &str) -> Option<String> {
    let hash = value_after(url, "link=")?;
    let index: u32 = value_after(url, "index=")?.parse().ok()?;
    Some(format!("magnet:?xt=urn:btih:{hash}&index={index}"))
}

/// The text after `key` up to the next `&`. An empty value is `None`.
fn value_after<'a>(url: &'a str, key: &str) -> Option<&'a str> {
    let value = url.split_once(key)?.1.split('&').next()?;
    (!value.is_empty()).then_some(value)
}

/// Refuses a torrent link when this build cannot take it. Without the `torrent-server` feature the answer is
/// `TorrentUnavailable`. With the feature the stream from TorrServer is still to be wired, so the link fails with an
/// internal error and is not played as a plain URL. A link that is not a torrent passes.
pub fn check_torrent(url: &str, server_host: &str) -> Result<(), DownloadError> {
    if !is_torrent(url, server_host) {
        return Ok(());
    }
    if cfg!(feature = "torrent-server") {
        Err(DownloadError::Internal("the TorrServer stream is not wired in this build yet".into()))
    } else {
        Err(DownloadError::TorrentUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOST: &str = "http://127.0.0.1:8090";

    #[test]
    fn magnet_torrent_file_and_host_links_are_torrents() {
        assert!(is_torrent("magnet:?xt=urn:btih:abc", HOST));
        assert!(is_torrent("https://h.org/show.torrent", HOST));
        assert!(is_torrent("http://127.0.0.1:8090/stream?link=abc&index=1", HOST));
        assert!(!is_torrent("https://h.org/video.m3u8", HOST));
        assert!(!is_torrent("https://h.org/video.m3u8", ""), "an empty host matches nothing");
    }

    #[test]
    fn host_link_becomes_a_magnet_with_its_index() {
        assert_eq!(
            host_link_to_magnet("http://127.0.0.1:8090/stream?link=abc123&index=2&play").as_deref(),
            Some("magnet:?xt=urn:btih:abc123&index=2")
        );
        assert_eq!(host_link_to_magnet("http://127.0.0.1:8090/stream?link=&index=2"), None, "an empty hash is refused");
        assert_eq!(host_link_to_magnet("http://127.0.0.1:8090/stream?link=abc"), None, "no index");
        assert_eq!(host_link_to_magnet("http://127.0.0.1:8090/stream?link=abc&index=x"), None, "index is not a number");
    }

    #[test]
    fn without_the_feature_a_torrent_link_is_torrent_unavailable() {
        if cfg!(feature = "torrent-server") {
            return;
        }
        assert!(matches!(check_torrent("magnet:?xt=urn:btih:abc", HOST), Err(DownloadError::TorrentUnavailable)));
        assert!(check_torrent("https://h.org/video.m3u8", HOST).is_ok());
    }
}
