//! The app-update downloader (T-35a-050, T-35a-051, T-35a-052, T-35a-053; design 8.6): one download at a time, written
//! to `.kuro-update-<name>.part` and renamed to `<name>` on success. Starting a download replaces a running one.
//!
//! Decisions:
//! - `kuro-platform` has no `Paths::update_dir` and is outside this lane, so the folder is an argument; the app passes
//!   the downloads folder (the file is meant to be found and run by the user). Because that folder holds the user's
//!   own files, only `.kuro-update-*.part` files are swept, and only when older than [`STALE_AFTER`].
//! - An offline start shows "Waiting for network" and sends no request until the connectivity adapter reports a
//!   network (T-35a-057). Metered connections are allowed, as in Aniyomi (the adapter only reports availability).
//! - The checksum is hashed while the bytes are written, so the finished file is never read again.
//! - A notice step is a plain [`Op`] handed to a sink, so the order of notices is tested without GTK.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use kuro_track::connectivity::Connectivity;
use sha2::{Digest, Sha256};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::app_update::{self, Phase};
use super::notify::Op;
use super::throttle::ProgressThrottle;

/// A part file older than this is deleted when a download starts.
pub const STALE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);
const PART_PREFIX: &str = ".kuro-update-";
const PART_SUFFIX: &str = ".part";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const FAILED_BODY: &str = "Download error";

/// What to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub url: String,
    pub asset_name: String,
    /// Shown as the progress title.
    pub version: Option<String>,
    /// Lowercase or uppercase hex; the finished file must match it.
    pub sha256: Option<String>,
}

/// Where the notice steps go.
pub type Sink = Arc<dyn Fn(Op) + Send + Sync>;

#[derive(Debug, PartialEq, Eq)]
pub enum DownloadError {
    /// The asset name or the url cannot be used; nothing was started.
    Rejected(&'static str),
}

enum Outcome {
    Done { path: PathBuf, verified: bool },
    Cancelled,
    Failed(String),
}

struct Running {
    cancel: CancellationToken,
    handle: JoinHandle<()>,
}

pub struct Downloader {
    dir: PathBuf,
    sink: Sink,
    handle: Handle,
    net: Arc<dyn Connectivity>,
    slot: Mutex<Option<Running>>,
}

/// The desktop's network state (T-35a-057). GIO reports link availability, not internet reachability.
pub struct GioNetwork;

#[async_trait::async_trait]
impl Connectivity for GioNetwork {
    fn is_online(&self) -> bool {
        use gtk::gio::prelude::NetworkMonitorExt as _;
        gtk::gio::NetworkMonitor::default().is_network_available()
    }
}

impl Downloader {
    pub fn new(dir: PathBuf, sink: Sink, handle: Handle, net: Arc<dyn Connectivity>) -> Arc<Self> {
        Arc::new(Self { dir, sink, handle, net, slot: Mutex::new(None) })
    }

    /// Starts `request`, replacing a running download: the old one is cancelled and has ended before the new one begins.
    pub fn start(self: &Arc<Self>, request: Request) -> Result<(), DownloadError> {
        if request.url.is_empty() {
            return Err(DownloadError::Rejected("empty url"));
        }
        let name = kuro_platform::filename::valid_filename(&request.asset_name);
        if name != request.asset_name || name == kuro_platform::filename::EMPTY_NAME {
            return Err(DownloadError::Rejected("asset name is not a plain file name"));
        }
        let cancel = CancellationToken::new();
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        let previous = slot.take();
        if let Some(old) = &previous {
            old.cancel.cancel();
        }
        let (this, token) = (Arc::clone(self), cancel.clone());
        let handle = self.handle.spawn(async move {
            if let Some(old) = previous {
                let _ = old.handle.await;
            }
            this.run(request, name, token).await;
        });
        *slot = Some(Running { cancel, handle });
        Ok(())
    }

    /// The Cancel button: stops the running download; the task withdraws the progress notice and shows no error.
    pub fn stop(&self) {
        if let Some(running) = self.slot.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            running.cancel.cancel();
        }
    }

    #[cfg(test)]
    async fn finished(&self) {
        let running = self.slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(running) = running {
            let _ = running.handle.await;
        }
    }

    async fn run(&self, request: Request, name: String, cancel: CancellationToken) {
        sweep_stale(&self.dir, SystemTime::now());
        let part = self.dir.join(format!("{PART_PREFIX}{name}{PART_SUFFIX}"));
        let version = request.version.as_deref();
        let online = self.net.is_online();
        let first = if online { Phase::Downloading(None) } else { Phase::WaitingForNetwork };
        (self.sink)(Op::Show(app_update::progress(version, first)));
        let outcome = tokio::select! {
            () = cancel.cancelled() => Outcome::Cancelled,
            outcome = self.fetch_when_online(&request, &name, &part, online) => outcome,
        };
        let steps = match outcome {
            Outcome::Done { path, verified } => app_update::finished_steps(&path, verified),
            Outcome::Cancelled => {
                let _ = std::fs::remove_file(&part);
                app_update::cancelled_steps()
            }
            Outcome::Failed(message) => {
                let _ = std::fs::remove_file(&part);
                tracing::warn!(url = %request.url, %message, "the app update download failed");
                app_update::failed_steps(version, &request.url, request.sha256.as_deref(), FAILED_BODY)
            }
        };
        for step in steps {
            (self.sink)(step);
        }
    }

    /// Waits for the network when the start found none, then downloads. No request is sent before that (T-35a-057).
    async fn fetch_when_online(&self, request: &Request, name: &str, part: &Path, online: bool) -> Outcome {
        if !online {
            self.net.wait_online().await;
            (self.sink)(Op::Show(app_update::progress(request.version.as_deref(), Phase::Downloading(None))));
        }
        self.fetch(request, name, part).await
    }

    async fn fetch(&self, request: &Request, name: &str, part: &Path) -> Outcome {
        match self.fetch_inner(request, name, part).await {
            Ok(done) => done,
            Err(message) => Outcome::Failed(message),
        }
    }

    async fn fetch_inner(&self, request: &Request, name: &str, part: &Path) -> Result<Outcome, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("cannot create {}: {e}", self.dir.display()))?;
        let client = reqwest::Client::builder().no_proxy().connect_timeout(CONNECT_TIMEOUT).build().map_err(|e| e.to_string())?;
        let mut response = client.get(&request.url).send().await.map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status().as_u16()));
        }
        let total = response.content_length();
        let mut file = std::fs::File::create(part).map_err(|e| format!("cannot write {}: {e}", part.display()))?;
        let (mut hasher, mut bytes, mut throttle) = (Sha256::new(), 0_u64, ProgressThrottle::new());
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            hasher.update(&chunk);
            bytes += chunk.len() as u64;
            if let Some(percent) = throttle.offer(Instant::now(), bytes, total) {
                (self.sink)(Op::Show(app_update::progress(request.version.as_deref(), Phase::Downloading(Some(percent)))));
            }
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        let digest = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>();
        if let Some(expected) = &request.sha256 {
            if !expected.trim().eq_ignore_ascii_case(&digest) {
                return Err("Checksum mismatch".to_owned());
            }
        }
        let path = self.dir.join(name);
        std::fs::rename(part, &path).map_err(|e| e.to_string())?;
        Ok(Outcome::Done { path, verified: request.sha256.is_some() })
    }
}

thread_local! {
    static INSTANCE: std::cell::RefCell<Option<Arc<Downloader>>> = const { std::cell::RefCell::new(None) };
}

/// Creates the app's downloader once: notices go to the GTK notifier and files to `dir`. Call from the GTK main thread.
pub fn install(dir: PathBuf, handle: Handle) {
    INSTANCE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let sink: Sink = Arc::new(|op| {
                super::notify::send(op);
            });
            *slot = Some(Downloader::new(dir, sink, handle, Arc::new(GioNetwork)));
        }
    });
}

/// The file name an asset url ends in, or `None` when the url has no path segment to name it.
pub fn asset_name_of(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    parsed.path_segments()?.next_back().filter(|n| !n.is_empty()).map(str::to_owned)
}

/// The `StartAppUpdate` action: downloads `url`. A url with no usable file name shows the failure notice.
/// `false` before [`install`] ran.
pub fn start_app_update(url: &str, version: Option<String>, sha256: Option<String>) -> bool {
    let Some(downloader) = INSTANCE.with(|slot| slot.borrow().clone()) else { return false };
    let request = Request { url: url.to_owned(), asset_name: asset_name_of(url).unwrap_or_default(), version, sha256 };
    if let Err(error) = downloader.start(request.clone()) {
        tracing::warn!(?error, url, "the app update download was not started");
        for step in app_update::failed_steps(request.version.as_deref(), url, request.sha256.as_deref(), FAILED_BODY) {
            (downloader.sink)(step);
        }
    }
    true
}

/// The `CancelAppUpdate` action.
pub fn cancel_app_update() {
    if let Some(downloader) = INSTANCE.with(|slot| slot.borrow().clone()) {
        downloader.stop();
    }
}

/// Deletes the `.kuro-update-*.part` files of `dir` last written more than [`STALE_AFTER`] before `now`.
pub fn sweep_stale(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(PART_PREFIX) && name.ends_with(PART_SUFFIX)) {
            continue;
        }
        let age = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| now.duration_since(t).ok());
        if age.is_some_and(|age| age > STALE_AFTER) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};

    const BODY: &[u8] = b"kurobayomi-appimage-bytes";

    fn sha(bytes: &[u8]) -> String {
        Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn respond(mut stream: TcpStream) {
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).unwrap_or(0) == 0 {
                return;
            }
            head.push(byte[0]);
        }
        let line = String::from_utf8_lossy(&head).lines().next().unwrap_or_default().to_owned();
        let path = line.split(' ').nth(1).unwrap_or("/").to_owned();
        let whole = |status: &str, body: &[u8]| {
            let mut out = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
            out.extend_from_slice(body);
            out
        };
        match path.as_str() {
            "/ok" => {
                let _ = stream.write_all(&whole("200 OK", BODY));
            }
            "/other" => {
                let _ = stream.write_all(&whole("200 OK", b"the newer download"));
            }
            "/stall" => {
                // the length says 1000 bytes; 10 arrive, then the connection idles until the client leaves
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n0123456789");
                let _ = stream.flush();
                let mut sink = [0_u8; 16];
                while stream.read(&mut sink).unwrap_or(0) > 0 {}
            }
            _ => {
                let _ = stream.write_all(&whole("404 Not Found", b"no"));
            }
        }
    }

    fn server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || respond(stream));
            }
        });
        format!("http://{addr}")
    }

    /// A network that answers from a flag. Its `wait_online` is the trait's default poll.
    struct Net(Arc<AtomicBool>);

    #[async_trait::async_trait]
    impl Connectivity for Net {
        fn is_online(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn setup() -> (tempfile::TempDir, Arc<Downloader>, Arc<Mutex<Vec<Op>>>) {
        setup_online(Arc::new(AtomicBool::new(true)))
    }

    fn setup_online(online: Arc<AtomicBool>) -> (tempfile::TempDir, Arc<Downloader>, Arc<Mutex<Vec<Op>>>) {
        let dir = tempfile::tempdir().unwrap();
        let ops = Arc::new(Mutex::new(Vec::new()));
        let log = ops.clone();
        let sink: Sink = Arc::new(move |op| log.lock().unwrap().push(op));
        let downloader = Downloader::new(dir_path(&dir), sink, Handle::current(), Arc::new(Net(online)));
        (dir, downloader, ops)
    }

    fn dir_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("updates")
    }

    fn request(url: String, sha256: Option<String>) -> Request {
        Request { url, asset_name: "Kurobayomi-x86_64.AppImage".to_owned(), version: Some("v1.0.0".to_owned()), sha256 }
    }

    /// The ids of the last steps, as `withdraw:id` and `show:id`.
    fn tail(ops: &Arc<Mutex<Vec<Op>>>, count: usize) -> Vec<String> {
        let ops = ops.lock().unwrap();
        let all: Vec<String> = ops
            .iter()
            .map(|op| match op {
                Op::Withdraw(id) => format!("withdraw:{id}"),
                Op::Show(post) => format!("show:{}", post.id),
            })
            .collect();
        all[all.len().saturating_sub(count)..].to_vec()
    }

    fn parts(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(PART_SUFFIX)).collect())
            .unwrap_or_default()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn success_renames_the_part_and_posts_the_finished_notice_without_running_it() {
        let (dir, dl, ops) = setup();
        dl.start(request(format!("{}/ok", server()), Some(sha(BODY).to_uppercase()))).unwrap();
        dl.finished().await;
        let file = dir_path(&dir).join("Kurobayomi-x86_64.AppImage");
        assert_eq!(std::fs::read(&file).unwrap(), BODY);
        assert!(parts(&dir_path(&dir)).is_empty());
        assert_eq!(tail(&ops, 2), ["withdraw:app-update.progress", "show:app-update.install"]);
        let last = ops.lock().unwrap().last().cloned();
        let Some(Op::Show(post)) = last else { panic!("a finished notice") };
        assert_eq!(post.body.as_deref(), Some("Downloaded Kurobayomi-x86_64.AppImage"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_404_withdraws_the_progress_before_the_retry_notice_and_leaves_no_file() {
        let (dir, dl, ops) = setup();
        let url = format!("{}/missing", server());
        dl.start(request(url.clone(), None)).unwrap();
        dl.finished().await;
        assert_eq!(tail(&ops, 2), ["withdraw:app-update.progress", "show:app-update.error"]);
        let last = ops.lock().unwrap().last().cloned();
        let Some(Op::Show(post)) = last else { panic!("an error notice") };
        assert_eq!(post.buttons.len(), 2);
        assert!(parts(&dir_path(&dir)).is_empty());
        assert!(!dir_path(&dir).join("Kurobayomi-x86_64.AppImage").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_removes_the_part_and_shows_no_error() {
        let (dir, dl, ops) = setup();
        dl.start(request(format!("{}/stall", server()), None)).unwrap();
        // wait until the part file exists, then cancel
        for _ in 0..200 {
            if !parts(&dir_path(&dir)).is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(!parts(&dir_path(&dir)).is_empty(), "the download began");
        dl.stop();
        dl.finished().await;
        assert!(parts(&dir_path(&dir)).is_empty());
        assert_eq!(tail(&ops, 1), ["withdraw:app-update.progress"]);
        assert!(!ops.lock().unwrap().iter().any(|op| matches!(op, Op::Show(p) if p.id == crate::misc::ids::APP_UPDATE_ERROR)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_checksum_mismatch_deletes_the_part_and_shows_the_error() {
        let (dir, dl, ops) = setup();
        dl.start(request(format!("{}/ok", server()), Some(sha(b"something else")))).unwrap();
        dl.finished().await;
        assert!(parts(&dir_path(&dir)).is_empty());
        assert!(!dir_path(&dir).join("Kurobayomi-x86_64.AppImage").exists());
        assert_eq!(tail(&ops, 2), ["withdraw:app-update.progress", "show:app-update.error"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_second_start_replaces_the_running_download() {
        let (dir, dl, ops) = setup();
        let base = server();
        dl.start(request(format!("{base}/stall"), None)).unwrap();
        for _ in 0..200 {
            if !parts(&dir_path(&dir)).is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        dl.start(request(format!("{base}/other"), None)).unwrap();
        dl.finished().await;
        let file = dir_path(&dir).join("Kurobayomi-x86_64.AppImage");
        assert_eq!(std::fs::read(&file).unwrap(), b"the newer download");
        assert!(parts(&dir_path(&dir)).is_empty());
        assert_eq!(tail(&ops, 1), ["show:app-update.install"]);
        assert!(!ops.lock().unwrap().iter().any(|op| matches!(op, Op::Show(p) if p.id == crate::misc::ids::APP_UPDATE_ERROR)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn offline_waits_for_the_network_and_sends_no_request_until_it_is_back() {
        // T-35a-057 (U19): no part file and no download while offline; the download runs once the network returns.
        let online = Arc::new(AtomicBool::new(false));
        let (dir, dl, ops) = setup_online(online.clone());
        dl.start(request(format!("{}/ok", server()), None)).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        let waiting = ops.lock().unwrap().iter().any(|op| matches!(op, Op::Show(p) if p.body.as_deref() == Some("Waiting for network")));
        assert!(waiting, "the progress notice says Waiting for network");
        assert!(parts(&dir_path(&dir)).is_empty(), "no request was sent");
        online.store(true, Ordering::SeqCst);
        dl.finished().await;
        assert_eq!(std::fs::read(dir_path(&dir).join("Kurobayomi-x86_64.AppImage")).unwrap(), BODY);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_refused_connection_while_online_shows_the_retry_notice() {
        let (dir, dl, ops) = setup();
        // a port nothing listens on
        let closed = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap()
        };
        dl.start(request(format!("http://{closed}/ok"), None)).unwrap();
        dl.finished().await;
        assert_eq!(tail(&ops, 2), ["withdraw:app-update.progress", "show:app-update.error"]);
        assert!(parts(&dir_path(&dir)).is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_bad_asset_name_or_empty_url_starts_nothing() {
        let (_dir, dl, ops) = setup();
        assert!(dl.start(request(String::new(), None)).is_err());
        let mut request = request("http://127.0.0.1:1/x".to_owned(), None);
        request.asset_name = "../evil".to_owned();
        assert!(dl.start(request).is_err());
        assert!(ops.lock().unwrap().is_empty());
    }

    #[test]
    fn the_asset_name_is_the_last_path_segment() {
        assert_eq!(asset_name_of("https://github.com/o/r/releases/download/v1/App.AppImage").as_deref(), Some("App.AppImage"));
        assert_eq!(asset_name_of("https://github.com/o/r/"), None);
        assert_eq!(asset_name_of("nope"), None);
    }

    #[test]
    fn only_old_update_part_files_are_swept() {
        // T-35a-050 stale cleanup
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join(".kuro-update-a.part");
        let fresh = dir.path().join(".kuro-update-b.part");
        let foreign = dir.path().join("movie.part");
        for path in [&old, &fresh, &foreign] {
            std::fs::write(path, b"x").unwrap();
        }
        let now = SystemTime::now();
        std::fs::File::options().write(true).open(&old).unwrap().set_modified(now - STALE_AFTER - Duration::from_secs(60)).unwrap();
        sweep_stale(dir.path(), now);
        assert!(!old.exists());
        assert!(fresh.exists() && foreign.exists());
    }
}
