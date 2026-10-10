//! The browser fallback for [`super::WebViewHost`] (T-req-platform-006). With no embedded view, the app opens the system
//! browser (through `opener::Opener`), receives the OAuth redirect on a loopback port with [`LoopbackRedirect`], and
//! imports the cookies a source needs from a `cookies.txt` export with [`parse_cookies_txt`].

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use url::Url;

use super::{SolveFailure, SolveOutcome, SolveRequest, WebCookie, WebViewHost};
use crate::opener::Opener;

/// The answer to the browser. It names no query value, so nothing from the redirect is echoed back.
const DONE_PAGE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n\
<!doctype html><title>Kurobayomi</title><p>Login received. You can close this tab.</p>";
const NOT_FOUND: &str = "HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";

/// Receives one OAuth redirect on `127.0.0.1` at a port the OS picks.
pub struct LoopbackRedirect {
    listener: TcpListener,
}

impl LoopbackRedirect {
    pub async fn bind() -> io::Result<Self> {
        Ok(Self { listener: TcpListener::bind(("127.0.0.1", 0)).await? })
    }

    /// The redirect URI to register with the provider, for example `http://127.0.0.1:41234/callback`.
    pub fn redirect_uri(&self, path: &str) -> io::Result<String> {
        Ok(format!("http://127.0.0.1:{}{path}", self.listener.local_addr()?.port()))
    }

    /// Waits up to `limit` for a request that carries a query, answers it with a short page, and returns the query pairs.
    /// Requests without a query (a browser's preconnect or favicon fetch) get a 404 and the wait goes on. The caller
    /// checks `state` against the value it sent, since any local process can reach the port.
    pub async fn receive(self, limit: Duration) -> io::Result<Vec<(String, String)>> {
        tokio::time::timeout(limit, self.next_redirect())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "no redirect arrived"))?
    }

    async fn next_redirect(&self) -> io::Result<Vec<(String, String)>> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).await? == 0 {
                continue;
            }
            let target = request_line.split_whitespace().nth(1).unwrap_or("/");
            let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(io::Error::other)?;
            if url.query().is_none() {
                reader.get_mut().write_all(NOT_FOUND.as_bytes()).await?;
                continue;
            }
            reader.get_mut().write_all(DONE_PAGE.as_bytes()).await?;
            return Ok(url.query_pairs().into_owned().collect());
        }
    }
}

/// Parses a Netscape `cookies.txt` export and keeps the cookies that `source_domain` can send, that is those whose
/// domain is the source's host or one of its parents.
///
/// Each data line has seven tab-separated fields: domain, include-subdomains, path, secure, expiry (Unix seconds, `0`
/// for a session cookie), name and value. `#HttpOnly_` before the domain marks an HTTP-only cookie. Other `#` lines and
/// lines without seven fields are skipped. The format has no SameSite field, so the cookies carry `None`.
pub fn parse_cookies_txt(text: &str, source_domain: &str) -> Vec<WebCookie> {
    let source = source_domain.trim_start_matches('.').to_ascii_lowercase();
    text.lines().filter_map(|line| parse_line(line, &source)).collect()
}

fn parse_line(line: &str, source: &str) -> Option<WebCookie> {
    let (http_only, line) = match line.strip_prefix("#HttpOnly_") {
        Some(rest) => (true, rest),
        None if line.starts_with('#') || line.trim().is_empty() => return None,
        None => (false, line),
    };
    let fields: Vec<&str> = line.split('\t').collect();
    let [domain, _subdomains, path, secure, expiry, name, value] = fields[..] else {
        return None;
    };
    let host = domain.trim_start_matches('.').to_ascii_lowercase();
    if source != host && !source.ends_with(&format!(".{host}")) {
        return None;
    }
    let expiry: i64 = expiry.parse().ok()?;
    Some(WebCookie {
        name: name.to_owned(),
        value: value.to_owned(),
        domain: domain.to_owned(),
        path: path.to_owned(),
        secure: secure.eq_ignore_ascii_case("TRUE"),
        http_only,
        expires: (expiry > 0).then_some(expiry),
        same_site: None,
    })
}

/// A [`WebViewHost`] for a machine with no embedded view. A challenge opens in the system browser; the user exports
/// the cookies with a `cookies.txt` extension to `cookies_file`, and the host polls that file until a clearance cookie
/// newer than the old one shows up. [`BrowserHost::oauth_login`] covers tracker login through the loopback redirect.
pub struct BrowserHost {
    opener: Arc<dyn Opener>,
    cookies_file: PathBuf,
    poll: Duration,
}

impl BrowserHost {
    /// `cookies_file` is `Paths::cookies_import_file()`.
    pub fn new(opener: Arc<dyn Opener>, cookies_file: PathBuf) -> Self {
        Self { opener, cookies_file, poll: Duration::from_secs(1) }
    }

    /// Opens `build_url(redirect_uri)` in the system browser and returns the query pairs of the redirect that comes back
    /// within `limit`. The caller checks `state` against the value it put in the URL.
    pub async fn oauth_login(&self, build_url: impl FnOnce(&str) -> Url, limit: Duration) -> io::Result<Vec<(String, String)>> {
        let redirect = LoopbackRedirect::bind().await?;
        let uri = redirect.redirect_uri("/callback")?;
        self.opener.open_target(build_url(&uri).as_str())?;
        redirect.receive(limit).await
    }

    /// The cookies in the import file that `url`'s host can send. A missing or unreadable file gives none.
    pub fn imported_cookies(&self, url: &Url) -> Vec<WebCookie> {
        let text = std::fs::read_to_string(&self.cookies_file).unwrap_or_default();
        parse_cookies_txt(&text, url.host_str().unwrap_or_default())
    }
}

#[async_trait]
impl WebViewHost for BrowserHost {
    fn available(&self) -> bool {
        true
    }

    async fn get_html(&self) -> Option<String> {
        None
    }

    async fn solve_challenge(&self, req: SolveRequest, cancel: CancellationToken) -> SolveOutcome {
        let failed = |reason| SolveOutcome::Failed { reason, outdated: false };
        if let Err(e) = self.opener.open_target(req.url.as_str()) {
            return failed(SolveFailure::LoadError(e.to_string()));
        }
        let deadline = Instant::now() + req.timeout;
        loop {
            let cookies = self.imported_cookies(&req.url);
            let cleared = cookies
                .iter()
                .any(|c| req.cookie_names.contains(&c.name.as_str()) && Some(&c.value) != req.old_clearance.as_ref());
            if cleared {
                return SolveOutcome::Cleared { cookies, outdated: false };
            }
            if cancel.is_cancelled() {
                return failed(SolveFailure::Cancelled);
            }
            if Instant::now() >= deadline {
                return failed(SolveFailure::Timeout);
            }
            tokio::time::sleep(self.poll).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tokio::io::AsyncReadExt;
    use tokio::net::TcpStream;

    use super::*;

    const EXPORT: &str = "# Netscape HTTP Cookie File\n\
.site.test\tTRUE\t/\tTRUE\t1893456000\tcf_clearance\tabc\n\
#HttpOnly_site.test\tFALSE\t/\tFALSE\t0\tsession\txyz\n\
.other.test\tTRUE\t/\tFALSE\t0\tnot_mine\tq\n\
broken line\n";

    #[test]
    fn keeps_the_cookies_the_source_can_send() {
        let cookies = parse_cookies_txt(EXPORT, "site.test");
        let names: Vec<&str> = cookies.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["cf_clearance", "session"]);
    }

    #[test]
    fn a_subdomain_source_receives_its_parent_domain_cookies() {
        let cookies = parse_cookies_txt(EXPORT, "api.site.test");
        assert_eq!(cookies.len(), 2);
    }

    #[test]
    fn expiry_zero_is_a_session_cookie_and_http_only_is_kept() {
        let cookies = parse_cookies_txt(EXPORT, "site.test");
        assert_eq!(cookies[0].expires, Some(1893456000));
        assert!(cookies[0].secure && !cookies[0].http_only);
        assert_eq!(cookies[1].expires, None);
        assert!(cookies[1].http_only && !cookies[1].secure);
        assert_eq!(cookies[1].domain, "site.test");
    }

    #[test]
    fn a_non_matching_host_gets_nothing() {
        assert!(parse_cookies_txt(EXPORT, "evil.test").is_empty());
        assert!(parse_cookies_txt(EXPORT, "xsite.test").is_empty());
    }

    #[tokio::test]
    async fn the_redirect_query_comes_back_and_the_browser_gets_a_page() {
        let redirect = LoopbackRedirect::bind().await.unwrap();
        let uri = redirect.redirect_uri("/callback").unwrap();
        let port: u16 = Url::parse(&uri).unwrap().port().unwrap();
        let client = tokio::spawn(async move {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            stream.write_all(b"GET /callback?code=abc&state=xyz HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
            let mut answer = String::new();
            stream.read_to_string(&mut answer).await.unwrap();
            answer
        });
        let pairs = redirect.receive(Duration::from_secs(5)).await.unwrap();
        assert_eq!(pairs, vec![("code".to_owned(), "abc".to_owned()), ("state".to_owned(), "xyz".to_owned())]);
        assert!(client.await.unwrap().starts_with("HTTP/1.1 200 OK"));
    }

    #[tokio::test]
    async fn a_request_without_a_query_does_not_end_the_wait() {
        let redirect = LoopbackRedirect::bind().await.unwrap();
        let port: u16 = Url::parse(&redirect.redirect_uri("/callback").unwrap()).unwrap().port().unwrap();
        let client = tokio::spawn(async move {
            let mut favicon = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            favicon.write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n").await.unwrap();
            let mut answer = String::new();
            favicon.read_to_string(&mut answer).await.unwrap();
            assert!(answer.starts_with("HTTP/1.1 404"));
            let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            stream.write_all(b"GET /callback?code=ok HTTP/1.1\r\n\r\n").await.unwrap();
            let mut answer = String::new();
            stream.read_to_string(&mut answer).await.unwrap();
        });
        let pairs = redirect.receive(Duration::from_secs(5)).await.unwrap();
        assert_eq!(pairs, vec![("code".to_owned(), "ok".to_owned())]);
        client.await.unwrap();
    }

    #[tokio::test]
    async fn no_redirect_in_time_is_a_timeout() {
        let redirect = LoopbackRedirect::bind().await.unwrap();
        let err = redirect.receive(Duration::from_millis(50)).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);

    impl Opener for Recorder {
        fn open_target(&self, target: &str) -> io::Result<()> {
            self.0.lock().unwrap().push(target.to_owned());
            Ok(())
        }
    }

    fn request(old: Option<&str>, timeout: Duration) -> SolveRequest {
        SolveRequest {
            url: Url::parse("https://site.test/x").unwrap(),
            headers: Vec::new(),
            user_agent: String::new(),
            cookies: Vec::new(),
            old_clearance: old.map(str::to_owned),
            timeout,
            cookie_names: &["cf_clearance"],
            visible: false,
        }
    }

    fn host(file: PathBuf) -> (BrowserHost, Arc<Recorder>) {
        let rec = Arc::new(Recorder::default());
        let mut host = BrowserHost::new(rec.clone(), file);
        host.poll = Duration::from_millis(10);
        (host, rec)
    }

    #[tokio::test]
    async fn a_new_clearance_cookie_in_the_import_file_clears_the_challenge() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("cookies.txt");
        std::fs::write(&file, EXPORT).unwrap();
        let (host, rec) = host(file);
        let outcome = host.solve_challenge(request(Some("old"), Duration::from_secs(5)), CancellationToken::new()).await;
        let SolveOutcome::Cleared { cookies, .. } = outcome else { panic!("{outcome:?}") };
        assert_eq!(cookies.len(), 2);
        assert_eq!(*rec.0.lock().unwrap(), vec!["https://site.test/x".to_owned()]);
    }

    #[tokio::test]
    async fn the_old_clearance_value_does_not_clear_and_the_wait_times_out() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("cookies.txt");
        std::fs::write(&file, EXPORT).unwrap();
        let (host, _) = host(file);
        let outcome = host.solve_challenge(request(Some("abc"), Duration::from_millis(50)), CancellationToken::new()).await;
        assert_eq!(outcome, SolveOutcome::Failed { reason: SolveFailure::Timeout, outdated: false });
    }

    #[tokio::test]
    async fn cancelling_ends_the_wait() {
        let dir = tempfile::tempdir().unwrap();
        let (host, _) = host(dir.path().join("missing.txt"));
        let cancel = CancellationToken::new();
        cancel.cancel();
        let outcome = host.solve_challenge(request(None, Duration::from_secs(5)), cancel).await;
        assert_eq!(outcome, SolveOutcome::Failed { reason: SolveFailure::Cancelled, outdated: false });
    }

    #[tokio::test]
    async fn oauth_login_opens_the_browser_with_the_redirect_uri_and_returns_the_query() {
        let dir = tempfile::tempdir().unwrap();
        let (host, rec) = host(dir.path().join("cookies.txt"));
        let host = Arc::new(host);
        let task = {
            let host = host.clone();
            tokio::spawn(async move {
                host.oauth_login(
                    |redirect| Url::parse_with_params("https://auth.test/authorize", [("redirect_uri", redirect)]).unwrap(),
                    Duration::from_secs(5),
                )
                .await
            })
        };
        let opened = loop {
            if let Some(url) = rec.0.lock().unwrap().first().cloned() {
                break url;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        let redirect = Url::parse(&opened).unwrap().query_pairs().find(|(k, _)| k == "redirect_uri").unwrap().1.into_owned();
        let port = Url::parse(&redirect).unwrap().port().unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(b"GET /callback?code=c1 HTTP/1.1\r\n\r\n").await.unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).await.unwrap();
        assert_eq!(task.await.unwrap().unwrap(), vec![("code".to_owned(), "c1".to_owned())]);
    }
}
