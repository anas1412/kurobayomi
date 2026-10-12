//! The app-side port of the manga downloads: the live manga source as the downloader's `ChapterSource`, and the database
//! rows as the downloader's chapter and manga refs. Page lists come from the source; each image goes over the shared
//! client with the source's request headers, and its bytes pass through the source's fix-up, as the reader's page images
//! do. No GTK here. The calls run on the runtime or the blocking pool.

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use kuro_data::chapter::Chapter;
use kuro_data::manga::Manga;
use kuro_downloads::model::{ChapterRef, MangaRef, Page};
use kuro_downloads::source::{ChapterSource, ImageResponse};
use kuro_source::model::SChapter;
use kuro_source::{MangaSource, SourceError};
use reqwest::header::CONTENT_TYPE;

/// The live manga source of a download. Its display name and id are the ones the downloads folder and the index use.
pub struct LiveChapterSource(Arc<dyn MangaSource>);

impl LiveChapterSource {
    pub fn new(source: Arc<dyn MangaSource>) -> Self {
        Self(source)
    }
}

#[async_trait]
impl ChapterSource for LiveChapterSource {
    fn id(&self) -> i64 {
        self.0.id()
    }

    fn display_name(&self) -> String {
        kuro_source::display_name(self.0.name(), self.0.lang())
    }

    fn is_unmetered(&self) -> bool {
        self.0.is_unmetered()
    }

    async fn page_list(&self, chapter: &ChapterRef) -> Result<Vec<Page>, SourceError> {
        let listed = SChapter::new(chapter.url.clone(), chapter.name.clone());
        let pages = kuro_source::isolate(self.0.get_page_list(&listed)).await?;
        Ok(pages.into_iter().enumerate().map(|(index, page)| Page::new(index as u32, page.url, page.image_url)).collect())
    }

    async fn image_url(&self, page: &Page) -> Result<String, SourceError> {
        // `MangaSource` has no image-url call, so only a page the list gave an image url for has one
        page.image_url.clone().filter(|url| !url.is_empty()).ok_or(SourceError::MissingImageUrl)
    }

    async fn fetch_image(&self, url: &str) -> Result<ImageResponse, SourceError> {
        let client = crate::recipe_sources::client().ok_or_else(|| SourceError::Transport("the network client is not running".to_owned()))?;
        let headers = crate::reader::source_pages::page_image_headers(self.0.as_ref(), crate::recipe_sources::image_headers(self.0.id())).await;
        let request = kuro_net::get(url).map_err(|e| SourceError::Url(e.to_string()))?.headers(headers);
        let response = client.execute(request).await.map_err(|e| SourceError::Transport(e.to_string()))?;
        if !(response.status.is_success() || response.status.as_u16() == 304) {
            return Err(SourceError::Http(response.status.as_u16()));
        }
        let content_type = response.headers.get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).map(str::to_owned);
        let pairs: Vec<(String, String)> = response
            .headers
            .iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str().to_owned(), value.to_owned())))
            .collect();
        let body = response.bytes().await.map_err(|e| SourceError::Transport(e.to_string()))?.to_vec();
        let body = match self.0.unscramble(&body, &pairs).await? {
            Some(bytes) => bytes,
            None => body,
        };
        let length = body.len() as u64;
        let body = futures::stream::once(async move { Ok::<Vec<u8>, SourceError>(body) }).boxed();
        Ok(ImageResponse { content_type, content_length: Some(length), body })
    }
}

/// A chapter row as the downloader reads it.
pub fn chapter_ref(chapter: &Chapter) -> ChapterRef {
    ChapterRef {
        id: chapter.id,
        manga_id: chapter.manga_id,
        url: chapter.url.clone(),
        name: chapter.name.clone(),
        scanlator: chapter.scanlator.clone(),
        read: chapter.read,
        bookmark: chapter.bookmark,
        source_order: chapter.source_order,
        chapter_number: chapter.chapter_number,
        date_upload: chapter.date_upload,
    }
}

/// A manga row as the downloader reads it.
pub fn manga_ref(manga: &Manga) -> MangaRef {
    MangaRef {
        id: manga.id,
        url: manga.url.clone(),
        title: manga.title.clone(),
        source_id: manga.source,
        description: manga.description.clone(),
        author: manga.author.clone(),
        artist: manga.artist.clone(),
        genre: manga.genre.clone(),
        status: manga.status,
    }
}

/// A manga download manager over in-memory stores, for the backend tests. The connectivity is offline, so queued
/// chapters stay queued and nothing downloads. The sender keeps the connectivity watch open and must live as long as
/// the manager.
#[cfg(test)]
pub(crate) fn test_manager(base: &std::path::Path) -> (Arc<kuro_downloads::manager::DownloadManager>, tokio::sync::watch::Sender<kuro_downloads::downloader::Connectivity>) {
    use kuro_downloads::downloader::{Connectivity, DownloadSettings, NoHold};
    use kuro_downloads::index::DownloadIndex;
    use kuro_downloads::layout::DownloadLayout;
    use kuro_downloads::manager::{DownloadManager, ManagerParams};
    use kuro_downloads::pending::MangaPendingStore;
    use kuro_downloads::store::QueueStore;

    let root = base.join("downloads");
    std::fs::create_dir_all(&root).unwrap();
    let (connectivity_tx, connectivity) = tokio::sync::watch::channel(Connectivity { online: false, metered: false });
    let manager = DownloadManager::spawn(ManagerParams {
        layout: Arc::new(DownloadLayout::new(&root)),
        index: Arc::new(DownloadIndex::open(&root, &base.join("cache"))),
        store: Arc::new(QueueStore::open_in_memory().unwrap()),
        free_space: crate::downloads::free_space,
        settings: DownloadSettings::default(),
        pending: Arc::new(MangaPendingStore::open_in_memory().unwrap()),
        page_cache: None,
        connectivity,
        hold: Arc::new(NoHold),
        restored: Vec::new(),
        kept_rows: Vec::new(),
    });
    (Arc::new(manager), connectivity_tx)
}
