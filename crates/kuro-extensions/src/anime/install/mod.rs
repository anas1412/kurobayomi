//! The install pipeline (T-19a-032, `p1-rust-design.md` section 6.1). One install goes Pending, Downloading, Installing,
//! then Installed, Idle (cancelled) or Error. `InstallTracker` keeps one live entry per package, so a second request
//! for a package that is already running gets the running entry back, and a stale cancel cannot touch a newer install.

pub mod download;
pub mod place;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tokio_util::sync::CancellationToken;

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
    on_step: &mut dyn FnMut(InstallStep),
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
    on_step: &mut dyn FnMut(InstallStep),
) -> Result<PathBuf, InstallError> {
    let kuroext = match (available.kind, available.kuroext.as_ref()) {
        (Kind::Wasm, Some(kuroext)) => kuroext,
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
