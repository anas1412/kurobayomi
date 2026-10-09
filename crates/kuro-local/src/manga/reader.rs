//! Manga reader: format resolution (area:14 behavior 10.1, T-14a-016) and the page list of a folder chapter
//! (behavior 10.2, T-14a-017).

use std::io;
use std::path::{Path, PathBuf};

use kuro_core::image::is_image_name;

use crate::error::LocalSourceError;
use crate::format::{Format, format_of};
use crate::fs::{find_child, list_dir};
use crate::images::head_of;
use crate::natsort::natural_cmp_ci;

/// Kotlin `getFormat(chapter)`: the chapter URL `<manga>/<entry>` resolved under `root` and classified. A URL without
/// a `/`, a missing entry, or an unsafe URL is `ChapterNotFound`. An entry that is no folder, EPUB or archive is
/// `InvalidFormat`.
pub fn format_of_chapter(
    root: Option<&Path>,
    chapter_url: &str,
) -> Result<Format, LocalSourceError> {
    if !chapter_url.contains('/') {
        return Err(LocalSourceError::ChapterNotFound);
    }
    let path = root
        .and_then(|root| find_child(root, chapter_url))
        .ok_or(LocalSourceError::ChapterNotFound)?;
    format_of(&path, path.is_dir()).ok_or(LocalSourceError::InvalidFormat)
}

/// Kotlin `DirectoryPageLoader.getPages`: the image files of a folder chapter in natural, case-insensitive order.
/// Folders are skipped; hidden files are not (the loader does not filter them).
pub fn directory_pages(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut images: Vec<_> = list_dir(dir)?
        .into_iter()
        .filter(|e| !e.is_dir && is_image_name(&e.name, || head_of(&e.path)))
        .collect();
    images.sort_by(|a, b| natural_cmp_ci(&a.name, &b.name));
    Ok(images.into_iter().map(|e| e.path).collect())
}

/// The pages of a local chapter, by its `Format` (Kotlin `ChapterLoader.getPageLoader` for a local source). A folder is
/// listed here. An archive or an EPUB is returned as its path: its page list comes from the `kuro-archive` readers
/// (area:13), which are still stubs, so T-14a-017 stays blocked on them. Downloaded-state precedence belongs to the
/// reader, which checks downloads before it asks the source; a local chapter is never a download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageSource {
    Directory(Vec<PathBuf>),
    Archive(PathBuf),
    Epub(PathBuf),
}

/// The single entry point: resolve the chapter's format, then take the matching page source.
pub fn page_source(root: Option<&Path>, chapter_url: &str) -> Result<PageSource, LocalSourceError> {
    Ok(match format_of_chapter(root, chapter_url)? {
        Format::Directory(dir) => PageSource::Directory(directory_pages(&dir)?),
        Format::Archive(path) => PageSource::Archive(path),
        Format::Epub(path) => PageSource::Epub(path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_chapter_resolves_to_its_format() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let show = root.path().join("Show");
        fs::create_dir_all(show.join("Ch 1"))?;
        fs::write(show.join("Ch 2.cbz"), b"x")?;
        fs::write(show.join("Ch 3.epub"), b"x")?;
        fs::write(show.join("notes.txt"), b"x")?;
        let r = Some(root.path());
        assert_eq!(
            format_of_chapter(r, "Show/Ch 1").ok(),
            Some(Format::Directory(show.join("Ch 1")))
        );
        assert_eq!(
            format_of_chapter(r, "Show/Ch 2.cbz").ok(),
            Some(Format::Archive(show.join("Ch 2.cbz")))
        );
        assert_eq!(
            format_of_chapter(r, "Show/Ch 3.epub").ok(),
            Some(Format::Epub(show.join("Ch 3.epub")))
        );
        Ok(())
    }

    #[test]
    fn a_missing_or_unsafe_chapter_is_not_found() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let r = Some(root.path());
        assert!(matches!(
            format_of_chapter(r, "Show/Ch 9.cbz"),
            Err(LocalSourceError::ChapterNotFound)
        ));
        assert!(matches!(
            format_of_chapter(r, "Show"),
            Err(LocalSourceError::ChapterNotFound)
        ));
        assert!(matches!(
            format_of_chapter(r, "../etc/passwd"),
            Err(LocalSourceError::ChapterNotFound)
        ));
        assert!(matches!(
            format_of_chapter(None, "Show/Ch 1"),
            Err(LocalSourceError::ChapterNotFound)
        ));
        Ok(())
    }

    #[test]
    fn an_entry_of_another_kind_is_an_invalid_format() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("Show"))?;
        fs::write(root.path().join("Show").join("notes.txt"), b"x")?;
        let got = format_of_chapter(Some(root.path()), "Show/notes.txt");
        assert!(matches!(got, Err(LocalSourceError::InvalidFormat)));
        Ok(())
    }

    #[test]
    fn directory_pages_are_images_in_natural_order() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        for name in ["10.png", "2.jpg", "notes.txt"] {
            fs::write(dir.path().join(name), b"x")?;
        }
        fs::create_dir(dir.path().join("sub.jpg"))?;
        let got = directory_pages(dir.path())?;
        let names: Vec<_> = got
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["2.jpg", "10.png"]);
        Ok(())
    }

    #[test]
    fn a_chapter_loads_through_the_source_matching_its_format() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let show = root.path().join("Show");
        fs::create_dir_all(show.join("Ch 1"))?;
        for name in ["10.png", "2.jpg"] {
            fs::write(show.join("Ch 1").join(name), b"x")?;
        }
        fs::write(show.join("Ch 2.cbz"), b"x")?;
        fs::write(show.join("Ch 3.epub"), b"x")?;
        let r = Some(root.path());
        let dir = show.join("Ch 1");
        assert_eq!(
            page_source(r, "Show/Ch 1").ok(),
            Some(PageSource::Directory(vec![
                dir.join("2.jpg"),
                dir.join("10.png")
            ]))
        );
        assert_eq!(
            page_source(r, "Show/Ch 2.cbz").ok(),
            Some(PageSource::Archive(show.join("Ch 2.cbz")))
        );
        assert_eq!(
            page_source(r, "Show/Ch 3.epub").ok(),
            Some(PageSource::Epub(show.join("Ch 3.epub")))
        );
        assert!(matches!(
            page_source(r, "Show/Ch 9.cbz"),
            Err(LocalSourceError::ChapterNotFound)
        ));
        Ok(())
    }
}
