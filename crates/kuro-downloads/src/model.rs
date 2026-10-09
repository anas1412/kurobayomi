//! Download state, page state, page, queued download and chapter, manga and source refs (port of MangaDownload.kt).

use std::path::PathBuf;
use std::sync::Arc;

use crate::source::ChapterSource;

/// Shared handle to a source. The queue keeps one per item, so a running chapter can reach its source.
pub type SourceRef = Arc<dyn ChapterSource>;

/// `MangaDownload.State`. The numeric values are stored in the queue table, and "active" means `<= Downloading`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DownloadState {
    NotDownloaded = 0,
    Queue = 1,
    Downloading = 2,
    Downloaded = 3,
    Error = 4,
}

impl DownloadState {
    /// Kotlin `status.value <= DOWNLOADING.value`: states 0, 1 and 2.
    pub fn is_active(self) -> bool {
        (self as u8) <= (Self::Downloading as u8)
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

/// The manga fields the downloader reads.
#[derive(Clone, Debug, PartialEq)]
pub struct MangaRef {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub source_id: i64,
    pub description: Option<String>,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: i64,
}

/// One queue item. Shared with the UI as an `Arc` snapshot; only the downloader actor mutates it.
#[derive(Clone)]
pub struct QueuedDownload {
    pub source: SourceRef,
    pub manga: MangaRef,
    pub chapter: ChapterRef,
    pub status: DownloadState,
    pub pages: Option<Vec<Page>>,
}

impl QueuedDownload {
    /// Sum of page progress (Kotlin `totalProgress`).
    pub fn total_progress(&self) -> i64 {
        self.pages.as_ref().map_or(0, |pages| pages.iter().map(|p| p.progress as i64).sum())
    }

    /// Number of pages in `Ready` state (Kotlin `downloadedImages`).
    pub fn downloaded_images(&self) -> usize {
        self.pages.as_ref().map_or(0, |pages| pages.iter().filter(|p| p.state == PageState::Ready).count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_state_values_are_stable() {
        assert_eq!(DownloadState::NotDownloaded as u8, 0);
        assert_eq!(DownloadState::Queue as u8, 1);
        assert_eq!(DownloadState::Downloading as u8, 2);
        assert_eq!(DownloadState::Downloaded as u8, 3);
        assert_eq!(DownloadState::Error as u8, 4);
    }

    #[test]
    fn only_states_zero_to_two_are_active() {
        let active: Vec<u8> = [
            DownloadState::NotDownloaded,
            DownloadState::Queue,
            DownloadState::Downloading,
            DownloadState::Downloaded,
            DownloadState::Error,
        ]
        .into_iter()
        .filter(|s| s.is_active())
        .map(|s| s as u8)
        .collect();
        assert_eq!(active, vec![0, 1, 2]);
    }

    #[test]
    fn page_number_is_one_based() {
        let page = Page::new(4, "u", None);
        assert_eq!(page.number(), 5);
        assert_eq!(page.state, PageState::Queue);
    }

    #[test]
    fn progress_sums_pages_and_counts_ready_ones() {
        let src = crate::testing::fake_source(1);
        let mut item = crate::testing::download(&src, 1, 1, DownloadState::Downloading);
        assert_eq!(item.total_progress(), 0);
        assert_eq!(item.downloaded_images(), 0);
        let mut ready = Page::new(0, "a", None);
        ready.state = PageState::Ready;
        ready.progress = 100;
        let mut busy = Page::new(1, "b", None);
        busy.state = PageState::DownloadImage;
        busy.progress = 40;
        item.pages = Some(vec![ready, busy, Page::new(2, "c", None)]);
        assert_eq!(item.total_progress(), 140);
        assert_eq!(item.downloaded_images(), 1);
    }
}
