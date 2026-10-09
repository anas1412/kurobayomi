//! Manga chapter list (area:14 behavior section 8, T-14a-014).

use std::path::Path;

use kuro_source::model::{SChapter, SManga};

use crate::error::LocalSourceError;
use crate::format::{is_epub, is_manga_archive};
use crate::fs::{FsEntry, find_child, list_dir};
use crate::names::{is_hidden, stem};
use crate::natsort::natural_cmp_ci;
use crate::recognition::chapter_number;

/// The chapters of the folder named by `manga.url`: non-hidden folders, manga archives and EPUB files. Newest number
/// first, then name descending, natural and case-insensitive. A missing folder gives an empty list.
///
/// The EPUB metadata fill (T-14a-041) and the `chapters.json` overrides (T-14a-039) are not in this step: an EPUB
/// keeps its file-name title and modification date.
pub fn chapters(root: &Path, manga: &SManga) -> Result<Vec<SChapter>, LocalSourceError> {
    let Some(dir) = find_child(root, &manga.url).filter(|p| p.is_dir()) else {
        return Ok(Vec::new());
    };
    let mut chapters: Vec<SChapter> = list_dir(&dir)?
        .iter()
        .filter(|e| is_chapter_entry(e))
        .map(|e| chapter_of(manga, e))
        .collect();
    chapters.sort_by(|a, b| {
        b.chapter_number
            .total_cmp(&a.chapter_number)
            .then_with(|| natural_cmp_ci(&b.name, &a.name))
    });
    Ok(chapters)
}

/// Kotlin's chapter filter: `!startsWith('.') && (isDirectory || manga archive || epub)`.
fn is_chapter_entry(entry: &FsEntry) -> bool {
    !is_hidden(&entry.name) && (entry.is_dir || is_manga_archive(&entry.name) || is_epub(&entry.name))
}

fn chapter_of(manga: &SManga, entry: &FsEntry) -> SChapter {
    let name = if entry.is_dir {
        entry.name.clone()
    } else {
        stem(&entry.name).to_owned()
    };
    let mut chapter = SChapter::new(format!("{}/{}", manga.url, entry.name), name);
    chapter.date_upload = entry.modified_ms;
    chapter.chapter_number = chapter_number(&manga.title, &chapter.name);
    chapter
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io;

    /// A manga folder `Show` under a root, with the given entries. A name ending in `/` is a folder.
    fn manga_with(entries: &[&str]) -> io::Result<(tempfile::TempDir, SManga)> {
        let root = tempfile::tempdir()?;
        let show = root.path().join("Show");
        fs::create_dir(&show)?;
        for entry in entries {
            match entry.strip_suffix('/') {
                Some(dir) => fs::create_dir(show.join(dir))?,
                None => fs::write(show.join(entry), b"x")?,
            }
        }
        Ok((root, SManga::new("Show", "Show")))
    }

    fn names(chapters: &[SChapter]) -> Vec<&str> {
        chapters.iter().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn only_folders_manga_archives_and_epubs_are_chapters() -> io::Result<()> {
        let (root, manga) = manga_with(&[
            "Ch 01/",
            "Ch 02.cbz",
            "Ch 03.EPUB",
            "readme.txt",
            ".hidden/",
            ".x.cbz",
            "page.jpg",
        ])?;
        let got = chapters(root.path(), &manga).map_err(io::Error::other)?;
        assert_eq!(got.len(), 3, "{:?}", names(&got));
        let mut sorted = names(&got);
        sorted.sort_unstable();
        assert_eq!(sorted, ["Ch 01", "Ch 02", "Ch 03"]);
        Ok(())
    }

    #[test]
    fn the_url_is_manga_folder_slash_entry_name_with_extension() -> io::Result<()> {
        let (root, manga) = manga_with(&["Ch 02.cbz"])?;
        let got = chapters(root.path(), &manga).map_err(io::Error::other)?;
        assert_eq!(got[0].url, "Show/Ch 02.cbz");
        assert_eq!(got[0].name, "Ch 02");
        assert_eq!(got[0].chapter_number, 2.0);
        assert!(got[0].date_upload > 0);
        Ok(())
    }

    #[test]
    fn newest_number_first_and_unrecognised_last() -> io::Result<()> {
        let (root, manga) = manga_with(&["Show 2.cbz", "Show 10.cbz", "Show 1.cbz", "Notes.cbz"])?;
        let got = chapters(root.path(), &manga).map_err(io::Error::other)?;
        assert_eq!(names(&got), ["Show 10", "Show 2", "Show 1", "Notes"]);
        assert_eq!(got.last().map(|c| c.chapter_number), Some(-1.0));
        Ok(())
    }

    #[test]
    fn a_missing_manga_folder_is_an_empty_list() -> io::Result<()> {
        let (root, _) = manga_with(&[])?;
        let gone = SManga::new("Gone", "Gone");
        assert!(chapters(root.path(), &gone).map_err(io::Error::other)?.is_empty());
        Ok(())
    }
}
