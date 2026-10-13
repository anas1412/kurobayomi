//! The source port of a manga download and the page types its signatures use. The app implements it over `MangaSource`
//! and the HTTP client, so this crate does not depend on `reqwest`. Errors are `kuro_source::SourceError`.

use std::path::PathBuf;

use async_trait::async_trait;
use futures::stream::BoxStream;
use kuro_source::SourceError;


/// The body of an image response, as a stream of chunks.
pub type ImageStream = BoxStream<'static, Result<Vec<u8>, SourceError>>;

/// An image response: the Content-Type header, the Content-Length header when present, and the body.
pub struct ImageResponse {
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub body: ImageStream,
}

#[async_trait]
pub trait ChapterSource: Send + Sync {
    fn id(&self) -> i64;
    /// `"Name (LANG)"`, as `HttpSource.toString()`. The folder name is derived from it.
    fn display_name(&self) -> String;
    /// `false` for local sources, which the queue refuses (EC-08).
    fn is_http(&self) -> bool {
        true
    }
    /// `true` for sources on the user's own machine, which the queue-size warnings leave out.
    fn is_unmetered(&self) -> bool {
        false
    }
    /// The manga folder of the local source, whose chapters are the folders and archives inside it. `None` for every
    /// other source, which the download index counts (T-23a-042).
    fn local_manga_dir(&self, _manga_title: &str) -> Option<std::path::PathBuf> {
        None
    }
    async fn page_list(&self, chapter: &ChapterRef) -> Result<Vec<Page>, SourceError>;
    async fn image_url(&self, page: &Page) -> Result<String, SourceError>;
    async fn fetch_image(&self, image_url: &str) -> Result<ImageResponse, SourceError>;
}

/// A source that is not installed, known only by its id and the download folder the index found for it
/// ([`crate::index::DownloadIndex::manga_source_dir`]). It names that folder so the downloads can still be deleted; it
/// fetches nothing.
pub struct FolderOnlySource {
    pub id: i64,
    pub folder: String,
}

#[async_trait]
impl ChapterSource for FolderOnlySource {
    fn id(&self) -> i64 {
        self.id
    }

    /// The folder name itself: the layout sanitises it again, which leaves a name it produced unchanged.
    fn display_name(&self) -> String {
        self.folder.clone()
    }

    async fn page_list(&self, _chapter: &ChapterRef) -> Result<Vec<Page>, SourceError> {
        Err(SourceError::Unsupported("the source is not installed"))
    }

    async fn image_url(&self, _page: &Page) -> Result<String, SourceError> {
        Err(SourceError::Unsupported("the source is not installed"))
    }

    async fn fetch_image(&self, _image_url: &str) -> Result<ImageResponse, SourceError> {
        Err(SourceError::Unsupported("the source is not installed"))
    }
}

/// `Page.State`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageState {
    Queue,
    LoadPage,
    DownloadImage,
    Ready,
    Error,
}

/// One page of a chapter. `index` is 0-based and reassigned by the downloader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub index: u32,
    pub url: String,
    pub image_url: Option<String>,
    /// The finished file, set once the page is `Ready`.
    pub file: Option<PathBuf>,
    pub state: PageState,
    /// 0..=100, or -1 when the content length is unknown.
    pub progress: i32,
}

impl Page {
    pub fn new(index: u32, url: impl Into<String>, image_url: Option<String>) -> Self {
        Self { index, url: url.into(), image_url, file: None, state: PageState::Queue, progress: 0 }
    }

    /// 1-based page number, as used in file names.
    pub fn number(&self) -> u32 {
        self.index + 1
    }
}

/// The chapter fields the downloader reads (the Kotlin `Chapter` subset used by this area).
#[derive(Clone, Debug, PartialEq)]
pub struct ChapterRef {
    pub id: i64,
    pub manga_id: i64,
    pub url: String,
    pub name: String,
    pub scanlator: Option<String>,
    pub read: bool,
    pub bookmark: bool,
    pub source_order: i64,
    pub chapter_number: f64,
    pub date_upload: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_number_is_one_based() {
        let page = Page::new(4, "u", None);
        assert_eq!(page.number(), 5);
        assert_eq!(page.state, PageState::Queue);
    }
}
