//! Startup loading of the installed packages (T-18a-049). It runs on the tokio runtime and reports on a channel, so the
//! window opens first and the Browse list fills as each [`LoadEvent::Loaded`] arrives. Nothing here blocks the caller,
//! and nothing calls `block_on` (this replaces `runBlocking` at `MangaExtensionLoader.kt:177`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_source::source::CatalogueSource;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::manifest::Manifest;
use crate::package::read_package;
use crate::source::{HostEnv, instantiate};

/// What the loader reports. `Done` is always the last event.
pub enum LoadEvent {
    Loaded { manifest: Box<Manifest>, sources: Vec<Arc<dyn CatalogueSource>> },
    /// One package did not load. The others are unaffected.
    Failed { file: PathBuf, error: String },
    Done,
}

/// Starts loading every `*.kuroext` in `dir` on `handle` and returns at once. The task ends after it sends `Done`.
pub fn load_all(
    handle: &Handle,
    dir: PathBuf,
    env: HostEnv,
    events: async_channel::Sender<LoadEvent>,
) -> JoinHandle<()> {
    handle.spawn(async move {
        for file in package_files(&dir).await {
            let event = match load_one(&file, env.clone()).await {
                Ok((manifest, sources)) => LoadEvent::Loaded { manifest: Box::new(manifest), sources },
                Err(error) => {
                    tracing::warn!(file = %file.display(), "Extension load error: {error}");
                    LoadEvent::Failed { file, error }
                }
            };
            if events.send(event).await.is_err() {
                return;
            }
        }
        let _ = events.send(LoadEvent::Done).await;
    })
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

async fn load_one(file: &Path, env: HostEnv) -> Result<(Manifest, Vec<Arc<dyn CatalogueSource>>), String> {
    let bytes = tokio::fs::read(file).await.map_err(|e| e.to_string())?;
    let package = tokio::task::spawn_blocking(move || read_package(&bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let manifest = package.manifest;
    let sources = instantiate(package.wasm, manifest.hosts.clone(), manifest.factory.is_some(), env)
        .await
        .map_err(|e| e.to_string())?;
    Ok((manifest, sources))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::fixtures::{archive, manifest_json};
    use crate::test_guest::{Behaviour, env, guest};

    const INFO: &str = r#"{"id":42,"name":"Fixture","lang":"en","supportsLatest":true}"#;

    fn write_package(dir: &Path, pkg: &str, wasm: &[u8]) {
        let bytes = archive(&[("manifest.json", manifest_json(pkg, 1).as_bytes()), ("source.wasm", wasm)]);
        std::fs::write(dir.join(format!("{pkg}.kuroext")), bytes).unwrap();
    }

    #[tokio::test]
    async fn the_call_returns_before_loading_and_loaded_arrives_after() {
        let store = tempfile::tempdir().unwrap();
        write_package(store.path(), "eu.example.one", &guest(Behaviour::Returns(INFO.into())));
        let (host_env, _dir) = env();
        let (tx, rx) = async_channel::unbounded();

        let task = load_all(&Handle::current(), store.path().to_owned(), host_env, tx);

        // The runtime has not polled the task yet: the caller was not blocked and nothing has loaded.
        assert!(!task.is_finished());
        assert!(rx.try_recv().is_err());

        match rx.recv().await.unwrap() {
            LoadEvent::Loaded { manifest, sources } => {
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
        load_all(&Handle::current(), store.path().to_owned(), host_env, tx);

        let mut loaded = Vec::new();
        let mut failed = Vec::new();
        loop {
            match rx.recv().await.unwrap() {
                LoadEvent::Loaded { manifest, .. } => loaded.push(manifest.pkg),
                LoadEvent::Failed { file, .. } => failed.push(file.file_name().unwrap().to_string_lossy().into_owned()),
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
        load_all(&Handle::current(), PathBuf::from("/nonexistent/kuro/extensions"), host_env, tx);
        assert!(matches!(rx.recv().await.unwrap(), LoadEvent::Done));
    }
}
