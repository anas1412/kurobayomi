//! Placement of a verified package (T-19a-033, `p1-rust-design.md` section 6.4). It refuses a downgrade and a signer
//! change, then writes `<pkg>.kuroext.part`, syncs it, renames it into place, makes it read-only, and writes the
//! `<pkg>.signer` sidecar. A failure removes the temporary file, and the previous package is kept. A package file that
//! is already there but whose version or signer cannot be read is not a fresh install: the check fails closed.

use std::fs::{self, File};
use std::io;
use std::path::Path;

use crate::anime::loader::{SIGNER_EXT, read_signer};
use crate::anime::logs;
use crate::anime::model::{LoadError, SignerDigest};
use crate::anime::package;
use crate::anime::store::map::derive_lang;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlaceError {
    #[error("Installed extension version is higher. Downgrading is not allowed.")]
    Downgrade,
    #[error("Installed extension signature is not matched.")]
    SignerChanged,
    #[error("the installed package cannot be read, so its version and signer cannot be checked")]
    InstalledUnreadable,
    #[error("the package is for {found}, but the store lists {expected}")]
    PkgMismatch { expected: String, found: String },
    #[error("the package language does not match its sources")]
    LangMismatch,
    #[error("the package is not signed by the store's key")]
    SignerNotStore,
    #[error("the package is not valid: {0}")]
    Package(LoadError),
    #[error("could not write the package: {0}")]
    Io(String),
}

/// Places the verified package at `source` as `<root>/<pkg>.kuroext`. Returns the final path.
pub fn place(
    root: &Path,
    source: &Path,
    pkg: &str,
    store_signing_key: &str,
    signer: &SignerDigest,
) -> Result<std::path::PathBuf, PlaceError> {
    let package = package::open(source).map_err(PlaceError::Package)?;
    let manifest = &package.manifest;
    if manifest.pkg != pkg {
        return Err(PlaceError::PkgMismatch { expected: pkg.to_owned(), found: manifest.pkg.clone() });
    }
    if derive_lang(manifest.sources.iter().map(|source| source.lang.as_str())) != manifest.lang {
        return Err(PlaceError::LangMismatch);
    }
    if signer.0 != store_signing_key {
        return Err(PlaceError::SignerNotStore);
    }
    if let Some((current_signer, current_code)) = installed_identity(root, pkg)? {
        if manifest.version_code < current_code {
            tracing::error!(pkg = %pkg, version_code = manifest.version_code, "{}", logs::DOWNGRADE);
            return Err(PlaceError::Downgrade);
        }
        if current_signer != *signer {
            tracing::error!(pkg = %pkg, version_code = manifest.version_code, "{}", logs::SIGNATURE_NOT_MATCHED);
            return Err(PlaceError::SignerChanged);
        }
    }
    let target = root.join(format!("{pkg}.kuroext"));
    let part = root.join(format!("{pkg}.kuroext.part"));
    let written = write_atomically(source, &part, &target, root, pkg, signer);
    if let Err(err) = written {
        tracing::error!(pkg = %pkg, error = %err, "{}", logs::COPY_FAILED);
        let _ = fs::remove_file(&part);
        return Err(PlaceError::Io(err.to_string()));
    }
    Ok(target)
}

fn write_atomically(
    source: &Path,
    part: &Path,
    target: &Path,
    root: &Path,
    pkg: &str,
    signer: &SignerDigest,
) -> io::Result<()> {
    fs::create_dir_all(root)?;
    fs::copy(source, part)?;
    File::open(part)?.sync_all()?;
    fs::rename(part, target)?;
    let mut perms = fs::metadata(target)?.permissions();
    perms.set_readonly(true);
    fs::set_permissions(target, perms)?;
    let sidecar = root.join(format!("{pkg}.{SIGNER_EXT}"));
    let sidecar_part = root.join(format!("{pkg}.{SIGNER_EXT}.part"));
    fs::write(&sidecar_part, &signer.0)?;
    File::open(&sidecar_part)?.sync_all()?;
    fs::rename(&sidecar_part, &sidecar)?;
    sync_dir(root);
    Ok(())
}

/// Syncs the directory so the renames survive a crash. Some platforms cannot open a directory for sync, and there the
/// rename is already durable enough, so the error is ignored.
fn sync_dir(dir: &Path) {
    let _ = File::open(dir).and_then(|handle| handle.sync_all());
}

/// The signer and version code of the package already installed for `pkg`. `None` means no file is there, which is a
/// fresh install. A file that is there but cannot be opened, or whose `<pkg>.signer` sidecar is missing or invalid (a
/// crash between the package rename and the sidecar write leaves that), is `InstalledUnreadable`: placing over it
/// would skip the downgrade and signer checks. The package has to be removed first.
fn installed_identity(root: &Path, pkg: &str) -> Result<Option<(SignerDigest, i64)>, PlaceError> {
    let path = root.join(format!("{pkg}.kuroext"));
    // A path that cannot exist (a parent that is a file) holds no package either.
    let absent = |kind: io::ErrorKind| matches!(kind, io::ErrorKind::NotFound | io::ErrorKind::NotADirectory);
    if matches!(fs::symlink_metadata(&path), Err(err) if absent(err.kind())) {
        return Ok(None);
    }
    let unreadable = || {
        tracing::error!(pkg, "The installed extension cannot be read, so its version and signer are not checked.");
        PlaceError::InstalledUnreadable
    };
    let version_code = package::open(&path).map_err(|_| unreadable())?.manifest.version_code;
    let signer = read_signer(&path.with_extension(SIGNER_EXT)).ok_or_else(unreadable)?;
    Ok(Some((signer, version_code)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::testkit::{
        Fixture, MANIFEST, fixture_key, manifest_version, other_key, signed_package, signer_hex,
    };

    /// A verified download of `fixture` in `dir`. Placement reads it from there.
    fn download_of(dir: &Path, name: &str, fixture: &Fixture) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, &fixture.bytes).unwrap();
        path
    }

    const PKG: &str = crate::anime::testkit::PKG;

    fn key() -> String {
        signer_hex(&fixture_key())
    }

    #[test]
    fn a_fresh_install_writes_the_package_read_only_with_its_sidecar() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let source = download_of(work.path(), "dl.part", &fixture);
        let placed = place(&root, &source, PKG, &key(), &SignerDigest(key())).expect("places");
        assert_eq!(fs::read(&placed).unwrap(), fixture.bytes);
        assert!(fs::metadata(&placed).unwrap().permissions().readonly());
        assert_eq!(fs::read_to_string(root.join(format!("{PKG}.signer"))).unwrap(), key());
        assert!(!root.join(format!("{PKG}.kuroext.part")).exists(), "no part file is left");
    }

    #[test]
    fn a_downgrade_is_refused_and_the_installed_file_is_kept() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let newer = signed_package(&fixture_key(), &manifest_version("1.0.8", 8), b"\0asm");
        let older = signed_package(&fixture_key(), &manifest_version("1.0.6", 6), b"\0asm");
        let first = download_of(work.path(), "a.part", &newer);
        place(&root, &first, PKG, &key(), &SignerDigest(key())).unwrap();
        let second = download_of(work.path(), "b.part", &older);
        let result = place(&root, &second, PKG, &key(), &SignerDigest(key()));
        assert_eq!(result, Err(PlaceError::Downgrade));
        assert_eq!(fs::read(root.join(format!("{PKG}.kuroext"))).unwrap(), newer.bytes);
    }

    #[test]
    fn an_equal_version_reinstall_is_allowed() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let source = download_of(work.path(), "a.part", &fixture);
        place(&root, &source, PKG, &key(), &SignerDigest(key())).unwrap();
        assert!(place(&root, &source, PKG, &key(), &SignerDigest(key())).is_ok());
    }

    #[test]
    fn an_upgrade_signed_by_another_key_is_refused() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let first = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let source = download_of(work.path(), "a.part", &first);
        place(&root, &source, PKG, &key(), &SignerDigest(key())).unwrap();
        let other = signer_hex(&other_key());
        let upgrade = signed_package(&other_key(), &manifest_version("1.0.8", 8), b"\0asm");
        let source = download_of(work.path(), "b.part", &upgrade);
        let result = place(&root, &source, PKG, &other, &SignerDigest(other.clone()));
        assert_eq!(result, Err(PlaceError::SignerChanged));
    }

    #[test]
    fn a_first_install_must_be_signed_by_the_store() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let fixture = signed_package(&other_key(), MANIFEST, b"\0asm");
        let source = download_of(work.path(), "a.part", &fixture);
        let result = place(&root, &source, PKG, &key(), &SignerDigest(signer_hex(&other_key())));
        assert_eq!(result, Err(PlaceError::SignerNotStore));
        assert!(!root.join(format!("{PKG}.kuroext")).exists());
    }

    #[test]
    fn a_manifest_for_another_package_is_refused() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let source = download_of(work.path(), "a.part", &fixture);
        let result = place(&root, &source, "eu.other.pkg", &key(), &SignerDigest(key()));
        assert!(matches!(result, Err(PlaceError::PkgMismatch { .. })), "{result:?}");
    }

    #[test]
    fn a_language_that_differs_from_the_sources_is_refused() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let manifest = MANIFEST.replace("\"lang\": \"en\", \"name\": \"Demo\"", "\"lang\": \"ja\", \"name\": \"Demo\"");
        assert_ne!(manifest, MANIFEST, "the source language was changed");
        let fixture = signed_package(&fixture_key(), &manifest, b"\0asm");
        let source = download_of(work.path(), "a.part", &fixture);
        let result = place(&root, &source, PKG, &key(), &SignerDigest(key()));
        assert_eq!(result, Err(PlaceError::LangMismatch));
    }

    #[test]
    fn a_failed_write_removes_the_temporary_file() {
        let work = tempfile::tempdir().unwrap();
        let blocker = work.path().join("root-is-a-file");
        fs::write(&blocker, b"file").unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let source = download_of(work.path(), "a.part", &fixture);
        let result = place(&blocker, &source, PKG, &key(), &SignerDigest(key()));
        assert!(matches!(result, Err(PlaceError::Io(_))), "{result:?}");
        assert!(!blocker.join(format!("{PKG}.kuroext.part")).exists());
    }

    /// Places `fixture` as the installed package, the way a finished install leaves it.
    fn install_first(work: &Path, root: &Path, fixture: &Fixture) {
        let source = download_of(work, "first.part", fixture);
        place(root, &source, PKG, &key(), &SignerDigest(key())).expect("the first install places");
    }

    #[test]
    fn a_lost_sidecar_refuses_a_lower_version_and_keeps_the_installed_file() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let installed = signed_package(&fixture_key(), &manifest_version("1.0.8", 8), b"\0asm");
        install_first(work.path(), &root, &installed);
        fs::remove_file(root.join(format!("{PKG}.signer"))).unwrap();
        let older = signed_package(&fixture_key(), &manifest_version("1.0.6", 6), b"\0asm");
        let source = download_of(work.path(), "older.part", &older);
        let result = place(&root, &source, PKG, &key(), &SignerDigest(key()));
        assert_eq!(result, Err(PlaceError::InstalledUnreadable));
        assert_eq!(fs::read(root.join(format!("{PKG}.kuroext"))).unwrap(), installed.bytes);
        assert!(!root.join(format!("{PKG}.kuroext.part")).exists());
    }

    #[test]
    fn a_lost_sidecar_refuses_another_signer_and_even_the_same_one() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let installed = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        install_first(work.path(), &root, &installed);
        fs::remove_file(root.join(format!("{PKG}.signer"))).unwrap();
        let other = signer_hex(&other_key());
        let upgrade = signed_package(&other_key(), &manifest_version("1.0.8", 8), b"\0asm");
        let foreign = download_of(work.path(), "foreign.part", &upgrade);
        let result = place(&root, &foreign, PKG, &other, &SignerDigest(other.clone()));
        assert_eq!(result, Err(PlaceError::InstalledUnreadable), "another store's package cannot take over");
        let same = download_of(work.path(), "same.part", &installed);
        let result = place(&root, &same, PKG, &key(), &SignerDigest(key()));
        assert_eq!(result, Err(PlaceError::InstalledUnreadable), "the check cannot pass without an identity");
        assert_eq!(fs::read(root.join(format!("{PKG}.kuroext"))).unwrap(), installed.bytes);
    }

    #[test]
    fn an_invalid_sidecar_or_an_unreadable_package_file_also_refuses() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        let installed = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        install_first(work.path(), &root, &installed);
        let sidecar = root.join(format!("{PKG}.signer"));
        fs::write(&sidecar, "not a digest").unwrap();
        let source = download_of(work.path(), "again.part", &installed);
        assert_eq!(place(&root, &source, PKG, &key(), &SignerDigest(key())), Err(PlaceError::InstalledUnreadable));
        fs::write(&sidecar, key()).unwrap();
        let target = root.join(format!("{PKG}.kuroext"));
        fs::remove_file(&target).unwrap();
        fs::write(&target, b"not a zip").unwrap();
        assert_eq!(place(&root, &source, PKG, &key(), &SignerDigest(key())), Err(PlaceError::InstalledUnreadable));
        assert_eq!(fs::read(&target).unwrap(), b"not a zip", "the unreadable file is left for the user to remove");
    }

    /// The one event `place` logged, checked against the text, level and `pkg`/`version_code` fields.
    fn assert_logged_once(logged: &[crate::anime::testkit::Logged], text: &str, version_code: Option<&str>) {
        assert_eq!(logged.len(), 1, "{logged:?}");
        assert_eq!(logged[0].message, text);
        assert_eq!(logged[0].level, tracing::Level::ERROR);
        assert!(logged[0].fields.iter().any(|(name, value)| name == "pkg" && value == PKG));
        if let Some(code) = version_code {
            assert!(logged[0].fields.iter().any(|(name, value)| name == "version_code" && value == code));
        }
    }

    #[test]
    fn tst_19_081_a_downgrade_logs_the_kotlin_text_with_the_package_and_version() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        install_first(work.path(), &root, &signed_package(&fixture_key(), &manifest_version("1.0.8", 8), b"\0asm"));
        let older = signed_package(&fixture_key(), &manifest_version("1.0.6", 6), b"\0asm");
        let source = download_of(work.path(), "older.part", &older);
        let (result, logged) =
            crate::anime::testkit::capture(|| place(&root, &source, PKG, &key(), &SignerDigest(key())));
        assert_eq!(result, Err(PlaceError::Downgrade));
        assert_logged_once(&logged, logs::DOWNGRADE, Some("6"));
    }

    #[test]
    fn tst_19_081_a_signer_change_logs_the_kotlin_text_with_the_package_and_version() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        install_first(work.path(), &root, &signed_package(&fixture_key(), MANIFEST, b"\0asm"));
        let other = signer_hex(&other_key());
        let upgrade = signed_package(&other_key(), &manifest_version("1.0.8", 8), b"\0asm");
        let source = download_of(work.path(), "upgrade.part", &upgrade);
        let (result, logged) =
            crate::anime::testkit::capture(|| place(&root, &source, PKG, &other, &SignerDigest(other.clone())));
        assert_eq!(result, Err(PlaceError::SignerChanged));
        assert_logged_once(&logged, logs::SIGNATURE_NOT_MATCHED, Some("8"));
    }

    #[test]
    fn tst_19_081_a_failed_copy_logs_the_kotlin_text_with_the_package() {
        let work = tempfile::tempdir().unwrap();
        let blocker = work.path().join("root-is-a-file");
        fs::write(&blocker, b"file").unwrap();
        let source = download_of(work.path(), "a.part", &signed_package(&fixture_key(), MANIFEST, b"\0asm"));
        let (result, logged) =
            crate::anime::testkit::capture(|| place(&blocker, &source, PKG, &key(), &SignerDigest(key())));
        assert!(matches!(result, Err(PlaceError::Io(_))), "{result:?}");
        assert_logged_once(&logged, logs::COPY_FAILED, None);
    }

    #[test]
    fn a_stray_sidecar_without_a_package_file_is_still_a_fresh_install() {
        let work = tempfile::tempdir().unwrap();
        let root = work.path().join("anime");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(format!("{PKG}.signer")), "stale").unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let source = download_of(work.path(), "fresh.part", &fixture);
        let placed = place(&root, &source, PKG, &key(), &SignerDigest(key())).expect("places");
        assert_eq!(fs::read(placed).unwrap(), fixture.bytes);
        assert_eq!(fs::read_to_string(root.join(format!("{PKG}.signer"))).unwrap(), key());
    }
}
