//! Manga package icons (T-18a-011). An `icon.png` in a package directory is copied into the icon cache. The copy is
//! rewritten only when the bytes change, so each package version writes once. A missing icon, a vanished package or a
//! bad name gives `None`, and the UI draws the generic glyph.

use std::fs;
use std::path::{Path, PathBuf};

use crate::anime::package::manifest::valid_pkg;
use kuro_platform::paths::Paths;

/// `<cache>/extensions/icons`, one `<pkg>.png` per package.
pub fn icon_cache_dir(paths: &dyn Paths) -> PathBuf {
    paths.cache_dir().join("extensions").join("icons")
}

/// The cache file of `pkg`, or `None` for a name that is not a valid package name.
pub fn cached_path(icons: &Path, pkg: &str) -> Option<PathBuf> {
    valid_pkg(pkg).then(|| icons.join(format!("{pkg}.png")))
}

/// Copies `<dir>/icon.png` into the cache and returns the cache file. `None` when the package has no icon or the copy
/// fails; a failure is logged, never raised.
pub fn extract(icons: &Path, dir: &Path, pkg: &str) -> Option<PathBuf> {
    let target = cached_path(icons, pkg)?;
    let bytes = fs::read(dir.join("icon.png")).ok()?;
    if fs::read(&target).ok().as_deref() != Some(bytes.as_slice()) {
        let written = fs::create_dir_all(icons).and_then(|()| fs::write(&target, &bytes));
        if let Err(err) = written {
            tracing::warn!(pkg, error = %err, "could not cache the package icon");
            return None;
        }
    }
    Some(target)
}

/// The cached icon of `pkg`, if one exists. A package that was removed has no file, so this is `None`, not an error.
pub fn lookup(icons: &Path, pkg: &str) -> Option<PathBuf> {
    cached_path(icons, pkg).filter(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PKG: &str = "eu.example.manga";

    fn package(root: &Path, icon: Option<&[u8]>) -> PathBuf {
        let dir = root.join("pkg");
        fs::create_dir_all(&dir).unwrap();
        if let Some(bytes) = icon {
            fs::write(dir.join("icon.png"), bytes).unwrap();
        }
        dir
    }

    #[test]
    fn an_icon_in_the_package_is_copied_once_and_rewritten_only_when_it_changes() {
        let root = tempfile::tempdir().unwrap();
        let icons = root.path().join("icons");
        let dir = package(root.path(), Some(b"v1"));
        let first = extract(&icons, &dir, PKG).expect("copied");
        assert_eq!(fs::read(&first).unwrap(), b"v1");
        assert_eq!(extract(&icons, &dir, PKG), Some(first.clone()));
        fs::write(dir.join("icon.png"), b"v2").unwrap();
        extract(&icons, &dir, PKG).expect("updated");
        assert_eq!(fs::read(&first).unwrap(), b"v2");
    }

    #[test]
    fn a_package_without_an_icon_gives_none_and_a_vanished_package_has_no_cache_file() {
        let root = tempfile::tempdir().unwrap();
        let icons = root.path().join("icons");
        let dir = package(root.path(), None);
        assert_eq!(extract(&icons, &dir, PKG), None);
        assert_eq!(lookup(&icons, PKG), None);
        let dir = package(root.path(), Some(b"x"));
        let cached = extract(&icons, &dir, PKG).unwrap();
        fs::remove_file(&cached).unwrap();
        assert_eq!(lookup(&icons, PKG), None, "a removed icon is a fallback, not a panic");
    }

    #[test]
    fn a_bad_package_name_never_reaches_the_disk() {
        let root = tempfile::tempdir().unwrap();
        let icons = root.path().join("icons");
        let dir = package(root.path(), Some(b"x"));
        assert_eq!(cached_path(&icons, "../escape"), None);
        assert_eq!(extract(&icons, &dir, "../escape"), None);
        assert!(!root.path().join("escape.png").exists());
    }
}
