//! Offline playback of a downloaded episode (T-24a-039; behaviour 6.6, data 4.3).
//!
//! The Kotlin code takes the first `video/*` file in whatever order the folder lists it. This port sorts by name and
//! prefers the extensions in [`PREFERRED_EXTENSIONS`] order, so the choice is stable.

use std::path::Path;

use crate::anime::model::{Video, VideoStatus};
use crate::error::DownloadError;

/// Playable extensions, most preferred first.
pub const PREFERRED_EXTENSIONS: [&str; 7] = ["mkv", "mp4", "webm", "m4v", "mov", "avi", "ts"];

fn extension_rank(name: &str) -> Option<usize> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    PREFERRED_EXTENSIONS.iter().position(|e| *e == ext)
}

/// The file to play: among the playable names, the first in name order with the best extension.
pub fn pick_playable(names: &[String]) -> Option<&String> {
    let mut sorted: Vec<&String> = names.iter().filter(|n| extension_rank(n).is_some()).collect();
    sorted.sort();
    sorted.into_iter().min_by_key(|n| extension_rank(n).unwrap_or(usize::MAX))
}

/// A `file://` URI for an absolute local path, percent-encoded by the `url` crate for this system (`file:///C:/…` on
/// Windows), as `kuro_local::fs::file_url`. Empty for a relative path.
pub fn file_url(path: &Path) -> String {
    url::Url::from_file_path(path).map(String::from).unwrap_or_default()
}

/// The video for offline playback (`buildVideo`). `episode_dir` is what `AnimeLayout::find_episode_dir` returned.
/// Without a playable file the error is `NoVideo` ("No video found").
pub fn build_video(episode_dir: Option<&Path>) -> Result<Video, DownloadError> {
    let dir = episode_dir.ok_or(DownloadError::NoVideo)?;
    let names: Vec<String> = std::fs::read_dir(dir)?
        .flatten()
        .filter(|e| e.metadata().is_ok_and(|m| m.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let chosen = pick_playable(&names).ok_or(DownloadError::NoVideo)?;
    let url = file_url(&dir.join(chosen));
    Ok(Video {
        title: format!("download: {url}"),
        url,
        initialized: true,
        status: VideoStatus::Ready,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_container_preference_beats_name_order() {
        let list = names(&["a.txt", "b.mp4", "c.mkv", "a.webm"]);
        assert_eq!(pick_playable(&list).map(String::as_str), Some("c.mkv"));
        let list = names(&["a.webm", "b.mp4"]);
        assert_eq!(pick_playable(&list).map(String::as_str), Some("b.mp4"));
    }

    #[test]
    fn within_one_extension_the_first_name_wins() {
        let list = names(&["Episode 2.mkv", "Episode 1.mkv"]);
        assert_eq!(pick_playable(&list).map(String::as_str), Some("Episode 1.mkv"));
    }

    #[test]
    fn non_video_files_are_never_picked() {
        assert!(pick_playable(&names(&["cover.jpg", "subs.srt"])).is_none());
        assert_eq!(pick_playable(&names(&["X.MKV"])).map(String::as_str), Some("X.MKV"), "extensions are case-insensitive");
    }

    #[test]
    fn file_urls_encode_spaces_and_reserved_bytes() {
        let dir = std::env::temp_dir().join("d");
        let url = file_url(&dir.join("Show (EN)").join("a b#1.mkv"));
        assert!(url.starts_with("file:///") && url.ends_with("/d/Show%20(EN)/a%20b%231.mkv"), "{url}");
        assert!(file_url(&dir.join("ok-1.mkv")).ends_with("/d/ok-1.mkv"));
    }

    #[test]
    fn build_video_returns_a_ready_file_video_with_the_download_title() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Episode 1.mkv"), b"x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        let video = build_video(Some(dir.path())).unwrap();
        assert!(video.url.starts_with("file://"));
        assert!(video.url.ends_with("/Episode%201.mkv"));
        assert_eq!(video.title, format!("download: {}", video.url));
        assert!(video.initialized);
        assert_eq!(video.status, VideoStatus::Ready);
    }

    #[test]
    fn no_folder_or_no_video_is_no_video() {
        assert!(matches!(build_video(None), Err(DownloadError::NoVideo)));
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cover.jpg"), b"x").unwrap();
        let err = build_video(Some(dir.path())).unwrap_err();
        assert_eq!(err.to_string(), "No video found");
    }
}
