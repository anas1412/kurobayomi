//! Cover fallback from the lowest chapter (area:14 behavior 9.3, T-14a-015).

use std::fs;
use std::io;
use std::path::Path;

use kuro_source::model::{SChapter, SManga};

use crate::archive::first_page_bytes;
use crate::epub::first_image_bytes;
use crate::format::Format;
use crate::fs::find_child;
use crate::images::write_image;
use crate::manga::reader::{directory_pages, format_of_chapter};

/// Kotlin `updateCover` (`getChapterList`, when the manga has no thumbnail): the lowest chapter's first image becomes
/// the manga cover. The cover keeps an existing `cover.*` name, else `cover.jpg`. Failures are logged and swallowed.
pub fn cover_from_lowest_chapter(root: &Path, manga: &SManga, chapters: &[SChapter]) {
    let blank = manga
        .thumbnail_url
        .as_deref()
        .is_none_or(|url| url.trim().is_empty());
    if !blank {
        return;
    }
    let Some(lowest) = chapters.last() else {
        return;
    };
    if let Err(error) = write_cover(root, manga, &lowest.url) {
        tracing::error!(
            "Error updating cover for {}: {error} ({})",
            manga.title,
            root.join(&manga.url).display()
        );
    }
}

fn write_cover(root: &Path, manga: &SManga, chapter_url: &str) -> io::Result<()> {
    let dir = find_child(root, &manga.url)
        .filter(|path| path.is_dir())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "manga folder is missing"))?;
    let format = format_of_chapter(Some(root), chapter_url).map_err(io::Error::other)?;
    let bytes = match format {
        Format::Directory(folder) => match directory_pages(&folder)?.first() {
            Some(page) => Some(fs::read(page)?),
            None => None,
        },
        Format::Archive(path) => first_page_bytes(&path)?,
        Format::Epub(path) => first_image_bytes(&path)?,
    };
    if let Some(bytes) = bytes {
        write_image(&dir, "cover", "cover.jpg", &bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A manga `Show` with the given chapter folders; the lowest chapter is the last of the returned list.
    fn setup(chapter_names: &[&str]) -> io::Result<(tempfile::TempDir, SManga, Vec<SChapter>)> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("Show"))?;
        let mut chapters = Vec::new();
        for name in chapter_names {
            fs::create_dir_all(root.path().join("Show").join(name))?;
            chapters.push(SChapter::new(format!("Show/{name}"), *name));
        }
        Ok((root, SManga::new("Show", "Show"), chapters))
    }

    #[test]
    fn t_14_cover_fallback_writes_the_first_natural_image_of_the_lowest_chapter() -> io::Result<()>
    {
        let (root, manga, chapters) = setup(&["Ch 2", "Ch 1"])?;
        let ch1 = root.path().join("Show").join("Ch 1");
        fs::write(ch1.join("10.jpg"), b"ten")?;
        fs::write(ch1.join("2.jpg"), b"two")?;
        fs::write(ch1.join("notes.txt"), b"x")?;
        cover_from_lowest_chapter(root.path(), &manga, &chapters);
        assert_eq!(
            fs::read(root.path().join("Show").join("cover.jpg"))?,
            b"two"
        );
        Ok(())
    }

    #[test]
    fn t_14_cover_fallback_only_when_blank() -> io::Result<()> {
        let (root, mut manga, chapters) = setup(&["Ch 1"])?;
        fs::write(root.path().join("Show").join("Ch 1").join("1.jpg"), b"one")?;
        manga.thumbnail_url = Some("file:///elsewhere/cover.jpg".into());
        cover_from_lowest_chapter(root.path(), &manga, &chapters);
        assert!(!root.path().join("Show").join("cover.jpg").exists());
        Ok(())
    }

    #[test]
    fn t_14_cover_fallback_corrupt_archive_swallowed() -> io::Result<()> {
        let (root, manga, _) = setup(&[])?;
        fs::write(
            root.path().join("Show").join("Ch 1.cbz"),
            b"not a zip at all",
        )?;
        let chapters = [SChapter::new("Show/Ch 1.cbz", "Ch 1")];
        cover_from_lowest_chapter(root.path(), &manga, &chapters);
        assert!(!root.path().join("Show").join("cover.jpg").exists());
        Ok(())
    }

    #[test]
    fn an_existing_cover_name_is_kept() -> io::Result<()> {
        let (root, manga, chapters) = setup(&["Ch 1"])?;
        fs::write(root.path().join("Show").join("Ch 1").join("1.png"), b"one")?;
        fs::write(
            root.path().join("Show").join("Cover.PNG"),
            b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR",
        )?;
        cover_from_lowest_chapter(root.path(), &manga, &chapters);
        assert_eq!(
            fs::read(root.path().join("Show").join("Cover.PNG"))?,
            b"one"
        );
        Ok(())
    }

    #[test]
    fn a_chapter_without_images_writes_nothing() -> io::Result<()> {
        let (root, manga, chapters) = setup(&["Ch 1"])?;
        cover_from_lowest_chapter(root.path(), &manga, &chapters);
        assert!(!root.path().join("Show").join("cover.jpg").exists());
        Ok(())
    }

    #[test]
    fn t_14_cover_archive_first_image_natural() -> io::Result<()> {
        let (root, manga, _) = setup(&[])?;
        fs::write(
            root.path().join("Show").join("Ch 1.cbz"),
            include_bytes!("../../tests/fixtures/archive_pages.cbz"),
        )?;
        let chapters = [SChapter::new("Show/Ch 1.cbz", "Ch 1")];
        cover_from_lowest_chapter(root.path(), &manga, &chapters);
        assert_eq!(
            fs::read(root.path().join("Show").join("cover.jpg"))?,
            b"two-bytes"
        );
        Ok(())
    }

    #[test]
    fn t_14_cover_from_an_epub_chapter_uses_the_first_spine_image() -> io::Result<()> {
        let (root, manga, _) = setup(&[])?;
        fs::write(
            root.path().join("Show").join("Ch 1.epub"),
            include_bytes!("../../tests/fixtures/minimal.epub"),
        )?;
        let chapters = [SChapter::new("Show/Ch 1.epub", "Ch 1")];
        cover_from_lowest_chapter(root.path(), &manga, &chapters);
        assert_eq!(
            fs::read(root.path().join("Show").join("cover.jpg"))?,
            b"epub-image"
        );
        Ok(())
    }
}
