//! `LocalMangaSource`: the public API for local manga (area:14 section 1).
//!
//! Registration: id 0, name `Local manga source`, lang `other`, unmetered, latest supported. The source is listed
//! whatever the language filter says (`kuro-domain` owns that rule).

pub mod chapters;
pub mod cover;
pub mod reader;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use kuro_archive::ArchiveReader;
use kuro_source::model::{MangasPage, Page, SChapter, SManga};
use kuro_source::{CatalogueSource, FilterList, MangaSource, SortSelection, SourceError};

use crate::LOCAL_SOURCE_ID;
use crate::bridge;
use crate::browse::browse;
use crate::archive as archive_entry;
use crate::comicinfo::{COMIC_INFO_FILE, ComicInfo};
use crate::error::LocalSourceError;
use crate::format::{Format, is_manga_archive};
use crate::natsort::natural_cmp_ci;
use crate::fs::{FsEntry, file_url, find_child, list_dir};
use crate::images::find_image;
use crate::json_side::{MangaDetails, apply_manga_details, details_file, read_details_json};
use crate::locks::KeyedLocks;
use crate::sort::{LATEST_SORT, POPULAR_SORT, order_by, sort_from};

/// `LocalMangaSource.name`: `local_manga_source` in the i18n table.
pub const NAME: &str = "Local manga source";

/// The local manga source over `<storage base>/local`. `root` is `None` while the storage base is missing; every
/// listing is then empty.
#[derive(Debug, Clone)]
pub struct LocalMangaSource {
    root: Option<PathBuf>,
    /// One lock per manga URL, so two details fetches for the same manga never write its files at once (RC-3).
    details_locks: Arc<KeyedLocks>,
}

impl LocalMangaSource {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            root,
            details_locks: Arc::new(KeyedLocks::default()),
        }
    }

    /// Kotlin `getFormat`: how the chapter at `chapter_url` is stored, for the reader.
    pub async fn format_of_chapter(&self, chapter_url: &str) -> Result<Format, LocalSourceError> {
        let root = self.root.clone();
        let url = chapter_url.to_owned();
        crate::blocking(move || reader::format_of_chapter(root.as_deref(), &url)).await
    }

    /// The reader's entry point for a local chapter: its pages through the loader matching its format.
    pub async fn page_source(
        &self,
        chapter_url: &str,
    ) -> Result<reader::PageSource, LocalSourceError> {
        let root = self.root.clone();
        let url = chapter_url.to_owned();
        crate::blocking(move || reader::page_source(root.as_deref(), &url)).await
    }

    /// Kotlin `getMangaDetails`: the cover step (behavior 7.1 step 1), then the legacy `details.json` branch (T-14a-037).
    /// The ComicInfo branch (T-14a-035) and the archive copy (T-14a-036) are not in this step.
    async fn details(&self, manga: &SManga) -> Result<SManga, LocalSourceError> {
        let root = self.root.clone();
        let manga = manga.clone();
        crate::blocking(move || {
            let mut updated = manga;
            let dir = root.and_then(|root| find_child(&root, &updated.url));
            let Some(dir) = dir else {
                tracing::error!(
                    "Error setting manga details from local metadata: folder not found ({})",
                    updated.url
                );
                return Ok(updated);
            };
            if let Some(cover) = find_image(&dir, "cover").and_then(|p| file_url(&p)) {
                updated.thumbnail_url = Some(cover);
            }
            apply_details_files(&dir, &mut updated);
            Ok(updated)
        })
        .await
    }

    async fn browse_page(
        &self,
        query: String,
        latest: bool,
        sort: Option<SortSelection>,
    ) -> Result<MangasPage, LocalSourceError> {
        let mangas = browse(self.root.clone(), query, latest, sort, manga_entry).await?;
        Ok(MangasPage {
            mangas,
            has_next_page: false,
        })
    }
}

/// One browse row: the folder name is both title and URL, and the cover is `cover.*` when there is one.
/// Marker file that says a manga folder was scanned for ComicInfo and none was found (behavior 7.1 step 6c).
const NO_XML_FILE: &str = ".noxml";

/// Kotlin `getMangaDetails`, steps 5 and 6 (behavior 7.1). The first matching branch runs:
/// (a) a top-level `ComicInfo.xml` is applied and `.noxml` is removed; (b) a legacy `details.json` is applied, written
/// out as `ComicInfo.xml`, and deleted only after that write; (c) with no marker, the first archive that holds a root
/// `ComicInfo.xml` (natural name order) has it copied into the folder, or `.noxml` is created. A failure is logged and
/// the manga keeps the fields it already has (behavior 7.6).
fn apply_details_files(dir: &Path, manga: &mut SManga) {
    let entries = match list_dir(dir) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::error!("Error reading manga folder for details: {error} ({})", dir.display());
            return;
        }
    };
    let comic_info = entries.iter().find(|e| e.name == COMIC_INFO_FILE);
    let no_xml = entries.iter().find(|e| e.name == NO_XML_FILE);
    if let Some(info) = comic_info {
        if let Some(marker) = no_xml {
            if let Err(error) = fs::remove_file(&marker.path) {
                tracing::error!("Error deleting {NO_XML_FILE}: {error} ({})", marker.path.display());
            }
        }
        apply_comic_info_file(&info.path, manga);
        return;
    }
    if let Some(json) = details_file(&entries) {
        apply_legacy_details(dir, json, manga);
        return;
    }
    if no_xml.is_some() {
        return;
    }
    match copy_comic_info_from_archive(dir, &entries) {
        Some(copied) => apply_comic_info_file(&copied, manga),
        None => create_no_xml_marker(dir),
    }
}

/// Reads a ComicInfo file and copies its fields into the manga. A read or parse failure is logged; the file stays.
fn apply_comic_info_file(path: &Path, manga: &mut SManga) {
    let parsed = fs::read(path).map_err(LocalSourceError::from).and_then(|bytes| ComicInfo::from_xml(&bytes));
    match parsed {
        Ok(info) => bridge::apply_comic_info(manga, &info),
        Err(error) => tracing::error!("Error reading {COMIC_INFO_FILE}: {error} ({})", path.display()),
    }
}

/// Branch (b): decodes `details.json`, copies its fields, writes `ComicInfo.xml`, and deletes the JSON only after the
/// write succeeds. A decode or write failure is logged and the file is kept.
fn apply_legacy_details(dir: &Path, json: &FsEntry, manga: &mut SManga) {
    let details = match read_details_json::<MangaDetails>(&json.path) {
        Ok(details) => details,
        Err(error) => {
            tracing::error!("Error reading details.json: {error} ({})", json.path.display());
            return;
        }
    };
    apply_manga_details(manga, &details);
    let xml = bridge::manga_to_comic_info(manga).to_xml();
    match fs::write(dir.join(COMIC_INFO_FILE), xml) {
        Ok(()) => {
            if let Err(error) = fs::remove_file(&json.path) {
                tracing::error!("Error deleting details.json: {error} ({})", json.path.display());
            }
        }
        Err(error) => tracing::error!("Error writing ComicInfo.xml from details.json: {error} ({})", dir.display()),
    }
}

/// Branch (c), `copyComicInfoFileFromArchive`: the first archive (natural name order) whose root holds exactly
/// `ComicInfo.xml` is copied into `dir`. Directories are skipped, and an archive that cannot be opened is logged and
/// passed over. `None` when no archive has one.
fn copy_comic_info_from_archive(dir: &Path, entries: &[FsEntry]) -> Option<PathBuf> {
    let mut archives: Vec<&FsEntry> = entries.iter().filter(|e| e.is_file && is_manga_archive(&e.name)).collect();
    archives.sort_by(|a, b| natural_cmp_ci(&a.name, &b.name));
    for archive in archives {
        let reader = match ArchiveReader::open(&archive.path) {
            Ok(reader) => reader,
            Err(error) => {
                tracing::error!("Error opening archive for ComicInfo: {error} ({})", archive.path.display());
                continue;
            }
        };
        match archive_entry::read_entry(&reader, COMIC_INFO_FILE) {
            Ok(Some(bytes)) => {
                let target = dir.join(COMIC_INFO_FILE);
                return match fs::write(&target, bytes) {
                    Ok(()) => Some(target),
                    Err(error) => {
                        tracing::error!("Error copying {COMIC_INFO_FILE}: {error} ({})", dir.display());
                        None
                    }
                };
            }
            Ok(None) => {}
            Err(error) => tracing::error!("Error reading archive entry: {error} ({})", archive.path.display()),
        }
    }
    None
}

/// Creates the empty `.noxml` marker so later details fetches skip the archive scan. An existing marker is kept.
fn create_no_xml_marker(dir: &Path) {
    match fs::OpenOptions::new().write(true).create_new(true).open(dir.join(NO_XML_FILE)) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => tracing::error!("Error creating {NO_XML_FILE}: {error} ({})", dir.display()),
    }
}

fn manga_entry(entry: &FsEntry) -> SManga {
    let mut manga = SManga::new(entry.name.clone(), entry.name.clone());
    manga.thumbnail_url = find_image(&entry.path, "cover").and_then(|p| file_url(&p));
    manga
}

#[async_trait]
impl MangaSource for LocalMangaSource {
    fn id(&self) -> i64 {
        LOCAL_SOURCE_ID
    }

    fn name(&self) -> &str {
        NAME
    }

    fn lang(&self) -> &str {
        "other"
    }

    fn is_unmetered(&self) -> bool {
        true
    }

    /// A failed details read is logged and the manga comes back unchanged (behavior 31, E-MANGA-1).
    async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
        let _serialised = self.details_locks.lock(&manga.url).await;
        match self.details(manga).await {
            Ok(updated) => Ok(updated),
            Err(error) => {
                tracing::error!(
                    "Error setting manga details from local metadata for {}: {error} ({})",
                    manga.title,
                    manga.url
                );
                Ok(manga.clone())
            }
        }
    }

    async fn get_chapter_list(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
        let root = self.root.clone();
        let mut manga = manga.clone();
        let list = crate::blocking(move || match root {
            Some(root) => {
                let list = chapters::chapters(&root, &mut manga)?;
                cover::cover_from_lowest_chapter(&root, &manga, &list);
                Ok(list)
            }
            None => Err(LocalSourceError::folder_missing()),
        })
        .await?;
        Ok(list)
    }

    /// Kotlin `getPageList` throws `UnsupportedOperationException("Unused")`: pages come from the reader.
    async fn get_page_list(&self, _chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
        Err(SourceError::Unsupported("get_page_list"))
    }
}

#[async_trait]
impl CatalogueSource for LocalMangaSource {
    fn supports_latest(&self) -> bool {
        true
    }

    fn filter_list(&self) -> FilterList {
        FilterList(vec![order_by(Some(POPULAR_SORT))])
    }

    async fn popular(&self, _page: i32) -> Result<MangasPage, SourceError> {
        Ok(self
            .browse_page(String::new(), false, Some(POPULAR_SORT))
            .await?)
    }

    async fn latest(&self, _page: i32) -> Result<MangasPage, SourceError> {
        Ok(self
            .browse_page(String::new(), true, Some(LATEST_SORT))
            .await?)
    }

    async fn search(
        &self,
        _page: i32,
        query: &str,
        filters: &FilterList,
    ) -> Result<MangasPage, SourceError> {
        Ok(self
            .browse_page(query.to_owned(), false, sort_from(filters))
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io;

    fn source_with(folders: &[&str]) -> io::Result<(tempfile::TempDir, LocalMangaSource)> {
        let root = tempfile::tempdir()?;
        for folder in folders {
            fs::create_dir(root.path().join(folder))?;
        }
        let source = LocalMangaSource::new(Some(root.path().to_path_buf()));
        Ok((root, source))
    }

    #[test]
    fn the_registration_is_the_kotlin_identity() {
        let source = LocalMangaSource::new(None);
        assert_eq!(MangaSource::id(&source), 0);
        assert_eq!(MangaSource::name(&source), "Local manga source");
        assert_eq!(MangaSource::lang(&source), "other");
        assert!(MangaSource::is_unmetered(&source));
        assert!(CatalogueSource::supports_latest(&source));
        assert_eq!(CatalogueSource::filter_list(&source)[0].name(), "Order by");
    }

    #[tokio::test]
    async fn popular_is_title_ascending_with_no_next_page() -> io::Result<()> {
        let (_root, source) = source_with(&["9", "10", "a"])?;
        let page = CatalogueSource::popular(&source, 1)
            .await
            .map_err(io::Error::other)?;
        let titles: Vec<&str> = page.mangas.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(titles, ["10", "9", "a"]);
        assert!(!page.has_next_page);
        Ok(())
    }

    #[tokio::test]
    async fn a_cover_file_becomes_the_thumbnail() -> io::Result<()> {
        let (root, source) = source_with(&["Show"])?;
        fs::write(root.path().join("Show").join("cover.jpg"), b"\xFF\xD8\xFF")?;
        let page = CatalogueSource::popular(&source, 1)
            .await
            .map_err(io::Error::other)?;
        let thumb = page.mangas[0].thumbnail_url.clone().unwrap_or_default();
        assert!(
            thumb.starts_with("file://") && thumb.ends_with("/Show/cover.jpg"),
            "{thumb}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_matches_the_name_and_uses_the_sort_filter() -> io::Result<()> {
        let (_root, source) = source_with(&["Bleach", "Naruto", "naruto x"])?;
        let filters = FilterList(vec![order_by(Some(SortSelection {
            index: 0,
            ascending: false,
        }))]);
        let page = CatalogueSource::search(&source, 1, "naruto", &filters)
            .await
            .map_err(io::Error::other)?;
        let titles: Vec<&str> = page.mangas.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(titles, ["naruto x", "Naruto"]);
        Ok(())
    }

    #[tokio::test]
    async fn a_missing_root_lists_nothing() -> io::Result<()> {
        let source = LocalMangaSource::new(None);
        let page = CatalogueSource::latest(&source, 1)
            .await
            .map_err(io::Error::other)?;
        assert!(page.mangas.is_empty());
        assert!(!page.has_next_page);
        Ok(())
    }

    #[tokio::test]
    async fn the_page_list_is_unsupported_like_kotlin() {
        let source = LocalMangaSource::new(None);
        let chapter = SChapter::new("Show/Ch 1", "Ch 1");
        let got = MangaSource::get_page_list(&source, &chapter).await;
        assert_eq!(got.err(), Some(SourceError::Unsupported("get_page_list")));
    }

    #[tokio::test]
    async fn the_chapter_list_comes_from_the_folder() -> io::Result<()> {
        let (root, source) = source_with(&["Show"])?;
        fs::create_dir(root.path().join("Show").join("Show 2"))?;
        fs::write(root.path().join("Show").join("Show 1.cbz"), b"x")?;
        let manga = SManga::new("Show", "Show");
        let got = MangaSource::get_chapter_list(&source, &manga)
            .await
            .map_err(io::Error::other)?;
        let urls: Vec<&str> = got.iter().map(|c| c.url.as_str()).collect();
        assert_eq!(urls, ["Show/Show 2", "Show/Show 1.cbz"]);
        Ok(())
    }

    #[tokio::test]
    async fn details_fills_only_the_cover_here() -> io::Result<()> {
        let (root, source) = source_with(&["Show"])?;
        fs::write(
            root.path().join("Show").join("cover.png"),
            b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR",
        )?;
        let manga = SManga::new("Show", "Show");
        let updated = MangaSource::get_manga_details(&source, &manga)
            .await
            .map_err(io::Error::other)?;
        assert!(updated.thumbnail_url.is_some());
        assert_eq!(updated.title, "Show");
        Ok(())
    }

    #[tokio::test]
    async fn legacy_details_json_is_applied_then_replaced_by_comicinfo() -> io::Result<()> {
        let (root, source) = source_with(&["Show"])?;
        let show = root.path().join("Show");
        fs::write(
            show.join("details.json"),
            r#"{"title":"Real","author":"A","genre":["Action","Drama"],"status":1}"#,
        )?;
        let updated = MangaSource::get_manga_details(&source, &SManga::new("Show", "Show"))
            .await
            .map_err(io::Error::other)?;
        assert_eq!(updated.title, "Real");
        assert_eq!(updated.genre.as_deref(), Some("Action, Drama"));
        assert_eq!(updated.status, 1);
        assert!(show.join("ComicInfo.xml").is_file());
        assert!(!show.join("details.json").exists());
        Ok(())
    }

    #[tokio::test]
    async fn a_bad_legacy_details_json_is_kept_and_changes_nothing() -> io::Result<()> {
        let (root, source) = source_with(&["Show"])?;
        let show = root.path().join("Show");
        fs::write(show.join("details.json"), r#"{"title": 5"#)?;
        let updated = MangaSource::get_manga_details(&source, &SManga::new("Show", "Show"))
            .await
            .map_err(io::Error::other)?;
        assert_eq!(updated.title, "Show");
        assert!(show.join("details.json").is_file());
        assert!(!show.join("ComicInfo.xml").exists());
        Ok(())
    }

    #[tokio::test]
    async fn t_14_details_concurrent_same_manga_single_write() -> io::Result<()> {
        let (_root, source) = source_with(&["Show", "Other"])?;
        // Another details write holds `Show`: a second fetch for `Show` waits, a fetch for `Other` does not.
        let held = source.details_locks.lock("Show").await;
        let same = source.clone();
        let waiting = tokio::spawn(async move {
            MangaSource::get_manga_details(&same, &SManga::new("Show", "Show")).await
        });
        let other = MangaSource::get_manga_details(&source, &SManga::new("Other", "Other"))
            .await
            .map_err(io::Error::other)?;
        assert_eq!(other.title, "Other");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !waiting.is_finished(),
            "the second fetch must wait for the first"
        );
        drop(held);
        waiting
            .await
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
        Ok(())
    }

    #[tokio::test]
    async fn a_missing_manga_folder_returns_the_manga_unchanged() -> io::Result<()> {
        let (_root, source) = source_with(&[])?;
        let manga = SManga::new("Gone", "Gone");
        let updated = MangaSource::get_manga_details(&source, &manga)
            .await
            .map_err(io::Error::other)?;
        assert_eq!(updated, manga);
        Ok(())
    }

    #[tokio::test]
    async fn format_of_chapter_goes_through_the_root() -> io::Result<()> {
        let (root, source) = source_with(&["Show"])?;
        fs::create_dir(root.path().join("Show").join("Ch 1"))?;
        let format = source.format_of_chapter("Show/Ch 1").await.ok();
        assert_eq!(
            format,
            Some(Format::Directory(root.path().join("Show").join("Ch 1")))
        );
        Ok(())
    }
}
