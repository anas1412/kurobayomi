//! The source port the downloader needs. Area 15 and 17 implement it over `MangaSource` and the HTTP client, so this
//! crate does not depend on `reqwest`. Errors are `kuro_source::SourceError`.

use async_trait::async_trait;
use futures::stream::BoxStream;
use kuro_source::SourceError;

use crate::model::{ChapterRef, Page};

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
