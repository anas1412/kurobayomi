//! `LocalMangaSource`: the public API for local manga (area:14 section 1).
//!
//! Registration: id 0, name `Local manga source`, lang `other`, unmetered, latest supported. The source is listed
//! whatever the language filter says (`kuro-domain` owns that rule).

pub mod chapters;
pub mod reader;

use std::path::PathBuf;

use async_trait::async_trait;
use kuro_source::model::{MangasPage, Page, SChapter, SManga};
use kuro_source::{CatalogueSource, FilterList, MangaSource, SortSelection, SourceError};

use crate::LOCAL_SOURCE_ID;
use crate::browse::browse;
use crate::error::LocalSourceError;
use crate::format::Format;
use crate::fs::{FsEntry, file_url, find_child};
use crate::images::find_image;
use crate::sort::{LATEST_SORT, POPULAR_SORT, order_by, sort_from};

/// `LocalMangaSource.name`: `local_manga_source` in the i18n table.
pub const NAME: &str = "Local manga source";

/// The local manga source over `<storage base>/local`. `root` is `None` while the storage base is missing; every
/// listing is then empty.
#[derive(Debug, Clone)]
pub struct LocalMangaSource {
    root: Option<PathBuf>,
}

impl LocalMangaSource {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self { root }
    }

    /// Kotlin `getFormat`: how the chapter at `chapter_url` is stored, for the reader.
    pub async fn format_of_chapter(&self, chapter_url: &str) -> Result<Format, LocalSourceError> {
        let root = self.root.clone();
        let url = chapter_url.to_owned();
        crate::blocking(move || reader::format_of_chapter(root.as_deref(), &url)).await
    }

    /// The reader's entry point for a local chapter: its pages through the loader matching its format.
    pub async fn page_source(&self, chapter_url: &str) -> Result<reader::PageSource, LocalSourceError> {
        let root = self.root.clone();
        let url = chapter_url.to_owned();
        crate::blocking(move || reader::page_source(root.as_deref(), &url)).await
    }

    /// Kotlin `getMangaDetails`, cover step only (behavior 7.1 step 1). The ComicInfo and legacy JSON branches are
    /// T-14a-035 and later, and are not in this step.
    async fn details(&self, manga: &SManga) -> Result<SManga, LocalSourceError> {
        let root = self.root.clone();
        let url = manga.url.clone();
        let cover = crate::blocking(move || {
            let dir = root.and_then(|root| find_child(&root, &url));
            Ok(dir.and_then(|dir| find_image(&dir, "cover")).and_then(|p| file_url(&p)))
        })
        .await?;
        let mut updated = manga.clone();
        if cover.is_some() {
            updated.thumbnail_url = cover;
        }
        Ok(updated)
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

    async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
        Ok(self.details(manga).await?)
    }

    async fn get_chapter_list(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
        let root = self.root.clone();
        let manga = manga.clone();
        let list = crate::blocking(move || match root {
            Some(root) => chapters::chapters(&root, &manga),
            None => Ok(Vec::new()),
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
        Ok(self.browse_page(String::new(), false, Some(POPULAR_SORT)).await?)
    }

    async fn latest(&self, _page: i32) -> Result<MangasPage, SourceError> {
        Ok(self.browse_page(String::new(), true, Some(LATEST_SORT)).await?)
    }

    async fn search(&self, _page: i32, query: &str, filters: &FilterList) -> Result<MangasPage, SourceError> {
        Ok(self.browse_page(query.to_owned(), false, sort_from(filters)).await?)
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
        let page = CatalogueSource::popular(&source, 1).await.map_err(io::Error::other)?;
        let titles: Vec<&str> = page.mangas.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(titles, ["10", "9", "a"]);
        assert!(!page.has_next_page);
        Ok(())
    }

    #[tokio::test]
    async fn a_cover_file_becomes_the_thumbnail() -> io::Result<()> {
        let (root, source) = source_with(&["Show"])?;
        fs::write(root.path().join("Show").join("cover.jpg"), b"\xFF\xD8\xFF")?;
        let page = CatalogueSource::popular(&source, 1).await.map_err(io::Error::other)?;
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
        let page = CatalogueSource::latest(&source, 1).await.map_err(io::Error::other)?;
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
    async fn format_of_chapter_goes_through_the_root() -> io::Result<()> {
        let (root, source) = source_with(&["Show"])?;
        fs::create_dir(root.path().join("Show").join("Ch 1"))?;
        let format = source.format_of_chapter("Show/Ch 1").await.ok();
        assert_eq!(format, Some(Format::Directory(root.path().join("Show").join("Ch 1"))));
        Ok(())
    }
}
