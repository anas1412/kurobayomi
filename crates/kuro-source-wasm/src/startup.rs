//! Startup loading of the installed packages (T-18a-049). It runs on the tokio runtime and reports on a channel, so the
//! window opens first and the Browse list fills as each [`LoadEvent::Loaded`] arrives. Nothing here blocks the caller,
//! and nothing calls `block_on` (this replaces `runBlocking` at `MangaExtensionLoader.kt:177`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_source::source::CatalogueSource;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use kuro_prefs::source::SourcePrefs;

use crate::error::WasmError;
use crate::manifest::{ABIS, Manifest};
use crate::package::read_package;
use crate::source::{HostEnv, instantiate};

/// Whether the user trusts a package: `(pkg, version_code, signer public key)`. The trust store lives in
/// `kuro-extensions`, which passes this in (design 9.2 step 5).
pub type TrustCheck = Arc<dyn Fn(&str, i64, &[u8; 32]) -> bool + Send + Sync>;

/// What the loader reports. `Done` is always the last event.
pub enum LoadEvent {
    /// `lang` is `""` for no sources, the language for one, `"all"` for several (T-18a-009).
    Loaded { manifest: Box<Manifest>, sources: Vec<Arc<dyn CatalogueSource>>, lang: String },
    /// The package is signed but not trusted. It is reported before the NSFW gate applies (edge cases L12, L13).
    Untrusted { manifest: Box<Manifest> },
    /// One package did not load. The others are unaffected.
    Failed { file: PathBuf, error: String },
    Done,
}

/// Why one package failed, with the name and package for the log line when the manifest was read.
struct Failure {
    label: Option<(String, String)>,
    error: WasmError,
}

enum Loaded {
    Ready(Manifest, Vec<Arc<dyn CatalogueSource>>),
    Untrusted(Manifest),
}

/// Starts loading every `*.kuroext` in `dir` on `handle` and returns at once. The task ends after it sends `Done`.
pub fn load_all(
    handle: &Handle,
    dir: PathBuf,
    env: HostEnv,
    trust: TrustCheck,
    events: async_channel::Sender<LoadEvent>,
) -> JoinHandle<()> {
    handle.spawn(async move {
        for file in package_files(&dir).await {
            let event = match load_one(&file, env.clone(), &trust).await {
                Ok(Loaded::Ready(manifest, sources)) => {
                    let lang = derive_lang(&sources);
                    LoadEvent::Loaded { manifest: Box::new(manifest), sources, lang }
                }
                Ok(Loaded::Untrusted(manifest)) => {
                    tracing::warn!(file = %file.display(), "Extension {} isn't trusted", manifest.pkg);
                    LoadEvent::Untrusted { manifest: Box::new(manifest) }
                }
                Err(Failure { label, error }) => {
                    match (&label, &error) {
                        (Some((name, pkg)), WasmError::Instantiate(cause)) => {
                            tracing::warn!(file = %file.display(), %cause, "Extension load error: {name} ({pkg})")
                        }
                        _ => tracing::warn!(file = %file.display(), "{error}"),
                    }
                    LoadEvent::Failed { file, error: error.to_string() }
                }
            };
            if events.send(event).await.is_err() {
                return;
            }
        }
        let _ = events.send(LoadEvent::Done).await;
    })
}

/// `""` for no sources, the language for one, `"all"` for several (`MangaExtensionLoader.kt:346-353`).
pub fn derive_lang(sources: &[Arc<dyn CatalogueSource>]) -> String {
    let langs: BTreeSet<&str> = sources.iter().map(|s| s.lang()).collect();
    match langs.len() {
        0 => String::new(),
        1 => langs.into_iter().next().unwrap_or_default().to_owned(),
        _ => "all".to_owned(),
    }
}

/// The package files of `dir` in name order. A missing directory holds none.
async fn package_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else { return Vec::new() };
    let mut files = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "kuroext") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Design 9.2 steps 3 to 6 in order: lib gate, signed, trust, NSFW. `Ok(true)` means the package may load and
/// `Ok(false)` means it is untrusted. An untrusted package is never an NSFW error, so the user can still trust it.
fn gate(manifest: &Manifest, allow_nsfw: bool, trust: &TrustCheck) -> Result<bool, WasmError> {
    if !ABIS.contains(&manifest.abi.as_str()) {
        return Err(WasmError::LibVersion(manifest.abi.clone()));
    }
    let Some(signer) = manifest.signer_pub else { return Err(WasmError::NotSigned(manifest.pkg.clone())) };
    if !trust(&manifest.pkg, manifest.version_code, &signer) {
        return Ok(false);
    }
    if manifest.nsfw && !allow_nsfw {
        return Err(WasmError::NsfwDisabled(manifest.pkg.clone()));
    }
    Ok(true)
}

async fn load_one(file: &Path, env: HostEnv, trust: &TrustCheck) -> Result<Loaded, Failure> {
    let plain = |error: WasmError| Failure { label: None, error };
    let bytes = tokio::fs::read(file).await.map_err(|e| plain(WasmError::Io(e.to_string())))?;
    if !bytes.starts_with(b"PK") {
        return Err(plain(WasmError::NotAnExtension));
    }
    let package = tokio::task::spawn_blocking(move || read_package(&bytes))
        .await
        .map_err(|e| plain(WasmError::Engine(e.to_string())))?
        .map_err(|e| plain(WasmError::Manifest(e.to_string())))?;
    let manifest = package.manifest;
    let label = Some((manifest.name.clone(), manifest.pkg.clone()));
    let fail = |error: WasmError| Failure { label: label.clone(), error };
    // T-18a-010: read on every load, so a change of the setting applies without a restart (edge case L14).
    let allow_nsfw = SourcePrefs::new(env.prefs.clone()).show_nsfw_source.get();
    if !gate(&manifest, allow_nsfw, trust).map_err(&fail)? {
        return Ok(Loaded::Untrusted(manifest));
    }
    // Edge case L17: a single-source package declares its source in `sources`, so an empty list loads none.
    if manifest.factory.is_none() && manifest.sources.is_empty() {
        return Ok(Loaded::Ready(manifest, Vec::new()));
    }
    let sources = instantiate(package.wasm, manifest.hosts.clone(), manifest.factory.is_some(), env)
        .await
        .map_err(fail)?;
    Ok(Loaded::Ready(manifest, sources))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::fixtures::{archive, manifest_json};
    use crate::test_guest::{Behaviour, env, guest};
    use base64::Engine as _;
    use kuro_prefs::source::SourcePrefs;

    /// The signer key every test package carries (a test value, not a real key).
    const SIGNER: [u8; 32] = [7; 32];

    fn trusting(trusted: bool) -> TrustCheck {
        Arc::new(move |_, _, key| trusted && *key == SIGNER)
    }

    const INFO: &str = r#"{"id":42,"name":"Fixture","lang":"en","supportsLatest":true}"#;

    /// A package of `pkg` whose manifest declares one source (id 42), and whose NSFW flag is `nsfw`.
    fn write_package_with(dir: &Path, pkg: &str, nsfw: bool, signed: bool, wasm: &[u8]) {
        let mut manifest: serde_json::Value = serde_json::from_str(&manifest_json(pkg, 1)).unwrap();
        manifest["sources"] =
            serde_json::json!([{"id": 42, "lang": "en", "name": "Fixture", "baseUrl": "https://example.org"}]);
        manifest["nsfw"] = serde_json::json!(nsfw);
        if signed {
            manifest["signerPub"] = serde_json::json!(base64::engine::general_purpose::STANDARD.encode(SIGNER));
        }
        let bytes = archive(&[("manifest.json", manifest.to_string().as_bytes()), ("source.wasm", wasm)]);
        std::fs::write(dir.join(format!("{pkg}.kuroext")), bytes).unwrap();
    }

    fn write_package(dir: &Path, pkg: &str, wasm: &[u8]) {
        write_package_with(dir, pkg, false, true, wasm);
    }

    #[tokio::test]
    async fn the_call_returns_before_loading_and_loaded_arrives_after() {
        let store = tempfile::tempdir().unwrap();
        write_package(store.path(), "eu.example.one", &guest(Behaviour::Returns(INFO.into())));
        let (host_env, _dir) = env();
        let (tx, rx) = async_channel::unbounded();

        let task = load_all(&Handle::current(), store.path().to_owned(), host_env, trusting(true), tx);

        // The runtime has not polled the task yet: the caller was not blocked and nothing has loaded.
        assert!(!task.is_finished());
        assert!(rx.try_recv().is_err());

        match rx.recv().await.unwrap() {
            LoadEvent::Loaded { manifest, sources, .. } => {
                assert_eq!(manifest.pkg, "eu.example.one");
                assert_eq!(sources[0].id(), 42);
            }
            _ => panic!("expected Loaded first"),
        }
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Done));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn a_broken_package_fails_alone_and_done_comes_last() {
        let store = tempfile::tempdir().unwrap();
        write_package(store.path(), "eu.example.a_bad", b"not a component");
        write_package(store.path(), "eu.example.b_good", &guest(Behaviour::Returns(INFO.into())));
        std::fs::write(store.path().join("eu.example.c_zip.kuroext"), b"not a zip").unwrap();
        std::fs::write(store.path().join("notes.txt"), b"ignored").unwrap();
        let (host_env, _dir) = env();
        let (tx, rx) = async_channel::unbounded();
        load_all(&Handle::current(), store.path().to_owned(), host_env, trusting(true), tx);

        let mut loaded = Vec::new();
        let mut failed = Vec::new();
        loop {
            match rx.recv().await.unwrap() {
                LoadEvent::Loaded { manifest, .. } => loaded.push(manifest.pkg),
                LoadEvent::Failed { file, .. } => failed.push(file.file_name().unwrap().to_string_lossy().into_owned()),
                LoadEvent::Untrusted { .. } => panic!("fixtures are trusted"),
                LoadEvent::Done => break,
            }
        }
        assert_eq!(loaded, vec!["eu.example.b_good"]);
        assert_eq!(failed, vec!["eu.example.a_bad.kuroext", "eu.example.c_zip.kuroext"]);
        assert!(rx.recv().await.is_err(), "the channel closes after Done");
    }

    #[tokio::test]
    async fn a_missing_directory_loads_nothing_and_still_finishes() {
        let (host_env, _dir) = env();
        let (tx, rx) = async_channel::unbounded();
        load_all(&Handle::current(), PathBuf::from("/nonexistent/kuro/extensions"), host_env, trusting(true), tx);
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Done));
    }

    #[tokio::test]
    async fn a_nsfw_package_is_refused_while_the_setting_is_off_and_loads_once_it_is_on() {
        let store = tempfile::tempdir().unwrap();
        write_package_with(store.path(), "eu.example.adult", true, true, &guest(Behaviour::Returns(INFO.into())));
        let (host_env, _dir) = env();
        SourcePrefs::new(host_env.prefs.clone()).show_nsfw_source.set(false).unwrap();
        let (tx, rx) = async_channel::unbounded();
        load_all(&Handle::current(), store.path().to_owned(), host_env.clone(), trusting(true), tx.clone());
        let refused = "NSFW extension eu.example.adult not allowed";
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Failed { error, .. } if error == refused));
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Done));

        SourcePrefs::new(host_env.prefs.clone()).show_nsfw_source.set(true).unwrap();
        load_all(&Handle::current(), store.path().to_owned(), host_env, trusting(true), tx);
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Loaded { .. }));
    }

    #[tokio::test]
    async fn a_single_source_package_with_an_empty_source_list_loads_no_sources() {
        let store = tempfile::tempdir().unwrap();
        let mut manifest: serde_json::Value = serde_json::from_str(&manifest_json("eu.example.empty", 1)).unwrap();
        manifest["signerPub"] = serde_json::json!(base64::engine::general_purpose::STANDARD.encode(SIGNER));
        let bytes = archive(&[("manifest.json", manifest.to_string().as_bytes()), ("source.wasm", b"x")]);
        std::fs::write(store.path().join("eu.example.empty.kuroext"), bytes).unwrap();
        let (host_env, _dir) = env();
        let (tx, rx) = async_channel::unbounded();
        load_all(&Handle::current(), store.path().to_owned(), host_env, trusting(true), tx);
        match rx.recv().await.unwrap() {
            LoadEvent::Loaded { manifest, sources, lang } => {
                assert_eq!(manifest.pkg, "eu.example.empty");
                assert!(sources.is_empty());
                assert_eq!(lang, "");
            }
            _ => panic!("expected Loaded with zero sources"),
        }
    }

    #[tokio::test]
    async fn an_untrusted_nsfw_package_is_untrusted_not_nsfw_disabled() {
        let store = tempfile::tempdir().unwrap();
        write_package_with(store.path(), "eu.example.adult", true, true, &guest(Behaviour::Returns(INFO.into())));
        let (host_env, _dir) = env();
        SourcePrefs::new(host_env.prefs.clone()).show_nsfw_source.set(false).unwrap();
        let (tx, rx) = async_channel::unbounded();
        load_all(&Handle::current(), store.path().to_owned(), host_env, trusting(false), tx);
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Untrusted { manifest } if manifest.pkg == "eu.example.adult"));
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Done));
    }

    #[tokio::test]
    async fn an_unsigned_package_fails_before_trust_is_asked() {
        let store = tempfile::tempdir().unwrap();
        write_package_with(store.path(), "eu.example.bare", false, false, &guest(Behaviour::Returns(INFO.into())));
        let (host_env, _dir) = env();
        let (tx, rx) = async_channel::unbounded();
        load_all(&Handle::current(), store.path().to_owned(), host_env, trusting(false), tx);
        let want = "Package eu.example.bare isn't signed";
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Failed { error, .. } if error == want));
    }

    #[tokio::test]
    async fn a_loaded_package_reports_its_derived_language_and_a_non_zip_is_not_an_extension() {
        let store = tempfile::tempdir().unwrap();
        write_package(store.path(), "eu.example.one", &guest(Behaviour::Returns(INFO.into())));
        std::fs::write(store.path().join("eu.example.zzz.kuroext"), b"text").unwrap();
        let (host_env, _dir) = env();
        let (tx, rx) = async_channel::unbounded();
        load_all(&Handle::current(), store.path().to_owned(), host_env, trusting(true), tx);
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Loaded { lang, .. } if lang == "en"));
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Failed { error, .. } if error == "Not an extension package"));
    }

    #[test]
    fn the_gate_runs_lib_gate_then_signed_then_trust_then_nsfw() {
        let mut manifest = Manifest::parse(manifest_json("eu.example.g", 1).as_bytes()).unwrap();
        let yes = trusting(true);
        manifest.abi = "1.6".into();
        assert_eq!(
            gate(&manifest, true, &yes).unwrap_err().to_string(),
            "Lib version is 1.6, while only versions 1.4 to 1.5 are allowed"
        );
        manifest.abi = "1.4".into();
        assert_eq!(gate(&manifest, true, &yes), Err(WasmError::NotSigned("eu.example.g".into())));
        manifest.signer_pub = Some(SIGNER);
        manifest.nsfw = true;
        assert_eq!(gate(&manifest, false, &trusting(false)), Ok(false));
        assert_eq!(gate(&manifest, false, &yes), Err(WasmError::NsfwDisabled("eu.example.g".into())));
        assert_eq!(gate(&manifest, true, &yes), Ok(true));
    }
}
