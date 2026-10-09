//! The install pipeline (T-19a-032, `p1-rust-design.md` section 6.1). One install goes Pending, Downloading, Installing,
//! then Installed, Idle (cancelled) or Error. `InstallTracker` keeps one live entry per package, so a second request
//! for a package that is already running gets the running entry back, and a stale cancel cannot touch a newer install.

pub mod download;
pub mod place;
pub mod queue;

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use tokio_util::sync::CancellationToken;

use crate::anime::loader::{PACKAGE_EXT, SIGNER_EXT};
use crate::anime::logs;
use crate::anime::model::{Available, InstallStep, Kind};
use crate::anime::package::manifest::valid_pkg;
use crate::anime::package::verify::{VerifyError, verify_download};
use download::{DownloadError, download};
use place::{PlaceError, place};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    #[error("this package cannot be installed: {0}")]
    Refused(String),
    #[error("No store lists this extension any more")]
    NotListed,
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Verify(#[from] VerifyError),
    #[error(transparent)]
    Place(#[from] PlaceError),
    #[error("the install was cancelled")]
    Cancelled,
}

/// Where an install writes: the package directory and the download folder.
#[derive(Debug, Clone, Copy)]
pub struct InstallDirs<'a> {
    pub package_dir: &'a Path,
    pub download_dir: &'a Path,
}

/// Installs one listed package. `on_step` sees every state change, and the last one is the outcome. The placement
/// lock keeps two installs from writing the package directory at once.
pub async fn install(
    client: &reqwest::Client,
    dirs: InstallDirs<'_>,
    available: &Available,
    store_signing_key: &str,
    placement_lock: &tokio::sync::Mutex<()>,
    cancel: &CancellationToken,
    on_step: &mut (dyn FnMut(InstallStep) + Send),
) -> Result<PathBuf, InstallError> {
    on_step(InstallStep::Pending);
    let outcome = run(client, dirs, available, store_signing_key, placement_lock, cancel, on_step).await;
    match &outcome {
        Ok(_) => on_step(InstallStep::Installed),
        Err(InstallError::Cancelled) => on_step(InstallStep::Idle),
        Err(_) => on_step(InstallStep::Error),
    }
    outcome
}

async fn run(
    client: &reqwest::Client,
    dirs: InstallDirs<'_>,
    available: &Available,
    store_signing_key: &str,
    placement_lock: &tokio::sync::Mutex<()>,
    cancel: &CancellationToken,
    on_step: &mut (dyn FnMut(InstallStep) + Send),
) -> Result<PathBuf, InstallError> {
    let kuroext = match (available.kind, available.kuroext.as_ref()) {
        (Kind::Wasm, Some(kuroext)) => kuroext,
        (Kind::Unsigned, _) => {
            tracing::error!(pkg = %available.pkg, store = %available.store.index_url, "{}", logs::NOT_SIGNED_FOR_INSTALL);
            return Err(InstallError::Refused("the store entry is not signed for this host".into()));
        }
        _ => return Err(InstallError::Refused("the store entry is not signed for this host".into())),
    };
    if !valid_pkg(&available.pkg) {
        return Err(InstallError::Refused(format!("{:?} is not a valid package name", available.pkg)));
    }
    let part = dirs.download_dir.join(format!("{}-{}.kuroext.part", available.pkg, available.version_code));
    on_step(InstallStep::Downloading);
    match download(client, &kuroext.url, &part, cancel, |_| {}).await {
        Ok(()) => {}
        Err(DownloadError::Cancelled) => return Err(InstallError::Cancelled),
        Err(err) => return Err(err.into()),
    }
    on_step(InstallStep::Installing);
    let checked = {
        let (path, expect, key) = (part.clone(), kuroext.clone(), store_signing_key.to_owned());
        tokio::task::spawn_blocking(move || verify_download(&path, &expect, &key))
            .await
            .map_err(|err| VerifyError::Io(err.to_string()))?
    };
    let verified = match checked {
        Ok(verified) => verified,
        Err(err) => {
            if matches!(err, VerifyError::Io(_)) {
                tracing::error!(pkg = %available.pkg, store = %available.store.index_url, "{}", logs::READ_DOWNLOADED_FAILED);
            }
            let _ = tokio::fs::remove_file(&part).await;
            return Err(err.into());
        }
    };
    let _placement = placement_lock.lock().await;
    if cancel.is_cancelled() {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(InstallError::Cancelled);
    }
    let (root, source, pkg, key) =
        (dirs.package_dir.to_path_buf(), part.clone(), available.pkg.clone(), store_signing_key.to_owned());
    let placed = tokio::task::spawn_blocking(move || place(&root, &source, &pkg, &key, &verified.signer))
        .await
        .map_err(|err| PlaceError::Io(err.to_string()));
    let _ = tokio::fs::remove_file(&part).await;
    Ok(placed??)
}

/// Deletes `<pkg>.kuroext` and its `<pkg>.signer` sidecar (T-19a-036). An absent file is not an error, so a second
/// uninstall returns `Ok`. An invalid package name is refused before the disk is touched. This touches the disk, so the
/// caller runs it off the main thread and then updates the state.
pub fn uninstall(package_dir: &Path, pkg: &str) -> io::Result<()> {
    if !valid_pkg(pkg) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{pkg:?} is not a valid package name")));
    }
    for ext in [PACKAGE_EXT, SIGNER_EXT] {
        match std::fs::remove_file(package_dir.join(format!("{pkg}.{ext}"))) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

/// A request for a package the store does not list any more is refused with the page's message.
pub fn require_listed(listing: Option<&Available>) -> Result<&Available, InstallError> {
    listing.ok_or(InstallError::NotListed)
}

/// One live install of a package. Its `cancel` token stops that install and no other.
#[derive(Debug, Clone)]
pub struct Ticket {
    pub pkg: String,
    serial: u64,
    pub cancel: CancellationToken,
}

/// What `begin` answers: a new install to run, or the install already running for the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Begin {
    Started(Ticket),
    Running(InstallStep),
}

#[derive(Debug)]
struct Entry {
    serial: u64,
    step: InstallStep,
    cancel: CancellationToken,
}

/// The step of each package, keyed by the bare package name. Keys are not Browse row keys.
#[derive(Debug, Default)]
pub struct InstallTracker {
    entries: BTreeMap<String, Entry>,
    next_serial: u64,
}

impl PartialEq for Ticket {
    fn eq(&self, other: &Self) -> bool {
        self.pkg == other.pkg && self.serial == other.serial
    }
}

impl Eq for Ticket {}

impl InstallTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts an install for `pkg`, unless one is still running for it.
    pub fn begin(&mut self, pkg: &str) -> Begin {
        if let Some(entry) = self.entries.get(pkg) {
            if !entry.step.is_completed() {
                return Begin::Running(entry.step);
            }
        }
        self.next_serial += 1;
        let serial = self.next_serial;
        let cancel = CancellationToken::new();
        self.entries.insert(pkg.to_owned(), Entry { serial, step: InstallStep::Pending, cancel: cancel.clone() });
        Begin::Started(Ticket { pkg: pkg.to_owned(), serial, cancel })
    }

    /// Records a step for the install behind `ticket`. A ticket for an older install changes nothing.
    pub fn set_step(&mut self, ticket: &Ticket, step: InstallStep) {
        if let Some(entry) = self.entries.get_mut(&ticket.pkg) {
            if entry.serial == ticket.serial {
                entry.step = step;
            }
        }
    }

    /// Cancels the running install of `pkg`. Returns false when nothing is running for it.
    pub fn cancel(&self, pkg: &str) -> bool {
        match self.entries.get(pkg) {
            Some(entry) if !entry.step.is_completed() => {
                entry.cancel.cancel();
                true
            }
            _ => false,
        }
    }

    pub fn step(&self, pkg: &str) -> Option<InstallStep> {
        self.entries.get(pkg).map(|entry| entry.step)
    }

    /// Drops the step of a package whose install has finished (`uninstall` clears it, behavior 5.11). A running install
    /// keeps its entry, and this returns false.
    pub fn forget(&mut self, pkg: &str) -> bool {
        match self.entries.get(pkg) {
            Some(entry) if entry.step.is_completed() => self.entries.remove(pkg).is_some(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::model::StoreRef;
    use crate::anime::testkit::test_client;
    use crate::anime::store::map::available;
    use crate::anime::store::wire::ExtensionJson;
    use crate::anime::testkit::{
        MANIFEST, PKG, fixture_key, kuroext_block, manifest_version, other_key, signed_package, signer_hex,
        store_extension,
    };
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn key() -> String {
        signer_hex(&fixture_key())
    }

    /// Builds the listing a store would show for `fixture` served at `url`.
    fn listing(fixture: &crate::anime::testkit::Fixture, url: &str, version_code: i64, pkg: &str) -> Available {
        let entry: ExtensionJson =
            serde_json::from_value(store_extension(pkg, version_code, Some(kuroext_block(fixture, url)))).unwrap();
        let store = StoreRef { index_url: "https://example.org/index.json".into(), name: "S".into() };
        available(&entry, &store).unwrap()
    }

    async fn serve(server: &MockServer, body: Vec<u8>) {
        Mock::given(method("GET"))
            .and(path("/pkg.kuroext"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(server)
            .await;
    }

    async fn run_install(
        package_dir: &Path,
        download_dir: &Path,
        available: &Available,
        store_key: &str,
        cancel: &CancellationToken,
    ) -> (Result<PathBuf, InstallError>, Vec<InstallStep>) {
        let lock = tokio::sync::Mutex::new(());
        let mut steps = Vec::new();
        let dirs = InstallDirs { package_dir, download_dir };
        let result =
            install(&test_client(), dirs, available, store_key, &lock, cancel, &mut |step| steps.push(step))
                .await;
        (result, steps)
    }

    #[tokio::test]
    async fn a_package_name_that_escapes_the_download_folder_is_refused_before_any_write() {
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let listed = listing(&fixture, "http://127.0.0.1:9/pkg.kuroext", 7, "../x");
        let work = tempfile::tempdir().unwrap();
        let downloads = work.path().join("a").join("dl");
        let (result, _) =
            run_install(&work.path().join("anime"), &downloads, &listed, &key(), &CancellationToken::new()).await;
        assert!(matches!(result, Err(InstallError::Refused(_))), "{result:?}");
        assert!(!work.path().join("a").exists() && !work.path().join("x-7.kuroext.part").exists());
    }

    #[tokio::test]
    async fn a_good_install_goes_through_installing_and_leaves_no_part_file() {
        let server = MockServer::start().await;
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        serve(&server, fixture.bytes.clone()).await;
        let url = format!("{}/pkg.kuroext", server.uri());
        let listed = listing(&fixture, &url, 7, PKG);
        let work = tempfile::tempdir().unwrap();
        let (result, steps) = run_install(
            &work.path().join("anime"),
            &work.path().join("dl"),
            &listed,
            &key(),
            &CancellationToken::new(),
        )
        .await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(
            steps,
            [InstallStep::Pending, InstallStep::Downloading, InstallStep::Installing, InstallStep::Installed]
        );
        assert!(fs_empty(&work.path().join("dl")), "no part file is left");
    }

    #[tokio::test]
    async fn tst_19_091_placement_waits_for_the_one_placement_lock() {
        let server = MockServer::start().await;
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        serve(&server, fixture.bytes.clone()).await;
        let url = format!("{}/pkg.kuroext", server.uri());
        let listed = listing(&fixture, &url, 7, PKG);
        let work = tempfile::tempdir().unwrap();
        let (package_dir, download_dir) = (work.path().join("anime"), work.path().join("dl"));
        let target = package_dir.join(format!("{PKG}.kuroext"));
        let lock = tokio::sync::Mutex::new(());
        let dirs = InstallDirs { package_dir: &package_dir, download_dir: &download_dir };
        let cancel = CancellationToken::new();

        // Another install holds the placement lock. This one downloads and verifies, then must wait for it.
        let held = lock.lock().await;
        let blocked = tokio::time::timeout(
            std::time::Duration::from_millis(800),
            install(&test_client(), dirs, &listed, &key(), &lock, &cancel, &mut |_| {}),
        )
        .await;
        assert!(blocked.is_err(), "the placement did not wait for the lock");
        assert!(!target.exists(), "nothing is placed while the lock is held");
        drop(held);

        let placed = install(&test_client(), dirs, &listed, &key(), &lock, &cancel, &mut |_| {}).await;
        assert!(placed.is_ok(), "{placed:?}");
        assert!(target.exists());
    }

    fn fs_empty(dir: &Path) -> bool {
        std::fs::read_dir(dir).map(|mut entries| entries.next().is_none()).unwrap_or(true)
    }

    #[tokio::test]
    async fn a_digest_mismatch_is_an_error_and_leaves_no_partial_file() {
        let server = MockServer::start().await;
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let mut tampered = fixture.bytes.clone();
        tampered[12] ^= 0xff;
        serve(&server, tampered).await;
        let url = format!("{}/pkg.kuroext", server.uri());
        let listed = listing(&fixture, &url, 7, PKG);
        let work = tempfile::tempdir().unwrap();
        let (result, steps) = run_install(
            &work.path().join("anime"),
            &work.path().join("dl"),
            &listed,
            &key(),
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(result, Err(InstallError::Verify(VerifyError::DigestMismatch)));
        assert_eq!(steps.last(), Some(&InstallStep::Error));
        assert!(fs_empty(&work.path().join("dl")));
        assert!(!work.path().join("anime").join(format!("{PKG}.kuroext")).exists());
    }

    #[tokio::test]
    async fn a_package_signed_by_another_key_is_refused_at_verification() {
        let server = MockServer::start().await;
        let fixture = signed_package(&other_key(), MANIFEST, b"\0asm");
        serve(&server, fixture.bytes.clone()).await;
        let url = format!("{}/pkg.kuroext", server.uri());
        let listed = listing(&fixture, &url, 7, PKG);
        let work = tempfile::tempdir().unwrap();
        let (result, _) = run_install(
            &work.path().join("anime"),
            &work.path().join("dl"),
            &listed,
            &key(),
            &CancellationToken::new(),
        )
        .await;
        assert!(matches!(result, Err(InstallError::Verify(VerifyError::UntrustedSigner(_)))), "{result:?}");
    }

    #[tokio::test]
    async fn an_unsigned_listing_is_refused_before_any_download() {
        let server = MockServer::start().await;
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let url = format!("{}/pkg.kuroext", server.uri());
        let mut listed = listing(&fixture, &url, 7, PKG);
        listed.kind = Kind::Unsigned;
        listed.kuroext = None;
        let work = tempfile::tempdir().unwrap();
        let (result, steps) = run_install(
            &work.path().join("anime"),
            &work.path().join("dl"),
            &listed,
            &key(),
            &CancellationToken::new(),
        )
        .await;
        assert!(matches!(result, Err(InstallError::Refused(_))));
        assert_eq!(steps, [InstallStep::Pending, InstallStep::Error]);
        assert!(server.received_requests().await.unwrap_or_default().is_empty(), "nothing was downloaded");
    }

    #[tokio::test]
    async fn a_cancel_during_the_download_returns_idle_and_removes_the_part_file() {
        let server = MockServer::start().await;
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        serve(&server, fixture.bytes.clone()).await;
        let url = format!("{}/pkg.kuroext", server.uri());
        let listed = listing(&fixture, &url, 7, PKG);
        let work = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let (result, steps) =
            run_install(&work.path().join("anime"), &work.path().join("dl"), &listed, &key(), &cancel).await;
        assert_eq!(result, Err(InstallError::Cancelled));
        assert_eq!(steps.last(), Some(&InstallStep::Idle));
        assert!(fs_empty(&work.path().join("dl")));
    }

    #[tokio::test]
    async fn tst_19_075_a_cancel_while_the_body_streams_ends_idle_and_removes_the_part_file() {
        use crate::anime::testkit::{has_nonempty_file, stalled_server, wait_until};
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let (url, _held) = stalled_server(vec![7u8; 4096], vec![0u8; 4096]).await;
        let listed = listing(&fixture, &url, 7, PKG);
        let work = tempfile::tempdir().unwrap();
        let downloads = work.path().join("dl");
        let cancel = CancellationToken::new();
        let (package_dir, store_key) = (work.path().join("anime"), key());
        let ((result, steps), ()) = tokio::join!(
            async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    run_install(&package_dir, &downloads, &listed, &store_key, &cancel),
                )
                .await
                .expect("the cancel ends the install, no hang")
            },
            async {
                wait_until(|| has_nonempty_file(&downloads)).await;
                cancel.cancel();
            }
        );
        assert_eq!(result, Err(InstallError::Cancelled));
        assert_eq!(steps.last(), Some(&InstallStep::Idle));
        assert!(fs_empty(&downloads), "no .part file is left");
        assert!(!work.path().join("anime").join(format!("{PKG}.kuroext")).exists());
    }

    #[tokio::test]
    async fn tst_19_081_an_unsigned_listing_logs_the_not_signed_text_with_the_package_and_store() {
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let mut listed = listing(&fixture, "http://127.0.0.1:9/pkg.kuroext", 7, PKG);
        listed.kind = Kind::Unsigned;
        listed.kuroext = None;
        let work = tempfile::tempdir().unwrap();
        let recording = crate::anime::testkit::start_capture();
        let _ = run_install(&work.path().join("anime"), &work.path().join("dl"), &listed, &key(), &CancellationToken::new())
            .await;
        let logged: Vec<_> = recording.events().into_iter().filter(|event| event.level <= tracing::Level::WARN).collect();
        assert_eq!(logged.len(), 1, "{logged:?}");
        assert_eq!(logged[0].message, logs::NOT_SIGNED_FOR_INSTALL);
        assert_eq!(logged[0].level, tracing::Level::ERROR);
        assert!(logged[0].fields.iter().any(|(name, value)| name == "pkg" && value == PKG));
        assert!(logged[0].fields.iter().any(|(name, value)| name == "store" && value == "https://example.org/index.json"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tst_19_081_an_unreadable_download_logs_the_read_failed_text_and_removes_the_part() {
        use crate::anime::testkit::{has_nonempty_file, stalled_server, wait_until};
        use std::os::unix::fs::PermissionsExt;
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let (head, tail) = fixture.bytes.split_at(fixture.bytes.len() / 2);
        let (url, release) = stalled_server(head.to_vec(), tail.to_vec()).await;
        let listed = listing(&fixture, &url, 7, PKG);
        let work = tempfile::tempdir().unwrap();
        let downloads = work.path().join("dl");
        let cancel = CancellationToken::new();
        let (package_dir, store_key) = (work.path().join("anime"), key());
        let recording = crate::anime::testkit::start_capture();
        let ((result, _), ()) = tokio::join!(
            run_install(&package_dir, &downloads, &listed, &store_key, &cancel),
            async {
                // Take the read permission away while the body is still arriving, then let the download finish.
                wait_until(|| has_nonempty_file(&downloads)).await;
                for entry in std::fs::read_dir(&downloads).unwrap().flatten() {
                    std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o000)).unwrap();
                }
                release.send(()).unwrap();
            }
        );
        // A root process can read a 000 file, so this check needs a normal user.
        assert!(matches!(result, Err(InstallError::Verify(VerifyError::Io(_)))), "{result:?}");
        let logged = recording.events();
        let texts: Vec<_> = logged.iter().filter(|event| event.message == logs::READ_DOWNLOADED_FAILED).collect();
        assert_eq!(texts.len(), 1, "{logged:?}");
        assert_eq!(texts[0].level, tracing::Level::ERROR);
        assert!(texts[0].fields.iter().any(|(name, value)| name == "pkg" && value == PKG));
        assert!(fs_empty(&downloads), "the part file is removed");
    }

    #[test]
    fn a_second_request_for_a_running_package_gets_the_running_entry() {
        let mut tracker = InstallTracker::new();
        let first = match tracker.begin(PKG) {
            Begin::Started(ticket) => ticket,
            other => panic!("{other:?}"),
        };
        tracker.set_step(&first, InstallStep::Downloading);
        assert_eq!(tracker.begin(PKG), Begin::Running(InstallStep::Downloading));
    }

    #[test]
    fn a_stale_cancel_does_not_touch_a_newer_install_of_the_same_package() {
        let mut tracker = InstallTracker::new();
        let first = match tracker.begin(PKG) {
            Begin::Started(ticket) => ticket,
            other => panic!("{other:?}"),
        };
        assert!(tracker.cancel(PKG));
        tracker.set_step(&first, InstallStep::Idle);
        let second = match tracker.begin(PKG) {
            Begin::Started(ticket) => ticket,
            other => panic!("{other:?}"),
        };
        first.cancel.cancel();
        assert!(!second.cancel.is_cancelled(), "the newer install keeps running");
        tracker.set_step(&first, InstallStep::Error);
        assert_eq!(tracker.step(PKG), Some(InstallStep::Pending), "a stale step does not overwrite the newer one");
    }

    #[test]
    fn a_cancel_after_installed_is_a_no_op_and_forget_drops_only_a_finished_step() {
        let mut tracker = InstallTracker::new();
        let ticket = match tracker.begin(PKG) {
            Begin::Started(ticket) => ticket,
            other => panic!("{other:?}"),
        };
        tracker.set_step(&ticket, InstallStep::Installed);
        assert!(!tracker.cancel(PKG));
        assert!(!ticket.cancel.is_cancelled());
        assert_eq!(tracker.step(PKG), Some(InstallStep::Installed));
        assert!(tracker.forget(PKG));
        assert_eq!(tracker.step(PKG), None);
        assert!(!tracker.forget(PKG), "nothing left to forget");
        let running = match tracker.begin(PKG) {
            Begin::Started(ticket) => ticket,
            other => panic!("{other:?}"),
        };
        tracker.set_step(&running, InstallStep::Downloading);
        assert!(!tracker.forget(PKG), "a running install keeps its entry");
        assert_eq!(tracker.step(PKG), Some(InstallStep::Downloading));
    }

    #[test]
    fn uninstall_removes_both_files_and_a_second_uninstall_is_ok() {
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join(format!("{PKG}.kuroext")), b"x").unwrap();
        std::fs::write(work.path().join(format!("{PKG}.signer")), b"x").unwrap();
        uninstall(work.path(), PKG).unwrap();
        assert!(fs_empty(work.path()));
        uninstall(work.path(), PKG).expect("an absent package is not an error");
    }

    #[test]
    fn uninstall_refuses_a_bad_name_before_touching_the_disk() {
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("keep.kuroext"), b"x").unwrap();
        let err = uninstall(work.path(), "../keep").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(work.path().join("keep.kuroext").exists());
    }

    #[test]
    fn an_update_for_a_package_no_store_lists_is_refused_with_the_page_message() {
        assert_eq!(require_listed(None), Err(InstallError::NotListed));
        assert_eq!(InstallError::NotListed.to_string(), "No store lists this extension any more");
    }

    #[test]
    fn a_downgrade_request_is_refused_by_the_placement_step() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let newer = signed_package(&fixture_key(), &manifest_version("1.0.8", 8), b"\0asm");
        let source = work.path().join("n.part");
        std::fs::write(&source, &newer.bytes).unwrap();
        place::place(&root, &source, PKG, &key(), &crate::anime::model::SignerDigest(key())).unwrap();
        let older = signed_package(&fixture_key(), &manifest_version("1.0.6", 6), b"\0asm");
        let source = work.path().join("o.part");
        std::fs::write(&source, &older.bytes).unwrap();
        let result = place::place(&root, &source, PKG, &key(), &crate::anime::model::SignerDigest(key()));
        assert_eq!(result, Err(PlaceError::Downgrade));
    }
}
