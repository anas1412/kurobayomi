//! Torrent info resolution (`TorrentUtils.getTorrentInfo` and `torrentToTorrentInfo`, T-16a-040) over the TorrServer
//! backend (T-16a-041).
//!
//! The chosen backend is the TorrServer-compatible HTTP API, not an embedded `librqbit`. A magnet link is added to the
//! server at `127.0.0.1:<port>` (`POST /torrents`). A `.torrent` URL is downloaded and parsed here, with no server. The
//! client and the bencode parser are the ones in `kuro-core::torrent`. With the backend switched off, a magnet link
//! fails before any request is sent, so the server is never called.

use kuro_core::torrent::TorrentError as CoreError;
use kuro_core::torrent::client::TorrServerClient;
use kuro_core::torrent::meta::{TorrentMeta, parse_torrent_file};
use kuro_core::torrent::model::{FileStatDto, TorrentDto};

use super::torrent::{TorrentError, TorrentFile, TorrentInfo};

/// `url.startsWith("magnet")`, but case-insensitive (E-17). Only the scheme is compared, so a multi-byte character in
/// the url cannot split a char boundary.
pub fn is_magnet(url: &str) -> bool {
    url.get(..6).is_some_and(|scheme| scheme.eq_ignore_ascii_case("magnet"))
}

/// The title of the result: the caller's title, or the torrent's own title when the caller's is empty (E-15).
pub fn pick_title(title: &str, torrent_title: &str) -> String {
    if title.is_empty() { torrent_title.to_owned() } else { title.to_owned() }
}

/// `torrentToTorrentInfo`. A missing hash is a parse error, not a panic (E-16). A missing size is -1.
pub fn info_from_dto(dto: &TorrentDto, title: &str) -> Result<TorrentInfo, TorrentError> {
    let hash = dto.hash.clone().ok_or_else(|| TorrentError::Parse("torrent hash missing".into()))?;
    let trackers = dto.trackers.clone().unwrap_or_default();
    let files = dto
        .file_stats
        .iter()
        .flatten()
        .map(|file| TorrentFile {
            path: file.path.clone(),
            index_file: file.id.unwrap_or(0),
            size: file.length,
            torrent_hash: hash.clone(),
            trackers: trackers.clone(),
        })
        .collect();
    Ok(TorrentInfo {
        title: pick_title(title, &dto.title),
        files,
        hash,
        size: dto.torrent_size.unwrap_or(-1),
        trackers,
    })
}

/// The server record of a parsed `.torrent` file. Single-file torrents have no file id (Kotlin `id = null`).
fn dto_from_meta(meta: &TorrentMeta) -> TorrentDto {
    TorrentDto {
        title: meta.title.clone(),
        hash: Some(meta.hash.clone()),
        torrent_size: i64::try_from(meta.size).ok(),
        trackers: Some(meta.trackers.clone()),
        file_stats: Some(
            meta.files
                .iter()
                .map(|file| FileStatDto {
                    id: file.id.and_then(|id| i32::try_from(id).ok()),
                    path: file.path.clone(),
                    length: i64::try_from(file.length).unwrap_or(i64::MAX),
                })
                .collect(),
        ),
    }
}

/// Maps a server failure to the user-facing lookup failure. Timeout is a dead torrent, a refused connection means the
/// server is not running, and every other failure keeps its own message (E-18).
fn map_server_error(err: CoreError) -> TorrentError {
    match err {
        CoreError::ServerTimeout => TorrentError::Dead,
        CoreError::ServerUnavailable => TorrentError::ServerNotRunning,
        CoreError::ServerRejected(code) => TorrentError::Http(code),
        CoreError::InvalidTorrent(msg) | CoreError::Bencode(msg) => TorrentError::Parse(msg),
        CoreError::Http(msg) | CoreError::Failed(msg) => TorrentError::Transport(msg),
        other => TorrentError::Transport(other.to_string()),
    }
}

async fn add_magnet(server: &TorrServerClient, url: &str, title: &str) -> Result<TorrentDto, TorrentError> {
    server.add(url, title, false).await.map_err(map_server_error)
}

async fn download_torrent(http: &reqwest::Client, url: &str) -> Result<TorrentDto, TorrentError> {
    let transport = |e: reqwest::Error| TorrentError::Transport(e.to_string());
    let resp = http.get(url).send().await.map_err(transport)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(TorrentError::Http(status.as_u16()));
    }
    let bytes = resp.bytes().await.map_err(transport)?;
    let meta = parse_torrent_file(&bytes).map_err(|e| TorrentError::Parse(e.to_string()))?;
    Ok(dto_from_meta(&meta))
}

/// `getTorrentInfo(url, title)`. `torrserver_enabled` is the `pref_torrserver_enable` setting: a magnet link needs the
/// server, so with the setting off it fails without a request.
pub async fn resolve_torrent_info(
    server: &TorrServerClient,
    torrserver_enabled: bool,
    http: &reqwest::Client,
    url: &str,
    title: &str,
) -> Result<TorrentInfo, TorrentError> {
    let dto = if is_magnet(url) {
        if !torrserver_enabled {
            return Err(TorrentError::ServerNotRunning);
        }
        add_magnet(server, url, title).await?
    } else {
        download_torrent(http, url).await?
    };
    info_from_dto(&dto, title)
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::time::Duration;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const TORRENT_FILE: &[u8] = include_bytes!("../../tests/fixtures/torrent/test-torrent-file.torrent");
    const TORRENT_DIR: &[u8] = include_bytes!("../../tests/fixtures/torrent/test-torrent-directory.torrent");

    fn client(port: u16) -> TorrServerClient {
        TorrServerClient::new(port, reqwest::Client::new())
    }

    fn record(json: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_raw(json.as_bytes().to_vec(), "application/json")
    }

    /// A port with nothing listening on it: a connect to it is refused.
    fn closed_port() -> u16 {
        TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
    }

    #[test]
    fn the_magnet_scheme_is_matched_in_any_case() {
        assert!(is_magnet("magnet:?xt=urn:btih:abc"));
        assert!(is_magnet("MAGNET:?xt=urn:btih:abc"));
        assert!(!is_magnet("https://example.test/a.torrent"));
        assert!(!is_magnet(""));
        assert!(!is_magnet("magne"));
        assert!(!is_magnet("ünicodé-link"));
    }

    #[test]
    fn an_empty_title_takes_the_torrents_own_title() {
        assert_eq!(pick_title("", "from torrent"), "from torrent");
        assert_eq!(pick_title("given", "from torrent"), "given");
    }

    #[test]
    fn a_missing_hash_is_a_parse_error() {
        let dto = TorrentDto { title: "t".into(), hash: None, torrent_size: None, trackers: None, file_stats: None };
        assert_eq!(info_from_dto(&dto, "x"), Err(TorrentError::Parse("torrent hash missing".into())));
    }

    #[test]
    fn a_missing_size_is_minus_one_and_missing_lists_are_empty() {
        let dto = TorrentDto { title: "t".into(), hash: Some("h".into()), torrent_size: None, trackers: None, file_stats: None };
        let info = info_from_dto(&dto, "x").unwrap();
        assert_eq!((info.size, info.trackers.len(), info.files.len()), (-1, 0, 0));
    }

    #[tokio::test]
    async fn a_disabled_backend_never_calls_the_server_for_a_magnet() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/torrents")).respond_with(record("{}")).expect(0).mount(&server).await;
        let http = reqwest::Client::new();
        let port = server.address().port();
        let result = resolve_torrent_info(&client(port), false, &http, "magnet:?xt=urn:btih:abc", "t").await;
        assert_eq!(result, Err(TorrentError::ServerNotRunning));
        server.verify().await;
    }

    #[tokio::test]
    async fn a_magnet_is_added_to_the_server_and_the_json_fields_are_read() {
        let server = MockServer::start().await;
        let body = r#"{"title":"Server title","hash":"abc","torrent_size":10,"trackers":["udp://t.example/a"],
            "file_stats":[{"id":1,"path":"3/ep.mkv","length":10}],"ignored":true}"#;
        Mock::given(method("POST")).and(path("/torrents")).respond_with(record(body)).expect(1).mount(&server).await;
        let info = resolve_torrent_info(&client(server.address().port()), true, &reqwest::Client::new(), "MAGNET:?xt=urn:btih:abc", "")
            .await
            .unwrap();
        assert_eq!(info.title, "Server title", "an empty title falls back to the torrent's title");
        assert_eq!((info.hash.as_str(), info.size), ("abc", 10));
        assert_eq!(info.trackers, vec!["udp://t.example/a".to_string()]);
        assert_eq!((info.files[0].path.as_str(), info.files[0].index_file, info.files[0].size), ("3/ep.mkv", 1, 10));
        server.verify().await;
    }

    #[tokio::test]
    async fn a_magnet_without_a_hash_in_the_server_reply_is_a_parse_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/torrents")).respond_with(record(r#"{"title":"x"}"#)).mount(&server).await;
        let result = resolve_torrent_info(&client(server.address().port()), true, &reqwest::Client::new(), "magnet:?xt=urn:btih:abc", "t").await;
        assert_eq!(result, Err(TorrentError::Parse("torrent hash missing".into())));
    }

    #[tokio::test]
    async fn a_refused_connection_says_the_server_is_not_running() {
        let result = resolve_torrent_info(&client(closed_port()), true, &reqwest::Client::new(), "magnet:?xt=urn:btih:abc", "t").await;
        assert_eq!(result, Err(TorrentError::ServerNotRunning));
    }

    #[tokio::test]
    async fn a_server_timeout_is_a_dead_torrent() {
        let server = MockServer::start().await;
        let slow = record("{}").set_delay(Duration::from_millis(400));
        Mock::given(method("POST")).and(path("/torrents")).respond_with(slow).mount(&server).await;
        let mut torrserver = client(server.address().port());
        torrserver.request_timeout = Duration::from_millis(40);
        let result = resolve_torrent_info(&torrserver, true, &reqwest::Client::new(), "magnet:?xt=urn:btih:abc", "t").await;
        assert_eq!(result, Err(TorrentError::Dead));
    }

    #[tokio::test]
    async fn a_server_error_status_keeps_its_own_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/torrents")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
        let result = resolve_torrent_info(&client(server.address().port()), true, &reqwest::Client::new(), "magnet:?xt=urn:btih:abc", "t").await;
        assert_eq!(result, Err(TorrentError::Http(500)));
        assert_eq!(TorrentError::Http(500).to_string(), "HTTP error 500");
    }

    #[tokio::test]
    async fn the_single_file_fixture_resolves_without_the_server() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/a.torrent")).respond_with(ResponseTemplate::new(200).set_body_bytes(TORRENT_FILE)).mount(&server).await;
        let url = format!("{}/a.torrent", server.uri());
        let info = resolve_torrent_info(&client(closed_port()), false, &reqwest::Client::new(), &url, "").await.unwrap();
        assert_eq!(info.hash, "104701de4b710ee9720ee249bf220384659e8c38");
        assert_eq!((info.title.as_str(), info.size), ("test-torrent-file", 18));
        assert_eq!(info.trackers, vec!["udp://tracker.opentrackr.org:1337/announce".to_string()]);
        assert_eq!(info.files.len(), 1);
        assert_eq!((info.files[0].index_file, info.files[0].size), (0, 18));
    }

    #[tokio::test]
    async fn the_directory_fixture_keeps_every_file_and_the_duplicate_tracker() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/d.torrent")).respond_with(ResponseTemplate::new(200).set_body_bytes(TORRENT_DIR)).mount(&server).await;
        let url = format!("{}/d.torrent", server.uri());
        let info = resolve_torrent_info(&client(closed_port()), false, &reqwest::Client::new(), &url, "given").await.unwrap();
        assert_eq!(info.hash, "863eb76f1cd0643d9fd2eb4a7da866c3752991b5");
        assert_eq!((info.title.as_str(), info.size), ("given", 12));
        assert_eq!(info.trackers.len(), 6);
        assert_eq!(info.trackers.iter().filter(|t| t.as_str() == "udp://tracker.opentrackr.org:1337/announce").count(), 2);
        let files: Vec<(i32, &str, i64)> = info.files.iter().map(|f| (f.index_file, f.path.as_str(), f.size)).collect();
        assert_eq!(files, vec![(1, "3/3.1", 4), (2, "2", 2), (3, "1", 2), (4, "3/3.2", 4)]);
    }

    #[tokio::test]
    async fn a_torrent_url_that_fails_keeps_its_http_status_and_parse_text() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/missing.torrent")).respond_with(ResponseTemplate::new(404)).mount(&server).await;
        Mock::given(method("GET")).and(path("/junk.torrent")).respond_with(ResponseTemplate::new(200).set_body_bytes(b"not bencode".to_vec())).mount(&server).await;
        let http = reqwest::Client::new();
        let missing = format!("{}/missing.torrent", server.uri());
        let junk = format!("{}/junk.torrent", server.uri());
        assert_eq!(resolve_torrent_info(&client(closed_port()), false, &http, &missing, "t").await, Err(TorrentError::Http(404)));
        assert!(matches!(resolve_torrent_info(&client(closed_port()), false, &http, &junk, "t").await, Err(TorrentError::Parse(_))));
    }
}
