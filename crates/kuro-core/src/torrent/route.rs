//! Playback routing and play links (`PlayerActivity.torrentLinkHandler`, `TorrentServerUtils.getTorrentPlayLink`).

use super::model::TorrentDto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Direct,
    TorrServer,
}

/// The torrent predicate of the player: torrents on, and the URL is a magnet link, a `.torrent` URL or
/// already under the local server.
pub fn classify(url: &str, server_base: &str, enabled: bool) -> Route {
    if enabled && (url.starts_with(server_base) || url.starts_with("magnet") || url.ends_with("torrent")) {
        Route::TorrServer
    } else {
        Route::Direct
    }
}

/// The `index=N` of a magnet link; 0 when absent or not a number. A negative value also gives 0, since the
/// play link index is unsigned.
pub fn magnet_index(url: &str) -> u32 {
    let Some((_, rest)) = url.split_once("index=") else { return 0 };
    let value = rest.split('&').next().unwrap_or("");
    value.parse::<i32>().ok().and_then(|n| u32::try_from(n).ok()).unwrap_or(0)
}

/// `java.net.URLEncoder.encode(s, "utf8")`: alphanumerics and `.-*_` stay, a space is `+`, the rest `%XX`.
fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' | b'*' | b'_' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `http://127.0.0.1:{port}/stream/{name}?link={hash}&index={index}&play`. The name is the file whose
/// `id == index` (last path component), else the torrent title.
pub fn play_link(port: u16, torrent: &TorrentDto, index: u32) -> String {
    let file = torrent.file_stats.as_ref().and_then(|f| f.iter().find(|f| f.id == i32::try_from(index).ok()));
    let name = file.map_or(torrent.title.as_str(), |f| f.path.rsplit('/').next().unwrap_or(&f.path));
    let hash = torrent.hash.as_deref().unwrap_or("");
    format!("http://127.0.0.1:{port}/stream/{}?link={hash}&index={index}&play", form_encode(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::torrent::model::FileStatDto;

    const BASE: &str = "http://127.0.0.1:8090";

    #[test]
    fn classify_vectors() {
        assert_eq!(classify("magnet:?xt=urn:btih:abc", BASE, true), Route::TorrServer);
        assert_eq!(classify("https://x/y.torrent", BASE, true), Route::TorrServer);
        assert_eq!(classify("http://127.0.0.1:8090/stream/a?link=h", BASE, true), Route::TorrServer);
        assert_eq!(classify("https://x/video.mp4", BASE, true), Route::Direct);
        assert_eq!(classify("magnet:?xt=urn:btih:abc", BASE, false), Route::Direct);
        assert_eq!(classify("https://x/y.torrent", BASE, false), Route::Direct);
    }

    #[test]
    fn magnet_index_vectors() {
        assert_eq!(magnet_index("magnet:?xt=a&index=3&x=1"), 3);
        assert_eq!(magnet_index("magnet:?index=abc"), 0);
        assert_eq!(magnet_index("magnet:?xt=a"), 0);
        assert_eq!(magnet_index("magnet:?xt=a&index=7"), 7);
        assert_eq!(magnet_index("magnet:?index=-2"), 0);
        assert_eq!(magnet_index("magnet:?index="), 0);
    }

    fn dto(files: Option<Vec<FileStatDto>>) -> TorrentDto {
        TorrentDto { title: "Title".into(), hash: Some("abc".into()), torrent_size: None, trackers: None, file_stats: files }
    }

    #[test]
    fn play_link_uses_the_file_name_for_a_matching_id() {
        let t = dto(Some(vec![FileStatDto { id: Some(2), path: "3/3.1".into(), length: 1 }]));
        assert_eq!(play_link(8090, &t, 2), "http://127.0.0.1:8090/stream/3.1?link=abc&index=2&play");
    }

    #[test]
    fn play_link_form_encodes_the_name() {
        let t = dto(Some(vec![FileStatDto { id: Some(1), path: "a b.mkv".into(), length: 1 }]));
        assert_eq!(play_link(1, &t, 1), "http://127.0.0.1:1/stream/a+b.mkv?link=abc&index=1&play");
    }

    #[test]
    fn play_link_falls_back_to_the_title() {
        assert_eq!(play_link(1, &dto(None), 0), "http://127.0.0.1:1/stream/Title?link=abc&index=0&play");
        let t = dto(Some(vec![FileStatDto { id: None, path: "x".into(), length: 1 }]));
        assert!(play_link(1, &t, 0).contains("/stream/Title?"));
    }

    #[test]
    fn form_encode_matches_url_encoder() {
        assert_eq!(form_encode("é/&=~"), "%C3%A9%2F%26%3D%7E");
    }
}
