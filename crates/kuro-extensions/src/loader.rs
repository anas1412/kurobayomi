//! Finds the extension packages under `<data>/extensions` and checks each one (T-20a-001). [`scan_into`] only checks;
//! with the `wasm` feature, `instantiate::scan_and_instantiate` also loads each verified component through the WASM host.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use kuro_core::hash::sha256_hex;

use crate::error::LoadError;
use crate::icon;
use crate::manifest::Manifest;
use crate::registry::{ExtensionRegistry, Installed, Untrusted};

pub const MANIFEST_FILE: &str = "manifest.toml";
pub const COMPONENT_FILE: &str = "component.wasm";

/// The outcome of checking one package directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadResult {
    /// The manifest is valid and the component's sha256 equals the digest the repo index signs for the package.
    Verified { manifest: Manifest, component: PathBuf },
    /// The digest differs from the repo-signed one, or no repo signs the package.
    Untrusted { manifest: Manifest, component: PathBuf },
    /// The package cannot be used. The directory is kept so the UI can show the error.
    Error { dir: PathBuf, error: LoadError },
}

/// Checks the package in `dir`. `signed` maps a package name to the sha256 hex the repo index lists for it.
pub fn load_package(dir: &Path, signed: &BTreeMap<String, String>) -> LoadResult {
    match check(dir, signed) {
        Ok(result) => result,
        Err(error) => LoadResult::Error { dir: dir.to_path_buf(), error },
    }
}

fn check(dir: &Path, signed: &BTreeMap<String, String>) -> Result<LoadResult, LoadError> {
    let manifest_path = dir.join(MANIFEST_FILE);
    let text = fs::read_to_string(&manifest_path).map_err(|_| LoadError::Io(manifest_path.display().to_string()))?;
    let manifest = Manifest::parse(&text)?;
    let component = dir.join(COMPONENT_FILE);
    let bytes = fs::read(&component).map_err(|_| LoadError::MissingComponent)?;
    let digest = sha256_hex(&bytes);
    let trusted = signed.get(&manifest.pkg).is_some_and(|expected| *expected == digest);
    Ok(if trusted {
        LoadResult::Verified { manifest, component }
    } else {
        LoadResult::Untrusted { manifest, component }
    })
}

/// Every package directory under `extensions_dir` that holds a `manifest.toml`, in path order. Hidden folders (such as
/// the `.tmp` download folder) and folders without a manifest are not packages and are skipped.
pub fn discover(extensions_dir: &Path, signed: &BTreeMap<String, String>) -> Vec<LoadResult> {
    let Ok(entries) = fs::read_dir(extensions_dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir() && !is_hidden(path) && path.join(MANIFEST_FILE).is_file())
        .collect();
    dirs.sort();
    dirs.iter().map(|dir| load_package(dir, signed)).collect()
}

/// The load path of the manga packages (T-18a-011, T-18a-054): [`discover`]s `extensions_dir`, publishes what loaded to
/// `registry`, copies each loaded package's `icon.png` into `icons` (once per version, see [`icon::extract`]), and keeps
/// each folder's last load problem in the registry keyed by its folder name. A folder that loads again, or is gone,
/// loses its problem.
pub fn scan_into(
    registry: &ExtensionRegistry,
    extensions_dir: &Path,
    signed: &BTreeMap<String, String>,
    icons: &Path,
) -> Vec<LoadResult> {
    let results = discover(extensions_dir, signed);
    publish(registry, extensions_dir, &results, icons);
    results
}

/// Publishes checked `results` to `registry` (the second half of [`scan_into`]).
pub(crate) fn publish(registry: &ExtensionRegistry, extensions_dir: &Path, results: &[LoadResult], icons: &Path) {
    let (mut installed, mut untrusted, mut seen) = (Vec::new(), Vec::new(), Vec::new());
    for result in results {
        match result {
            LoadResult::Verified { manifest, component } | LoadResult::Untrusted { manifest, component } => {
                let dir = component.parent().unwrap_or(extensions_dir);
                seen.push(folder_name(dir));
                registry.clear_problem(&folder_name(dir));
                icon::extract(icons, dir, &manifest.pkg);
                if matches!(result, LoadResult::Verified { .. }) {
                    installed.push(Installed {
                        pkg: manifest.pkg.clone(),
                        name: manifest.name.clone(),
                        version_code: manifest.version_code,
                        lib_version: manifest.api.parse().unwrap_or(0.0),
                        has_update: false,
                        is_obsolete: false,
                    });
                } else {
                    untrusted.push(Untrusted { pkg: manifest.pkg.clone(), version_code: manifest.version_code });
                }
            }
            LoadResult::Error { dir, error } => {
                seen.push(folder_name(dir));
                registry.set_problem(&folder_name(dir), error);
            }
        }
    }
    for gone in registry.snapshot().problems.keys().filter(|name| !seen.contains(name)) {
        registry.clear_problem(gone);
    }
    registry.set_installed(installed);
    registry.set_untrusted(untrusted);
}

fn folder_name(dir: &Path) -> String {
    dir.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

pub(crate) fn is_hidden(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PKG: &str = "app.kurobayomi.ext.en.example";
    const COMPONENT: &[u8] = b"\0asm-fake-component";

    fn manifest_text(api: &str) -> String {
        format!("pkg = \"{PKG}\"\nname = \"Example\"\nversion = \"1.0.0\"\nversion_code = 1\napi = \"{api}\"\n")
    }

    /// Writes one package folder under `root` and returns its path.
    fn package(root: &Path, folder: &str, api: &str, component: Option<&[u8]>) -> PathBuf {
        let dir = root.join(folder);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(MANIFEST_FILE), manifest_text(api)).unwrap();
        if let Some(bytes) = component {
            fs::write(dir.join(COMPONENT_FILE), bytes).unwrap();
        }
        dir
    }

    fn signed_with(digest: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(PKG.to_owned(), digest.to_owned())])
    }

    #[test]
    fn a_component_whose_digest_matches_the_repo_is_verified() {
        let root = tempfile::tempdir().unwrap();
        let dir = package(root.path(), "example", "1.0", Some(COMPONENT));
        let result = load_package(&dir, &signed_with(&sha256_hex(COMPONENT)));
        assert!(matches!(result, LoadResult::Verified { ref manifest, .. } if manifest.pkg == PKG), "{result:?}");
    }

    #[test]
    fn a_digest_that_differs_from_the_repo_is_untrusted() {
        let root = tempfile::tempdir().unwrap();
        let dir = package(root.path(), "example", "1.0", Some(COMPONENT));
        let result = load_package(&dir, &signed_with(&sha256_hex(b"other bytes")));
        assert!(matches!(result, LoadResult::Untrusted { .. }), "{result:?}");
    }

    #[test]
    fn a_package_no_repo_signs_is_untrusted() {
        let root = tempfile::tempdir().unwrap();
        let dir = package(root.path(), "example", "1.0", Some(COMPONENT));
        let result = load_package(&dir, &BTreeMap::new());
        assert!(matches!(result, LoadResult::Untrusted { .. }), "{result:?}");
    }

    #[test]
    fn a_manifest_without_a_component_is_an_error_that_names_the_missing_file() {
        let root = tempfile::tempdir().unwrap();
        let dir = package(root.path(), "example", "1.0", None);
        let result = load_package(&dir, &signed_with(&sha256_hex(COMPONENT)));
        assert_eq!(result, LoadResult::Error { dir, error: LoadError::MissingComponent });
    }

    #[test]
    fn an_unsupported_api_is_an_error_and_the_component_is_not_read() {
        let root = tempfile::tempdir().unwrap();
        let dir = package(root.path(), "example", "2.0", Some(COMPONENT));
        let result = load_package(&dir, &signed_with(&sha256_hex(COMPONENT)));
        assert_eq!(result, LoadResult::Error { dir, error: LoadError::UnsupportedApi("2.0".into()) });
    }

    #[test]
    fn discovery_skips_hidden_folders_and_folders_without_a_manifest_in_path_order() {
        let root = tempfile::tempdir().unwrap();
        package(root.path(), "zeta", "1.0", Some(COMPONENT));
        package(root.path(), "alpha", "1.0", Some(COMPONENT));
        package(root.path(), ".tmp", "1.0", Some(COMPONENT));
        fs::create_dir_all(root.path().join("no-manifest")).unwrap();
        fs::write(root.path().join("loose.txt"), b"not a folder").unwrap();
        let found = discover(root.path(), &signed_with(&sha256_hex(COMPONENT)));
        let names: Vec<String> = found
            .iter()
            .map(|result| match result {
                LoadResult::Verified { component, .. } | LoadResult::Untrusted { component, .. } => {
                    component.parent().unwrap().file_name().unwrap().to_string_lossy().into_owned()
                }
                LoadResult::Error { dir, .. } => dir.file_name().unwrap().to_string_lossy().into_owned(),
            })
            .collect();
        assert_eq!(names, ["alpha", "zeta"]);
    }

    #[test]
    fn discovery_of_a_missing_extensions_folder_finds_nothing() {
        let root = tempfile::tempdir().unwrap();
        assert!(discover(&root.path().join("absent"), &BTreeMap::new()).is_empty());
    }

    #[test]
    fn scan_into_publishes_packages_caches_icons_and_keeps_problems_by_folder() {
        let root = tempfile::tempdir().unwrap();
        let icons = root.path().join("icons");
        let exts = root.path().join("exts");
        let good = package(&exts, "good", "1.0", Some(COMPONENT));
        fs::write(good.join("icon.png"), b"png").unwrap();
        let bad = package(&exts, "bad", "1.0", None);
        let registry = ExtensionRegistry::new();
        scan_into(&registry, &exts, &signed_with(&sha256_hex(COMPONENT)), &icons);
        let snapshot = registry.snapshot();
        assert!(snapshot.installed.contains_key(PKG));
        assert_eq!(fs::read(icons.join(format!("{PKG}.png"))).unwrap(), b"png", "the icon is cached");
        assert_eq!(snapshot.problems.get("bad").map(String::as_str), Some("component.wasm is missing"));
        assert!(!snapshot.problems.contains_key("good"));

        // the broken package gets its component and loads: its problem is cleared; a vanished folder loses its problem too
        fs::write(bad.join(COMPONENT_FILE), COMPONENT).unwrap();
        let other = package(&exts, "other", "2.0", Some(COMPONENT));
        scan_into(&registry, &exts, &signed_with(&sha256_hex(COMPONENT)), &icons);
        assert!(registry.snapshot().problems.contains_key("other"));
        assert!(!registry.snapshot().problems.contains_key("bad"));
        fs::remove_dir_all(other).unwrap();
        scan_into(&registry, &exts, &signed_with(&sha256_hex(COMPONENT)), &icons);
        assert!(registry.snapshot().problems.is_empty());
    }
}
