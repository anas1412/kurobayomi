//! External downloader handoff (T-24a-024, T-24a-025). The Android hand-off to 1DM and ADM is dropped: Android intents have
//! no Linux equivalent. The desktop option sends the video URL and its headers to aria2 over JSON-RPC, and it is off by
//! default. On handoff the download location is copied to the clipboard. A failed copy is logged and does not fail the
//! handoff.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::warn;

use crate::anime::ffmpeg::sanitize_headers;
use crate::error::DownloadError;
use crate::layout::TMP_DIR_SUFFIX;

/// The clipboard label of the download location (T-24a-025).
pub const LOCATION_LABEL: &str = "Episode download location";

/// The text copied on handoff: the staging folder path without its `_tmp` suffix (T-24a-025).
pub fn handoff_location(staging: &Path) -> PathBuf {
    let name = staging.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    staging.with_file_name(name.strip_suffix(TMP_DIR_SUFFIX).unwrap_or(&name))
}

/// The desktop clipboard. The GUI implements it over the GTK clipboard, which has no labels, so `label` is only for the
/// logs and the GUI may ignore it.
pub trait Clipboard: Send + Sync {
    fn set_text(&self, label: &str, text: &str) -> Result<(), String>;
}

/// Copies the download location to the clipboard. Returns `true` when it was copied. A failure is logged and is not
/// fatal (T-24a-025).
pub fn copy_location(clipboard: &dyn Clipboard, staging: &Path) -> bool {
    let text = handoff_location(staging).display().to_string();
    match clipboard.set_text(LOCATION_LABEL, &text) {
        Ok(()) => true,
        Err(err) => {
            warn!(target: "kuro_downloads", error = %err, "could not copy the download location to the clipboard");
            false
        }
    }
}

/// The aria2 `aria2.addUri` request for one video: the URL, its headers as `Name: value` options (sanitised as for
/// ffmpeg, T-24a-070) and the folder to save into.
pub fn aria2_add_uri(url: &str, headers: &[(String, String)], dir: &Path) -> Value {
    let header: Vec<String> = sanitize_headers(headers).into_iter().map(|(name, value)| format!("{name}: {value}")).collect();
    json!({
        "jsonrpc": "2.0",
        "id": "kurobayomi",
        "method": "aria2.addUri",
        "params": [[url], { "dir": dir.display().to_string(), "header": header }],
    })
}

/// Posts one JSON-RPC request to an `http://host:port/path` endpoint and returns the GID aria2 answers with. This is a
/// plain HTTP/1.1 POST, which is enough for aria2 on the local machine. A refusal or a JSON-RPC error is an error.
pub async fn post_rpc(endpoint: &str, body: &Value) -> Result<String, DownloadError> {
    let rest = endpoint
        .strip_prefix("http://")
        .ok_or_else(|| DownloadError::Internal("the aria2 endpoint must be an http:// address".into()))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let payload = body.to_string();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    let mut stream = TcpStream::connect(authority).await?;
    stream.write_all(request.as_bytes()).await?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    let text = String::from_utf8_lossy(&response);
    let (head, json_text) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| DownloadError::Internal("aria2 sent no HTTP body".into()))?;
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
        return Err(DownloadError::Internal(format!("aria2 refused the request: {}", head.lines().next().unwrap_or_default())));
    }
    let reply: Value = serde_json::from_str(json_text).map_err(|e| DownloadError::Internal(format!("aria2 sent bad JSON: {e}")))?;
    match reply.get("result").and_then(Value::as_str) {
        Some(gid) => Ok(gid.to_owned()),
        None => {
            let message = reply.pointer("/error/message").and_then(Value::as_str).unwrap_or("no GID in the reply");
            Err(DownloadError::Internal(format!("aria2 error: {message}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tokio::net::TcpListener;

    struct Fake(Mutex<Option<(String, String)>>, bool);

    impl Clipboard for Fake {
        fn set_text(&self, label: &str, text: &str) -> Result<(), String> {
            *self.0.lock().unwrap() = Some((label.to_owned(), text.to_owned()));
            if self.1 { Err("no clipboard".into()) } else { Ok(()) }
        }
    }

    #[test]
    fn the_location_drops_the_tmp_suffix() {
        assert_eq!(handoff_location(Path::new("/d/Show/Ep 1_tmp")), PathBuf::from("/d/Show/Ep 1"));
        assert_eq!(handoff_location(Path::new("/d/Show/Ep 1")), PathBuf::from("/d/Show/Ep 1"));
    }

    #[test]
    fn the_location_is_copied_with_its_label_and_a_failure_is_not_fatal() {
        let ok = Fake(Mutex::new(None), false);
        assert!(copy_location(&ok, Path::new("/d/Show/Ep 1_tmp")));
        assert_eq!(
            ok.0.lock().unwrap().clone(),
            Some(("Episode download location".to_owned(), "/d/Show/Ep 1".to_owned()))
        );
        let failing = Fake(Mutex::new(None), true);
        assert!(!copy_location(&failing, Path::new("/d/Show/Ep 1_tmp")), "logged, and the handoff goes on");
    }

    #[test]
    fn the_add_uri_request_carries_the_url_headers_and_folder() {
        let headers = vec![("Referer".to_owned(), "https://ref.example\r\nX-Evil: 1".to_owned())];
        let body = aria2_add_uri("https://cdn.example/v.m3u8", &headers, Path::new("/d/Show/Ep 1_tmp"));
        assert_eq!(body["method"], "aria2.addUri");
        assert_eq!(body["params"][0][0], "https://cdn.example/v.m3u8");
        assert_eq!(body["params"][1]["dir"], "/d/Show/Ep 1_tmp");
        assert_eq!(body["params"][1]["header"][0], "Referer: https://ref.exampleX-Evil: 1");
    }

    #[tokio::test]
    async fn the_gid_comes_back_from_a_json_rpc_reply() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/jsonrpc", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = socket.read(&mut buf).await.unwrap();
            let reply = r#"{"jsonrpc":"2.0","id":"kurobayomi","result":"2089b05ecca3d829"}"#;
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}", reply.len());
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let body = aria2_add_uri("https://cdn.example/v.mp4", &[], Path::new("/d/x"));
        assert_eq!(post_rpc(&endpoint, &body).await.unwrap(), "2089b05ecca3d829");
    }

    #[tokio::test]
    async fn an_https_endpoint_is_refused_and_a_closed_port_is_an_error() {
        let body = json!({});
        assert!(post_rpc("https://127.0.0.1:6800/jsonrpc", &body).await.is_err());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert!(post_rpc(&format!("http://127.0.0.1:{port}/jsonrpc"), &body).await.is_err(), "nothing listens there");
    }
}
