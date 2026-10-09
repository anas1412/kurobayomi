//! Folder and file names and path building (port of MangaDownloadProvider.kt).
//!
//! Layout: `<root>/<source dir>/<manga dir>/<chapter>` where chapter is a folder or `<chapter>.cbz`.
//! Lookups never create folders. Only [`DownloadLayout::manga_dir`] creates one.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::DownloadError;
use crate::model::ChapterRef;
use crate::sanitize::build_valid_filename;

/// Suffix of an in-progress chapter folder (`TMP_DIR_SUFFIX`).
pub const TMP_DIR_SUFFIX: &str = "_tmp";
pub const CBZ_EXTENSION: &str = "cbz";

/// `"<name>"` or `"<scanlator>_<name>"`, sanitised. A blank name becomes `Chapter`, a blank scanlator is ignored.
pub fn chapter_dir_name(name: &str, scanlator: Option<&str>) -> String {
    let name = if name.trim().is_empty() { "Chapter" } else { name };
    match scanlator.filter(|s| !s.trim().is_empty()) {
        Some(scanlator) => build_valid_filename(&format!("{scanlator}_{name}")),
        None => build_valid_filename(name),
    }
}

/// `[<chapter dir>, <chapter dir>.cbz]`, in lookup order: folder first.
pub fn valid_chapter_names(name: &str, scanlator: Option<&str>) -> [String; 2] {
    let dir = chapter_dir_name(name, scanlator);
    let cbz = format!("{dir}.{CBZ_EXTENSION}");
    [dir, cbz]
}

/// Root of the downloads tree, plus the naming rules for every level.
#[derive(Clone, Debug)]
pub struct DownloadLayout {
    root: PathBuf,
}

impl DownloadLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `"<name> (<LANG>)"` sanitised, e.g. `MangaDex (EN)`. The same for every source type.
    pub fn source_dir_name(source_display_name: &str) -> String {
        build_valid_filename(source_display_name)
    }

    /// Sanitised manga title.
    pub fn manga_dir_name(title: &str) -> String {
        build_valid_filename(title)
    }

    /// Creates `<root>/<source>/<manga>` when missing. Failure is `InvalidLocation` with the root path.
    pub fn manga_dir(&self, source_display_name: &str, manga_title: &str) -> Result<PathBuf, DownloadError> {
        let dir = self.manga_path(source_display_name, manga_title);
        fs::create_dir_all(&dir).map_err(|_| DownloadError::InvalidLocation(self.root.display().to_string()))?;
        Ok(dir)
    }

    /// The existing manga folder, if any. Does not create it.
    pub fn find_manga_dir(&self, source_display_name: &str, manga_title: &str) -> Option<PathBuf> {
        let dir = self.manga_path(source_display_name, manga_title);
        dir.is_dir().then_some(dir)
    }

    /// The existing source folder, if any. Does not create it.
    pub fn find_source_dir(&self, source_display_name: &str) -> Option<PathBuf> {
        let dir = self.root.join(Self::source_dir_name(source_display_name));
        dir.is_dir().then_some(dir)
    }

    /// The chapter folder or `.cbz` file, folder first. `None` when the manga folder is missing.
    pub fn find_chapter(
        &self,
        source_display_name: &str,
        manga_title: &str,
        name: &str,
        scanlator: Option<&str>,
    ) -> Option<PathBuf> {
        let manga_dir = self.find_manga_dir(source_display_name, manga_title)?;
        valid_chapter_names(name, scanlator).into_iter().map(|n| manga_dir.join(n)).find(|p| p.exists())
    }

    /// `(manga folder, found chapter paths)`. `(None, [])` when the manga folder does not exist.
    pub fn find_chapter_dirs(
        &self,
        source_display_name: &str,
        manga_title: &str,
        chapters: &[ChapterRef],
    ) -> (Option<PathBuf>, Vec<PathBuf>) {
        let Some(manga_dir) = self.find_manga_dir(source_display_name, manga_title) else {
            return (None, Vec::new());
        };
        let found = chapters
            .iter()
            .filter_map(|c| {
                valid_chapter_names(&c.name, c.scanlator.as_deref())
                    .into_iter()
                    .map(|n| manga_dir.join(n))
                    .find(|p| p.exists())
            })
            .collect();
        (Some(manga_dir), found)
    }

    /// The staging folder (`<chapter dir>_tmp`) of a chapter. Pure path building: nothing is created or checked.
    pub fn staging_dir(
        &self,
        source_display_name: &str,
        manga_title: &str,
        name: &str,
        scanlator: Option<&str>,
    ) -> PathBuf {
        self.manga_path(source_display_name, manga_title).join(format!("{}{TMP_DIR_SUFFIX}", chapter_dir_name(name, scanlator)))
    }

    fn manga_path(&self, source_display_name: &str, manga_title: &str) -> PathBuf {
        self.root.join(Self::source_dir_name(source_display_name)).join(Self::manga_dir_name(manga_title))
    }
}

/// Suffix of a CBZ archive that is still being written (`<name>.cbz.part`).
pub const CBZ_PART_SUFFIX: &str = ".cbz.part";

/// Startup sweep (T-23a-044). Under `root`, every chapter staging folder (`<name>_tmp`) and every `.cbz.part` file is
/// removed, except the staging folders in `keep`, which belong to queued chapters and wait for a resume. Only the
/// `<source>/<manga>` levels are walked. Returns how many entries were removed.
pub fn sweep_stray_chapters(root: &Path, keep: &[PathBuf]) -> usize {
    let mut removed = 0;
    for source in children(root).into_iter().filter(|p| p.is_dir()) {
        for manga in children(&source).into_iter().filter(|p| p.is_dir()) {
            for entry in children(&manga) {
                let name = entry.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let stray_tmp = entry.is_dir() && name.ends_with(TMP_DIR_SUFFIX) && !keep.contains(&entry);
                let stray_part = entry.is_file() && name.ends_with(CBZ_PART_SUFFIX);
                let gone = if stray_tmp {
                    fs::remove_dir_all(&entry).is_ok()
                } else {
                    stray_part && fs::remove_file(&entry).is_ok()
                };
                if gone {
                    removed += 1;
                }
            }
        }
    }
    removed
}

fn children(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path).into_iter().flatten().flatten().map(|e| e.path()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(name: &str, scanlator: Option<&str>) -> ChapterRef {
        ChapterRef {
            id: 1,
            manga_id: 1,
            url: String::new(),
            name: name.into(),
            scanlator: scanlator.map(str::to_owned),
            read: false,
            bookmark: false,
            source_order: 0,
            chapter_number: 1.0,
            date_upload: 0,
        }
    }

    // TST-35
    #[test]
    fn chapter_dir_name_with_scanlator() {
        assert_eq!(chapter_dir_name("Chapter 1", Some("Scan")), "Scan_Chapter 1");
        assert_eq!(chapter_dir_name("Chapter 1", Some("   ")), "Chapter 1");
        assert_eq!(chapter_dir_name("Chapter 1", None), "Chapter 1");
        assert_eq!(chapter_dir_name("  ", None), "Chapter");
        assert_eq!(chapter_dir_name("a/b", Some("S")), "S_a_b");
    }

    // TST-36
    #[test]
    fn valid_names_include_cbz_after_the_folder() {
        assert_eq!(valid_chapter_names("Ch 1", None), ["Ch 1".to_owned(), "Ch 1.cbz".to_owned()]);
    }

    #[test]
    fn source_and_manga_dir_names_are_sanitised() {
        assert_eq!(DownloadLayout::source_dir_name("MangaDex (EN)"), "MangaDex (EN)");
        assert_eq!(DownloadLayout::manga_dir_name("Mangá/Ż"), "Mangá_Ż");
    }

    #[test]
    fn manga_dir_creates_and_lookups_do_not() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = DownloadLayout::new(tmp.path());
        assert!(layout.find_manga_dir("S (EN)", "M").is_none());
        assert!(!tmp.path().join("S (EN)").exists());
        let dir = layout.manga_dir("S (EN)", "M").unwrap();
        assert!(dir.is_dir());
        assert_eq!(layout.find_manga_dir("S (EN)", "M"), Some(dir));
    }

    #[test]
    fn find_chapter_prefers_the_folder_and_returns_the_cbz_otherwise() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = DownloadLayout::new(tmp.path());
        let dir = layout.manga_dir("S (EN)", "M").unwrap();
        fs::write(dir.join("Ch 1.cbz"), b"x").unwrap();
        assert_eq!(layout.find_chapter("S (EN)", "M", "Ch 1", None), Some(dir.join("Ch 1.cbz")));
        fs::create_dir(dir.join("Ch 1")).unwrap();
        assert_eq!(layout.find_chapter("S (EN)", "M", "Ch 1", None), Some(dir.join("Ch 1")));
        assert_eq!(layout.find_chapter("S (EN)", "Other", "Ch 1", None), None);
    }

    #[test]
    fn find_chapter_dirs_reports_no_manga_folder_as_none() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = DownloadLayout::new(tmp.path());
        let chapters = vec![chapter("Ch 1", None)];
        assert_eq!(layout.find_chapter_dirs("S (EN)", "M", &chapters), (None, Vec::new()));
        let dir = layout.manga_dir("S (EN)", "M").unwrap();
        fs::create_dir(dir.join("Ch 1")).unwrap();
        let (found_dir, found) = layout.find_chapter_dirs("S (EN)", "M", &chapters);
        assert_eq!(found_dir, Some(dir.clone()));
        assert_eq!(found, vec![dir.join("Ch 1")]);
    }

    #[test]
    fn the_startup_sweep_removes_stray_staging_folders_and_part_files_only() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = DownloadLayout::new(tmp.path());
        let dir = layout.manga_dir("S (EN)", "M").unwrap();
        fs::create_dir(dir.join("Ch 1_tmp")).unwrap();
        fs::write(dir.join("Ch 1_tmp").join("0001.jpg.tmp"), b"x").unwrap();
        fs::create_dir(dir.join("Ch 2_tmp")).unwrap();
        fs::write(dir.join("Ch 3.cbz.part"), b"x").unwrap();
        fs::write(dir.join("Ch 4.cbz"), b"x").unwrap();
        fs::create_dir(dir.join("Ch 5")).unwrap();
        let kept = dir.join("Ch 2_tmp");
        assert_eq!(sweep_stray_chapters(tmp.path(), std::slice::from_ref(&kept)), 2, "Ch 1_tmp and Ch 3.cbz.part");
        assert!(!dir.join("Ch 1_tmp").exists());
        assert!(kept.is_dir(), "a queued chapter's staging folder waits for the resume");
        assert!(!dir.join("Ch 3.cbz.part").exists());
        assert!(dir.join("Ch 4.cbz").is_file() && dir.join("Ch 5").is_dir(), "finished chapters are never touched");
        assert_eq!(sweep_stray_chapters(tmp.path(), &[]), 1, "the kept folder goes once it is no longer queued");
    }
}
