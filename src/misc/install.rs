//! How this copy of the app was installed (T-35a-055). A package-managed copy hides the in-app download, because the
//! package manager owns updates; an AppImage or any other copy keeps it.
//!
//! Decision: the classifier lives in `misc`, not `kuro-platform::install_kind()` as the plan names it, because the
//! platform crate is outside this lane. It takes the executable path and the `APPIMAGE` variable as inputs, so the
//! rule is testable with fixture paths. The update screen and About row are in other lanes and call these helpers.

#![allow(dead_code)] // the update screen and About row call these from the settings lane

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    PackageManaged,
    AppImage,
    Other,
}

/// Folders where a system package manager puts executables.
const PACKAGE_FOLDERS: [&str; 4] = ["/usr/bin", "/usr/sbin", "/bin", "/sbin"];

/// Classifies an executable. `appimage_env` is the `APPIMAGE` variable the AppImage runtime sets; an empty value does not
/// count. Anything outside the package folders, such as `/usr/local/bin` or a home folder, is `Other`.
pub fn classify(executable: &Path, appimage_env: Option<&str>) -> InstallKind {
    if appimage_env.is_some_and(|value| !value.trim().is_empty()) {
        return InstallKind::AppImage;
    }
    if PACKAGE_FOLDERS.iter().any(|folder| executable.starts_with(folder)) {
        InstallKind::PackageManaged
    } else {
        InstallKind::Other
    }
}

/// The install kind of the running app.
pub fn current() -> InstallKind {
    let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::new());
    classify(&executable, std::env::var("APPIMAGE").ok().as_deref())
}

/// Whether the update screen offers the in-app download (T-35a-055).
pub fn download_offered(kind: InstallKind) -> bool {
    kind != InstallKind::PackageManaged
}

/// The About row subtitle of an update check (T-35a-055). `None` keeps the row's default.
pub fn about_update_subtitle(kind: InstallKind) -> Option<&'static str> {
    (kind == InstallKind::PackageManaged).then_some("Update with your package manager")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_package_installed_executable_is_package_managed() {
        // T-35a-055
        assert_eq!(classify(Path::new("/usr/bin/kurobayomi"), None), InstallKind::PackageManaged);
        assert_eq!(classify(Path::new("/bin/kurobayomi"), None), InstallKind::PackageManaged);
    }

    #[test]
    fn an_appimage_is_detected_from_the_runtime_variable() {
        assert_eq!(classify(Path::new("/tmp/.mount_KurobX/usr/bin/kurobayomi"), Some("/home/u/Kurobayomi.AppImage")), InstallKind::AppImage);
    }

    #[test]
    fn an_empty_appimage_variable_does_not_count() {
        assert_eq!(classify(Path::new("/home/u/bin/kurobayomi"), Some("  ")), InstallKind::Other);
    }

    #[test]
    fn a_manual_copy_in_a_home_or_local_folder_is_other() {
        assert_eq!(classify(Path::new("/home/u/bin/kurobayomi"), None), InstallKind::Other);
        assert_eq!(classify(Path::new("/usr/local/bin/kurobayomi"), None), InstallKind::Other);
    }

    #[test]
    fn a_package_managed_copy_hides_the_download_and_says_so_in_about() {
        assert!(!download_offered(InstallKind::PackageManaged));
        assert!(download_offered(InstallKind::AppImage));
        assert!(download_offered(InstallKind::Other));
        assert_eq!(about_update_subtitle(InstallKind::PackageManaged), Some("Update with your package manager"));
        assert_eq!(about_update_subtitle(InstallKind::Other), None);
    }
}
