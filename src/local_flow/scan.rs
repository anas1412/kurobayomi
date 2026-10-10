//! Scans one local manga folder (area 14, behaviour sections 8 and 9.3). The folder is the manga; its visible
//! subfolders, manga archives and EPUB files are its chapters, listed in kuro-local's order: newest number first.

use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};

use kuro_local::archive::first_page_bytes;
use kuro_local::epub::first_image_bytes;
use kuro_local::format::{Format, format_of};
use kuro_local::fs::{file_url, list_dir};
use kuro_local::images::{find_image, set_cover};
use kuro_local::manga::reader::directory_pages;
use kuro_local::names::{is_hidden, stem};
use kuro_local::natsort::natural_cmp_ci;
use kuro_local::recognition::chapter_number;

/// One chapter found in the folder.
#[derive(Clone, Debug, PartialEq)]
pub struct ScannedChapter {
    /// The absolute path of the folder, archive or EPUB file. It is the chapter's url.
    pub path: PathBuf,
    pub name: String,
    pub date_upload: i64,
    /// `-1.0` when the name holds no number.
    pub chapter_number: f64,
}

/// One manga folder and its chapters.
#[derive(Clone, Debug, PartialEq)]
pub struct ScannedManga {
    /// The absolute path of the manga folder. It is the manga's url.
    pub path: PathBuf,
    pub title: String,
    pub chapters: Vec<ScannedChapter>,
    /// The `file://` URL of the cover, or `None` when the folder has no image at all.
    pub cover: Option<String>,
}

/// Scans `folder`. Hidden entries and files that are neither a folder, an EPUB nor a manga archive are skipped. The
/// cover is the folder's own `cover.*` image, else the first page of the lowest chapter (written as `cover.jpg` for an
/// archive, as kuro-local does).
pub fn scan(folder: &Path) -> io::Result<ScannedManga> {
    let title = folder
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("the folder name is not UTF-8"))?
        .to_owned();
    let mut found = Vec::new();
    for entry in list_dir(folder)? {
        if !is_hidden(&entry.name) && format_of(&entry.path, entry.is_dir).is_some() {
            found.push(entry);
        }
    }
    let stem_of = |e: &kuro_local::fs::FsEntry| if e.is_dir { e.name.clone() } else { stem(&e.name).to_owned() };
    let mut chapters = Vec::new();
    for entry in &found {
        let mut name = stem_of(entry);
        // a file whose name matches another chapter keeps its extension, so every chapter name is unique
        if !entry.is_dir && found.iter().filter(|other| stem_of(other) == name).count() > 1 {
            name = entry.name.clone();
        }
        chapters.push(ScannedChapter {
            chapter_number: f64::from(chapter_number(&title, &name)),
            name,
            date_upload: entry.modified_ms,
            path: entry.path.clone(),
        });
    }
    chapters.sort_by(newest_first);
    let cover = cover_of(folder, chapters.last())?;
    Ok(ScannedManga { path: folder.to_path_buf(), title, chapters, cover })
}

/// kuro-local's chapter order: the larger number first, then the name descending, natural and case-insensitive.
pub fn newest_first(a: &ScannedChapter, b: &ScannedChapter) -> Ordering {
    b.chapter_number.total_cmp(&a.chapter_number).then_with(|| natural_cmp_ci(&b.name, &a.name))
}

fn cover_of(folder: &Path, lowest: Option<&ScannedChapter>) -> io::Result<Option<String>> {
    if let Some(cover) = find_image(folder, "cover").and_then(|p| file_url(&p)) {
        return Ok(Some(cover));
    }
    let Some(lowest) = lowest else {
        return Ok(None);
    };
    if lowest.path.is_dir() {
        let first = directory_pages(&lowest.path)?.into_iter().next();
        return Ok(first.and_then(|p| file_url(&p)));
    }
    let bytes = match format_of(&lowest.path, false) {
        Some(Format::Epub(path)) => first_image_bytes(&path)?,
        _ => first_page_bytes(&lowest.path)?,
    };
    match bytes {
        Some(bytes) => Ok(Some(set_cover(folder, &bytes)?.thumbnail_url)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(name: &str, number: f64) -> ScannedChapter {
        ScannedChapter { path: PathBuf::from(name), name: name.to_owned(), date_upload: 0, chapter_number: number }
    }

    #[test]
    fn the_larger_number_comes_first_then_the_name_descends() {
        let mut list = vec![chapter("Ch 2", 2.0), chapter("Ch 10", 10.0), chapter("Ch 9", 9.0), chapter("Extra", -1.0)];
        list.sort_by(newest_first);
        let names: Vec<&str> = list.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Ch 10", "Ch 9", "Ch 2", "Extra"]);
    }

    #[test]
    fn equal_numbers_fall_back_to_a_descending_name() {
        let mut list = vec![chapter("a", 1.0), chapter("B", 1.0)];
        list.sort_by(newest_first);
        assert_eq!(list[0].name, "B");
    }
}
