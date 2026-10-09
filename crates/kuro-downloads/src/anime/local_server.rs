//! Local HTTP server for `localhost:1` video URLs (T-24a-022; behaviour 5.3, data 1.4, design 5).
//!
//! Some sources hand out `http://localhost:1...` URLs that only work through a server the app runs. The server itself
//! belongs to the source runtime; this module owns the seam ([`LocalHttpServer`]), the URL rules, and the lifetime of
//! one server session. The session stops the server when it is dropped, so success, failure and cancel all stop it.

use std::time::Duration;

use futures::future::BoxFuture;

use crate::anime::model::Video;
use crate::error::DownloadError;

/// How long to wait for the server to start (behaviour 13, `MainActivity.kt:647`).
pub const START_TIMEOUT: Duration = Duration::from_secs(5);

/// A server the source runtime provides for one source (`createHttpServer`).
pub trait LocalHttpServer: Send + Sync {
    /// Starts the server and returns the port it listens on. Port 0 is treated as a failure by the caller (E-X07).
    fn start(&self) -> BoxFuture<'_, Result<u16, DownloadError>>;
    /// Stops the server on `port`. Called once per session, from `Drop`.
    fn stop(&self, port: u16);
}

const PLACEHOLDER_PREFIX: &str = "http://localhost:1";

/// `true` for the placeholder `http://localhost:1` followed by a non-digit (or the end of the string). Matches the
/// Kotlin regex `http:\/\/localhost:1(?!\d)`, which also covers `localhost:1/...` and not `localhost:10...`.
pub fn is_placeholder_url(url: &str) -> bool {
    let mut rest = url;
    while let Some(at) = rest.find(PLACEHOLDER_PREFIX) {
        let after = &rest[at + PLACEHOLDER_PREFIX.len()..];
        if !after.starts_with(|c: char| c.is_ascii_digit()) {
            return true;
        }
        rest = after;
    }
    false
}

/// `true` when the video or any of its subtitle or audio tracks needs the local server (`usesHttpServer`).
pub fn uses_http_server(video: &Video) -> bool {
    is_placeholder_url(&video.url)
        || video.audio_tracks.iter().any(|t| is_placeholder_url(&t.url))
        || video.subtitle_tracks.iter().any(|t| is_placeholder_url(&t.url))
}

/// Replaces every placeholder prefix in `url` with `http://localhost:<port>` (`copyHttpServer`).
pub fn substitute_port(url: &str, port: u16) -> String {
    let host = format!("http://localhost:{port}");
    let mut out = String::with_capacity(url.len());
    let mut rest = url;
    while let Some(at) = rest.find(PLACEHOLDER_PREFIX) {
        let after = &rest[at + PLACEHOLDER_PREFIX.len()..];
        out.push_str(&rest[..at]);
        if after.starts_with(|c: char| c.is_ascii_digit()) {
            out.push_str(PLACEHOLDER_PREFIX);
        } else {
            out.push_str(&host);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// A copy of `video` with every localhost placeholder pointed at `port`.
pub fn copy_http_server(video: &Video, port: u16) -> Video {
    let mut copy = video.clone();
    copy.url = substitute_port(&video.url, port);
    for track in &mut copy.audio_tracks {
        track.url = substitute_port(&track.url, port);
    }
    for track in &mut copy.subtitle_tracks {
        track.url = substitute_port(&track.url, port);
    }
    copy
}

/// Starts the server with the 5 s timeout. Port 0 and a timeout are both `HttpServer` errors.
pub async fn start_with_timeout(server: &dyn LocalHttpServer) -> Result<u16, DownloadError> {
    let port = match tokio::time::timeout(START_TIMEOUT, server.start()).await {
        Ok(result) => result?,
        Err(_) => return Err(DownloadError::HttpServer("start timed out".into())),
    };
    if port == 0 {
        return Err(DownloadError::HttpServer("port 0".into()));
    }
    Ok(port)
}

/// A started server. Dropping it stops the server, so every exit path of a download stops it.
pub struct ServerSession<'a> {
    server: &'a dyn LocalHttpServer,
    port: u16,
}

impl<'a> ServerSession<'a> {
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for ServerSession<'_> {
    fn drop(&mut self) {
        self.server.stop(self.port);
    }
}

/// Starts the server when `video` needs it, and returns the session together with the video rewritten to the port.
/// `Ok(None)` when no server is needed.
pub async fn prepare<'a>(
    server: &'a dyn LocalHttpServer,
    video: &Video,
) -> Result<Option<(ServerSession<'a>, Video)>, DownloadError> {
    if !uses_http_server(video) {
        return Ok(None);
    }
    let port = start_with_timeout(server).await?;
    let rewritten = copy_http_server(video, port);
    Ok(Some((ServerSession { server, port }, rewritten)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::model::Track;
    use std::sync::Mutex;

    fn video(url: &str) -> Video {
        Video { url: url.into(), ..Default::default() }
    }

    #[test]
    fn placeholder_matches_port_one_only_when_no_digit_follows() {
        assert!(is_placeholder_url("http://localhost:1/video.m3u8"));
        assert!(is_placeholder_url("http://localhost:1"));
        assert!(!is_placeholder_url("http://localhost:10/video.m3u8"));
        assert!(!is_placeholder_url("http://localhost:8080/x"));
        assert!(!is_placeholder_url("https://localhost:1/x"));
        assert!(is_placeholder_url("http://localhost:10 http://localhost:1/x"), "a later match still counts");
    }

    #[test]
    fn any_track_can_require_the_server() {
        let mut v = video("https://cdn.example/v.m3u8");
        assert!(!uses_http_server(&v));
        v.subtitle_tracks.push(Track { url: "http://localhost:1/en.vtt".into(), lang: "en".into() });
        assert!(uses_http_server(&v));
    }

    #[test]
    fn substitution_rewrites_only_the_placeholder() {
        assert_eq!(substitute_port("http://localhost:1/a.m3u8", 40123), "http://localhost:40123/a.m3u8");
        assert_eq!(substitute_port("http://localhost:10/a", 40123), "http://localhost:10/a");
        let mut v = video("http://localhost:1/v");
        v.audio_tracks.push(Track { url: "http://localhost:1/ja".into(), lang: "ja".into() });
        let copy = copy_http_server(&v, 5000);
        assert_eq!(copy.url, "http://localhost:5000/v");
        assert_eq!(copy.audio_tracks[0].url, "http://localhost:5000/ja");
    }

    struct Fake {
        port: u16,
        stopped: Mutex<Vec<u16>>,
    }

    impl LocalHttpServer for Fake {
        fn start(&self) -> BoxFuture<'_, Result<u16, DownloadError>> {
            let port = self.port;
            Box::pin(async move { Ok(port) })
        }
        fn stop(&self, port: u16) {
            self.stopped.lock().unwrap().push(port);
        }
    }

    struct Hanging;

    impl LocalHttpServer for Hanging {
        fn start(&self) -> BoxFuture<'_, Result<u16, DownloadError>> {
            Box::pin(async {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(1)
            })
        }
        fn stop(&self, _port: u16) {}
    }

    #[tokio::test]
    async fn port_zero_is_a_failure() {
        let fake = Fake { port: 0, stopped: Mutex::new(Vec::new()) };
        let err = start_with_timeout(&fake).await.unwrap_err();
        assert_eq!(err.to_string(), "Failed to start server");
    }

    #[tokio::test(start_paused = true)]
    async fn a_server_that_never_starts_times_out_after_five_seconds() {
        let err = start_with_timeout(&Hanging).await.unwrap_err();
        assert!(matches!(err, DownloadError::HttpServer(ref m) if m == "start timed out"));
    }

    #[tokio::test]
    async fn the_session_stops_the_server_on_drop_even_without_success() {
        let fake = Fake { port: 4242, stopped: Mutex::new(Vec::new()) };
        let (session, rewritten) = prepare(&fake, &video("http://localhost:1/v")).await.unwrap().unwrap();
        assert_eq!(session.port(), 4242);
        assert_eq!(rewritten.url, "http://localhost:4242/v");
        assert!(fake.stopped.lock().unwrap().is_empty());
        drop(session);
        assert_eq!(*fake.stopped.lock().unwrap(), vec![4242]);
    }

    #[tokio::test]
    async fn no_server_is_started_for_plain_urls() {
        let fake = Fake { port: 9, stopped: Mutex::new(Vec::new()) };
        assert!(prepare(&fake, &video("https://cdn.example/v.m3u8")).await.unwrap().is_none());
        assert!(fake.stopped.lock().unwrap().is_empty());
    }
}
