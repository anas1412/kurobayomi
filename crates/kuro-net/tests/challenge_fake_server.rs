//! Cloudflare challenge tests against a local fake server (T-12a-042). The server replays the recorded responses in
//! `tests/fixtures`. The WebView is a fake, because the WebKitGTK host is not built yet: the solve budget is checked
//! as the 30 s the WebView is asked for, and the fake reports the timeout at once instead of waiting it out.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use http::StatusCode;
use kuro_net::{NetError, NetworkClient};
use kuro_platform::toast::toast_channel;
use kuro_platform::webview::{SolveFailure, SolveOutcome, SolveRequest, WebCookie, WebViewHost};
use kuro_prefs::network::NetworkPrefs;
use kuro_prefs::store::PrefStore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

const CHALLENGE: &[u8] = include_bytes!("fixtures/cf_challenge_503.http");
const PAGE: &[u8] = include_bytes!("fixtures/page_200.http");

/// Serves `responses` in order, one per connection. The last response repeats. Returns the base URL and the count.
async fn fake_server(responses: Vec<&'static [u8]>) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let served = Arc::new(AtomicUsize::new(0));
    let counter = served.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            let n = counter.fetch_add(1, Ordering::SeqCst);
            let mut head = Vec::new();
            let mut chunk = [0u8; 1024];
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                match sock.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => head.extend_from_slice(&chunk[..read]),
                }
            }
            let _ = sock.write_all(responses[n.min(responses.len() - 1)]).await;
            let _ = sock.shutdown().await;
        }
    });
    (url, served)
}

/// A WebView that either clears the challenge with a `cf_clearance` cookie, or reports a timeout at once.
#[derive(Default)]
struct FakeWebView {
    clears: bool,
    solves: AtomicUsize,
    budgets: Mutex<Vec<Duration>>,
}

#[async_trait]
impl WebViewHost for FakeWebView {
    fn available(&self) -> bool {
        true
    }

    async fn get_html(&self) -> Option<String> {
        None
    }

    async fn solve_challenge(&self, req: SolveRequest, _cancel: CancellationToken) -> SolveOutcome {
        self.solves.fetch_add(1, Ordering::SeqCst);
        self.budgets.lock().unwrap().push(req.timeout);
        if !self.clears {
            return SolveOutcome::Failed { reason: SolveFailure::Timeout, outdated: false };
        }
        SolveOutcome::Cleared {
            cookies: vec![WebCookie {
                name: "cf_clearance".into(),
                value: "fresh".into(),
                domain: String::new(),
                path: "/".into(),
                secure: false,
                http_only: false,
                expires: None,
                same_site: None,
            }],
            outdated: false,
        }
    }
}

fn client(webview: Arc<FakeWebView>, dir: &std::path::Path) -> NetworkClient {
    let prefs = NetworkPrefs::new(PrefStore::in_memory());
    let (toaster, _feed) = toast_channel();
    NetworkClient::new(&prefs, dir.join("cookies.json"), webview, toaster).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cleared_challenge_is_solved_once_and_the_retry_reaches_the_page() {
    let (url, served) = fake_server(vec![CHALLENGE, PAGE]).await;
    let dir = tempfile::tempdir().unwrap();
    let webview = Arc::new(FakeWebView { clears: true, ..FakeWebView::default() });
    let resp = client(webview.clone(), dir.path()).get(&url).await.unwrap();
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.bytes().await.unwrap().as_ref(), b"<html>page</html>");
    assert_eq!(webview.solves.load(Ordering::SeqCst), 1);
    assert_eq!(served.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_that_never_sets_cf_clearance_fails_with_the_bypass_error_within_the_thirty_second_budget() {
    let (url, served) = fake_server(vec![CHALLENGE]).await;
    let dir = tempfile::tempdir().unwrap();
    let webview = Arc::new(FakeWebView::default());
    let err = client(webview.clone(), dir.path()).get(&url).await.err().unwrap();
    assert!(matches!(err, NetError::CloudflareBypassFailed), "{err:?}");
    assert_eq!(*webview.budgets.lock().unwrap(), vec![Duration::from_secs(30)]);
    assert_eq!(served.load(Ordering::SeqCst), 1, "a failed solve is not retried");
}
