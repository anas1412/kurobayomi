//! One chapter: precheck, tmp folder, page list, page fetch, integrity and finalise (port of
//! `MangaDownloader.downloadChapter`, `getOrDownloadImage` and `downloadImage`).
//!
//! The pipeline never mutates the queue. It reports through [`PipelineMsg`], and the downloader actor applies the
//! update only while the task generation still matches (RC-01, RC-02).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use kuro_source::SourceError;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::error::DownloadError;
use crate::images::{find_finished, is_complete, page_digits, page_stem, resolve_extension, tmp_dir_name};
use crate::index::DownloadIndex;
use crate::layout::{DownloadLayout, chapter_dir_name};
use crate::model::{ChapterRef, DownloadState, MangaRef, Page, PageState, SourceRef};
use crate::source::ChapterSource;

/// Free space below this blocks the chapter (`MIN_DISK_SPACE`, FM-02).
pub const MIN_FREE_BYTES: u64 = 200 * 1024 * 1024;
/// Pages in flight per chapter (`flatMapMerge(concurrency = 2)`).
pub const PAGE_CONCURRENCY: usize = 2;
/// Waits before retries 1, 2 and 3. Four attempts in total (`(2L shl attempt) * 1000`).
pub const RETRY_DELAYS: [Duration; 3] = [Duration::from_secs(2), Duration::from_secs(4), Duration::from_secs(8)];
/// Bytes read to decide the image type.
const HEAD_LEN: usize = 32;
const TMP_FILE_SUFFIX: &str = ".tmp";

/// Free space of the filesystem holding a path, `None` when unknown. The GUI passes the platform implementation.
pub type FreeSpaceFn = fn(&Path) -> Option<u64>;

/// A message from a chapter task to the actor.
#[derive(Debug)]
pub struct PipelineMsg {
    pub chapter_id: i64,
    pub generation: u64,
    pub update: Update,
}

#[derive(Debug)]
pub enum Update {
    Status(DownloadState),
    Pages(Vec<Page>),
    Page { index: usize, state: PageState, progress: i32 },
    PageFailed { index: usize, message: String },
}

/// Everything a chapter task needs. It is owned, so the task can run on the runtime.
pub struct PipelineCtx {
    pub layout: Arc<DownloadLayout>,
    pub index: Arc<DownloadIndex>,
    pub free_space: FreeSpaceFn,
    pub chapter_id: i64,
    pub generation: u64,
    pub tx: mpsc::UnboundedSender<PipelineMsg>,
}

impl PipelineCtx {
    fn emit(&self, update: Update) {
        let _ = self.tx.send(PipelineMsg { chapter_id: self.chapter_id, generation: self.generation, update });
    }
}

pub struct ChapterJob {
    pub source: SourceRef,
    pub manga: MangaRef,
    pub chapter: ChapterRef,
    /// Pages from an earlier, interrupted run of this process. `None` fetches them.
    pub pages: Option<Vec<Page>>,
}

#[derive(Debug)]
pub enum ChapterOutcome {
    Downloaded,
    Errored(DownloadError),
}

/// Runs one chapter to the end. The caller applies the outcome.
pub async fn run_chapter(ctx: PipelineCtx, job: ChapterJob) -> ChapterOutcome {
    match run_steps(&ctx, job).await {
        Ok(()) => ChapterOutcome::Downloaded,
        Err(err) => ChapterOutcome::Errored(err),
    }
}

async fn run_steps(ctx: &PipelineCtx, job: ChapterJob) -> Result<(), DownloadError> {
    let ChapterJob { source, manga, chapter, pages } = job;
    let source_name = source.display_name();

    // Precheck: the manga folder (created here), then free space.
    let manga_dir = ctx.layout.manga_dir(&source_name, &manga.title)?;
    // A pause drops the result of a chapter that had just finished, so it is still queued with its folder and index
    // entry in place. Running it again is a no-op, not a second fetch that fails at finalise (RC-01, TST-18).
    let scanlator = chapter.scanlator.as_deref();
    if ctx.index.is_chapter_downloaded(source.id(), &manga.title, &chapter.name, scanlator)
        && ctx.layout.find_chapter(&source_name, &manga.title, &chapter.name, scanlator).is_some()
    {
        return Ok(());
    }
    if let Some(free) = (ctx.free_space)(&manga_dir) {
        if free < MIN_FREE_BYTES {
            return Err(DownloadError::LowSpace { free, need: MIN_FREE_BYTES });
        }
    }

    // The tmp folder exists before the page list is fetched (Kotlin order, observable on disk).
    let chapter_dir = chapter_dir_name(&chapter.name, chapter.scanlator.as_deref());
    let tmp = manga_dir.join(tmp_dir_name(&chapter_dir));
    std::fs::create_dir_all(&tmp)?;

    let mut pages = match pages {
        Some(pages) => pages,
        None => source.page_list(&chapter).await?,
    };
    if pages.is_empty() {
        return Err(DownloadError::NoPages);
    }
    for (i, page) in pages.iter_mut().enumerate() {
        page.index = i as u32;
    }
    ctx.emit(Update::Pages(pages.clone()));

    remove_partial_files(&tmp)?;
    ctx.emit(Update::Status(DownloadState::Downloading));

    let width = page_digits(pages.len());
    let source_ref: &dyn ChapterSource = source.as_ref();
    let failed = fetch_all(ctx, source_ref, &mut pages, &tmp, width).await;
    if failed > 0 {
        return Err(DownloadError::PagesFailed(failed));
    }

    if !is_complete(&tmp, &pages)? {
        return Err(DownloadError::Incomplete(chapter_dir));
    }

    finalise_folder(&tmp, &manga_dir.join(&chapter_dir))?;
    ctx.index.add_chapter(source.id(), &DownloadLayout::source_dir_name(&source_name), &manga.title, &chapter_dir);
    Ok(())
}

/// Fetches every page, at most `PAGE_CONCURRENCY` at a time. Returns how many pages failed.
async fn fetch_all(ctx: &PipelineCtx, source: &dyn ChapterSource, pages: &mut [Page], tmp: &Path, width: usize) -> usize {
    let mut pending = pages.iter_mut().enumerate();
    let mut in_flight = FuturesUnordered::new();
    let mut failed = 0;
    loop {
        while in_flight.len() < PAGE_CONCURRENCY {
            match pending.next() {
                Some((index, page)) => in_flight.push(fetch_page(ctx, source, index, page, tmp, width)),
                None => break,
            }
        }
        match in_flight.next().await {
            Some(result) => failed += usize::from(result.is_err()),
            None => return failed,
        }
    }
}

/// Moves the finished tmp folder into place. An existing target is an error and the tmp folder is kept (EC-63).
fn finalise_folder(tmp: &Path, target: &Path) -> Result<(), DownloadError> {
    if target.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} already exists", target.display())).into());
    }
    std::fs::rename(tmp, target)?;
    Ok(())
}

/// Deletes `*.tmp` files left by an interrupted page write. Finished page files are kept for resume.
fn remove_partial_files(tmp: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(tmp)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "tmp") && path.is_file() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Fetches one page. A failure marks the page ERROR and reports it, and is returned to the caller.
async fn fetch_page(
    ctx: &PipelineCtx,
    source: &dyn ChapterSource,
    index: usize,
    page: &mut Page,
    tmp: &Path,
    width: usize,
) -> Result<(), DownloadError> {
    let result = fetch_page_steps(ctx, source, index, page, tmp, width).await;
    if let Err(err) = &result {
        page.state = PageState::Error;
        page.progress = 0;
        ctx.emit(Update::Page { index, state: PageState::Error, progress: 0 });
        ctx.emit(Update::PageFailed { index, message: err.to_string() });
    }
    result
}

async fn fetch_page_steps(
    ctx: &PipelineCtx,
    source: &dyn ChapterSource,
    index: usize,
    page: &mut Page,
    tmp: &Path,
    width: usize,
) -> Result<(), DownloadError> {
    let stem = page_stem(page.number(), width);

    if page.image_url.as_deref().is_none_or(str::is_empty) {
        page.state = PageState::LoadPage;
        ctx.emit(Update::Page { index, state: PageState::LoadPage, progress: page.progress });
        let url = source.image_url(page).await?;
        page.image_url = Some(url);
    }
    let url = page
        .image_url
        .clone()
        .filter(|u| !u.is_empty())
        .ok_or(DownloadError::Source(SourceError::MissingImageUrl))?;

    let _ = std::fs::remove_file(tmp.join(format!("{stem}{TMP_FILE_SUFFIX}")));
    if let Some(file) = find_finished(tmp, &stem) {
        mark_ready(ctx, index, page, file);
        return Ok(());
    }

    let mut attempt = 0;
    loop {
        match download_once(ctx, source, index, page, &url, tmp, &stem).await {
            Ok(file) => {
                mark_ready(ctx, index, page, file);
                return Ok(());
            }
            Err(err) if is_retryable(&err) && attempt < RETRY_DELAYS.len() => {
                tokio::time::sleep(RETRY_DELAYS[attempt]).await;
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }
}

/// Retries every failure except a missing image (HTTP 404 and 410 fail at once, D-DL-11) and cancellation.
fn is_retryable(err: &DownloadError) -> bool {
    match err {
        DownloadError::Source(SourceError::Http(404 | 410)) => false,
        DownloadError::Source(SourceError::MissingImageUrl | SourceError::Cancelled | SourceError::Unsupported(_)) => {
            false
        }
        DownloadError::Source(_) | DownloadError::Io(_) => true,
        _ => false,
    }
}

fn mark_ready(ctx: &PipelineCtx, index: usize, page: &mut Page, file: PathBuf) {
    page.file = Some(file);
    page.state = PageState::Ready;
    page.progress = 100;
    ctx.emit(Update::Page { index, state: PageState::Ready, progress: 100 });
}

async fn download_once(
    ctx: &PipelineCtx,
    source: &dyn ChapterSource,
    index: usize,
    page: &mut Page,
    url: &str,
    tmp: &Path,
    stem: &str,
) -> Result<PathBuf, DownloadError> {
    page.state = PageState::DownloadImage;
    page.progress = 0;
    ctx.emit(Update::Page { index, state: PageState::DownloadImage, progress: 0 });

    let partial = tmp.join(format!("{stem}{TMP_FILE_SUFFIX}"));
    match stream_to_file(ctx, source, index, page, url, &partial).await {
        Ok((content_type, head)) => {
            let extension = resolve_extension(content_type.as_deref(), &head);
            let target = tmp.join(format!("{stem}.{extension}"));
            std::fs::rename(&partial, &target)?;
            Ok(target)
        }
        Err(err) => {
            let _ = std::fs::remove_file(&partial);
            Err(err)
        }
    }
}

/// Streams the body into `partial`. Returns the Content-Type and the first bytes, for the extension.
async fn stream_to_file(
    ctx: &PipelineCtx,
    source: &dyn ChapterSource,
    index: usize,
    page: &mut Page,
    url: &str,
    partial: &Path,
) -> Result<(Option<String>, Vec<u8>), DownloadError> {
    let mut response = source.fetch_image(url).await?;
    let total = response.content_length.filter(|t| *t > 0);
    let mut file = tokio::fs::File::create(partial).await?;
    let mut head = Vec::with_capacity(HEAD_LEN);
    let mut read: u64 = 0;
    let mut reported = page.progress;
    while let Some(chunk) = response.body.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        if head.len() < HEAD_LEN {
            let take = (HEAD_LEN - head.len()).min(chunk.len());
            head.extend_from_slice(&chunk[..take]);
        }
        read += chunk.len() as u64;
        let progress = total.map_or(-1, |t| (100 * read / t).min(100) as i32);
        if progress != reported {
            reported = progress;
            page.progress = progress;
            ctx.emit(Update::Page { index, state: PageState::DownloadImage, progress });
        }
    }
    file.flush().await?;
    drop(file);
    if read == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty image body").into());
    }
    Ok((response.content_type, head))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{FakeSource, fake, jpeg_bytes};
    use std::sync::atomic::Ordering;

    struct Harness {
        root: tempfile::TempDir,
        index: Arc<DownloadIndex>,
        layout: Arc<DownloadLayout>,
        rx: mpsc::UnboundedReceiver<PipelineMsg>,
        tx: mpsc::UnboundedSender<PipelineMsg>,
        _cache: tempfile::TempDir,
    }

    fn harness() -> Harness {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::unbounded_channel();
        Harness {
            index: Arc::new(DownloadIndex::open(root.path(), cache.path())),
            layout: Arc::new(DownloadLayout::new(root.path())),
            root,
            rx,
            tx,
            _cache: cache,
        }
    }

    fn plenty(_: &Path) -> Option<u64> {
        Some(10 * 1024 * 1024 * 1024)
    }

    fn job(source: Arc<FakeSource>) -> ChapterJob {
        let source_ref: SourceRef = source;
        ChapterJob {
            source: source_ref,
            manga: crate::testing::manga(1, 1, "Manga 1"),
            chapter: crate::testing::chapter(7, 1, "Chapter 7"),
            pages: None,
        }
    }

    fn ctx(h: &Harness, free: FreeSpaceFn) -> PipelineCtx {
        PipelineCtx {
            layout: h.layout.clone(),
            index: h.index.clone(),
            free_space: free,
            chapter_id: 7,
            generation: 1,
            tx: h.tx.clone(),
        }
    }

    fn drain(rx: &mut mpsc::UnboundedReceiver<PipelineMsg>) -> Vec<Update> {
        let mut out = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            out.push(msg.update);
        }
        out
    }

    fn chapter_dir(h: &Harness) -> PathBuf {
        h.root.path().join("Source 1 (EN)").join("Manga 1").join("Chapter 7")
    }

    #[tokio::test]
    async fn a_chapter_lands_as_a_folder_and_joins_the_index() {
        let mut h = harness();
        let source = fake(1);
        let outcome = run_chapter(ctx(&h, plenty), job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "{outcome:?}");
        let msgs_pages: Vec<Vec<Page>> = drain(&mut h.rx).into_iter().filter_map(|u| match u {
            Update::Pages(p) => Some(p),
            _ => None,
        }).collect();
        assert!(msgs_pages[0].iter().enumerate().all(|(i, p)| p.index as usize == i), "page indices start at 0");
        let mut names: Vec<String> = std::fs::read_dir(chapter_dir(&h))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["001.jpg", "002.jpg", "003.jpg"]);
        assert!(!chapter_dir(&h).with_file_name("Chapter 7_tmp").exists());
        assert!(h.index.is_chapter_downloaded(1, "Manga 1", "Chapter 7", None));
        assert!(msgs_pages.iter().any(|p| p.len() == 3));
    }

    #[tokio::test]
    async fn low_space_fails_the_chapter_without_fetching() {
        let h = harness();
        fn tiny(_: &Path) -> Option<u64> {
            Some(MIN_FREE_BYTES - 1)
        }
        let source = fake(1);
        let outcome = run_chapter(ctx(&h, tiny), job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Errored(DownloadError::LowSpace { .. })));
        assert_eq!(source.calls(), 0);
    }

    #[tokio::test]
    async fn an_empty_page_list_is_an_error_and_the_tmp_folder_exists_first() {
        let h = harness();
        let source = Arc::new(FakeSource { page_count: 0, ..FakeSource::new(1) });
        let outcome = run_chapter(ctx(&h, plenty), job(source)).await;
        assert!(matches!(outcome, ChapterOutcome::Errored(DownloadError::NoPages)));
        assert!(chapter_dir(&h).with_file_name("Chapter 7_tmp").is_dir());
    }

    #[tokio::test]
    async fn resume_keeps_finished_pages_and_removes_partial_ones() {
        let h = harness();
        let tmp = chapter_dir(&h).with_file_name("Chapter 7_tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("001.jpg"), jpeg_bytes(10)).unwrap();
        std::fs::write(tmp.join("002.tmp"), b"half").unwrap();
        let source = fake(1);
        let outcome = run_chapter(ctx(&h, plenty), job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "{outcome:?}");
        assert_eq!(source.calls(), 2, "page 1 was reused, pages 2 and 3 fetched");
        assert!(!chapter_dir(&h).join("002.tmp").exists(), "partial files go before fetching");
    }

    #[tokio::test(start_paused = true)]
    async fn retries_follow_two_four_eight_seconds_and_stop_after_four_attempts() {
        let h = harness();
        let source = Arc::new(FakeSource {
            page_count: 1,
            failures: std::sync::Mutex::new((0..4).map(|_| SourceError::Transport("reset".into())).collect()),
            ..FakeSource::new(1)
        });
        let started = tokio::time::Instant::now();
        let outcome = run_chapter(ctx(&h, plenty), job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Errored(DownloadError::PagesFailed(1))), "{outcome:?}");
        assert_eq!(source.calls(), 4);
        assert_eq!(started.elapsed(), Duration::from_secs(14));
    }

    #[tokio::test(start_paused = true)]
    async fn a_third_failure_is_recovered_by_the_fourth_attempt() {
        let h = harness();
        let source = Arc::new(FakeSource {
            page_count: 1,
            failures: std::sync::Mutex::new((0..3).map(|_| SourceError::Transport("reset".into())).collect()),
            ..FakeSource::new(1)
        });
        let outcome = run_chapter(ctx(&h, plenty), job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "{outcome:?}");
        assert_eq!(source.calls(), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn http_404_is_not_retried() {
        let h = harness();
        let source = Arc::new(FakeSource {
            page_count: 1,
            failures: std::sync::Mutex::new(vec![SourceError::Http(404)].into()),
            ..FakeSource::new(1)
        });
        let outcome = run_chapter(ctx(&h, plenty), job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Errored(DownloadError::PagesFailed(1))));
        assert_eq!(source.calls(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn at_most_two_pages_are_in_flight_per_chapter() {
        let h = harness();
        let source = Arc::new(FakeSource { page_count: 6, ..FakeSource::new(1) });
        let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let peak = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        // A slow fetch makes overlap visible: the source records the concurrent calls it sees.
        let slow = Arc::new(SlowSource { inner: source.clone(), in_flight: in_flight.clone(), peak: peak.clone() });
        let outcome = run_chapter(
            ctx(&h, plenty),
            ChapterJob {
                source: slow.clone(),
                manga: crate::testing::manga(1, 1, "Manga 1"),
                chapter: crate::testing::chapter(7, 1, "Chapter 7"),
                pages: None,
            },
        )
        .await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "{outcome:?}");
        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    /// Wraps a fake source and holds each image fetch for a while, so the test can see how many overlap.
    struct SlowSource {
        inner: Arc<FakeSource>,
        in_flight: Arc<std::sync::atomic::AtomicUsize>,
        peak: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl ChapterSource for SlowSource {
        fn id(&self) -> i64 {
            1
        }
        fn display_name(&self) -> String {
            "Source 1 (EN)".into()
        }
        async fn page_list(&self, chapter: &ChapterRef) -> Result<Vec<Page>, SourceError> {
            self.inner.page_list(chapter).await
        }
        async fn image_url(&self, page: &Page) -> Result<String, SourceError> {
            self.inner.image_url(page).await
        }
        async fn fetch_image(&self, url: &str) -> Result<crate::source::ImageResponse, SourceError> {
            let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            let resp = self.inner.fetch_image(url).await;
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            resp
        }
    }

    #[tokio::test]
    async fn an_empty_body_fails_the_page_and_the_chapter() {
        let h = harness();
        let source = Arc::new(FakeSource { body: Vec::new(), page_count: 1, ..FakeSource::new(1) });
        let outcome = run_chapter(ctx(&h, plenty), job(source)).await;
        assert!(matches!(outcome, ChapterOutcome::Errored(DownloadError::PagesFailed(1))), "{outcome:?}");
    }

    #[tokio::test]
    async fn the_image_type_comes_from_the_bytes_not_the_header() {
        let h = harness();
        let source = Arc::new(FakeSource {
            content_type: Some("image/webp".into()),
            page_count: 1,
            ..FakeSource::new(1)
        });
        let outcome = run_chapter(ctx(&h, plenty), job(source)).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "{outcome:?}");
        assert!(chapter_dir(&h).join("001.jpg").is_file());
    }

    #[tokio::test]
    async fn an_existing_chapter_folder_fails_the_finalise_and_keeps_tmp() {
        let h = harness();
        std::fs::create_dir_all(chapter_dir(&h)).unwrap();
        let source = fake(1);
        let outcome = run_chapter(ctx(&h, plenty), job(source)).await;
        assert!(matches!(outcome, ChapterOutcome::Errored(DownloadError::Io(_))), "{outcome:?}");
        assert!(chapter_dir(&h).with_file_name("Chapter 7_tmp").is_dir());
    }

    #[tokio::test]
    async fn running_a_finished_chapter_again_is_a_no_op() {
        let h = harness();
        let first = fake(1);
        let outcome = run_chapter(ctx(&h, plenty), job(first.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "{outcome:?}");
        let second = fake(1);
        let outcome = run_chapter(ctx(&h, plenty), job(second.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "RC-01: no AlreadyExists error: {outcome:?}");
        assert_eq!(second.calls(), 0, "nothing is fetched twice");
        assert!(!chapter_dir(&h).with_file_name("Chapter 7_tmp").exists(), "no stray tmp folder");
    }

    #[tokio::test]
    async fn a_missing_image_url_fails_the_page_without_a_fetch() {
        let h = harness();
        let source = Arc::new(FakeSource {
            image_url_error: Some(SourceError::Http(500)),
            page_count: 1,
            ..FakeSource::new(1)
        });
        let outcome = run_chapter(ctx(&h, plenty), job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Errored(DownloadError::PagesFailed(1))));
        assert_eq!(source.calls(), 0);
    }
}
