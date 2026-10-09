//! TorrServer HTTP client (`TorrentServerApi.kt`): echo, add, upload and the play link.

use super::TorrentError;
use super::meta::{TorrentFile, TorrentInfo, TorrentMeta, parse_torrent_file};
use super::model::{FileStatDto, TorrentDto, TorrentRequest};
use super::route::play_link;
use std::time::Duration;

#[derive(Clone)]
pub struct TorrServerClient {
    port: u16,
    http: reqwest::Client,
    trackers: Vec<String>,
    pub echo_timeout: Duration,
    pub request_timeout: Duration,
}

fn map_err(e: reqwest::Error) -> TorrentError {
    if e.is_timeout() {
        TorrentError::ServerTimeout
    } else if e.is_connect() {
        TorrentError::ServerUnavailable
    } else {
        TorrentError::Http(e.to_string())
    }
}

/// Percent-encodes a tracker for the `tr=` query parameter of a magnet link.
fn query_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl TorrServerClient {
    /// Talks to `http://127.0.0.1:{port}`.
    pub fn new(port: u16, http: reqwest::Client) -> Self {
        Self { port, http, trackers: Vec::new(), echo_timeout: Duration::from_secs(2), request_timeout: Duration::from_secs(30) }
    }

    /// Trackers appended to every magnet link at `add` time (OQ-13a-1, option b: the server has no documented
    /// endpoint for a tracker list). Each line is trimmed and empty lines are dropped.
    pub fn with_trackers(mut self, trackers: &str) -> Self {
        self.trackers = trackers.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
        self
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    /// True when the server answers `/echo` with a success status. Never an error.
    pub async fn echo(&self) -> bool {
        match self.http.get(self.url("/echo")).timeout(self.echo_timeout).send().await {
            Ok(r) => r.status().is_success(),
            Err(e) => {
                tracing::debug!(error = %e, "torrent server echo failed");
                false
            }
        }
    }

    async fn decode(resp: reqwest::Response) -> Result<TorrentDto, TorrentError> {
        let status = resp.status();
        if !status.is_success() {
            return Err(TorrentError::ServerRejected(status.as_u16()));
        }
        resp.json::<TorrentDto>().await.map_err(map_err)
    }

    pub async fn add(&self, link: &str, title: &str, save: bool) -> Result<TorrentDto, TorrentError> {
        let mut link = link.to_string();
        if link.starts_with("magnet") {
            for t in &self.trackers {
                link.push_str("&tr=");
                link.push_str(&query_encode(t));
            }
        }
        let resp = self
            .http
            .post(self.url("/torrents"))
            .timeout(self.request_timeout)
            .json(&TorrentRequest::add(&link, title, save))
            .send()
            .await
            .map_err(map_err)?;
        Self::decode(resp).await
    }

    /// Multipart upload with the parts `file` (named after the title), `save` and `title`.
    pub async fn upload(&self, bytes: Vec<u8>, title: &str, save: bool) -> Result<TorrentDto, TorrentError> {
        let boundary = "kurobayomi-torrent-boundary";
        let quoted = title.replace('\\', "\\\\").replace('"', "\\\"");
        let mut body = Vec::new();
        let mut part = |header: String, data: &[u8]| {
            body.extend_from_slice(format!("--{boundary}\r\n{header}\r\n\r\n").as_bytes());
            body.extend_from_slice(data);
            body.extend_from_slice(b"\r\n");
        };
        part(format!("Content-Disposition: form-data; name=\"file\"; filename=\"{quoted}\"\r\nContent-Type: application/x-bittorrent"), &bytes);
        part("Content-Disposition: form-data; name=\"save\"".into(), save.to_string().as_bytes());
        part("Content-Disposition: form-data; name=\"title\"".into(), title.as_bytes());
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        let resp = self
            .http
            .post(self.url("/torrent/upload"))
            .timeout(self.request_timeout)
            .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
            .body(body)
            .send()
            .await
            .map_err(map_err)?;
        Self::decode(resp).await
    }

    pub fn stream_url(&self, torrent: &TorrentDto, index: u32) -> String {
        play_link(self.port, torrent, index)
    }
}

fn dto_from_meta(m: &TorrentMeta) -> TorrentDto {
    TorrentDto {
        title: m.title.clone(),
        hash: Some(m.hash.clone()),
        torrent_size: i64::try_from(m.size).ok(),
        trackers: Some(m.trackers.clone()),
        file_stats: Some(
            m.files
                .iter()
                .map(|f| FileStatDto { id: f.id.and_then(|i| i32::try_from(i).ok()), path: f.path.clone(), length: i64::try_from(f.length).unwrap_or(i64::MAX) })
                .collect(),
        ),
    }
}

/// `TorrentUtils.torrentToTorrentInfo`: a missing hash is an error (`hash!!`), a missing size is -1.
pub fn info_from_dto(torrent: &TorrentDto, override_title: Option<&str>) -> Result<TorrentInfo, TorrentError> {
    let hash = torrent.hash.clone().ok_or_else(|| TorrentError::InvalidTorrent("missing hash".into()))?;
    let trackers = torrent.trackers.clone().unwrap_or_default();
    let files = torrent
        .file_stats
        .iter()
        .flatten()
        .map(|f| TorrentFile { path: f.path.clone(), index: f.id.unwrap_or(0), size: f.length, hash: hash.clone(), trackers: trackers.clone() })
        .collect();
    Ok(TorrentInfo {
        title: override_title.map_or_else(|| torrent.title.clone(), String::from),
        files,
        hash,
        size: torrent.torrent_size.unwrap_or(-1),
        trackers,
    })
}

/// `TorrentUtils.getTorrentInfo`. A magnet link is added to the server (timeout is `ServerTimeout`, any other
/// failure `ServerUnavailable`); a `.torrent` URL is downloaded and parsed without the server.
pub async fn get_torrent_info(
    server: &TorrServerClient,
    http: &reqwest::Client,
    url: &str,
    title: &str,
) -> Result<TorrentInfo, TorrentError> {
    let dto = if url.starts_with("magnet") {
        server.add(url, title, false).await.map_err(|e| match e {
            TorrentError::ServerTimeout => TorrentError::ServerTimeout,
            _ => TorrentError::ServerUnavailable,
        })?
    } else {
        let bytes = http
            .get(url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| TorrentError::Http(e.to_string()))?
            .bytes()
            .await
            .map_err(|e| TorrentError::Http(e.to_string()))?;
        dto_from_meta(&parse_torrent_file(&bytes)?)
    };
    info_from_dto(&dto, Some(title))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    const FILE: &[u8] = include_bytes!("../../tests/fixtures/torrent/test-torrent-file.torrent");

    fn client_for(server: &MockServer) -> TorrServerClient {
        TorrServerClient::new(server.address().port(), reqwest::Client::new())
    }

    const DTO: &str = r#"{"title":"T","hash":"abc","torrent_size":5,"unknown":1,"file_stats":[{"id":2,"path":"3/3.1","length":4}]}"#;

    #[tokio::test]
    async fn echo_is_true_on_200_and_false_on_500() {
        let s = MockServer::start().await;
        Mock::given(path("/echo")).respond_with(ResponseTemplate::new(200).set_body_string("ok")).up_to_n_times(1).mount(&s).await;
        let c = client_for(&s);
        assert!(c.echo().await);
        Mock::given(path("/echo")).respond_with(ResponseTemplate::new(500)).mount(&s).await;
        assert!(!c.echo().await);
    }

    #[tokio::test]
    async fn echo_is_false_when_the_connection_is_refused_and_on_timeout() {
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        assert!(!TorrServerClient::new(port, reqwest::Client::new()).echo().await);
        let s = MockServer::start().await;
        Mock::given(path("/echo")).respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(400))).mount(&s).await;
        let mut c = client_for(&s);
        c.echo_timeout = Duration::from_millis(50);
        assert!(!c.echo().await);
    }

    #[tokio::test]
    async fn add_posts_the_json_body_and_decodes_the_response() {
        let s = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/torrents"))
            .and(body_json(serde_json::json!({"action":"add","link":"magnet:?xt=1","title":"Ep"})))
            .respond_with(ResponseTemplate::new(200).set_body_raw(DTO, "application/json"))
            .expect(1)
            .mount(&s)
            .await;
        let t = client_for(&s).add("magnet:?xt=1", "Ep", false).await.unwrap();
        assert_eq!(t.hash.as_deref(), Some("abc"));
        assert_eq!(t.torrent_size, Some(5));
    }

    #[tokio::test]
    async fn add_with_save_sends_the_flag() {
        let s = MockServer::start().await;
        Mock::given(body_json(serde_json::json!({"action":"add","link":"l","title":"t","save_to_db":true})))
            .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"title":"T"}"#, "application/json"))
            .expect(1)
            .mount(&s)
            .await;
        let t = client_for(&s).add("l", "t", true).await.unwrap();
        assert_eq!(t.file_stats, None);
    }

    #[tokio::test]
    async fn add_appends_trackers_to_magnet_links_only() {
        let s = MockServer::start().await;
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        Mock::given(path("/torrents"))
            .respond_with(move |r: &Request| {
                let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
                sink.lock().unwrap().push(v["link"].as_str().unwrap().to_string());
                ResponseTemplate::new(200).set_body_raw(r#"{"title":"T"}"#, "application/json")
            })
            .mount(&s)
            .await;
        let c = client_for(&s).with_trackers("udp://a:1/x\n\n  udp://b:2  \n");
        c.add("magnet:?xt=1", "t", false).await.unwrap();
        c.add("https://x/y.torrent", "t", false).await.unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0], "magnet:?xt=1&tr=udp%3A%2F%2Fa%3A1%2Fx&tr=udp%3A%2F%2Fb%3A2");
        assert_eq!(seen[1], "https://x/y.torrent");
    }

    #[tokio::test]
    async fn upload_sends_the_file_save_and_title_parts() {
        let s = MockServer::start().await;
        let body = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let sink = body.clone();
        Mock::given(method("POST"))
            .and(path("/torrent/upload"))
            .respond_with(move |r: &Request| {
                *sink.lock().unwrap() = r.body.clone();
                ResponseTemplate::new(200).set_body_raw(DTO, "application/json")
            })
            .mount(&s)
            .await;
        let t = client_for(&s).upload(b"BYTES".to_vec(), "t.torrent", false).await.unwrap();
        assert_eq!(t.title, "T");
        let text = String::from_utf8(body.lock().unwrap().clone()).unwrap();
        assert!(text.contains("name=\"file\"; filename=\"t.torrent\""));
        assert!(text.contains("BYTES"));
        assert!(text.contains("name=\"save\"\r\n\r\nfalse"));
        assert!(text.contains("name=\"title\"\r\n\r\nt.torrent"));
    }

    #[tokio::test]
    async fn add_times_out_and_maps_to_server_timeout() {
        let s = MockServer::start().await;
        Mock::given(path("/torrents")).respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(400))).mount(&s).await;
        let mut c = client_for(&s);
        c.request_timeout = Duration::from_millis(50);
        assert_eq!(c.add("magnet:?x", "t", false).await, Err(TorrentError::ServerTimeout));
    }

    #[tokio::test]
    async fn add_while_the_server_is_down_is_server_unavailable() {
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let c = TorrServerClient::new(port, reqwest::Client::new());
        assert_eq!(c.add("magnet:?x", "t", false).await, Err(TorrentError::ServerUnavailable));
    }

    #[tokio::test]
    async fn a_rejected_request_carries_the_status() {
        let s = MockServer::start().await;
        Mock::given(path("/torrents")).respond_with(ResponseTemplate::new(503)).mount(&s).await;
        assert_eq!(client_for(&s).add("l", "t", false).await, Err(TorrentError::ServerRejected(503)));
    }

    #[tokio::test]
    async fn stream_url_rows() {
        let c = TorrServerClient::new(8090, reqwest::Client::new());
        let t: TorrentDto = serde_json::from_str(DTO).unwrap();
        assert_eq!(c.stream_url(&t, 2), "http://127.0.0.1:8090/stream/3.1?link=abc&index=2&play");
        let single = TorrentDto { title: "a b.mkv".into(), hash: Some("h".into()), torrent_size: None, trackers: None, file_stats: None };
        assert_eq!(c.stream_url(&single, 0), "http://127.0.0.1:8090/stream/a+b.mkv?link=h&index=0&play");
    }

    #[tokio::test]
    async fn get_torrent_info_maps_magnet_failures() {
        let s = MockServer::start().await;
        let c = client_for(&s);
        let http = reqwest::Client::new();
        Mock::given(path("/torrents")).respond_with(ResponseTemplate::new(500)).mount(&s).await;
        assert_eq!(get_torrent_info(&c, &http, "magnet:?x", "t").await, Err(TorrentError::ServerUnavailable));
        let s2 = MockServer::start().await;
        Mock::given(path("/torrents")).respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(400))).mount(&s2).await;
        let mut c2 = client_for(&s2);
        c2.request_timeout = Duration::from_millis(50);
        assert_eq!(get_torrent_info(&c2, &http, "magnet:?x", "t").await, Err(TorrentError::ServerTimeout));
    }

    #[tokio::test]
    async fn get_torrent_info_parses_a_torrent_file_without_the_server() {
        let s = MockServer::start().await;
        Mock::given(path("/a.torrent")).respond_with(ResponseTemplate::new(200).set_body_bytes(FILE)).mount(&s).await;
        let url = format!("{}/a.torrent", s.uri());
        let down = TorrServerClient::new(1, reqwest::Client::new());
        let info = get_torrent_info(&down, &reqwest::Client::new(), &url, "Title").await.unwrap();
        assert_eq!(info.title, "Title");
        assert_eq!(info.hash, "104701de4b710ee9720ee249bf220384659e8c38");
        assert_eq!(info.size, 18);
        assert_eq!(info.files.len(), 1);
        assert_eq!(info.files[0].index, 0);
    }
}
