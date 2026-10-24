//! The compile cache. Compiling a component is the slow step of loading a plug-in (minutes in a debug build), so the
//! compiled form is kept on disk, keyed by the SHA-256 of the wasm bytes and a hash of the engine's compile settings.
//! A second start loads it in milliseconds. A file that is corrupt or was made by another wasmtime is ignored and
//! rebuilt. A file is touched each time it loads, and files left unused for [`UNUSED_FOR`] are deleted at start: the
//! old builds of an updated plug-in, of a removed one, or of an older wasmtime.

use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};
use wasmtime::Engine;
use wasmtime::component::Component;

use crate::error::WasmError;

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// How long a cache file may go unused before the start sweep deletes it. A plug-in unused this long compiles again.
pub const UNUSED_FOR: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Sets the folder the compiled components are kept in and deletes its files unused for [`UNUSED_FOR`]. Without it
/// nothing is cached. The first call wins.
pub fn set_dir(dir: PathBuf) {
    sweep(&dir, SystemTime::now());
    let _ = DIR.set(dir);
}

/// Deletes the files in `dir` last modified more than [`UNUSED_FOR`] before `now`.
fn sweep(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let modified = entry.metadata().and_then(|m| m.modified());
        if modified.is_ok_and(|at| now.duration_since(at).is_ok_and(|age| age > UNUSED_FOR)) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Marks a cache file as used now, so the start sweep keeps it.
fn touch(path: &Path) {
    let _ = std::fs::File::options().write(true).open(path).and_then(|file| file.set_modified(SystemTime::now()));
}

/// The cache file name of `bytes` under `engine`: the wasm digest, then a hash of the engine's compile settings and
/// wasmtime version (ponytail: `DefaultHasher` may change with the Rust version, which only costs one rebuild).
pub fn cache_key(engine: &Engine, bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    engine.precompile_compatibility_hash().hash(&mut hasher);
    let mut key = String::new();
    for byte in Sha256::digest(bytes).iter() {
        let _ = write!(key, "{byte:02x}");
    }
    let _ = write!(key, "-{:016x}.cwasm", hasher.finish());
    key
}

/// Compiles `bytes`, through the cache folder when one is set.
pub(crate) fn compile(engine: &Engine, bytes: &[u8]) -> Result<Component, WasmError> {
    compile_in(DIR.get().map(PathBuf::as_path), engine, bytes)
}

fn compile_in(dir: Option<&Path>, engine: &Engine, bytes: &[u8]) -> Result<Component, WasmError> {
    let Some(dir) = dir else { return fresh(engine, bytes) };
    let path = dir.join(cache_key(engine, bytes));
    if path.exists() {
        // before loading: the loaded component maps the file, and Windows refuses to open a mapped file for writing
        touch(&path);
        // SAFETY: only this app writes these files (atomically, below), and a file from another wasmtime is refused by the check inside.
        match unsafe { Component::deserialize_file(engine, &path) } {
            Ok(component) => return Ok(component),
            Err(error) => {
                tracing::warn!(%error, file = %path.display(), "the compile cache file is not usable; rebuilding it");
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    let component = fresh(engine, bytes)?;
    if let Err(error) = store(&path, &component) {
        tracing::warn!(%error, "the compiled plug-in could not be cached");
    }
    Ok(component)
}

fn fresh(engine: &Engine, bytes: &[u8]) -> Result<Component, WasmError> {
    Component::new(engine, bytes).map_err(|e| WasmError::Compile(format!("{e:#}")))
}

/// Writes through a `.part` file so a crash never leaves a half-written cache file under the final name.
fn store(path: &Path, component: &Component) -> Result<(), String> {
    let image = component.serialize().map_err(|e| e.to_string())?;
    let dir = path.parent().ok_or("no cache folder")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let part = path.with_extension(format!("{}.part", std::process::id()));
    std::fs::write(&part, image).map_err(|e| e.to_string())?;
    std::fs::rename(&part, path).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &[u8] = b"(component)";

    fn engine() -> &'static Engine {
        crate::limits::engine().expect("engine")
    }

    #[test]
    fn the_key_follows_the_bytes() {
        let (a, b) = (cache_key(engine(), EMPTY), cache_key(engine(), b"(component (core module))"));
        assert_ne!(a, b);
        assert_eq!(a, cache_key(engine(), EMPTY));
        assert!(a.ends_with(".cwasm"));
    }

    #[test]
    fn the_second_load_reads_the_file_and_a_corrupt_file_is_rebuilt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(cache_key(engine(), EMPTY));
        compile_in(Some(dir.path()), engine(), EMPTY).expect("first compile");
        assert!(path.exists(), "the first load wrote the cache file");
        compile_in(Some(dir.path()), engine(), EMPTY).expect("load from the cache");

        std::fs::write(&path, b"garbage, not a compiled component").unwrap();
        compile_in(Some(dir.path()), engine(), EMPTY).expect("a corrupt file falls back to compiling");
        let again = compile_in(Some(dir.path()), engine(), EMPTY);
        assert!(again.is_ok() && std::fs::read(&path).unwrap() != b"garbage, not a compiled component", "the file was rebuilt");
    }

    #[test]
    fn the_sweep_deletes_only_files_unused_for_longer_than_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let (old, recent) = (dir.path().join("old.cwasm"), dir.path().join("recent.cwasm"));
        std::fs::write(&old, b"x").unwrap();
        std::fs::write(&recent, b"x").unwrap();
        let now = SystemTime::now();
        let aged = std::fs::File::options().write(true).open(&old).unwrap();
        aged.set_modified(now - UNUSED_FOR - Duration::from_secs(60)).unwrap();
        sweep(dir.path(), now);
        assert!(!old.exists() && recent.exists());
    }

    #[test]
    fn a_cache_hit_touches_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(cache_key(engine(), EMPTY));
        compile_in(Some(dir.path()), engine(), EMPTY).expect("first compile");
        let long_ago = SystemTime::now() - UNUSED_FOR - Duration::from_secs(60);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(long_ago).unwrap();
        compile_in(Some(dir.path()), engine(), EMPTY).expect("load from the cache");
        assert!(std::fs::metadata(&path).unwrap().modified().unwrap() > long_ago, "the hit refreshed the time");
    }

    #[test]
    fn invalid_bytes_are_a_compile_error_and_leave_no_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(compile_in(Some(dir.path()), engine(), b"nope"), Err(WasmError::Compile(_))));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
