//! Which sources go to the torrent server (T-45a-074 and T-45a-078; `PlayerActivity.kt:1093-1197`, area 13 section 7).

/// How a routed source reaches the torrent server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TorrentSource {
    /// A local `.torrent` (`content://`): the bytes are uploaded and the file at index 0 plays.
    Upload,
    /// A magnet link: added by its link, and the file is the `index=` value.
    Magnet,
    /// Any other link: TorrServer fetches the `.torrent` itself, and the file at index 0 plays.
    Url,
}

/// Whether a video goes through the torrent server. Torrents must be enabled, and the URL must start with the server
/// base, start with `magnet`, or end with `torrent`. `server_base` is `http://127.0.0.1:<port>`; an empty base matches
/// nothing.
pub fn is_torrent_route(url: &str, enabled: bool, server_base: &str) -> bool {
    enabled
        && (url.starts_with("magnet")
            || url.ends_with("torrent")
            || (!server_base.is_empty() && url.starts_with(server_base)))
}

/// What a routed source is: `content://` is an upload, `magnet` is a magnet link, anything else is a URL.
pub fn classify(url: &str) -> TorrentSource {
    if url.starts_with("content://") {
        TorrentSource::Upload
    } else if url.starts_with("magnet") {
        TorrentSource::Magnet
    } else {
        TorrentSource::Url
    }
}

/// The file index of a magnet link: the `index=<n>` query value. A missing or non-integer value is `0`.
pub fn magnet_index(url: &str) -> i64 {
    url.split(['?', '&'])
        .find_map(|part| part.strip_prefix("index="))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "http://127.0.0.1:8090";

    #[test]
    fn the_route_needs_the_switch_and_a_torrent_shaped_url() {
        assert!(is_torrent_route(
            "http://127.0.0.1:8090/stream?link=a",
            true,
            BASE
        ));
        assert!(is_torrent_route("magnet:?xt=urn:btih:a", true, BASE));
        assert!(is_torrent_route("https://h.org/show.torrent", true, BASE));
        assert!(!is_torrent_route("https://h.org/video.m3u8", true, BASE));
        assert!(
            !is_torrent_route("magnet:?xt=urn:btih:a", false, BASE),
            "torrents disabled"
        );
        assert!(
            !is_torrent_route("https://h.org/a", true, ""),
            "an empty base matches nothing"
        );
    }

    #[test]
    fn sources_are_classified_by_their_prefix() {
        assert_eq!(classify("content://x/show.torrent"), TorrentSource::Upload);
        assert_eq!(
            classify("magnet:?xt=urn:btih:a&index=2"),
            TorrentSource::Magnet
        );
        assert_eq!(classify("https://h.org/show.torrent"), TorrentSource::Url);
    }

    #[test]
    fn the_magnet_index_is_the_index_value_or_zero() {
        assert_eq!(magnet_index("magnet:?xt=urn:btih:a&index=3"), 3);
        assert_eq!(magnet_index("magnet:?index=2&xt=urn:btih:a"), 2);
        assert_eq!(
            magnet_index("magnet:?xt=urn:btih:a&index=x"),
            0,
            "not an integer"
        );
        assert_eq!(magnet_index("magnet:?xt=urn:btih:a"), 0, "missing");
    }
}
