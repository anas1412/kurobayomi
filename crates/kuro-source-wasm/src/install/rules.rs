//! Install rules (T-18a-037, edge cases I1 to I3): a lower version code is a downgrade, an equal one is a reinstall,
//! and the signer may not change. The check runs before anything is committed.

use std::path::Path;

use super::InstallError;
use crate::package::read_manifest;

/// What the store already holds for a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledInfo {
    pub version_code: i64,
    pub signer: [u8; 32],
}

/// Aniyomi checks the version first, then the signature (`MangaExtensionLoader.kt`).
pub fn check_install(
    installed: Option<&InstalledInfo>,
    version_code: i64,
    signer: &[u8; 32],
) -> Result<(), InstallError> {
    let Some(installed) = installed else { return Ok(()) };
    if version_code < installed.version_code {
        return Err(InstallError::Downgrade);
    }
    if &installed.signer != signer {
        return Err(InstallError::SignerMismatch);
    }
    Ok(())
}

/// The version and signer of `<store>/<pkg>.kuroext`. A missing file is `None`. A file that cannot be read, or that
/// has no `signerPub`, is `None` too, so a damaged file never blocks its own replacement (logged).
pub fn read_installed(store_dir: &Path, pkg: &str) -> Option<InstalledInfo> {
    let bytes = std::fs::read(store_dir.join(format!("{pkg}.kuroext"))).ok()?;
    let manifest = match read_manifest(&bytes) {
        Ok(manifest) => manifest,
        Err(err) => {
            tracing::warn!(pkg, "installed package is unreadable and will be replaced: {err}");
            return None;
        }
    };
    let Some(signer) = manifest.signer_pub else {
        tracing::warn!(pkg, "installed package has no signer and will be replaced");
        return None;
    };
    Some(InstalledInfo { version_code: manifest.version_code, signer })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIGNER: [u8; 32] = [1; 32];
    const INSTALLED: InstalledInfo = InstalledInfo { version_code: 5, signer: SIGNER };

    #[test]
    fn lower_is_a_downgrade_equal_and_higher_are_accepted() {
        assert!(matches!(check_install(Some(&INSTALLED), 4, &SIGNER), Err(InstallError::Downgrade)));
        assert!(check_install(Some(&INSTALLED), 5, &SIGNER).is_ok());
        assert!(check_install(Some(&INSTALLED), 6, &SIGNER).is_ok());
    }

    #[test]
    fn a_different_signer_is_refused() {
        assert!(matches!(check_install(Some(&INSTALLED), 6, &[2; 32]), Err(InstallError::SignerMismatch)));
    }

    #[test]
    fn the_version_is_checked_before_the_signer() {
        assert!(matches!(check_install(Some(&INSTALLED), 4, &[2; 32]), Err(InstallError::Downgrade)));
    }

    #[test]
    fn nothing_installed_accepts_anything() {
        assert!(check_install(None, 0, &SIGNER).is_ok());
    }

    #[test]
    fn the_refusals_carry_the_kotlin_log_text() {
        assert_eq!(
            InstallError::Downgrade.to_string(),
            "Installed extension version is higher. Downgrading is not allowed."
        );
        assert_eq!(InstallError::SignerMismatch.to_string(), "Installed extension signature is not matched.");
    }
}
