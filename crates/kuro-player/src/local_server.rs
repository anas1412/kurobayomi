//! The local stream server that a player reads from (area 45 `HttpServerService.kt`, T-45a-069 and
//! T-45a-070). It wraps `kuro_source_http::local_server::LocalStreamServer`, which binds
//! `127.0.0.1` on an ephemeral port and stops when dropped.

use std::sync::Arc;

use kuro_source_http::local_server::{LocalHandler, LocalStreamServer};

use crate::errors::PlayerMessage;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LocalServerError {
    /// The lookup found no source that can answer (no `AnimeHttpSource`).
    #[error("no source serves this video")]
    NoSource,
    #[error("the local server did not start: {0}")]
    Start(String),
}

/// The app's one local server. Starting a new one stops the previous one first. Dropping it stops
/// the server.
#[derive(Default)]
pub struct LocalServer {
    inner: Option<LocalStreamServer>,
}

impl LocalServer {
    /// Stops the running server, then looks up the handler for the new one. With a handler, binds a
    /// new server and returns its `http://localhost:<port>` URL. A failed lookup starts nothing.
    pub fn start(
        &mut self,
        lookup: impl FnOnce() -> Option<Arc<dyn LocalHandler>>,
    ) -> Result<String, LocalServerError> {
        self.stop();
        let handler = lookup().ok_or(LocalServerError::NoSource)?;
        let server = LocalStreamServer::start(handler)
            .map_err(|e| LocalServerError::Start(e.to_string()))?;
        let url = server.url();
        self.inner = Some(server);
        Ok(url)
    }

    /// The bound port, while a server runs.
    pub fn port(&self) -> Option<u16> {
        self.inner.as_ref().map(LocalStreamServer::port)
    }

    pub fn stop(&mut self) {
        self.inner = None;
    }
}

/// Whether the URL asks for the local server: `http://localhost:1` not followed by a digit
/// (`usesHttpServer`, T-45a-070).
pub fn uses_local_server(url: &str) -> bool {
    kuro_source::anime::is_placeholder(url)
}

/// Replaces every placeholder in `url` with `http://localhost:<port>` and keeps the rest of the URL.
/// The same rule as kuro-source's `rewrite_to_local`, which is private there.
pub fn rewrite_placeholders(url: &str, port: u16) -> String {
    const PREFIX: &str = "http://localhost:1";
    let mut out = String::with_capacity(url.len());
    let mut last = 0;
    for (at, _) in url.match_indices(PREFIX) {
        let after = at + PREFIX.len();
        if url[after..].starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        out.push_str(&url[last..at]);
        out.push_str(&format!("http://localhost:{port}"));
        last = after;
    }
    out.push_str(&url[last..]);
    out
}

/// The URL to load for a video (T-45a-070). A placeholder URL starts a server through `lookup` and
/// is rewritten to its port. Any other URL is returned unchanged. A failed start gives the
/// `Failed to start http server` toast, and the load must stop.
pub fn prepare_url(
    url: &str,
    server: &mut LocalServer,
    lookup: impl FnOnce() -> Option<Arc<dyn LocalHandler>>,
) -> Result<String, PlayerMessage> {
    if !uses_local_server(url) {
        return Ok(url.to_owned());
    }
    match server.start(lookup) {
        Ok(_) => {
            let port = server.port().ok_or(PlayerMessage::HttpServerStartFailure)?;
            Ok(rewrite_placeholders(url, port))
        }
        Err(e) => {
            tracing::error!("local server failed: {e}");
            Err(PlayerMessage::HttpServerStartFailure)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_source::SourceError;
    use kuro_source_http::local_server::LocalResponse;

    struct Fixed;

    impl LocalHandler for Fixed {
        fn serve(&self, _path: &str, _range: Option<&str>) -> Result<LocalResponse, SourceError> {
            Ok(LocalResponse {
                status: 200,
                headers: Vec::new(),
                body: bytes::Bytes::from_static(b"ok"),
            })
        }
    }

    fn handler() -> Option<Arc<dyn LocalHandler>> {
        Some(Arc::new(Fixed))
    }

    #[test]
    fn placeholder_detection_matches_the_source_rule() {
        assert!(uses_local_server("http://localhost:1/s.m3u8"));
        assert!(uses_local_server("http://localhost:1"));
        assert!(!uses_local_server("http://localhost:12/x"));
        assert!(!uses_local_server("https://localhost:1/x"));
    }

    #[test]
    fn rewrite_replaces_each_placeholder_and_keeps_the_rest() {
        assert_eq!(
            rewrite_placeholders("http://localhost:1/a?b=http://localhost:1/c", 5555),
            "http://localhost:5555/a?b=http://localhost:5555/c"
        );
        assert_eq!(
            rewrite_placeholders("http://localhost:12/x", 5555),
            "http://localhost:12/x"
        );
    }

    #[test]
    fn server_binds_an_ephemeral_loopback_port_and_stops_on_drop() {
        let mut server = LocalServer::default();
        let url = server.start(handler).unwrap();
        let port = server.port().unwrap();
        assert_ne!(port, 0);
        assert_eq!(url, format!("http://localhost:{port}"));
        server.stop();
        assert_eq!(server.port(), None);
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    }

    #[test]
    fn starting_again_replaces_the_previous_server() {
        let mut server = LocalServer::default();
        server.start(handler).unwrap();
        let first = server.port().unwrap();
        server.start(handler).unwrap();
        // ponytail: a port reused by the OS right away would fail this check. Rare enough for a unit test.
        assert!(std::net::TcpStream::connect(("127.0.0.1", first)).is_err());
    }

    #[test]
    fn missing_source_starts_nothing() {
        let mut server = LocalServer::default();
        assert_eq!(server.start(|| None), Err(LocalServerError::NoSource));
        assert_eq!(server.port(), None);
    }

    #[test]
    fn prepare_url_rewrites_for_placeholders_only() {
        let mut server = LocalServer::default();
        let plain = prepare_url("https://cdn/x.m3u8", &mut server, || None).unwrap();
        assert_eq!(plain, "https://cdn/x.m3u8");
        assert_eq!(server.port(), None);

        let local = prepare_url("http://localhost:1/s.m3u8", &mut server, handler).unwrap();
        let port = server.port().unwrap();
        assert_eq!(local, format!("http://localhost:{port}/s.m3u8"));
    }

    #[test]
    fn failed_start_gives_the_toast_and_aborts() {
        let mut server = LocalServer::default();
        let err = prepare_url("http://localhost:1/s.m3u8", &mut server, || None).unwrap_err();
        assert_eq!(err, PlayerMessage::HttpServerStartFailure);
        assert_eq!(err.text(), "Failed to start http server");
    }
}
