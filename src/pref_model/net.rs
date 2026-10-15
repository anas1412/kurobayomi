//! The one HTTP client of the extension repo pages. The domain flow takes a blocking GET behind a port
//! ([`HttpGet`] for an index), so the page runs it on a worker thread.

use kuro_i18n::{MsgKey, tr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use kuro_domain::extension_repo::HttpGet;

/// A store document is limited to 16 MiB, as in `kuro-extensions`.
const BODY_LIMIT: u64 = 16 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

/// The client starts on the first request, which runs on a worker thread: building it loads the system certificates
/// and starts its own runtime, which the GTK thread must not wait for when a page opens.
pub struct BlockingHttp {
    client: OnceLock<Result<reqwest::blocking::Client, String>>,
}

impl BlockingHttp {
    pub fn new() -> Self {
        Self { client: OnceLock::new() }
    }

    /// The one client of the app's store and repo pages.
    pub fn shared() -> Arc<BlockingHttp> {
        static SHARED: OnceLock<Arc<BlockingHttp>> = OnceLock::new();
        SHARED.get_or_init(|| Arc::new(BlockingHttp::new())).clone()
    }

    fn client(&self) -> Result<&reqwest::blocking::Client, String> {
        self.client
            .get_or_init(|| {
                reqwest::blocking::Client::builder()
                    .timeout(TIMEOUT)
                    .user_agent(concat!("Kurobayomi/", env!("CARGO_PKG_VERSION")))
                    .build()
                    .map_err(|e| e.to_string())
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let response = self.client()?.get(url).send().map_err(|e| e.to_string())?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("HTTP {}", status.as_u16()));
        }
        if response.content_length().is_some_and(|n| n > BODY_LIMIT) {
            return Err(too_large());
        }
        let body = response.bytes().map_err(|e| e.to_string())?;
        if body.len() as u64 > BODY_LIMIT { Err(too_large()) } else { Ok(body.to_vec()) }
    }
}

fn too_large() -> String {
    tr(MsgKey::ui_the_document_is_larger_than_16_mib).to_owned()
}

impl HttpGet for BlockingHttp {
    fn get_text(&self, url: &str) -> Result<String, String> {
        self.bytes(url).and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    /// Serves one canned response on a loopback port and returns its URL.
    fn serve_once(status: &str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}/index.json", listener.local_addr().expect("addr"));
        let status = status.to_owned();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let reply = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = stream.write_all(reply.as_bytes());
        });
        url
    }

    #[test]
    fn a_success_returns_the_body_bytes_and_text() {
        let http = BlockingHttp::new();
        let url = serve_once("200 OK", "{\"a\":1}");
        assert_eq!(http.bytes(&url).expect("body"), b"{\"a\":1}".to_vec());
        let url = serve_once("200 OK", "hello");
        assert_eq!(http.get_text(&url).expect("text"), "hello");
    }

    #[test]
    fn a_failing_status_is_the_error_text() {
        let http = BlockingHttp::new();
        let url = serve_once("404 Not Found", "nope");
        assert_eq!(http.bytes(&url), Err("HTTP 404".to_owned()));
    }
}
