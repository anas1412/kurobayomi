//! The app-side port of the manga downloads: the live manga source as the downloader's `ChapterSource`. Page lists come from the source; each image goes over the shared
//! client with the source's request headers, and its bytes pass through the source's fix-up, as the reader's page images
//! do. No GTK here. The calls run on the runtime or the blocking pool.

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use kuro_downloads::model::{ChapterRef, Page};
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
