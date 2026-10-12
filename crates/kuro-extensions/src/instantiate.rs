//! The instantiate step of a verified package (T-20a-001). The component is loaded through the WASM host, and a failed
//! instantiate becomes `LoadResult::Error`, never a panic. Only the `wasm` feature builds this module, because wasmtime is
//! large. The host takes the factory and host lists from the package, so the caller passes them; the check itself makes no
//! request.

use std::fs;
use std::path::Path;

use kuro_platform::kind::Kind;
use kuro_source_wasm::HostEnv;

use crate::error::LoadError;
use crate::loader::LoadResult;

/// Instantiates the component of a verified package. Untrusted and failed results pass through unchanged, so a component
/// the repo did not sign is never run. The package is treated as a factory (`create-sources`), the common shape; a
/// single-source package that only answers `info` fails this check.
pub async fn instantiate_verified(result: LoadResult, env: HostEnv) -> LoadResult {
    let LoadResult::Verified { manifest, component } = result else {
        return result;
    };
    let dir = component.parent().map(Path::to_path_buf).unwrap_or_default();
    let bytes = match fs::read(&component) {
        Ok(bytes) => bytes,
        Err(_) => return LoadResult::Error { dir, error: LoadError::Io(component.display().to_string()) },
    };
    match kuro_source_wasm::instantiate(bytes, Vec::new(), true, env).await {
        Ok(_) => LoadResult::Verified { manifest, component },
        Err(err) => LoadResult::Error { dir, error: LoadError::Instantiate(err.to_string()) },
    }
}

/// Where compiled plug-ins are cached between starts (see `kuro_source_wasm::cache`). Call once at start.
pub fn set_compile_cache_dir(dir: std::path::PathBuf) {
    kuro_source_wasm::cache::set_dir(dir);
}

/// The sources of one plug-in of a repository link, by the kind its index entry names.
pub enum RepoPlugin {
    Manga(Vec<std::sync::Arc<dyn kuro_source::CatalogueSource>>),
    Anime(Vec<std::sync::Arc<dyn kuro_source::anime::AnimeSource>>),
}

/// Instantiates a `.wasm` plug-in fetched from a repository link. It is a factory, as the packages are. `hosts` is the
/// allow-list its index entry names; the plug-in cannot reach any other host.
pub async fn instantiate_repo_plugin(
    bytes: Vec<u8>,
    hosts: Vec<String>,
    kind: Kind,
    client: kuro_net::NetworkClient,
    prefs: kuro_prefs::store::PrefStore,
) -> Result<RepoPlugin, String> {
    let env = HostEnv { client, prefs };
    let loaded = if kind.is_anime() {
        kuro_source_wasm::instantiate_anime(bytes, hosts, true, env).await.map(RepoPlugin::Anime)
    } else {
        kuro_source_wasm::instantiate(bytes, hosts, true, env).await.map(RepoPlugin::Manga)
    };
    loaded.map_err(|error| error.to_string())
}

/// The production load path of the manga packages (T-20a-001): discovers `extensions_dir`, instantiates every verified
/// component through the WASM host, and publishes the outcome like [`crate::loader::scan_into`]. A component that fails
/// to instantiate is kept as a problem of its folder and is not listed as installed.
pub async fn scan_and_instantiate(
    registry: &crate::registry::ExtensionRegistry,
    extensions_dir: &Path,
    signed: &std::collections::BTreeMap<String, String>,
    icons: &Path,
    env: HostEnv,
) -> Vec<LoadResult> {
    let mut results = Vec::new();
    for result in crate::loader::discover(extensions_dir, signed) {
        results.push(instantiate_verified(result, env.clone()).await);
    }
    crate::loader::publish(registry, extensions_dir, &results, icons);
    results
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use kuro_core::hash::sha256_hex;

    use super::*;
    use crate::loader::{COMPONENT_FILE, MANIFEST_FILE, load_package};

    const PKG: &str = "app.kurobayomi.ext.en.example";

    fn env(dir: &Path) -> HostEnv {
        let prefs = kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory());
        let (toaster, _feed) = kuro_platform::toast::toast_channel();
        let client = kuro_net::NetworkClient::new(
            &prefs,
            dir.join("cookies.json"),
            Arc::new(kuro_platform::webview::NoWebView),
            toaster,
        )
        .expect("client");
        HostEnv { client, prefs: kuro_prefs::store::PrefStore::in_memory() }
    }

    fn package(root: &Path, component: &[u8]) -> std::path::PathBuf {
        let dir = root.join("pkg");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(MANIFEST_FILE),
            format!("pkg = \"{PKG}\"\nname = \"Example\"\nversion = \"1.0.0\"\nversion_code = 1\napi = \"1.0\"\n"),
        )
        .unwrap();
        fs::write(dir.join(COMPONENT_FILE), component).unwrap();
        dir
    }

    #[tokio::test]
    async fn a_component_that_does_not_instantiate_is_an_error_and_not_a_panic() {
        let root = tempfile::tempdir().unwrap();
        let dir = package(root.path(), b"not a wasm component");
        let verified = load_package(&dir, &BTreeMap::from([(PKG.to_owned(), sha256_hex(b"not a wasm component"))]));
        assert!(matches!(verified, LoadResult::Verified { .. }), "{verified:?}");
        let outcome = instantiate_verified(verified, env(root.path())).await;
        assert!(matches!(outcome, LoadResult::Error { error: LoadError::Instantiate(_), .. }), "{outcome:?}");
    }

    #[tokio::test]
    async fn scan_and_instantiate_lists_nothing_for_a_component_that_does_not_instantiate_and_keeps_its_problem() {
        let root = tempfile::tempdir().unwrap();
        let exts = root.path().join("exts");
        package(&exts, b"not a wasm component");
        let registry = crate::registry::ExtensionRegistry::new();
        let signed = BTreeMap::from([(PKG.to_owned(), sha256_hex(b"not a wasm component"))]);
        let results = scan_and_instantiate(&registry, &exts, &signed, &root.path().join("icons"), env(root.path())).await;
        assert!(matches!(results[..], [LoadResult::Error { error: LoadError::Instantiate(_), .. }]), "{results:?}");
        let snapshot = registry.snapshot();
        assert!(snapshot.installed.is_empty());
        assert!(snapshot.problems.get("pkg").is_some_and(|problem| problem.contains("does not instantiate")));
    }

    #[tokio::test]
    async fn an_untrusted_package_is_not_instantiated_and_passes_through() {
        let root = tempfile::tempdir().unwrap();
        let dir = package(root.path(), b"not a wasm component");
        let untrusted = load_package(&dir, &BTreeMap::new());
        let outcome = instantiate_verified(untrusted.clone(), env(root.path())).await;
        assert_eq!(outcome, untrusted);
    }
}
