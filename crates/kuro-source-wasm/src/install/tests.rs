//! Installer tests. Most downloads come from a small raw TCP server, because the cases that matter here (a connection
//! cut mid-body, a response held half sent, a chunked body with no length) are not ones a mock server can script.
//! One test uses `wiremock` for the plain happy path.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ed25519_dalek::SigningKey;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use super::*;
use crate::package::fixtures::{archive, manifest_json, package};
use crate::verify::fixtures::{key, sign};

const PKG: &str = "eu.example.one";

// ---- test server ----

#[derive(Clone)]
enum Reply {
    Ok(Vec<u8>),
    /// Claims 100 bytes more than it sends, then closes.
    Cut(Vec<u8>),
    /// Sends the headers and half the body, then waits for the gate.
    Gated(Vec<u8>),
    /// Reads the request and never answers until the gate opens.
    Stall,
    Chunked(Vec<u8>),
    Status(u16),
}

struct TestServer {
    url: Url,
    hits: Arc<AtomicUsize>,
}

/// Connection `n` gets `script[n]`; the last entry repeats.
async fn serve(script: Vec<Reply>) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = Url::parse(&format!("http://{}/pkg.kuroext", listener.local_addr().unwrap())).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(Semaphore::new(0));
    let (h, g) = (hits.clone(), gate);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let n = h.fetch_add(1, Ordering::SeqCst);
            let reply = script[n.min(script.len() - 1)].clone();
            let gate = g.clone();
            tokio::spawn(async move {
                let mut seen = Vec::new();
                let mut buf = [0u8; 1024];
                while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                    match socket.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => seen.extend_from_slice(&buf[..read]),
                    }
                }
                let head = |len: usize| format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n");
                let _ = match reply {
                    Reply::Ok(body) => {
                        socket.write_all(head(body.len()).as_bytes()).await.and(socket.write_all(&body).await)
                    }
                    Reply::Cut(body) => {
                        socket.write_all(head(body.len() + 100).as_bytes()).await.and(socket.write_all(&body).await)
                    }
                    Reply::Gated(body) => {
                        let half = body.len() / 2;
                        let _ = socket.write_all(head(body.len()).as_bytes()).await;
                        let _ = socket.write_all(&body[..half]).await;
                        let _ = socket.flush().await;
                        gate.acquire().await.unwrap().forget();
                        socket.write_all(&body[half..]).await
                    }
                    Reply::Stall => {
                        gate.acquire().await.unwrap().forget();
                        Ok(())
                    }
                    Reply::Chunked(body) => {
                        let head = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
                        let mut out = head.as_bytes().to_vec();
                        out.extend_from_slice(format!("{:x}\r\n", body.len()).as_bytes());
                        out.extend_from_slice(&body);
                        out.extend_from_slice(b"\r\n0\r\n\r\n");
                        socket.write_all(&out).await
                    }
                    Reply::Status(code) => {
                        let text = format!("HTTP/1.1 {code} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        socket.write_all(text.as_bytes()).await
                    }
                };
                let _ = socket.shutdown().await;
            });
        }
    });
    TestServer { url, hits }
}

// ---- rig ----

struct Rig {
    store: tempfile::TempDir,
    cache: tempfile::TempDir,
    installer: Arc<Installer>,
    rx: async_channel::Receiver<InstallEvent>,
}

fn new_rig(tweak: impl FnOnce(&mut InstallConfig)) -> Rig {
    let store = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let mut config = InstallConfig::new(cache.path().join("download"), store.path().join("extensions"));
    config.retry_delay = Duration::from_millis(10);
    tweak(&mut config);
    let (tx, rx) = async_channel::unbounded();
    let installer = Installer::new(config, Handle::current(), tx).unwrap();
    Rig { store, cache, installer, rx }
}

use tokio::runtime::Handle;

impl Rig {
    fn store_dir(&self) -> PathBuf {
        self.store.path().join("extensions")
    }

    fn download_dir(&self) -> PathBuf {
        self.cache.path().join("download")
    }

    fn files(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|entries| entries.map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Events until `pkg` reaches a completed step.
    async fn until_done(&self, pkg: &str) -> Vec<InstallEvent> {
        self.until(|e| matches!(e, InstallEvent::Step { pkg: p, step } if p == pkg && step.is_completed())).await
    }

    async fn until(&self, mut stop: impl FnMut(&InstallEvent) -> bool) -> Vec<InstallEvent> {
        let mut seen = Vec::new();
        loop {
            let event = tokio::time::timeout(Duration::from_secs(10), self.rx.recv())
                .await
                .expect("an event within 10 s")
                .unwrap();
            let done = stop(&event);
            seen.push(event);
            if done {
                return seen;
            }
        }
    }

    async fn until_step(&self, pkg: &str, wanted: InstallStep) -> Vec<InstallEvent> {
        self.until(|e| matches!(e, InstallEvent::Step { pkg: p, step } if p == pkg && *step == wanted)).await
    }
}

fn steps(events: &[InstallEvent]) -> Vec<InstallStep> {
    events
        .iter()
        .filter_map(|e| match e {
            InstallEvent::Step { step, .. } => Some(*step),
            _ => None,
        })
        .collect()
}

fn request_for(pkg: &str, bytes: &[u8], signing: &SigningKey, url: &Url) -> InstallRequest {
    let signed = sign(bytes, signing);
    InstallRequest {
        pkg: pkg.to_owned(),
        version_name: "1.4.1".to_owned(),
        url: url.clone(),
        sha256: signed.sha256,
        ed25519: signed.signature,
        ed25519_pub: signed.public_key,
        repo_fingerprint: signed.fingerprint,
    }
}

/// An installed package file as an earlier install would have left it.
fn put_installed(store: &std::path::Path, pkg: &str, version_code: i64, signer: &SigningKey) -> Vec<u8> {
    use base64::Engine as _;
    let mut manifest: serde_json::Value = serde_json::from_str(&manifest_json(pkg, version_code)).unwrap();
    manifest["signerPub"] =
        base64::engine::general_purpose::STANDARD.encode(signer.verifying_key().to_bytes()).into();
    let bytes = archive(&[("manifest.json", manifest.to_string().as_bytes()), ("source.wasm", b"old-wasm")]);
    std::fs::create_dir_all(store).unwrap();
    std::fs::write(store.join(format!("{pkg}.kuroext")), &bytes).unwrap();
    bytes
}

// ---- pipeline ----

#[tokio::test(flavor = "multi_thread")]
async fn a_good_install_runs_the_steps_in_order_and_commits_read_only() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let bytes = package(PKG, 7);
    let mock = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone())).mount(&mock).await;
    let url = Url::parse(&format!("{}/pkg.kuroext", mock.uri())).unwrap();
    let rig = new_rig(|_| {});

    assert_eq!(rig.installer.install(request_for(PKG, &bytes, &key(1), &url)).unwrap(), Enqueued::Started);
    let events = rig.until_done(PKG).await;

    let sequence: Vec<&InstallEvent> = events.iter().filter(|e| !matches!(e, InstallEvent::Progress { .. })).collect();
    assert_eq!(
        sequence,
        vec![
            &InstallEvent::Step { pkg: PKG.into(), step: InstallStep::Pending },
            &InstallEvent::Step { pkg: PKG.into(), step: InstallStep::Downloading },
            &InstallEvent::Step { pkg: PKG.into(), step: InstallStep::Installing },
            &InstallEvent::Installed { pkg: PKG.into(), version_code: 7, version_name: "1.4.1".into() },
            &InstallEvent::Step { pkg: PKG.into(), step: InstallStep::Installed },
        ]
    );
    assert!(events.iter().any(|e| matches!(e, InstallEvent::Progress { received, .. } if *received == bytes.len() as u64)));
    assert_eq!(rig.installer.step(PKG), InstallStep::Idle);

    let target = rig.store_dir().join(format!("{PKG}.kuroext"));
    assert_eq!(Rig::files(&rig.store_dir()), vec![format!("{PKG}.kuroext")], "no .part beside the target");
    assert!(Rig::files(&rig.download_dir()).is_empty(), "the download .part is gone");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o444);
    }
    let installed = crate::package::read_manifest(&std::fs::read(&target).unwrap()).unwrap();
    assert_eq!(installed.signer_pub, Some(key(1).verifying_key().to_bytes()), "the installer wrote signerPub");
    assert_eq!(installed.version_code, 7);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_check_ends_in_error_and_writes_nothing() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Ok(bytes.clone())]).await;
    let rig = new_rig(|_| {});
    let good = request_for(PKG, &bytes, &key(1), &server.url);

    let mut digest = good.clone();
    digest.sha256[0] ^= 1; // I6
    let mut signature = good.clone();
    signature.ed25519 = sign(&bytes, &key(2)).signature; // I7
    let mut wrong_key = good.clone();
    wrong_key.ed25519_pub = key(2).verifying_key().to_bytes(); // I9: not the repo's key
    let mut other_pkg = good.clone();
    other_pkg.pkg = "eu.example.two".to_owned(); // I12: the archive names eu.example.one

    for bad in [digest, signature, wrong_key, other_pkg] {
        let pkg = bad.pkg.clone();
        rig.installer.install(bad).unwrap();
        let events = rig.until_done(&pkg).await;
        assert_eq!(steps(&events), vec![InstallStep::Pending, InstallStep::Downloading, InstallStep::Error], "{pkg}");
        assert!(Rig::files(&rig.store_dir()).is_empty(), "nothing in the store");
        assert!(Rig::files(&rig.download_dir()).is_empty(), "the .part is removed");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_body_over_the_cap_is_refused_by_length_and_while_streaming() {
    let bytes = package(PKG, 7);
    for reply in [Reply::Ok(bytes.clone()), Reply::Chunked(bytes.clone())] {
        let server = serve(vec![reply]).await;
        let rig = new_rig(|c| c.max_bytes = bytes.len() as u64 - 1);
        rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
        let events = rig.until_done(PKG).await;
        assert_eq!(*steps(&events).last().unwrap(), InstallStep::Error);
        assert!(Rig::files(&rig.download_dir()).is_empty(), "the .part is removed");
    }
    // Exactly at the cap is fine.
    let server = serve(vec![Reply::Chunked(bytes.clone())]).await;
    let rig = new_rig(|c| c.max_bytes = bytes.len() as u64);
    rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
    assert_eq!(*steps(&rig.until_done(PKG).await).last().unwrap(), InstallStep::Installed);
}

#[tokio::test(flavor = "multi_thread")]
async fn one_network_error_is_retried_and_two_are_not() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Cut(bytes.clone()), Reply::Ok(bytes.clone())]).await;
    let rig = new_rig(|_| {});
    rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
    let events = rig.until_done(PKG).await;
    assert_eq!(
        steps(&events),
        vec![InstallStep::Pending, InstallStep::Downloading, InstallStep::Installing, InstallStep::Installed],
        "the retry does not repeat Downloading"
    );
    assert_eq!(server.hits.load(Ordering::SeqCst), 2);

    let server = serve(vec![Reply::Cut(bytes.clone())]).await;
    let rig = new_rig(|_| {});
    rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
    assert_eq!(*steps(&rig.until_done(PKG).await).last().unwrap(), InstallStep::Error);
    assert_eq!(server.hits.load(Ordering::SeqCst), 2, "one retry, then Error");
    assert!(Rig::files(&rig.download_dir()).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_http_error_status_is_not_retried() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Status(404)]).await;
    let rig = new_rig(|_| {});
    rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
    let events = rig.until_done(PKG).await;
    assert_eq!(steps(&events), vec![InstallStep::Pending, InstallStep::Error]);
    assert_eq!(server.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_total_timeout_ends_a_stalled_download() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Stall]).await;
    let rig = new_rig(|c| c.total_timeout = Duration::from_millis(200));
    rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
    assert_eq!(*steps(&rig.until_done(PKG).await).last().unwrap(), InstallStep::Error);
}

// ---- queue, cancel, join ----

#[tokio::test(flavor = "multi_thread")]
async fn only_three_downloads_run_at_once() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Gated(bytes.clone())]).await;
    let rig = new_rig(|_| {});
    let names: Vec<String> = (0..5).map(|i| format!("eu.example.p{i}")).collect();
    for name in &names {
        rig.installer.install(request_for(name, &bytes, &key(1), &server.url)).unwrap();
    }
    tokio::time::timeout(Duration::from_secs(10), async {
        while server.hits.load(Ordering::SeqCst) < 3 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(server.hits.load(Ordering::SeqCst), 3, "the other two wait for a permit");
    let running = names.iter().filter(|n| rig.installer.step(n) == InstallStep::Downloading).count();
    let waiting = names.iter().filter(|n| rig.installer.step(n) == InstallStep::Pending).count();
    assert_eq!((running, waiting), (3, 2));
    for name in &names {
        assert!(rig.installer.cancel(name));
    }
    let mut idle = 0;
    rig.until(|e| {
        idle += usize::from(matches!(e, InstallEvent::Step { step: InstallStep::Idle, .. }));
        idle == 5
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_queued_job_is_idle_at_once() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Gated(bytes.clone())]).await;
    let rig = new_rig(|c| c.permits = 1);
    rig.installer.install(request_for("eu.example.first", &bytes, &key(1), &server.url)).unwrap();
    rig.until_step("eu.example.first", InstallStep::Downloading).await;
    rig.installer.install(request_for("eu.example.queued", &bytes, &key(1), &server.url)).unwrap();
    assert_eq!(rig.installer.step("eu.example.queued"), InstallStep::Pending);

    assert!(rig.installer.cancel("eu.example.queued"));
    let events = rig.until_done("eu.example.queued").await;
    assert_eq!(steps(&events).last(), Some(&InstallStep::Idle));
    assert_eq!(server.hits.load(Ordering::SeqCst), 1, "the queued job never connected");
    assert_eq!(rig.installer.step("eu.example.first"), InstallStep::Downloading, "the other job is untouched");
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_download_removes_the_part_file_and_goes_idle() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Gated(bytes.clone())]).await;
    let rig = new_rig(|_| {});
    rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
    rig.until(|e| matches!(e, InstallEvent::Progress { .. })).await;
    assert_eq!(Rig::files(&rig.download_dir()), vec![format!("{PKG}-1.4.1.kuroext.part")]);

    assert!(rig.installer.cancel(PKG));
    let events = rig.until_done(PKG).await;
    assert_eq!(steps(&events), vec![InstallStep::Idle]);
    assert!(Rig::files(&rig.download_dir()).is_empty());
    assert!(Rig::files(&rig.store_dir()).is_empty());
    assert_eq!(rig.installer.step(PKG), InstallStep::Idle);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancel_while_waiting_for_the_commit_lock_is_accepted() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Ok(bytes.clone())]).await;
    let rig = new_rig(|_| {});
    let held = rig.installer.commit_lock.lock().await;
    rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
    rig.until_step(PKG, InstallStep::Downloading).await;
    tokio::time::sleep(Duration::from_millis(200)).await; // the job now waits for the lock
    assert!(rig.installer.cancel(PKG));
    assert_eq!(steps(&rig.until_done(PKG).await), vec![InstallStep::Idle]);
    drop(held);
    assert!(Rig::files(&rig.store_dir()).is_empty());
}

#[test]
fn a_cancel_is_ignored_once_the_commit_has_started() {
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let _guard = rt.enter();
    let rig = new_rig(|_| {});
    let add = |pkg: &str| {
        let token = CancellationToken::new();
        rig.installer
            .jobs()
            .insert(pkg.to_owned(), Job { token: token.clone(), step: InstallStep::Installing, committing: false });
        token
    };
    let late = add("eu.example.late");
    assert!(rig.installer.begin_commit("eu.example.late").is_ok());
    assert!(!rig.installer.cancel("eu.example.late"), "committing: ignored");
    assert!(!late.is_cancelled());

    let early = add("eu.example.early");
    assert!(rig.installer.cancel("eu.example.early"));
    assert!(early.is_cancelled());
    assert!(matches!(rig.installer.begin_commit("eu.example.early"), Err(Stop::Cancelled)), "cancelled first");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_request_for_the_same_package_joins_the_job() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Ok(bytes.clone())]).await;
    let rig = new_rig(|_| {});
    let request = request_for(PKG, &bytes, &key(1), &server.url);
    assert_eq!(rig.installer.install(request.clone()).unwrap(), Enqueued::Started);
    assert_eq!(rig.installer.install(request).unwrap(), Enqueued::Joined);
    let events = rig.until_done(PKG).await;
    assert_eq!(steps(&events).iter().filter(|s| **s == InstallStep::Pending).count(), 1);
    assert_eq!(server.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_update_without_an_available_entry_emits_nothing() {
    let rig = new_rig(|_| {});
    assert_eq!(rig.installer.update(PKG, None).unwrap(), Enqueued::NoEntry);
    assert!(rig.rx.try_recv().is_err());
    assert_eq!(rig.installer.step(PKG), InstallStep::Idle);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_all_enqueues_in_order_and_skips_rows_already_installing() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Stall]).await;
    let rig = new_rig(|_| {});
    let row = |pkg: &str| request_for(pkg, &bytes, &key(1), &server.url);
    assert_eq!(rig.installer.install(row("eu.example.a")).unwrap(), Enqueued::Started);
    let started = rig.installer.update_all(vec![row("eu.example.a"), row("eu.example.b"), row("eu.example.c")]);
    assert_eq!(started, vec!["eu.example.b", "eu.example.c"]);
    let pending: Vec<String> = std::iter::from_fn(|| rig.rx.try_recv().ok())
        .filter_map(|e| match e {
            InstallEvent::Step { pkg, step: InstallStep::Pending } => Some(pkg),
            _ => None,
        })
        .collect();
    assert_eq!(pending, vec!["eu.example.a", "eu.example.b", "eu.example.c"], "one Pending per package, in order");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_that_could_climb_out_of_the_cache_is_refused() {
    let bytes = package(PKG, 7);
    let server = serve(vec![Reply::Ok(bytes.clone())]).await;
    let rig = new_rig(|_| {});
    for pkg in ["../escape", "a/b", "", ".hidden"] {
        let request = request_for(pkg, &bytes, &key(1), &server.url);
        assert!(matches!(rig.installer.install(request), Err(InstallError::InvalidRequest(_))), "{pkg:?}");
    }
    assert!(rig.rx.try_recv().is_err());
}

// ---- install rules end to end ----

#[tokio::test(flavor = "multi_thread")]
async fn a_downgrade_is_refused_and_the_installed_file_is_untouched() {
    let rig = new_rig(|_| {});
    let before = put_installed(&rig.store_dir(), PKG, 5, &key(1));
    let bytes = package(PKG, 4);
    let server = serve(vec![Reply::Ok(bytes.clone())]).await;
    rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
    let events = rig.until_done(PKG).await;
    assert_eq!(
        steps(&events),
        vec![InstallStep::Pending, InstallStep::Downloading, InstallStep::Installing, InstallStep::Error]
    );
    assert_eq!(std::fs::read(rig.store_dir().join(format!("{PKG}.kuroext"))).unwrap(), before);
    assert_eq!(Rig::files(&rig.store_dir()), vec![format!("{PKG}.kuroext")]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_same_version_code_reinstalls_and_a_newer_one_replaces() {
    let rig = new_rig(|_| {});
    put_installed(&rig.store_dir(), PKG, 5, &key(1));
    for code in [5, 6] {
        let bytes = package(PKG, code);
        let server = serve(vec![Reply::Ok(bytes.clone())]).await;
        rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
        assert_eq!(*steps(&rig.until_done(PKG).await).last().unwrap(), InstallStep::Installed, "code {code}");
        let now = crate::package::read_manifest(&std::fs::read(rig.store_dir().join(format!("{PKG}.kuroext"))).unwrap());
        assert_eq!(now.unwrap().version_code, code);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_signer_is_refused_and_the_installed_file_is_untouched() {
    let rig = new_rig(|_| {});
    let before = put_installed(&rig.store_dir(), PKG, 5, &key(1));
    let bytes = package(PKG, 6);
    let server = serve(vec![Reply::Ok(bytes.clone())]).await;
    // Validly signed, but by another key (with its own repo fingerprint).
    rig.installer.install(request_for(PKG, &bytes, &key(2), &server.url)).unwrap();
    let events = rig.until_done(PKG).await;
    assert_eq!(*steps(&events).last().unwrap(), InstallStep::Error);
    assert!(steps(&events).contains(&InstallStep::Installing));
    assert_eq!(std::fs::read(rig.store_dir().join(format!("{PKG}.kuroext"))).unwrap(), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_commit_keeps_the_old_file_and_leaves_no_part() {
    let rig = new_rig(|_| {});
    let before = put_installed(&rig.store_dir(), PKG, 5, &key(1));
    // A directory where the .part file must go makes the write fail, like a full disk would.
    std::fs::create_dir(rig.store_dir().join(format!("{PKG}.kuroext.part"))).unwrap();
    let bytes = package(PKG, 6);
    let server = serve(vec![Reply::Ok(bytes.clone())]).await;
    rig.installer.install(request_for(PKG, &bytes, &key(1), &server.url)).unwrap();
    assert_eq!(*steps(&rig.until_done(PKG).await).last().unwrap(), InstallStep::Error);
    assert_eq!(std::fs::read(rig.store_dir().join(format!("{PKG}.kuroext"))).unwrap(), before);
    assert!(Rig::files(&rig.download_dir()).is_empty());
}
