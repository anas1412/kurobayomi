//! `Format`, the archive and episode extension lists, and `format_of` (area:14 section 4.2).

use std::path::{Path, PathBuf};

use crate::names::extension;

/// `ArchiveManga.SUPPORTED_ARCHIVE_TYPES`. Matched case-insensitively.
pub const MANGA_ARCHIVE_EXTENSIONS: [&str; 8] = ["zip", "cbz", "rar", "cbr", "7z", "cb7", "tar", "cbt"];
/// The video formats a local episode may have (`ArchiveAnime.SUPPORTED_ARCHIVE_TYPES` without the torrent file, which the
/// app does not play). Matched case-insensitively, as the manga archives are: `EP01.MKV` is an episode.
pub const ANIME_EPISODE_EXTENSIONS: [&str; 7] = ["avi", "flv", "mkv", "mov", "mp4", "webm", "wmv"];

pub fn is_manga_archive(name: &str) -> bool {
    let ext = extension(name).to_lowercase();
    MANGA_ARCHIVE_EXTENSIONS.contains(&ext.as_str())
}

pub fn is_anime_episode(name: &str) -> bool {
    ANIME_EPISODE_EXTENSIONS.contains(&extension(name).to_lowercase().as_str())
}

pub fn is_epub(name: &str) -> bool {
    extension(name).eq_ignore_ascii_case("epub")
}

/// How a manga chapter is stored. Each variant carries the chapter's path under the manga folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    Directory(PathBuf),
    Archive(PathBuf),
    Epub(PathBuf),
}

/// Kotlin `Format.valueOf`, in its check order: a directory first (even one named `x.epub`), then an EPUB file,
/// then a manga archive. `None` for anything else.
pub fn format_of(path: &Path, is_dir: bool) -> Option<Format> {
    let name = path.file_name()?.to_string_lossy();
    if is_dir {
        Some(Format::Directory(path.to_path_buf()))
    } else if is_epub(&name) {
        Some(Format::Epub(path.to_path_buf()))
    } else if is_manga_archive(&name) {
        Some(Format::Archive(path.to_path_buf()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manga_archives_are_case_insensitive() {
        for name in ["Vol.CBZ", "a.zip", "b.Cb7", "c.TAR", "d.rar"] {
            assert!(is_manga_archive(name), "{name}");
        }
        assert!(!is_manga_archive("Vol.mkv"));
        assert!(!is_manga_archive("page001.jpg"));
    }

    #[test]
    fn episodes_are_case_insensitive_and_a_torrent_file_is_not_one() {
        assert!(is_anime_episode("Show - 01.mkv"));
        assert!(is_anime_episode("EP01.MKV"));
        assert!(!is_anime_episode("ep.torrent"));
        assert!(!is_anime_episode("Vol.cbz"));
    }

    #[test]
    fn epub_is_case_insensitive() {
        assert!(is_epub("Book.EPUB"));
        assert!(!is_epub("Book.epub.zip"));
    }

    #[test]
    fn a_directory_named_epub_is_a_directory() {
        let dir = Path::new("/lib/Book.epub");
        assert_eq!(format_of(dir, true), Some(Format::Directory(dir.to_path_buf())));
    }

    #[test]
    fn epub_is_checked_before_archive_and_vol_cbz_is_an_archive() {
        let epub = Path::new("/lib/archive.epub");
        assert_eq!(format_of(epub, false), Some(Format::Epub(epub.to_path_buf())));
        let cbz = Path::new("/lib/Vol.CBZ");
        assert_eq!(format_of(cbz, false), Some(Format::Archive(cbz.to_path_buf())));
    }

    #[test]
    fn anything_else_is_not_a_format() {
        assert_eq!(format_of(Path::new("/lib/notes.txt"), false), None);
        assert_eq!(format_of(Path::new("/lib/Ep.MKV"), false), None);
    }
}
