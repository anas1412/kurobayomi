//! The HTTP client of the torrent server (T-45a-073; `TorrentServerApi.kt`). Every call goes to the loopback server.
//! The multipart body is written by hand, so no reqwest feature is needed.

use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};

/// The `echo` probe gives up after this long (area 13 section 9).
const ECHO_TIMEOUT: Duration = Duration::from_secs(2);
/// Add and upload calls give up after this long (area 13 section 9).
const ADD_TIMEOUT: Duration = Duration::from_secs(30);
/// The multipart boundary of an upload. It is fixed: the file bytes never contain it in practice, and the body is
/// checked against it in the tests.
const BOUNDARY: &str = "kuro-torrent-upload-7d3a";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TorrentError {
    #[error("TorrServer is not reachable: {0}")]
    Transport(String),
    #[error("TorrServer answered HTTP {0}")]
    Status(u16),
    #[error("TorrServer sent a reply that is not a torrent: {0}")]
    Decode(String),
}

/// One file of a torrent. `id` is the 1-based file index TorrServer reports.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FileStat {
    pub id: Option<i64>,
    pub path: String,
    pub length: i64,
}

/// A torrent as TorrServer reports it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Torrent {
    pub title: String,
    pub hash: Option<String>,
    #[serde(rename = "torrent_size")]
    pub torrent_size: Option<i64>,
    pub trackers: Option<Vec<String>>,
    #[serde(rename = "file_stats")]
    pub file_stats: Option<Vec<FileStat>>,
}

/// The body of `POST /torrents` (`TorrentRequest.kt`).
#[derive(Serialize)]
struct AddRequest<'a> {
    action: &'static str,
    link: &'a str,
    title: &'a str,
    poster: &'static str,
    data: &'static str,
    save_to_db: bool,
}

/// The multipart body of `POST /torrent/upload`: the file (`application/x-bittorrent`), `save` and `title`.
pub fn multipart_body(file: &[u8], title: &str, save: bool) -> Vec<u8> {
    let file_name = title.replace(['"', '\r', '\n'], "_");
    let mut body = Vec::with_capacity(file.len() + 512);
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: application/x-bittorrent\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file);
    body.extend_from_slice(
        format!(
            "\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"save\"\r\n\r\n{save}\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n{title}\r\n--{BOUNDARY}--\r\n"
        )
        .as_bytes(),
    );
    body
}

/// Client of one torrent server, for example `http://127.0.0.1:8090`.
#[derive(Clone)]
pub struct TorrentApi {
    http: reqwest::Client,
    base: String,
}

impl TorrentApi {
    pub fn new(base: impl Into<String>) -> Result<Self, TorrentError> {
        let http = reqwest::Client::builder()
            .build()
            .map_err(|e| TorrentError::Transport(e.to_string()))?;
        Ok(Self {
            http,
            base: base.into(),
        })
    }

    /// The client of the server on loopback port `port`.
    pub fn on_port(port: u16) -> Result<Self, TorrentError> {
        Self::new(format!("http://127.0.0.1:{port}"))
    }

    /// `GET /echo`: the body text, or an empty string on any error (`TorrentServerApi.kt`). An empty answer means the
    /// server is not running.
    pub async fn echo(&self) -> String {
        let sent = self
            .http
            .get(format!("{}/echo", self.base))
            .timeout(ECHO_TIMEOUT)
            .send()
            .await;
        match sent {
            Ok(resp) if resp.status().is_success() => resp.text().await.unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// `POST /torrents` with `{"action":"add", ...}`. `save` keeps the torrent in the server database.
    pub async fn add(&self, link: &str, title: &str, save: bool) -> Result<Torrent, TorrentError> {
        let body = serde_json::to_vec(&AddRequest {
            action: "add",
            link,
            title,
            poster: "",
            data: "",
            save_to_db: save,
        })
        .map_err(|e| TorrentError::Decode(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}/torrents", self.base))
            .header(CONTENT_TYPE, "application/json")
            .timeout(ADD_TIMEOUT)
            .body(body)
            .send()
            .await
            .map_err(|e| TorrentError::Transport(e.to_string()))?;
        read_torrent(resp).await
    }

    /// `POST /torrent/upload` with the `.torrent` bytes as a multipart form.
    pub async fn upload(
        &self,
        file: &[u8],
        title: &str,
        save: bool,
    ) -> Result<Torrent, TorrentError> {
        let resp = self
            .http
            .post(format!("{}/torrent/upload", self.base))
            .header(
                CONTENT_TYPE,
                format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .timeout(ADD_TIMEOUT)
            .body(multipart_body(file, title, save))
            .send()
            .await
            .map_err(|e| TorrentError::Transport(e.to_string()))?;
        read_torrent(resp).await
    }
}

async fn read_torrent(resp: reqwest::Response) -> Result<Torrent, TorrentError> {
    let status = resp.status();
    if !status.is_success() {
        return Err(TorrentError::Status(status.as_u16()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| TorrentError::Transport(e.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|e| TorrentError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_partial_json, body_string_contains, header_regex, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const TORRENT_JSON: &str = r#"{"title":"Show","hash":"abc","torrent_size":9,"trackers":["udp://t:1"],"file_stats":[{"id":1,"path":"S/1.mkv","length":5}]}"#;

    #[test]
    fn the_upload_body_carries_the_file_and_the_form_fields() {
        let body = multipart_body(b"d4:name", "My \"Show\"", false);
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"file\"; filename=\"My _Show_\""));
        assert!(text.contains("Content-Type: application/x-bittorrent"));
        assert!(text.contains("name=\"save\"\r\n\r\nfalse"));
        assert!(text.contains("name=\"title\"\r\n\r\nMy \"Show\""));
        assert!(body.windows(7).any(|w| w == b"d4:name"));
        assert!(text.ends_with(&format!("--{BOUNDARY}--\r\n")));
    }

    #[tokio::test]
    async fn echo_returns_the_body_text() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/echo"))
            .respond_with(ResponseTemplate::new(200).set_body_string("1.2.3"))
            .mount(&server)
            .await;
        let api = TorrentApi::new(server.uri()).unwrap();
        assert_eq!(api.echo().await, "1.2.3");
    }

    #[tokio::test]
    async fn echo_is_empty_on_an_error_status_or_when_nothing_listens() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/echo"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;
        assert_eq!(TorrentApi::new(server.uri()).unwrap().echo().await, "");
        assert_eq!(
            TorrentApi::new("http://127.0.0.1:1").unwrap().echo().await,
            ""
        );
    }

    #[tokio::test]
    async fn add_posts_the_add_action_and_reads_the_torrent() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/torrents"))
            .and(body_partial_json(serde_json::json!({
                "action": "add",
                "link": "magnet:?xt=urn:btih:abc",
                "title": "Show",
                "save_to_db": false
            })))
            .respond_with(ResponseTemplate::new(200).set_body_string(TORRENT_JSON))
            .mount(&server)
            .await;
        let torrent = TorrentApi::new(server.uri())
            .unwrap()
            .add("magnet:?xt=urn:btih:abc", "Show", false)
            .await
            .unwrap();
        assert_eq!(torrent.hash.as_deref(), Some("abc"));
        assert_eq!(torrent.file_stats.unwrap()[0].id, Some(1));
    }

    #[tokio::test]
    async fn upload_sends_a_multipart_form_and_reads_the_torrent() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/torrent/upload"))
            .and(header_regex(
                "content-type",
                "^multipart/form-data; boundary=",
            ))
            .and(body_string_contains("application/x-bittorrent"))
            .and(body_string_contains("name=\"save\"\r\n\r\nfalse"))
            .respond_with(ResponseTemplate::new(200).set_body_string(TORRENT_JSON))
            .mount(&server)
            .await;
        let torrent = TorrentApi::new(server.uri())
            .unwrap()
            .upload(b"d4:name", "Show", false)
            .await
            .unwrap();
        assert_eq!(torrent.title, "Show");
    }

    #[tokio::test]
    async fn a_failed_add_is_a_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/torrents"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let err = TorrentApi::new(server.uri())
            .unwrap()
            .add("magnet:?xt=urn:btih:abc", "Show", false)
            .await
            .unwrap_err();
        assert_eq!(err, TorrentError::Status(500));
    }
}
