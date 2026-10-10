//! Edge cases 12.6 (T-18a-044): package verification, placement and the install queue (section 2, I1 to I23). The
//! pipeline is `package::verify`, `install::place`, `install::download` and `install::{InstallTracker, queue}` of
//! `kuro-extensions::anime`. Differences from the manga plan: a request for a package that is already queued or
//! running replaces it in the queue (T-20a-030, Aniyomi `downloadAndInstall`), while `InstallTracker::begin` is the
//! join of I19.

mod common;

use std::fs;
use std::path::Path;
use std::time::Duration;

use common::{PKG, key, manifest, signed, signer_hex};
use kuro_extensions::anime::install::download::{DownloadError, download};
use kuro_extensions::anime::install::place::{PlaceError, place};
use kuro_extensions::anime::install::queue::InstallQueue;
use kuro_extensions::anime::install::{Begin, InstallTracker};
use kuro_extensions::anime::model::{InstallStep, SignerDigest};
use kuro_extensions::anime::package::verify::{VerifyError, verify_download};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn store_key() -> String {
    signer_hex(&key(7))
}

fn place_ok(root: &Path, source: &Path) -> Result<std::path::PathBuf, PlaceError> {
    place(root, source, PKG, &store_key(), &SignerDigest(store_key()))
}

/// `s + L` over little-endian bytes, the group order L being what a malleable signature adds to its scalar.
fn add_group_order(scalar: &mut [u8]) {
    const L: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0x10,
    ];
    let mut carry = 0u16;
    for (byte, order) in scalar.iter_mut().zip(L) {
        let sum = u16::from(*byte) + u16::from(order) + carry;
        *byte = sum as u8;
        carry = sum >> 8;
    }
}

#[test]
fn verify_accepts_valid_signature() {
    let dir = tempfile::tempdir().unwrap();
    let package = signed(&key(7), &manifest("1.0.7", 7), b"\0asm");
    let path = write(dir.path(), "dl.part", &package.bytes);
    let verified = verify_download(&path, &package.reference(), &store_key()).expect("verifies");
    assert_eq!(verified.signer, SignerDigest(store_key()));
}

#[test]
fn verify_rejects_digest_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let package = signed(&key(7), &manifest("1.0.7", 7), b"\0asm");
    let mut tampered = package.bytes.clone();
    tampered.push(0);
    let path = write(dir.path(), "dl.part", &tampered);
    assert_eq!(verify_download(&path, &package.reference(), &store_key()), Err(VerifyError::DigestMismatch));
}

#[test]
fn verify_rejects_malleable_signature() {
    let dir = tempfile::tempdir().unwrap();
    let package = signed(&key(7), &manifest("1.0.7", 7), b"\0asm");
    let path = write(dir.path(), "dl.part", &package.bytes);
    let mut reference = package.reference();
    add_group_order(&mut reference.ed25519[32..]);
    assert_eq!(verify_download(&path, &reference, &store_key()), Err(VerifyError::BadSignature), "S + L is the same signature, refused");
}

#[test]
fn verify_rejects_another_signer_and_a_bad_signature() {
    let dir = tempfile::tempdir().unwrap();
    let package = signed(&key(9), &manifest("1.0.7", 7), b"\0asm");
    let path = write(dir.path(), "dl.part", &package.bytes);
    assert!(matches!(verify_download(&path, &package.reference(), &store_key()), Err(VerifyError::UntrustedSigner(_))));
    let mut reference = signed(&key(7), &manifest("1.0.7", 7), b"\0asm").reference();
    let package = signed(&key(7), &manifest("1.0.7", 7), b"\0asm");
    reference.ed25519[0] ^= 0xff;
    let path = write(dir.path(), "dl2.part", &package.bytes);
    assert_eq!(verify_download(&path, &reference, &store_key()), Err(VerifyError::BadSignature));
}

#[test]
fn check_install_downgrade_and_equal() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("anime");
    let newer = signed(&key(7), &manifest("1.0.8", 8), b"\0asm");
    let older = signed(&key(7), &manifest("1.0.6", 6), b"\0asm");
    place_ok(&root, &write(work.path(), "a.part", &newer.bytes)).unwrap();
    assert_eq!(place_ok(&root, &write(work.path(), "b.part", &older.bytes)), Err(PlaceError::Downgrade), "I1");
    assert_eq!(fs::read(root.join(format!("{PKG}.kuroext"))).unwrap(), newer.bytes, "the installed file is kept");
    assert!(place_ok(&root, &write(work.path(), "c.part", &newer.bytes)).is_ok(), "I2: an equal version reinstalls");
}

#[test]
fn check_install_signer_mismatch() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("anime");
    place_ok(&root, &write(work.path(), "a.part", &signed(&key(7), &manifest("1.0.7", 7), b"\0asm").bytes)).unwrap();
    let other = signer_hex(&key(9));
    let upgrade = signed(&key(9), &manifest("1.0.8", 8), b"\0asm");
    let result = place(&root, &write(work.path(), "b.part", &upgrade.bytes), PKG, &other, &SignerDigest(other.clone()));
    assert_eq!(result, Err(PlaceError::SignerChanged), "I3");
}

#[test]
fn a_package_for_another_pkg_is_refused() {
    let work = tempfile::tempdir().unwrap();
    let package = signed(&key(7), &manifest("1.0.7", 7), b"\0asm");
    let source = write(work.path(), "a.part", &package.bytes);
    let result = place(&work.path().join("anime"), &source, "eu.other.pkg", &store_key(), &SignerDigest(store_key()));
    assert!(matches!(result, Err(PlaceError::PkgMismatch { .. })), "I12");
}

#[test]
fn commit_is_atomic_and_readonly() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("anime");
    let first = signed(&key(7), &manifest("1.0.7", 7), b"\0asm");
    let placed = place_ok(&root, &write(work.path(), "a.part", &first.bytes)).unwrap();
    assert!(fs::metadata(&placed).unwrap().permissions().readonly(), "the package is read-only");
    assert_eq!(fs::read_to_string(root.join(format!("{PKG}.signer"))).unwrap(), store_key());

    // a failed write (the part path is a directory) leaves the old package and its sidecar as they were
    fs::create_dir(root.join(format!("{PKG}.kuroext.part"))).unwrap();
    let upgrade = signed(&key(7), &manifest("1.0.8", 8), b"\0asm");
    let failed = place_ok(&root, &write(work.path(), "b.part", &upgrade.bytes));
    assert!(matches!(failed, Err(PlaceError::Io(_))), "{failed:?}");
    assert_eq!(fs::read(&placed).unwrap(), first.bytes, "the old file survives the failed commit");
    assert_eq!(fs::read_to_string(root.join(format!("{PKG}.signer"))).unwrap(), store_key());
}

#[test]
fn cancel_queued_sets_idle() {
    let (mut tracker, mut queue) = (InstallTracker::new(), InstallQueue::new());
    let ticket = queue.request(&mut tracker, "p.a");
    assert_eq!(tracker.step("p.a"), Some(InstallStep::Pending));
    assert!(queue.cancel(&mut tracker, "p.a"), "I16");
    assert_eq!((tracker.step("p.a"), ticket.cancel.is_cancelled(), queue.waiting().len()), (Some(InstallStep::Idle), true, 0));
    assert!(!queue.cancel(&mut tracker, "p.unknown"), "nothing to cancel");
}

#[tokio::test]
async fn cancel_downloading_removes_part() {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1u8; 16]).set_delay(Duration::from_secs(30)))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join(format!("{PKG}.kuroext.part"));
    fs::write(&part, b"left over").unwrap();
    let cancel = CancellationToken::new();
    let stopper = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        stopper.cancel();
    });
    let result = download(&reqwest::Client::new(), &format!("{}/p.kuroext", server.uri()), &part, &cancel, |_| {}).await;
    assert_eq!(result, Err(DownloadError::Cancelled), "I17");
    assert!(!part.exists(), "the partial file is removed");
}

#[test]
fn same_pkg_joins_active_job() {
    let mut tracker = InstallTracker::new();
    let Begin::Started(ticket) = tracker.begin("p.a") else { panic!("the first request starts a job") };
    tracker.set_step(&ticket, InstallStep::Downloading);
    assert_eq!(tracker.begin("p.a"), Begin::Running(InstallStep::Downloading), "I19: the second request joins");
    tracker.set_step(&ticket, InstallStep::Installed);
    assert!(matches!(tracker.begin("p.a"), Begin::Started(_)), "a finished job is not joined");
}

#[test]
fn the_queue_replaces_a_second_request_for_the_same_pkg() {
    let (mut tracker, mut queue) = (InstallTracker::new(), InstallQueue::new());
    let first = queue.request(&mut tracker, "p.a");
    let second = queue.request(&mut tracker, "p.a");
    assert!(first.cancel.is_cancelled() && !second.cancel.is_cancelled(), "T-20a-030: one live install per package");
    assert_eq!(queue.waiting(), ["p.a"]);
}
