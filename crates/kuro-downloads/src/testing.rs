//! Test fixtures shared by the unit tests: a scripted in-memory source and ref builders. Compiled only for tests.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::StreamExt;
use futures::stream;
use kuro_source::SourceError;

use crate::model::{ChapterRef, DownloadState, MangaRef, Page, QueuedDownload, SourceRef};
use crate::source::{ChapterSource, ImageResponse};

/// A JPEG-looking body: magic bytes followed by filler, so the sniffer sees a JPEG.
pub fn jpeg_bytes(len: usize) -> Vec<u8> {
    let mut body = vec![0xFF, 0xD8, 0xFF, 0xE0];
    body.resize(len.max(4), b'x');
    body
}

pub struct FakeSource {
    pub id: i64,
    /// Overrides the display name, for rename tests.
    pub name: Option<String>,
    pub http: bool,
    /// Makes this the local source: the manga folder is `<dir>/<title>`.
    pub local_dir: Option<std::path::PathBuf>,
    pub unmetered: bool,
    pub page_count: usize,
    pub page_list_error: Option<SourceError>,
    pub image_url_error: Option<SourceError>,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    /// Errors returned by `fetch_image` before the body is served, one per call.
    pub failures: Mutex<VecDeque<SourceError>>,
    pub fetch_calls: AtomicUsize,
    /// Body is streamed in chunks of this size.
    pub chunk_size: usize,
    /// Each image fetch waits this long first, so tests can observe chapters in flight.
    pub delay: std::time::Duration,
}

impl FakeSource {
    pub fn new(id: i64) -> Self {
        Self {
            id,
            name: None,
            http: true,
            local_dir: None,
            unmetered: false,
            page_count: 3,
            page_list_error: None,
            image_url_error: None,
            body: jpeg_bytes(2048),
            content_type: Some("image/jpeg".into()),
            content_length: None,
            failures: Mutex::new(VecDeque::new()),
            fetch_calls: AtomicUsize::new(0),
            chunk_size: 512,
            delay: std::time::Duration::ZERO,
        }
    }

    pub fn calls(&self) -> usize {
        self.fetch_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ChapterSource for FakeSource {
    fn id(&self) -> i64 {
        self.id
    }

    fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| format!("Source {} (EN)", self.id))
    }

    fn is_http(&self) -> bool {
        self.http
    }

    fn is_unmetered(&self) -> bool {
        self.unmetered
    }

    fn local_manga_dir(&self, manga_title: &str) -> Option<std::path::PathBuf> {
        self.local_dir.as_ref().map(|d| d.join(manga_title))
    }

    async fn page_list(&self, _chapter: &ChapterRef) -> Result<Vec<Page>, SourceError> {
        if let Some(e) = &self.page_list_error {
            return Err(e.clone());
        }
        Ok((0..self.page_count).map(|i| Page::new(i as u32, format!("https://fake/page/{i}"), None)).collect())
    }

    async fn image_url(&self, page: &Page) -> Result<String, SourceError> {
        if let Some(e) = &self.image_url_error {
            return Err(e.clone());
        }
        Ok(format!("https://fake/img/{}", page.index))
    }

    async fn fetch_image(&self, _image_url: &str) -> Result<ImageResponse, SourceError> {
        self.fetch_calls.fetch_add(1, Ordering::SeqCst);
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        if let Some(e) = self.failures.lock().unwrap().pop_front() {
            return Err(e);
        }
        let chunks: Vec<Result<Vec<u8>, SourceError>> =
            self.body.chunks(self.chunk_size.max(1)).map(|c| Ok(c.to_vec())).collect();
        Ok(ImageResponse {
            content_type: self.content_type.clone(),
            content_length: self.content_length,
            body: stream::iter(chunks).boxed(),
        })
    }
}

pub fn fake(id: i64) -> Arc<FakeSource> {
    Arc::new(FakeSource::new(id))
}

pub fn fake_source(id: i64) -> SourceRef {
    fake(id)
}

pub fn manga(id: i64, source_id: i64, title: &str) -> MangaRef {
    MangaRef {
        id,
        url: format!("/manga/{id}"),
        title: title.into(),
        source_id,
        description: None,
        author: None,
        artist: None,
        genre: None,
        status: 0,
    }
}

pub fn chapter(id: i64, manga_id: i64, name: &str) -> ChapterRef {
    ChapterRef {
        id,
        manga_id,
        url: format!("/chapter/{id}"),
        name: name.into(),
        scanlator: None,
        read: false,
        bookmark: false,
        source_order: id,
        chapter_number: id as f64,
        date_upload: 0,
    }
}

/// A queue item for `source`, with manga `manga_id` titled `Manga <id>` and chapter `chapter_id` named `Chapter <id>`.
pub fn download(source: &SourceRef, chapter_id: i64, manga_id: i64, status: DownloadState) -> QueuedDownload {
    QueuedDownload {
        source: source.clone(),
        manga: manga(manga_id, source.id(), &format!("Manga {manga_id}")),
        chapter: chapter(chapter_id, manga_id, &format!("Chapter {chapter_id}")),
        status,
        pages: None,
    }
}

/// A copy of `tests/fixtures/dl/<name>` in a fresh temp folder (edge-cases section 5). The tests change the copy, never
/// the checked-in tree. The copy's folder is the downloads root.
pub fn dl_fixture(name: &str) -> tempfile::TempDir {
    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
    }
    let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dl").join(name);
    assert!(from.is_dir(), "missing fixture {}", from.display());
    let dir = tempfile::tempdir().unwrap();
    copy(&from, dir.path());
    dir
}
