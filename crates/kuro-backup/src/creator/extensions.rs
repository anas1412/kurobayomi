//! Installed extension artifacts creator, opt-in (T-21a-019). A removed artifact is skipped with a warning and the
//! rest of the backup is still written (E-16).

use crate::schema::BackupExtension;

/// Installed extension packages with their artifact bytes, anime first, then manga. A package whose artifact is gone
/// reports `None`.
pub trait ExtensionArtifacts {
    fn installed(&self) -> Vec<(String, Option<Vec<u8>>)>;
}

/// Entries for the packages whose artifact could be read, and the package ids that were skipped.
pub fn collect(source: &dyn ExtensionArtifacts) -> (Vec<BackupExtension>, Vec<String>) {
    let mut written = Vec::new();
    let mut skipped = Vec::new();
    for (pkg_name, apk) in source.installed() {
        match apk {
            Some(apk) => written.push(BackupExtension { pkg_name, apk }),
            None => {
                tracing::warn!(pkg_name, "extension artifact missing, skipped in backup");
                skipped.push(pkg_name);
            }
        }
    }
    (written, skipped)
}
