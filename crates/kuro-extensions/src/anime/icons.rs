//! Extension icons (T-19a-046). The order is `icon.png` in the package, then the cached `ext-icons/<pkg>.png` that the
//! store `iconUrl` fetch wrote, then the generic icon. A missing icon is never an error. Files are written on the scan
//! worker, and the paths are applied to the state on the main thread, so the cache map has one owner.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use kuro_platform::paths::Paths;

use crate::anime::loader::PackageDir;
use crate::anime::model::LoadResult;
use crate::anime::package::{self, manifest::valid_pkg};

/// The freedesktop icon name drawn when a package has no icon file and no store icon.
pub const GENERIC_ICON: &str = "application-x-addon-symbolic";

/// What to draw for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    /// A PNG in the icon cache.
    File(PathBuf),
    /// The store `iconUrl`. The UI loads it through the images cache and saves it with [`IconCache::write`].
    Remote(String),
    /// The generic icon, [`GENERIC_ICON`].
    Generic,
}

/// `<cache>/ext-icons`, one `<pkg>.png` per package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconCache {
    dir: PathBuf,
}

impl IconCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn from_paths(paths: &dyn Paths) -> Self {
        Self::new(paths.cache_dir().join("ext-icons"))
    }

    /// The cache file of `pkg`, or `None` for a name that is not a valid package name.
    pub fn path(&self, pkg: &str) -> Option<PathBuf> {
        valid_pkg(pkg).then(|| self.dir.join(format!("{pkg}.png")))
    }

    /// Saves icon bytes for `pkg` (a fetched `iconUrl`), through a temporary file so a reader never sees half a PNG.
    pub fn write(&self, pkg: &str, bytes: &[u8]) -> io::Result<PathBuf> {
        let target = self
            .path(pkg)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{pkg:?} is not a valid package name")))?;
        fs::create_dir_all(&self.dir)?;
        let part = target.with_extension("png.part");
        fs::write(&part, bytes)?;
        fs::rename(&part, &target)?;
        Ok(target)
    }

    /// The icon file of the package at `package_path`: extracted from it when it holds one, else an earlier fetched
    /// file when the cache has one, else `None`.
    pub fn extract(&self, package_path: &Path, pkg: &str) -> Option<PathBuf> {
        let target = self.path(pkg)?;
        if let Some(bytes) = package::read_icon(package_path) {
            match self.write(pkg, &bytes) {
                Ok(path) => return Some(path),
                Err(err) => tracing::warn!(pkg, error = %err, "could not cache the package icon"),
            }
        }
        target.is_file().then_some(target)
    }

    /// Sets `icon` on every loaded package (the scan worker calls this before it sends the results).
    pub fn fill(&self, dir: &PackageDir, results: &mut [LoadResult]) {
        for result in results {
            if let LoadResult::Success(installed) = result {
                installed.icon = self.extract(&dir.package_path(&installed.pkg), &installed.pkg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::manager::ExtensionState;
    use crate::anime::testkit::{MANIFEST, PKG, build_zip};

    const PNG: &[u8] = b"\x89PNG fake icon";

    fn package_with(entries: &[(&str, &[u8])], dir: &Path) -> PathBuf {
        let path = dir.join(format!("{PKG}.kuroext"));
        fs::write(&path, build_zip(entries)).unwrap();
        path
    }

    #[test]
    fn the_icon_in_the_package_is_extracted_into_the_cache() {
        let root = tempfile::tempdir().unwrap();
        let path = package_with(
            &[("manifest.json", MANIFEST.as_bytes()), ("source.wasm", b"\0asm"), ("icon.png", PNG)],
            root.path(),
        );
        let cache = IconCache::new(root.path().join("ext-icons"));
        let icon = cache.extract(&path, PKG).expect("extracted");
        assert_eq!(icon, root.path().join("ext-icons").join(format!("{PKG}.png")));
        assert_eq!(fs::read(icon).unwrap(), PNG);
    }

    #[test]
    fn a_package_without_an_icon_uses_the_fetched_cache_file_else_none() {
        let root = tempfile::tempdir().unwrap();
        let path = package_with(&[("manifest.json", MANIFEST.as_bytes()), ("source.wasm", b"\0asm")], root.path());
        let cache = IconCache::new(root.path().join("ext-icons"));
        assert_eq!(cache.extract(&path, PKG), None, "nothing in the package and nothing cached");
        let fetched = cache.write(PKG, b"from the store").unwrap();
        assert_eq!(cache.extract(&path, PKG), Some(fetched), "the fetched iconUrl file is the second source");
    }

    #[test]
    fn a_bad_package_name_or_a_broken_file_gives_none_and_never_panics() {
        let root = tempfile::tempdir().unwrap();
        let cache = IconCache::new(root.path().join("ext-icons"));
        assert_eq!(cache.path("../escape"), None);
        assert!(cache.write("../escape", b"x").is_err());
        let broken = root.path().join(format!("{PKG}.kuroext"));
        fs::write(&broken, b"not a zip").unwrap();
        assert_eq!(cache.extract(&broken, PKG), None);
        assert_eq!(cache.extract(&root.path().join("missing.kuroext"), PKG), None);
    }

    #[test]
    fn the_state_answers_file_then_store_url_then_generic() {
        use crate::anime::model::{Available, Installed};
        let mut state = ExtensionState::new();
        assert_eq!(state.icon("p.gone"), Icon::Generic, "a package that is gone gets the generic icon");
        let listed = |icon_url: &str| -> Available {
            let mut doc = crate::anime::testkit::store_extension("p.a", 1, None);
            doc["iconUrl"] = serde_json::json!(icon_url);
            let entry = serde_json::from_value(doc).unwrap();
            crate::anime::store::map::available(
                &entry,
                &crate::anime::model::StoreRef { index_url: "https://example.org/index.json".into(), name: "S".into() },
            )
            .unwrap()
        };
        state.apply_available(vec![listed("https://example.org/p.a.png")]);
        assert_eq!(state.icon("p.a"), Icon::Remote(listed("https://example.org/p.a.png").icon_url));
        let mut installed = Installed {
            name: "x".into(),
            pkg: "p.a".into(),
            version_name: "1".into(),
            version_code: 1,
            abi: crate::anime::abi::AbiVersion { major: 1, minor: 0 },
            lang: "en".into(),
            nsfw: false,
            torrent: false,
            sources: Vec::new(),
            icon: None,
            signer: crate::anime::model::SignerDigest("ab".repeat(32)),
            has_update: false,
            is_obsolete: false,
            store: None,
        };
        installed.icon = Some(PathBuf::from("/cache/ext-icons/p.a.png"));
        state.apply_loaded(vec![LoadResult::Success(installed)]);
        assert_eq!(state.icon("p.a"), Icon::File(PathBuf::from("/cache/ext-icons/p.a.png")));
    }
}
