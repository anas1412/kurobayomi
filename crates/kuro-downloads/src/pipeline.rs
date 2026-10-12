//! One chapter: precheck, tmp folder, page list, page fetch, integrity and finalise (port of
//! `MangaDownloader.downloadChapter`, `getOrDownloadImage` and `downloadImage`).
//!
//! The pipeline never mutates the queue. It reports through [`PipelineMsg`], and the downloader actor applies the
//! update only while the task generation still matches (RC-01, RC-02).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use kuro_source::SourceError;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::error;

use crate::error::error_chain;

use crate::comicinfo::{ComicInfo, FILE_NAME, to_xml, web_value};
use crate::error::DownloadError;
use crate::images::{
    PageImageCache, copy_from_cache, find_finished, is_complete, page_digits, page_stem, resolve_extension,
    split_part_name, split_tall_image, tmp_dir_name,
};
use crate::index::DownloadIndex;
use crate::layout::{CBZ_EXTENSION, CBZ_PART_SUFFIX, DownloadLayout, chapter_dir_name};
use crate::model::{ChapterRef, DownloadState, MangaRef, Page, PageState, SourceRef};
use crate::source::ChapterSource;

/// Free space below this blocks the chapter (`MIN_DISK_SPACE`, FM-02).
pub const MIN_FREE_BYTES: u64 = 200 * 1024 * 1024;
/// Pages in flight per chapter (`flatMapMerge(concurrency = 2)`).
pub const PAGE_CONCURRENCY: usize = 2;
/// Waits before retries 1, 2 and 3. Four attempts in total (`(2L shl attempt) * 1000`).
pub use crate::retry::RETRY_DELAYS;
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
    /// Reader page cache, checked before the network (T-23a-016). `None` when the reader cache is not wired.
    pub page_cache: Option<Arc<dyn PageImageCache>>,
    /// `split_tall_images` (T-23a-018).
    pub split_tall_images: bool,
    /// Part height for the tall split, `2 * max(monitor)` from GDK, or `DEFAULT_OPTIMAL_HEIGHT`.
    pub optimal_height: u32,
    /// Shared speed limit of the downloader (T-23a-015).
    pub speed: Arc<crate::throttle::SpeedLimiter>,
    /// `save_chapter_as_cbz`: pack the finished chapter as a `.cbz` instead of keeping the folder (T-23a-022).
    pub save_as_cbz: bool,
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

    // ComicInfo.xml is written before archiving only (behaviour 10.9); a folder keeps its files as they are.
    if ctx.save_as_cbz {
        write_comic_info(&tmp, &manga, &chapter, &source_name)?;
        finalise_cbz(&tmp, &manga_dir.join(format!("{chapter_dir}.{CBZ_EXTENSION}")))?;
    } else {
        finalise_folder(&tmp, &manga_dir.join(&chapter_dir))?;
    }
    // The index gets the bare name, with or without `.cbz` on disk (TST-49 covers rename, this covers add).
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

/// Moves the finished tmp folder into place. An older copy of the same chapter is replaced: the user asked for it again,
/// and failing at the last step would leave a finished download marked as failed.
fn finalise_folder(tmp: &Path, target: &Path) -> Result<(), DownloadError> {
    if target.is_dir() {
        std::fs::remove_dir_all(target)?;
    } else if target.exists() {
        std::fs::remove_file(target)?;
    }
    std::fs::rename(tmp, target)?;
    Ok(())
}

/// Writes `ComicInfo.xml` into the finished tmp folder, replacing an older one (behaviour 10.9). Tracker URLs and
/// category names are not carried by the queue yet, so `Web` holds the chapter URL only and `Categories` is omitted.
fn write_comic_info(tmp: &Path, manga: &MangaRef, chapter: &ChapterRef, source_name: &str) -> io::Result<()> {
    let web = web_value(&[], &chapter.url);
    let info = ComicInfo {
        chapter_name: &chapter.name,
        series: &manga.title,
        number: Some(chapter.chapter_number),
        summary: manga.description.as_deref(),
        writer: manga.author.as_deref(),
        penciller: manga.artist.as_deref(),
        translator: chapter.scanlator.as_deref(),
        genre: manga.genre.as_deref(),
        web: &web,
        status: manga.status,
        categories: &[],
        source_name,
    };
    let path = tmp.join(FILE_NAME);
    let _ = std::fs::remove_file(&path);
    std::fs::write(path, to_xml(&info))
}

/// Packs the finished tmp folder as a stored CBZ at `target` (`<name>.cbz`). The archive is written to `<name>.cbz.part`
/// and renamed when complete (the rename replaces an older copy of the same chapter), then the tmp folder is removed.
fn finalise_cbz(tmp: &Path, target: &Path) -> Result<(), DownloadError> {
    let part = PathBuf::from(format!("{}{CBZ_PART_SUFFIX}", target.display()));
    let mut writer = kuro_archive::CbzWriter::create(&part)?;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(tmp)?.map(|e| e.map(|e| e.path())).collect::<io::Result<_>>()?;
    entries.retain(|p| p.is_file());
    for entry in &entries {
        writer.add_file(entry)?;
    }
    writer.finish()?;
    std::fs::rename(&part, target)?;
    std::fs::remove_dir_all(tmp)?;
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
        finish_page(ctx, index, page, file, tmp, width);
        return Ok(());
    }
    if let Some(file) = take_from_cache(ctx, &url, tmp, &stem)? {
        finish_page(ctx, index, page, file, tmp, width);
        return Ok(());
    }

    let mut attempt = 0;
    loop {
        match download_once(ctx, source, index, page, &url, tmp, &stem).await {
            Ok(file) => {
                finish_page(ctx, index, page, file, tmp, width);
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

/// Copies a reader-cache hit into the chapter. `None` when there is no cache, no entry, or an unknown image type, in
/// which case the page is downloaded (EC-47, EC-48). The cache entry is removed only after a successful copy.
fn take_from_cache(ctx: &PipelineCtx, url: &str, tmp: &Path, stem: &str) -> Result<Option<PathBuf>, DownloadError> {
    let Some(cache) = ctx.page_cache.as_ref() else { return Ok(None) };
    let Some(cached) = cache.cached_file(url) else { return Ok(None) };
    let copied = copy_from_cache(&cached, tmp, stem)?;
    if copied.is_some() {
        cache.remove(url);
    }
    Ok(copied)
}

/// Splits a tall page when enabled, then marks it ready with the file to record.
fn finish_page(ctx: &PipelineCtx, index: usize, page: &mut Page, file: PathBuf, tmp: &Path, width: usize) {
    let file = if ctx.split_tall_images { split_if_tall(ctx, tmp, page.number(), width, file) } else { file };
    mark_ready(ctx, index, page, file);
}

/// `splitTallImageIfNeeded`: the prefix has the same width as the page file name, so page 1 of a 1000-page chapter
/// never matches page 10 (EC-55). A split failure is logged and the whole page is kept (Kotlin logs only).
fn split_if_tall(ctx: &PipelineCtx, tmp: &Path, number: u32, width: usize, file: PathBuf) -> PathBuf {
    let prefix = page_stem(number, width);
    let already_split = file.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&format!("{prefix}__")));
    if already_split {
        return file;
    }
    match split_tall_image(tmp, &file, &prefix, ctx.optimal_height) {
        Ok(true) => tmp.join(split_part_name(&prefix, 1)),
        Ok(false) => file,
        Err(err) => {
            error!(target: "kuro_downloads", page = number, error = %error_chain(&err), "tall page split failed; page kept");
            file
        }
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
        ctx.speed.take(chunk.len() as u64).await;
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
    use std::time::Duration;

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
            page_cache: None,
            split_tall_images: false,
            optimal_height: crate::images::DEFAULT_OPTIMAL_HEIGHT,
            speed: Arc::new(crate::throttle::SpeedLimiter::new(None)),
            save_as_cbz: false,
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
    async fn a_chapter_is_packed_as_a_cbz_with_comic_info_and_the_folder_goes() {
        let h = harness();
        let source = fake(1);
        let mut ctx = ctx(&h, plenty);
        ctx.save_as_cbz = true;
        let outcome = run_chapter(ctx, job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "{outcome:?}");
        let manga_dir = h.root.path().join("Source 1 (EN)").join("Manga 1");
        let cbz = manga_dir.join("Chapter 7.cbz");
        assert!(cbz.is_file(), "the archive is renamed into place");
        assert!(!manga_dir.join("Chapter 7.cbz.part").exists(), "no part file is left");
        assert!(!chapter_dir(&h).exists(), "the tmp folder is removed after packing");
        let bytes = std::fs::read(&cbz).unwrap();
        assert!(bytes.windows(b"ComicInfo.xml".len()).any(|w| w == b"ComicInfo.xml"), "the archive holds ComicInfo.xml");
        assert!(h.index.is_chapter_downloaded(1, "Manga 1", "Chapter 7", None), "the index gets the bare name");
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

    // TST-33
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

    // TST-24
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

    // TST-14
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
    async fn an_older_copy_of_the_chapter_is_replaced_not_an_error() {
        let h = harness();
        std::fs::create_dir_all(chapter_dir(&h)).unwrap();
        std::fs::write(chapter_dir(&h).join("stale.jpg"), b"old").unwrap();
        let source = fake(1);
        let outcome = run_chapter(ctx(&h, plenty), job(source)).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "{outcome:?}");
        assert!(chapter_dir(&h).join("001.jpg").is_file(), "the new pages are in place");
        assert!(!chapter_dir(&h).join("stale.jpg").exists(), "the older copy is gone");
        assert!(!chapter_dir(&h).with_file_name("Chapter 7_tmp").exists(), "no stray tmp folder");
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

#[cfg(test)]
mod cache_and_split_tests {
    use super::*;
    use crate::testing::{FakeSource, jpeg_bytes};
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// A reader cache holding fixed entries and recording removals.
    struct FakeCache {
        files: HashMap<String, PathBuf>,
        removed: Mutex<Vec<String>>,
    }

    impl PageImageCache for FakeCache {
        fn cached_file(&self, image_url: &str) -> Option<PathBuf> {
            self.files.get(image_url).cloned()
        }
        fn remove(&self, image_url: &str) {
            self.removed.lock().unwrap().push(image_url.to_owned());
        }
    }

    fn plenty(_: &Path) -> Option<u64> {
        Some(10 * 1024 * 1024 * 1024)
    }

    fn chapter_job(source: Arc<FakeSource>) -> ChapterJob {
        let source_ref: SourceRef = source;
        ChapterJob {
            source: source_ref,
            manga: crate::testing::manga(1, 1, "Manga 1"),
            chapter: crate::testing::chapter(7, 1, "Chapter 7"),
            pages: None,
        }
    }

    fn setup(root: &Path, cache: Option<Arc<dyn PageImageCache>>) -> (PipelineCtx, mpsc::UnboundedReceiver<PipelineMsg>) {
        let index_cache = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::unbounded_channel();
        let ctx = PipelineCtx {
            layout: Arc::new(DownloadLayout::new(root)),
            index: Arc::new(DownloadIndex::open(root, index_cache.path())),
            free_space: plenty,
            chapter_id: 7,
            generation: 1,
            tx,
            page_cache: cache,
            split_tall_images: true,
            optimal_height: 2000,
            speed: Arc::new(crate::throttle::SpeedLimiter::new(None)),
            save_as_cbz: false,
        };
        std::mem::forget(index_cache);
        (ctx, rx)
    }

    #[tokio::test]
    async fn a_cache_hit_is_copied_and_the_entry_removed_without_a_fetch() {
        let root = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        let cached = cache_dir.path().join("entry.0");
        std::fs::write(&cached, jpeg_bytes(64)).unwrap();
        let url = "https://fake/img/0".to_owned();
        let cache = Arc::new(FakeCache { files: HashMap::from([(url.clone(), cached)]), removed: Mutex::new(Vec::new()) });
        let (ctx, _rx) = setup(root.path(), Some(cache.clone()));
        let source = Arc::new(FakeSource { page_count: 1, ..FakeSource::new(1) });
        let outcome = run_chapter(ctx, chapter_job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "{outcome:?}");
        assert_eq!(source.calls(), 0, "EC-47: the cache serves the page");
        assert_eq!(*cache.removed.lock().unwrap(), vec![url]);
        assert!(root.path().join("Source 1 (EN)/Manga 1/Chapter 7/001.jpg").is_file());
    }

    #[tokio::test]
    async fn an_unknown_cached_type_falls_back_to_the_network() {
        let root = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        let cached = cache_dir.path().join("entry.0");
        std::fs::write(&cached, b"junk").unwrap();
        let url = "https://fake/img/0".to_owned();
        let cache = Arc::new(FakeCache { files: HashMap::from([(url, cached)]), removed: Mutex::new(Vec::new()) });
        let (ctx, _rx) = setup(root.path(), Some(cache.clone()));
        let source = Arc::new(FakeSource { page_count: 1, ..FakeSource::new(1) });
        let outcome = run_chapter(ctx, chapter_job(source.clone())).await;
        assert!(matches!(outcome, ChapterOutcome::Downloaded), "TST-27: {outcome:?}");
        assert_eq!(source.calls(), 1);
        assert!(cache.removed.lock().unwrap().is_empty(), "the entry is kept when it was not used");
        let names: Vec<String> = std::fs::read_dir(root.path().join("Source 1 (EN)/Manga 1/Chapter 7"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["001.jpg"]);
    }

    #[test]
    fn page_one_never_splits_page_ten() {
        // TST-25 / EC-55: a 1234-page chapter. Page 1 is ordinary, page 10 is tall and must stay whole.
        let root = tempfile::tempdir().unwrap();
        let (ctx, _rx) = setup(root.path(), None);
        let tmp = root.path().join("chapter_tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        let normal = tmp.join("0001.png");
        let tall = tmp.join("0010.png");
        let small = image::RgbaImage::from_pixel(10, 10, image::Rgba([1, 2, 3, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(small).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        std::fs::write(&normal, bytes.get_ref()).unwrap();
        let tall_img = image::RgbaImage::from_pixel(10, 400, image::Rgba([1, 2, 3, 255]));
        let mut tall_bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(tall_img).write_to(&mut tall_bytes, image::ImageFormat::Png).unwrap();
        std::fs::write(&tall, tall_bytes.get_ref()).unwrap();
        let kept = split_if_tall(&ctx, &tmp, 1, 4, normal.clone());
        assert_eq!(kept, normal);
        assert!(tall.is_file(), "page 10 is untouched");
        assert!(!tmp.join("0001__001.jpg").exists());
    }

    #[test]
    fn an_already_split_page_is_not_split_again() {
        let root = tempfile::tempdir().unwrap();
        let (ctx, _rx) = setup(root.path(), None);
        let tmp = root.path().join("chapter_tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        let part = tmp.join("001__001.jpg");
        std::fs::write(&part, jpeg_bytes(16)).unwrap();
        assert_eq!(split_if_tall(&ctx, &tmp, 1, 3, part.clone()), part);
        assert!(part.is_file());
    }

}

#[cfg(test)]
mod archive_tests {
    // TST-30
    #[test]
    fn tst_30_a_cbz_is_stored_flat_with_utf8_names() {
        let base = tempfile::tempdir().unwrap();
        let page = base.path().join("ページ.jpg");
        std::fs::write(&page, b"not really a jpeg").unwrap();
        let target = base.path().join("Chapter 1.cbz");
        let mut writer = kuro_archive::CbzWriter::create(&target).unwrap();
        writer.add_file(&page).unwrap();
        writer.finish().unwrap();
        let bytes = std::fs::read(&target).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04", "a ZIP local header starts the file");
        assert_eq!(&bytes[8..10], &[0, 0], "method 0 is STORE");
        let name = "ページ.jpg".as_bytes();
        assert!(bytes.windows(name.len()).any(|w| w == name), "the UTF-8 name is stored as written");
    }
}

