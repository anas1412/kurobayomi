//! The file step of one install (`InstallStep.kt` and `InstallerManga.kt`, T-20a-030). The bytes are written to a
//! temporary file, checked against the repo's sha256 and renamed into `<extensions>/<pkg>/`. A failed install leaves
//! the previous component in place. The HTTP download and the one-install-per-package rule belong to the caller.

use std::fs;
use std::path::{Path, PathBuf};

use kuro_core::hash::sha256_hex;

use crate::loader::{COMPONENT_FILE, MANIFEST_FILE};
use crate::manifest::Manifest;

/// The states of one install, as `InstallStep` names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    Idle,
    Pending,
    Downloading,
    Installing,
    Installed,
    Error,
}

impl InstallStep {
    /// `Installed`, `Error` and `Idle` are final. The other states are still running.
    pub fn is_completed(self) -> bool {
        matches!(self, Self::Installed | Self::Error | Self::Idle)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("the package name {0:?} is not a reverse-DNS id")]
    InvalidPackage(String),
    #[error("manifest.toml is not valid: {0}")]
    Manifest(String),
    #[error("the manifest names {manifest:?} but the folder is {folder:?}")]
    PackageMismatch { folder: String, manifest: String },
    #[error("the download does not match the sha256 the repo lists")]
    ChecksumMismatch,
    #[error("could not write the package: {0}")]
    Io(String),
}

/// `^[a-z0-9_]+(\.[a-z0-9_]+)+$` (`p1-topic-wasm-component-design.md`, manifest rule 2). The name becomes a folder
/// name, so anything with a separator or a dot-dot is refused.
pub fn is_valid_package(pkg: &str) -> bool {
    let segments: Vec<&str> = pkg.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|segment| {
            !segment.is_empty() && segment.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
}

/// Writes one package to `extensions_dir/<pkg>/`. The component is checked before anything is renamed into place.
pub fn install_package(
    extensions_dir: &Path,
    pkg: &str,
    component: &[u8],
    manifest: &str,
    expected_sha256: Option<&str>,
) -> Result<PathBuf, InstallError> {
    if !is_valid_package(pkg) {
        return Err(InstallError::InvalidPackage(pkg.to_owned()));
    }
    // The loader looks the package up by the manifest's `pkg`, so a manifest that names another package would be signed
    // under a name the folder does not show.
    let parsed = Manifest::parse(manifest).map_err(|err| InstallError::Manifest(err.to_string()))?;
    if parsed.pkg != pkg {
        return Err(InstallError::PackageMismatch { folder: pkg.to_owned(), manifest: parsed.pkg });
    }
    if expected_sha256.is_some_and(|expected| sha256_hex(component) != expected) {
        return Err(InstallError::ChecksumMismatch);
    }
    let tmp_dir = extensions_dir.join(".tmp");
    let dir = extensions_dir.join(pkg);
    fs::create_dir_all(&tmp_dir).and_then(|()| fs::create_dir_all(&dir)).map_err(io)?;
    let part = tmp_dir.join(format!("{pkg}.part"));
    fs::write(&part, component).map_err(io)?;
    fs::rename(&part, dir.join(COMPONENT_FILE)).map_err(io)?;
    let manifest_part = tmp_dir.join(format!("{pkg}.manifest.part"));
    fs::write(&manifest_part, manifest).map_err(io)?;
    fs::rename(&manifest_part, dir.join(MANIFEST_FILE)).map_err(io)?;
    Ok(dir)
}

fn io(err: std::io::Error) -> InstallError {
    InstallError::Io(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{LoadResult, load_package};
    use std::collections::BTreeMap;

    const PKG: &str = "app.kurobayomi.ext.en.example";
    const COMPONENT: &[u8] = b"\0asm-component-v2";

    fn manifest() -> String {
        format!("pkg = \"{PKG}\"\nname = \"Example\"\nversion = \"2.0.0\"\nversion_code = 2\napi = \"1.0\"\n")
    }

    #[test]
    fn a_checked_install_writes_the_component_and_manifest_and_leaves_no_part_file() {
        let root = tempfile::tempdir().unwrap();
        let digest = sha256_hex(COMPONENT);
        let dir = install_package(root.path(), PKG, COMPONENT, &manifest(), Some(&digest)).expect("install");
        assert_eq!(fs::read(dir.join(COMPONENT_FILE)).unwrap(), COMPONENT);
        assert_eq!(fs::read_to_string(dir.join(MANIFEST_FILE)).unwrap(), manifest());
        assert!(fs::read_dir(root.path().join(".tmp")).unwrap().next().is_none(), "no .part file is left");
    }

    #[test]
    fn an_installed_package_loads_as_verified_against_the_same_digest() {
        let root = tempfile::tempdir().unwrap();
        let dir = install_package(root.path(), PKG, COMPONENT, &manifest(), None).expect("install");
        let signed = BTreeMap::from([(PKG.to_owned(), sha256_hex(COMPONENT))]);
        assert!(matches!(load_package(&dir, &signed), LoadResult::Verified { .. }));
    }

    #[test]
    fn a_checksum_mismatch_keeps_the_previous_component() {
        let root = tempfile::tempdir().unwrap();
        install_package(root.path(), PKG, b"old", &manifest(), None).expect("first install");
        let result = install_package(root.path(), PKG, COMPONENT, &manifest(), Some(&sha256_hex(b"other")));
        assert!(matches!(result, Err(InstallError::ChecksumMismatch)));
        assert_eq!(fs::read(root.path().join(PKG).join(COMPONENT_FILE)).unwrap(), b"old");
        assert!(fs::read_dir(root.path().join(".tmp")).unwrap().next().is_none(), "nothing is left behind");
    }

    #[test]
    fn a_name_that_is_not_reverse_dns_is_refused_before_any_write() {
        let root = tempfile::tempdir().unwrap();
        for bad in ["../escape", "a/b", "single", "a..b", "A.b", "", ".hidden.pkg"] {
            let result = install_package(root.path(), bad, COMPONENT, &manifest(), None);
            assert!(matches!(result, Err(InstallError::InvalidPackage(_))), "{bad:?}");
        }
        assert!(fs::read_dir(root.path()).unwrap().next().is_none(), "nothing was written");
    }

    #[test]
    fn a_manifest_that_names_another_package_is_refused_before_any_write() {
        let root = tempfile::tempdir().unwrap();
        let result = install_package(root.path(), "app.kurobayomi.ext.en.other", COMPONENT, &manifest(), None);
        assert!(matches!(result, Err(InstallError::PackageMismatch { .. })), "{result:?}");
        assert!(fs::read_dir(root.path()).unwrap().next().is_none(), "nothing was written");
    }

    #[test]
    fn a_manifest_that_does_not_parse_is_refused_before_any_write() {
        let root = tempfile::tempdir().unwrap();
        let result = install_package(root.path(), PKG, COMPONENT, "pkg = [", None);
        assert!(matches!(result, Err(InstallError::Manifest(_))), "{result:?}");
        assert!(fs::read_dir(root.path()).unwrap().next().is_none(), "nothing was written");
    }

    #[test]
    fn only_the_final_states_count_as_completed() {
        assert!(InstallStep::Installed.is_completed());
        assert!(InstallStep::Error.is_completed());
        assert!(InstallStep::Idle.is_completed());
        for running in [InstallStep::Pending, InstallStep::Downloading, InstallStep::Installing] {
            assert!(!running.is_completed(), "{running:?}");
        }
    }
}
