//! Scans one local folder (area 14, behaviour sections 8 and 9.3; wire-local-anime). The folder is the entry and its
//! name is the title. A manga folder's visible subfolders, manga archives and EPUB files are its chapters, in kuro-local's
//! order (newest number first); an anime folder's video files are its episodes, as kuro-local lists them. Every item's
//! url is its absolute path.

use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};

use kuro_platform::kind::Kind;
use kuro_local::anime::episodes::episode_list;
use kuro_local::archive::first_page_bytes;
use kuro_local::epub::first_image_bytes;
use kuro_local::format::{Format, format_of};
use kuro_local::fs::{FsEntry, file_url, list_dir};
use kuro_local::images::{find_image, set_cover};
use kuro_local::manga::reader::directory_pages;
use kuro_local::names::{is_hidden, stem};
use kuro_local::natsort::natural_cmp_ci;
use kuro_local::recognition::chapter_number;
use kuro_source::anime::SAnime;

/// One chapter or episode found in the folder.
#[derive(Clone, Debug, PartialEq)]
pub struct ScannedItem {
    /// The absolute path of the chapter folder, archive or EPUB, or of the video file. It is the item's url.
    pub path: PathBuf,
    pub name: String,
    pub date_upload: i64,
    /// The chapter or episode number, `-1.0` when the name holds none.
    pub number: f64,
}

/// One local folder and its items.
#[derive(Clone, Debug, PartialEq)]
pub struct ScannedFolder {
    /// The absolute path of the folder. It is the entry's url.
    pub path: PathBuf,
    pub title: String,
    pub items: Vec<ScannedItem>,
    /// The `file://` URL of a manga cover, or `None` when the folder has no image. An anime folder has none here: its
    /// cover is a video frame (`library::frame_cover`).
    pub cover: Option<String>,
}

/// Scans `folder` as an entry of `kind`.
pub fn scan(kind: Kind, folder: &Path) -> io::Result<ScannedFolder> {
    let title = folder
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("the folder name is not UTF-8"))?
        .to_owned();
    let (items, cover) = match kind {
        Kind::Manga => {
            let chapters = chapters(folder, &title)?;
            let cover = cover_of(folder, chapters.last())?;
            (chapters, cover)
        }
        Kind::Anime => (episodes(folder, &title)?, None),
    };
    Ok(ScannedFolder { path: folder.to_path_buf(), title, items, cover })
}

/// The visible folders directly under `root`, in name order: an anime root holds one folder per series. Files and
/// hidden folders are left out.
pub fn series_in(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found: Vec<FsEntry> = list_dir(root)?.into_iter().filter(|e| e.is_dir && !is_hidden(&e.name)).collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found.into_iter().map(|e| e.path).collect())
}

/// The chapters of a manga folder. Hidden entries and files that are neither a folder, an EPUB nor a manga archive are
/// skipped.
fn chapters(folder: &Path, title: &str) -> io::Result<Vec<ScannedItem>> {
    let mut found = Vec::new();
    for entry in list_dir(folder)? {
        if !is_hidden(&entry.name) && format_of(&entry.path, entry.is_dir).is_some() {
            found.push(entry);
        }
    }
    let stem_of = |e: &FsEntry| if e.is_dir { e.name.clone() } else { stem(&e.name).to_owned() };
    let mut chapters = Vec::new();
    for entry in &found {
        let mut name = stem_of(entry);
        // a file whose name matches another chapter keeps its extension, so every chapter name is unique
        if !entry.is_dir && found.iter().filter(|other| stem_of(other) == name).count() > 1 {
            name = entry.name.clone();
        }
        chapters.push(ScannedItem {
            number: f64::from(chapter_number(title, &name)),
            name,
            date_upload: entry.modified_ms,
            path: entry.path.clone(),
        });
    }
    chapters.sort_by(newest_first);
    Ok(chapters)
}

/// The episodes of an anime folder, newest number first as kuro-local lists them. kuro-local's url is
/// `<series>/<file>` relative to the parent folder, so the item's path is that url joined onto the parent.
fn episodes(folder: &Path, title: &str) -> io::Result<Vec<ScannedItem>> {
    let root = folder.parent().ok_or_else(|| io::Error::other("the folder has no parent folder"))?;
    let anime = SAnime { url: title.to_owned(), title: title.to_owned(), ..SAnime::default() };
    let found = episode_list(root, &anime).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(found
        .into_iter()
        .map(|e| ScannedItem {
            path: root.join(&e.url),
            name: e.name,
            date_upload: e.date_upload,
            number: f64::from(e.episode_number),
        })
        .collect())
}

/// kuro-local's chapter order: the larger number first, then the name descending, natural and case-insensitive.
pub fn newest_first(a: &ScannedItem, b: &ScannedItem) -> Ordering {
    b.number.total_cmp(&a.number).then_with(|| natural_cmp_ci(&b.name, &a.name))
}

/// The folder's own `cover.*` image, else the first page of the lowest chapter (written as `cover.jpg` for an archive,
/// as kuro-local does).
fn cover_of(folder: &Path, lowest: Option<&ScannedItem>) -> io::Result<Option<String>> {
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
    use std::fs;

    use super::*;

    fn chapter(name: &str, number: f64) -> ScannedItem {
        ScannedItem { path: PathBuf::from(name), name: name.to_owned(), date_upload: 0, number }
    }

    /// A temporary root with these files (and their folders).
    fn tree(files: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temp dir");
        for file in files {
            let path = root.path().join(file);
            fs::create_dir_all(path.parent().expect("parent")).expect("series folder");
            fs::write(&path, b"video").expect("video file");
        }
        root
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

    #[test]
    fn series_are_visible_folders_in_name_order() {
        let root = tree(&["Zeta/ep.mkv", "Alpha/ep.mkv", ".hidden/ep.mkv", "loose.mkv"]);
        fs::create_dir(root.path().join("Empty")).expect("empty folder");
        let names: Vec<PathBuf> = series_in(root.path()).expect("list");
        assert_eq!(names, ["Alpha", "Empty", "Zeta"].map(|n| root.path().join(n)));
    }

    #[test]
    fn a_missing_root_is_an_error_not_an_empty_library() {
        let root = tempfile::tempdir().expect("temp dir");
        assert!(series_in(&root.path().join("gone")).is_err());
    }

    #[test]
    fn episodes_get_absolute_urls_and_numbers() {
        let root = tree(&["Show/Show - 1.mkv", "Show/Show - 2.mp4", "Show/notes.txt"]);
        let scanned = scan(Kind::Anime, &root.path().join("Show")).expect("scan");
        assert_eq!(scanned.title, "Show");
        assert_eq!(scanned.cover, None);
        let numbers: Vec<f64> = scanned.items.iter().map(|e| e.number).collect();
        assert_eq!(numbers, [2.0, 1.0]);
        assert_eq!(scanned.items[0].path, root.path().join("Show").join("Show - 2.mp4"));
    }

    #[test]
    fn an_anime_folder_without_episode_files_gives_no_items() {
        let root = tree(&["Show/cover.jpg"]);
        assert!(scan(Kind::Anime, &root.path().join("Show")).expect("scan").items.is_empty());
    }
}
