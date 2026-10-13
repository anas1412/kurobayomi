//! Counts and sizes of the local manga source (T-23a-042; behaviour 3.1 of area 23). The local source is not in the
//! download index: its chapters are the folders and archives of the local manga folder, counted and sized from disk.
//! Every other source keeps using the index. The extension list is the one of `kuro-local::format`.

use std::fs;
use std::io;
use std::path::Path;

/// Archive extensions that count as a chapter (`ArchiveManga.isSupported`; same list as `kuro-local`).
pub const ARCHIVE_EXTENSIONS: [&str; 8] = ["zip", "cbz", "rar", "cbr", "7z", "cb7", "tar", "cbt"];

/// `true` when the path has one of [`ARCHIVE_EXTENSIONS`], ignoring case.
pub fn is_archive(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| ARCHIVE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The number of chapters in the local manga folder: its directories and archives. A missing folder has none.
pub fn chapter_count(manga_dir: &Path) -> io::Result<usize> {
    let entries = match fs::read_dir(manga_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut count = 0;
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() || (path.is_file() && is_archive(&path)) {
            count += 1;
        }
    }
    Ok(count)
}

/// The size of the local manga folder, recursively (sum of every file below it).
pub fn size(manga_dir: &Path) -> u64 {
    kuro_core::storage::dir_size(manga_dir).unwrap_or(0)
}

/// Episode extensions of the local anime source (`ArchiveAnime.SUPPORTED_ARCHIVE_TYPES`; same list as `kuro-local`).
/// Matched case-sensitively, so `EP01.MKV` is not an episode.
pub const ANIME_EPISODE_EXTENSIONS: [&str; 8] = ["avi", "flv", "mkv", "mov", "mp4", "webm", "wmv", "torrent"];

/// The number of episodes in the local anime folder: its files with a supported extension (`getDownloadCount` for the
/// local source, T-24a-038). Folders are not counted and a missing folder has none. The caller uses this instead of the
/// index for the local source only.
pub fn anime_episode_count(anime_dir: &Path) -> io::Result<usize> {
    let entries = match fs::read_dir(anime_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut count = 0;
    for entry in entries {
        let path = entry?.path();
        let supported = path.extension().and_then(|e| e.to_str()).is_some_and(|e| ANIME_EPISODE_EXTENSIONS.contains(&e));
        if path.is_file() && supported {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_and_archives_count_and_loose_files_do_not() {
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("Local Manga");
        fs::create_dir_all(dir.join("Chapter 1")).unwrap();
        fs::write(dir.join("Chapter 2.CBZ"), b"zip").unwrap();
        fs::write(dir.join("notes.txt"), b"not a chapter").unwrap();
        assert_eq!(chapter_count(&dir).unwrap(), 2, "a folder and an archive count");
        assert_eq!(chapter_count(&base.path().join("missing")).unwrap(), 0);
    }

    #[test]
    fn anime_episodes_are_supported_files_matched_case_sensitively() {
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("Local Anime");
        fs::create_dir_all(dir.join("Season folder.mkv")).unwrap();
        fs::write(dir.join("Ep 1.mkv"), b"v").unwrap();
        fs::write(dir.join("Ep 2.mp4"), b"v").unwrap();
        fs::write(dir.join("Ep 3.MKV"), b"v").unwrap();
        fs::write(dir.join("notes.txt"), b"x").unwrap();
        assert_eq!(anime_episode_count(&dir).unwrap(), 2, "a folder and EP3.MKV do not count");
        assert_eq!(anime_episode_count(&base.path().join("missing")).unwrap(), 0);
    }

    #[test]
    fn the_size_is_recursive() {
        let base = tempfile::tempdir().unwrap();
        let dir = base.path().join("Local Manga");
        fs::create_dir_all(dir.join("Chapter 1").join("deep")).unwrap();
        fs::write(dir.join("Chapter 1").join("001.jpg"), vec![0u8; 100]).unwrap();
        fs::write(dir.join("Chapter 1").join("deep").join("002.jpg"), vec![0u8; 50]).unwrap();
        assert_eq!(size(&dir), 150);
    }
}
