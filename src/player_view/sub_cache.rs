//! Side subtitle files of a streamed episode (`.vtt`, `.srt`, `.ass`), kept by their address in the subtitle cache, so
//! reopening an episode does not download them again (owner rule, 2026-10-23). A published file does not change, so a
//! saved copy is used until the cache size limit removes it; the Reload action of the episode drops its copies first.
//! The fetching runs on mpv's background command thread, never on the GTK thread.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

use http::{HeaderMap, HeaderName, HeaderValue};
use kuro_core::hash::md5_hex;
use kuro_platform::paths::Paths;

/// A track command rewritten before mpv runs it.
pub type Prepare = Box<dyn Fn(Vec<String>) -> Vec<String> + Send>;

/// Episodes whose next play fetches their subtitle files again (their Reload action).
static FRESH: LazyLock<Mutex<HashSet<i64>>> = LazyLock::new(Mutex::default);

/// The Reload action of an episode: its next play downloads the subtitle files again.
pub fn fetch_again(episode_id: i64) {
    FRESH.lock().unwrap_or_else(PoisonError::into_inner).insert(episode_id);
}

fn take_fresh(episode_id: i64) -> bool {
    FRESH.lock().unwrap_or_else(PoisonError::into_inner).remove(&episode_id)
}

/// The cache file of the subtitle at `url`: the md5 of the address, with the file's own extension so mpv reads the
/// right format.
pub fn file_for(dir: &Path, url: &str) -> PathBuf {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = path.rsplit('/').next().and_then(|name| name.rsplit_once('.')).map(|(_, ext)| ext.to_ascii_lowercase());
    let ext = ext.filter(|ext| ["vtt", "srt", "ass", "ssa"].contains(&ext.as_str())).unwrap_or_else(|| "sub".to_owned());
    dir.join(format!("{}.{ext}", md5_hex(url.as_bytes())))
}

/// A `sub-add` command with a web address gets the cached copy instead: the saved file, else the file `fetch` gets now,
/// saved first. `fresh` drops the saved copy. A file that cannot be fetched or saved keeps its address, so mpv still
/// tries it. Every other command is returned as it is.
pub fn localize(dir: &Path, mut command: Vec<String>, fresh: bool, fetch: &dyn Fn(&str) -> Option<Vec<u8>>) -> Vec<String> {
    let Some(url) = command.get(1).filter(|url| command[0] == "sub-add" && (url.starts_with("https://") || url.starts_with("http://"))) else {
        return command;
    };
    let file = file_for(dir, url);
    if fresh {
        let _ = std::fs::remove_file(&file);
    }
    if file.is_file() {
        crate::cache_limit::touch(&file);
    } else {
        let Some(bytes) = fetch(url) else { return command };
        if let Err(error) = std::fs::create_dir_all(dir).and_then(|()| crate::reader::cache::write_atomic(&file, &bytes)) {
            tracing::warn!(%error, "a subtitle file could not be cached");
            return command;
        }
    }
    command[1] = file.to_string_lossy().into_owned();
    command
}

/// The rewrite of the track commands of `episode_id`, which fetches with the stream's own request headers through the
/// app's client. `None` before the app's client and runtime exist (the commands then run as they are).
pub fn preparer(episode_id: i64, headers: &[(String, String)]) -> Option<Prepare> {
    let dir = kuro_platform::system_paths::SystemPaths::system().ok()?.subtitle_cache_dir();
    let runtime = tokio::runtime::Handle::try_current().ok()?;
    let client = crate::recipe_sources::client()?;
    let fresh = take_fresh(episode_id);
    let mut map = HeaderMap::new();
    for (name, value) in headers {
        if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value)) {
            map.append(name, value);
        }
    }
    Some(Box::new(move |command| {
        let fetch = |url: &str| {
            let cancel = tokio_util::sync::CancellationToken::new();
            let request = kuro_images::http::image_request(url, &map, kuro_images::CachePolicy::Enabled, &cancel).ok()?;
            runtime
                .block_on(kuro_images::http::fetch_bytes(&client, request))
                .map_err(|error| tracing::warn!(%error, "a subtitle file did not download; mpv fetches it itself"))
                .ok()
                .map(|bytes| bytes.to_vec())
        };
        localize(&dir, command, fresh, &fetch)
    }))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn sub(url: &str) -> Vec<String> {
        ["sub-add", url, "auto", "English"].map(str::to_owned).to_vec()
    }

    #[test]
    fn a_web_subtitle_is_fetched_once_and_then_read_from_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let asked = Cell::new(0);
        let fetch = |_: &str| {
            asked.set(asked.get() + 1);
            Some(b"WEBVTT".to_vec())
        };
        let first = localize(dir.path(), sub("https://cdn/a/en.vtt?t=1"), false, &fetch);
        let cached = file_for(dir.path(), "https://cdn/a/en.vtt?t=1");
        assert_eq!(first[1], cached.to_string_lossy());
        assert!(cached.to_string_lossy().ends_with(".vtt"));
        assert_eq!(localize(dir.path(), sub("https://cdn/a/en.vtt?t=1"), false, &fetch), first);
        assert_eq!(asked.get(), 1, "the second open reads the saved file");
        localize(dir.path(), sub("https://cdn/a/en.vtt?t=1"), true, &fetch);
        assert_eq!(asked.get(), 2, "Reload fetches it again");
    }

    #[test]
    fn other_commands_local_files_and_failed_fetches_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let none = |_: &str| None;
        assert_eq!(localize(dir.path(), sub("https://cdn/x"), false, &none), sub("https://cdn/x"));
        assert_eq!(localize(dir.path(), sub("/home/u/en.srt"), false, &none), sub("/home/u/en.srt"));
        let audio = ["audio-add", "https://cdn/a.m4a", "auto", "Japanese"].map(str::to_owned).to_vec();
        assert_eq!(localize(dir.path(), audio.clone(), false, &|_: &str| Some(vec![1])), audio);
        assert!(file_for(dir.path(), "https://cdn/x").to_string_lossy().ends_with(".sub"));
    }
}
