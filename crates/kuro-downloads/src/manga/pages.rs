//! The pages of a downloaded chapter, as the reader opens them (was `DownloadManager::build_page_list`).

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use kuro_archive::ArchiveReader;
use kuro_core::image::is_image_name;

use crate::error::DownloadError;
use crate::layout::Layout;
use crate::naming::UnitName;
use crate::unit::{SourceKey, UnitRef};

/// One page of a downloaded chapter, as the reader opens it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalPage {
    pub index: u32,
    /// The image file, or the `.cbz` archive that holds the page.
    pub path: PathBuf,
    /// The entry name inside the `.cbz`. `None` for a file in a chapter folder.
    pub is_cbz_entry: Option<String>,
}

/// The pages of a downloaded chapter in reading order: image files of the folder sorted by name, or the image entries
/// of the `.cbz`. No pages is an error, as when the chapter is not downloaded (behaviour 9).
pub async fn build_page_list(
    layout: Layout,
    source: &SourceKey,
    title: &str,
    chapter: &UnitRef,
) -> Result<Vec<LocalPage>, DownloadError> {
    let source_dir = Layout::source_dir_name(&source.name, &source.lang);
    let (title, chapter) = (title.to_owned(), chapter.clone());
    tokio::task::spawn_blocking(move || {
        let name = UnitName::new(&chapter.name, chapter.scanlator.as_deref());
        let path = layout.find_unit(&source_dir, &title, name).ok_or(DownloadError::NoPages)?;
        let pages = if path.is_dir() { folder_pages(&path)? } else { cbz_pages(&path)? };
        if pages.is_empty() { Err(DownloadError::NoPages) } else { Ok(pages) }
    })
    .await
    .map_err(|e| DownloadError::Internal(e.to_string()))?
}

/// Up to 32 bytes of a file, for the image sniff.
fn head(path: &Path) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    fs::File::open(path).ok()?.take(32).read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn folder_pages(dir: &Path) -> io::Result<Vec<LocalPage>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| is_image_name(n, || head(p))))
        .collect();
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files.into_iter().enumerate().map(|(i, path)| LocalPage { index: i as u32, path, is_cbz_entry: None }).collect())
}

fn cbz_pages(cbz: &Path) -> Result<Vec<LocalPage>, DownloadError> {
    let entries = ArchiveReader::open(cbz)
        .and_then(|reader| reader.page_entries())
        .map_err(|e| DownloadError::Io(io::Error::other(e)))?;
    Ok(entries
        .into_iter()
        .enumerate()
        .map(|(i, entry)| LocalPage { index: i as u32, path: cbz.to_owned(), is_cbz_entry: Some(entry.name) })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::fixtures;
    use kuro_platform::kind::Kind;

    fn chapter(name: &str) -> UnitRef {
        UnitRef { name: name.to_owned(), ..fixtures::queued(Kind::Manga, 1, 1, 1).unit }
    }

    fn source() -> SourceKey {
        SourceKey { name: "Source 1".to_owned(), ..fixtures::source(1) }
    }

    #[tokio::test]
    async fn build_page_list_sorts_the_images_of_a_folder_and_skips_other_files() {
        let root = tempfile::tempdir().unwrap();
        let manga_dir = root.path().join("Source 1 (EN)").join("Berserk");
        let dir = manga_dir.join("Chapter 1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("002.jpg"), crate::testing::jpeg_bytes(16)).unwrap();
        std::fs::write(dir.join("001.jpg"), crate::testing::jpeg_bytes(16)).unwrap();
        std::fs::write(dir.join("ComicInfo.xml"), b"<ComicInfo/>").unwrap();
        std::fs::create_dir_all(manga_dir.join("Chapter 2")).unwrap();
        std::fs::write(manga_dir.join("Chapter 2").join("notes.txt"), b"text").unwrap();
        let layout = Layout::new(Kind::Manga, root.path());
        let pages = build_page_list(layout.clone(), &source(), "Berserk", &chapter("Chapter 1")).await.unwrap();
        let names: Vec<_> = pages.iter().map(|p| p.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["001.jpg", "002.jpg"]);
        assert_eq!(pages.iter().map(|p| p.index).collect::<Vec<_>>(), vec![0, 1]);
        assert!(pages.iter().all(|p| p.is_cbz_entry.is_none()));
        for missing in ["Chapter 2", "Chapter 3"] {
            let result = build_page_list(layout.clone(), &source(), "Berserk", &chapter(missing)).await;
            assert!(matches!(result, Err(DownloadError::NoPages)), "EC-37: {missing}: {result:?}");
        }
    }

    #[tokio::test]
    async fn build_page_list_reads_the_image_entries_of_a_cbz_in_natural_order() {
        let root = tempfile::tempdir().unwrap();
        let manga_dir = root.path().join("Source 1 (EN)").join("Berserk");
        let parts = root.path().join("parts");
        std::fs::create_dir_all(&manga_dir).unwrap();
        std::fs::create_dir_all(&parts).unwrap();
        let jpeg = crate::testing::jpeg_bytes(16);
        let cbz = manga_dir.join("Chapter 1.cbz");
        let mut writer = kuro_archive::CbzWriter::create(&cbz).unwrap();
        for (name, data) in [("10.jpg", jpeg.as_slice()), ("2.jpg", jpeg.as_slice()), ("readme.txt", b"hello".as_slice())] {
            std::fs::write(parts.join(name), data).unwrap();
            writer.add_file(&parts.join(name)).unwrap();
        }
        writer.finish().unwrap();
        let layout = Layout::new(Kind::Manga, root.path());
        let pages = build_page_list(layout, &source(), "Berserk", &chapter("Chapter 1")).await.unwrap();
        let entries: Vec<_> = pages.iter().map(|p| p.is_cbz_entry.clone().unwrap()).collect();
        assert_eq!(entries, vec!["2.jpg", "10.jpg"]);
        assert!(pages.iter().all(|p| p.path == cbz));
    }
}
