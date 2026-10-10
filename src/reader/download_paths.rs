//! Where a downloaded chapter is on disk. The downloads folder is `<root>/<source folder>/<manga folder>/<chapter>`, and
//! the app has no source registry yet to say which source folder a manga's source maps to. The chapter is looked for
//! under every source folder instead; callers use a result only when it is unambiguous.

use std::path::{Path, PathBuf};

use kuro_downloads::layout::{DownloadLayout, valid_chapter_names};

/// Every place under `root` that holds the chapter: `<root>/<source folder>/<manga folder>/<chapter folder or .cbz>`.
pub fn find_downloads(root: &Path, manga_title: &str, chapter_name: &str, scanlator: Option<&str>) -> Vec<PathBuf> {
    let Ok(sources) = std::fs::read_dir(root) else { return Vec::new() };
    let manga_dir = DownloadLayout::manga_dir_name(manga_title);
    let names = valid_chapter_names(chapter_name, scanlator);
    sources
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|source| {
            let manga = source.path().join(&manga_dir);
            names.iter().map(|name| manga.join(name)).find(|path| path.exists())
        })
        .collect()
}

/// The chapter's download, when exactly one source folder holds it. Two folders holding a chapter of the same name are
/// ambiguous, and nothing is returned rather than the wrong one.
pub fn unique_download(root: &Path, manga_title: &str, chapter_name: &str, scanlator: Option<&str>) -> Option<PathBuf> {
    let mut found = find_downloads(root, manga_title, chapter_name, scanlator);
    if found.len() == 1 { found.pop() } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter_dir(root: &Path, source: &str, manga: &str, chapter: &str) -> PathBuf {
        let path = root.join(source).join(manga).join(chapter);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn the_search_finds_a_chapter_folder_or_cbz_under_each_source_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = chapter_dir(dir.path(), "Source (EN)", "Manga Title", "Group_Chapter 1");
        assert_eq!(find_downloads(dir.path(), "Manga Title", "Chapter 1", Some("Group")), vec![folder]);
        let cbz = dir.path().join("Source (EN)/Manga Title/Chapter 2.cbz");
        std::fs::write(&cbz, b"zip").unwrap();
        assert_eq!(find_downloads(dir.path(), "Manga Title", "Chapter 2", None), vec![cbz]);
        assert!(find_downloads(dir.path(), "Other", "Chapter 1", None).is_empty());
        assert!(find_downloads(Path::new("/definitely/not/here"), "Manga Title", "Chapter 1", None).is_empty());
    }

    #[test]
    fn only_a_chapter_held_by_exactly_one_source_folder_gives_a_download() {
        let dir = tempfile::tempdir().unwrap();
        let one = chapter_dir(dir.path(), "Source A (EN)", "Manga Title", "Chapter 1");
        assert_eq!(unique_download(dir.path(), "Manga Title", "Chapter 1", None), Some(one));
        chapter_dir(dir.path(), "Source B (EN)", "Manga Title", "Chapter 1");
        assert_eq!(unique_download(dir.path(), "Manga Title", "Chapter 1", None), None, "two folders are ambiguous");
        assert_eq!(unique_download(dir.path(), "Manga Title", "Chapter 9", None), None);
    }
}
