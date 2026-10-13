//! One chapter: tmp folder, page list, page fetch, integrity and finalise (port of `MangaDownloader.downloadChapter`,
//! `getOrDownloadImage` and `downloadImage`).
//!
//! The pipeline never touches the queue. It reports pages through [`UnitProgress`] and returns the finished folder name;
//! the actor does the free-space precheck, the `Downloading` state and the index update (docs/design/unified-downloads.md
//! 2.3).

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use kuro_platform::kind::Kind;
use kuro_source::SourceError;
use tokio::io::AsyncWriteExt;
use tracing::error;

use super::comicinfo::{ComicInfo, FILE_NAME, to_xml, web_value};
use super::images::{
    PageImageCache, copy_from_cache, find_finished, is_complete, page_digits, page_stem, resolve_extension,
    split_part_name, split_tall_image, tmp_dir_name,
};
use super::source::{ChapterRef, ChapterSource, Page, PageState};
use crate::error::{DownloadError, error_chain};
use crate::index::DownloadIndex;
use crate::layout::Layout;
use crate::naming::{CBZ_EXTENSION, CBZ_PART_SUFFIX, UnitName, unit_dir_name};
use crate::unit::{EntryRef, SourceKey, UnitProgress, UnitRef};
use crate::throttle::SpeedLimiter;

/// Pages in flight per chapter (`flatMapMerge(concurrency = 2)`).
pub const PAGE_CONCURRENCY: usize = 2;
/// Waits before retries 1, 2 and 3. Four attempts in total (`(2L shl attempt) * 1000`).
pub use crate::retry::RETRY_DELAYS;
/// Bytes read to decide the image type.
const HEAD_LEN: usize = 32;
const TMP_FILE_SUFFIX: &str = ".tmp";

/// The page list of each chapter that was started in this process, by chapter id. A pause and resume skips the page-list
/// request (B37). After a restart the list is fetched again and the staged pages are found on disk.
pub type PageLists = Mutex<HashMap<i64, Vec<Page>>>;

/// Everything a chapter task needs. It is owned, so the task can run on the runtime.
pub struct PipelineCtx {
    pub layout: Layout,
    pub index: DownloadIndex,
    pub progress: Arc<UnitProgress>,
    pub pages: Arc<PageLists>,
    /// Reader page cache, checked before the network (T-23a-016). `None` when the reader cache is not wired.
    pub page_cache: Option<Arc<dyn PageImageCache>>,
    /// `split_tall_images` (T-23a-018).
    pub split_tall_images: bool,
    /// Part height for the tall split, `2 * max(monitor)` from GDK, or `DEFAULT_OPTIMAL_HEIGHT`.
    pub optimal_height: u32,
    /// Shared speed limit of the downloader (T-23a-015).
    pub speed: Arc<SpeedLimiter>,
    /// `save_chapter_as_cbz`: pack the finished chapter as a `.cbz` instead of keeping the folder (T-23a-022).
    pub save_as_cbz: bool,
}

pub struct ChapterJob {
    pub source: Arc<dyn ChapterSource>,
    pub key: SourceKey,
    pub entry: EntryRef,
    pub chapter: UnitRef,
}

/// The `ChapterRef` the source port wants, from the shared unit type.
pub fn chapter_ref(unit: &UnitRef) -> ChapterRef {
    ChapterRef {
        id: unit.id,
        manga_id: unit.entry_id,
        url: unit.url.clone(),
        name: unit.name.clone(),
        scanlator: unit.scanlator.clone(),
        read: unit.done,
        bookmark: unit.bookmark,
        source_order: unit.source_order,
        chapter_number: unit.number,
        date_upload: unit.date_upload,
    }
}

/// Runs one chapter to the end. `Ok` is the finished folder name (the bare name, with or without `.cbz` on disk).
pub async fn run_chapter(ctx: PipelineCtx, job: ChapterJob) -> Result<String, DownloadError> {
    let chapter_id = job.chapter.id;
    let dir_name = run_steps(&ctx, job).await?;
    ctx.pages.lock().unwrap_or_else(|e| e.into_inner()).remove(&chapter_id);
    Ok(dir_name)
}

async fn run_steps(ctx: &PipelineCtx, job: ChapterJob) -> Result<String, DownloadError> {
    let ChapterJob { source, key, entry, chapter } = job;
    let source_dir = Layout::source_dir_name(&key.name, &key.lang);
    let scanlator = chapter.scanlator.as_deref();
    let name = UnitName::new(&chapter.name, scanlator);

    // The entry folder (created here).
    let manga_dir = ctx.layout.entry_dir(&source_dir, &entry.title)?;
    let chapter_dir = unit_dir_name(Kind::Manga, name);
    // A pause drops the result of a chapter that had just finished, so it is still queued with its folder and index
    // entry in place. Running it again is a no-op, not a second fetch that fails at finalise (RC-01, TST-18).
    if ctx.index.is_downloaded(Kind::Manga, key.id, &entry.title, &chapter.name, scanlator)
        && ctx.layout.find_unit(&source_dir, &entry.title, name).is_some()
    {
        return Ok(chapter_dir);
    }

    // The tmp folder exists before the page list is fetched (Kotlin order, observable on disk).
    let tmp = manga_dir.join(tmp_dir_name(&chapter_dir));
    std::fs::create_dir_all(&tmp)?;

    let known = ctx.pages.lock().unwrap_or_else(|e| e.into_inner()).get(&chapter.id).cloned();
    let mut pages = match known {
        Some(pages) => pages,
        None => source.page_list(&chapter_ref(&chapter)).await?,
    };
    if pages.is_empty() {
        return Err(DownloadError::NoPages);
    }
    for (i, page) in pages.iter_mut().enumerate() {
        page.index = i as u32;
    }
    ctx.pages.lock().unwrap_or_else(|e| e.into_inner()).insert(chapter.id, pages.clone());
    ctx.progress.set_pages(0, pages.len() as u32);

    remove_partial_files(&tmp)?;

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
        write_comic_info(&tmp, &entry, &chapter, &key.display_name())?;
        finalise_cbz(&tmp, &manga_dir.join(format!("{chapter_dir}.{CBZ_EXTENSION}")))?;
    } else {
        finalise_folder(&tmp, &manga_dir.join(&chapter_dir))?;
    }
    Ok(chapter_dir)
}

/// Fetches every page, at most `PAGE_CONCURRENCY` at a time. Returns how many pages failed.
async fn fetch_all(ctx: &PipelineCtx, source: &dyn ChapterSource, pages: &mut [Page], tmp: &Path, width: usize) -> usize {
    let total = pages.len() as u32;
    let mut pending = pages.iter_mut().enumerate();
    let mut in_flight = FuturesUnordered::new();
    let mut failed = 0;
    let mut done = 0;
    loop {
        while in_flight.len() < PAGE_CONCURRENCY {
            match pending.next() {
                Some((index, page)) => in_flight.push(fetch_page(ctx, source, index, page, tmp, width)),
                None => break,
            }
        }
        match in_flight.next().await {
            Some(Ok(())) => {
                done += 1;
                ctx.progress.set_pages(done, total);
                ctx.progress.set_percent((100 * done / total) as u8);
            }
            Some(Err(_)) => failed += 1,
            None => return failed,
        }
    }
}

/// Moves the finished tmp folder into place. An older copy of the same chapter is replaced: the user asked for it again,
/// and failing at the last step would leave a finished download marked as failed.
fn finalise_folder(tmp: &Path, target: &Path) -> Result<(), DownloadError> {
    Layout::remove_path(target)?;
    std::fs::rename(tmp, target)?;
    Ok(())
}

/// Writes `ComicInfo.xml` into the finished tmp folder, replacing an older one (behaviour 10.9). Tracker URLs and
/// category names are not carried by the queue yet, so `Web` holds the chapter URL only and `Categories` is omitted.
fn write_comic_info(tmp: &Path, entry: &EntryRef, chapter: &UnitRef, source_name: &str) -> io::Result<()> {
    let web = web_value(&[], &chapter.url);
    let info = ComicInfo {
        chapter_name: &chapter.name,
        series: &entry.title,
        number: Some(chapter.number),
        summary: entry.description.as_deref(),
        writer: entry.author.as_deref(),
        penciller: entry.artist.as_deref(),
        translator: chapter.scanlator.as_deref(),
        genre: entry.genre.as_deref(),
        web: &web,
        status: entry.status,
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
    // A folder left by an older download of the same chapter would shadow the archive.
    let folder = target.with_extension("");
    if folder.is_dir() {
        Layout::remove_path(&folder)?;
    }
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

/// Fetches one page. A failure marks the page ERROR, is logged, and is returned to the caller.
async fn fetch_page(
    ctx: &PipelineCtx,
    source: &dyn ChapterSource,
    index: usize,
    page: &mut Page,
    tmp: &Path,
    width: usize,
) -> Result<(), DownloadError> {
    let result = fetch_page_steps(ctx, source, page, tmp, width).await;
    if let Err(err) = &result {
        page.state = PageState::Error;
        page.progress = 0;
        error!(target: "kuro_downloads", page = index, error = %error_chain(err), "page failed");
    }
    result
}

async fn fetch_page_steps(
    ctx: &PipelineCtx,
    source: &dyn ChapterSource,
    page: &mut Page,
    tmp: &Path,
    width: usize,
) -> Result<(), DownloadError> {
    let stem = page_stem(page.number(), width);

    if page.image_url.as_deref().is_none_or(str::is_empty) {
        page.state = PageState::LoadPage;
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
        finish_page(ctx, page, file, tmp, width);
        return Ok(());
    }
    if let Some(file) = take_from_cache(ctx, &url, tmp, &stem)? {
        finish_page(ctx, page, file, tmp, width);
        return Ok(());
    }

    let mut attempt = 0;
    loop {
        match download_once(ctx, source, page, &url, tmp, &stem).await {
            Ok(file) => {
                finish_page(ctx, page, file, tmp, width);
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
fn finish_page(ctx: &PipelineCtx, page: &mut Page, file: PathBuf, tmp: &Path, width: usize) {
    let file = if ctx.split_tall_images { split_if_tall(ctx, tmp, page.number(), width, file) } else { file };
    mark_ready(page, file);
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

fn mark_ready(page: &mut Page, file: PathBuf) {
    page.file = Some(file);
    page.state = PageState::Ready;
    page.progress = 100;
}

async fn download_once(
    ctx: &PipelineCtx,
    source: &dyn ChapterSource,
    page: &mut Page,
    url: &str,
    tmp: &Path,
    stem: &str,
) -> Result<PathBuf, DownloadError> {
    page.state = PageState::DownloadImage;
    page.progress = 0;

    let partial = tmp.join(format!("{stem}{TMP_FILE_SUFFIX}"));
    match stream_to_file(ctx, source, page, url, &partial).await {
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
    page: &mut Page,
    url: &str,
    partial: &Path,
) -> Result<(Option<String>, Vec<u8>), DownloadError> {
    let mut response = source.fetch_image(url).await?;
    let total = response.content_length.filter(|t| *t > 0);
    let mut file = tokio::fs::File::create(partial).await?;
    let mut head = Vec::with_capacity(HEAD_LEN);
    let mut read: u64 = 0;
    while let Some(chunk) = response.body.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        if head.len() < HEAD_LEN {
            let take = (HEAD_LEN - head.len()).min(chunk.len());
            head.extend_from_slice(&chunk[..take]);
        }
        read += chunk.len() as u64;
        ctx.speed.take(chunk.len() as u64).await;
        page.progress = total.map_or(-1, |t| (100 * read / t).min(100) as i32);
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
    use crate::index::IndexRoots;
    use crate::unit::fixtures;
    use crate::testing::{FakeSource, fake, jpeg_bytes};
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    struct Harness {
        root: tempfile::TempDir,
        index: DownloadIndex,
        layout: Layout,
        _cache: tempfile::TempDir,
    }

    fn harness() -> Harness {
        let root = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let roots = IndexRoots { manga: root.path().to_path_buf(), anime: cache.path().join("anime") };
        Harness {
            index: DownloadIndex::open(roots, cache.path()),
            layout: Layout::new(Kind::Manga, root.path()),
            root,
            _cache: cache,
        }
    }

    fn job(source: Arc<dyn ChapterSource>) -> ChapterJob {
        let q = fixtures::queued(Kind::Manga, 1, 7, 1);
        ChapterJob {
            source,
            key: q.source,
            entry: EntryRef { title: "Manga 1".into(), ..q.entry },
            chapter: UnitRef { name: "Chapter 7".into(), ..q.unit },
        }
    }

    fn ctx(h: &Harness) -> PipelineCtx {
        PipelineCtx {
            layout: h.layout.clone(),
            index: h.index.clone(),
            progress: Arc::default(),
            pages: Arc::default(),
            page_cache: None,
            split_tall_images: false,
            optimal_height: super::super::images::DEFAULT_OPTIMAL_HEIGHT,
            speed: Arc::new(SpeedLimiter::new(None)),
            save_as_cbz: false,
        }
    }

    fn chapter_dir(h: &Harness) -> PathBuf {
        h.root.path().join("Source 1 (EN)").join("Manga 1").join("Chapter 7")
    }

    #[tokio::test]
    async fn a_chapter_is_packed_as_a_cbz_with_comic_info_and_the_folder_goes() {
        let h = harness();
        let source = fake(1);
        let mut ctx = ctx(&h);
        ctx.save_as_cbz = true;
        let done = run_chapter(ctx, job(source.clone())).await.unwrap();
        assert_eq!(done, "Chapter 7", "the actor gets the bare name, with or without `.cbz` on disk");
        let manga_dir = h.root.path().join("Source 1 (EN)").join("Manga 1");
        let cbz = manga_dir.join("Chapter 7.cbz");
        assert!(cbz.is_file(), "the archive is renamed into place");
        assert!(!manga_dir.join("Chapter 7.cbz.part").exists(), "no part file is left");
        assert!(!chapter_dir(&h).exists(), "the tmp folder is removed after packing");
        let bytes = std::fs::read(&cbz).unwrap();
        assert!(bytes.windows(b"ComicInfo.xml".len()).any(|w| w == b"ComicInfo.xml"), "the archive holds ComicInfo.xml");
    }

    #[tokio::test]
    async fn a_chapter_lands_as_a_folder_and_reports_its_pages() {
        let h = harness();
        let source = fake(1);
        let ctx = ctx(&h);
        let progress = ctx.progress.clone();
        let done = run_chapter(ctx, job(source.clone())).await.unwrap();
        assert_eq!(done, "Chapter 7");
        let mut names: Vec<String> = std::fs::read_dir(chapter_dir(&h))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["001.jpg", "002.jpg", "003.jpg"]);
        assert!(!chapter_dir(&h).with_file_name("Chapter 7_tmp").exists());
        assert_eq!(progress.pages(), Some((3, 3)));
        assert_eq!(progress.percent(), 100);
    }

    #[tokio::test]
    async fn an_empty_page_list_is_an_error_and_the_tmp_folder_exists_first() {
        let h = harness();
        let source = Arc::new(FakeSource { page_count: 0, ..FakeSource::new(1) });
        let result = run_chapter(ctx(&h), job(source)).await;
        assert!(matches!(result, Err(DownloadError::NoPages)));
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
        run_chapter(ctx(&h), job(source.clone())).await.unwrap();
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
        let result = run_chapter(ctx(&h), job(source.clone())).await;
        assert!(matches!(result, Err(DownloadError::PagesFailed(1))), "{result:?}");
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
        run_chapter(ctx(&h), job(source.clone())).await.unwrap();
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
        let result = run_chapter(ctx(&h), job(source.clone())).await;
        assert!(matches!(result, Err(DownloadError::PagesFailed(1))));
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
        run_chapter(ctx(&h), job(slow.clone())).await.unwrap();
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
        async fn fetch_image(&self, url: &str) -> Result<super::super::source::ImageResponse, SourceError> {
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
        let result = run_chapter(ctx(&h), job(source)).await;
        assert!(matches!(result, Err(DownloadError::PagesFailed(1))), "{result:?}");
    }

    #[tokio::test]
    async fn the_image_type_comes_from_the_bytes_not_the_header() {
        let h = harness();
        let source = Arc::new(FakeSource {
            content_type: Some("image/webp".into()),
            page_count: 1,
            ..FakeSource::new(1)
        });
        run_chapter(ctx(&h), job(source)).await.unwrap();
        assert!(chapter_dir(&h).join("001.jpg").is_file());
    }

    #[tokio::test]
    async fn an_older_copy_of_the_chapter_is_replaced_not_an_error() {
        let h = harness();
        std::fs::create_dir_all(chapter_dir(&h)).unwrap();
        std::fs::write(chapter_dir(&h).join("stale.jpg"), b"old").unwrap();
        let source = fake(1);
        run_chapter(ctx(&h), job(source)).await.unwrap();
        assert!(chapter_dir(&h).join("001.jpg").is_file(), "the new pages are in place");
        assert!(!chapter_dir(&h).join("stale.jpg").exists(), "the older copy is gone");
        assert!(!chapter_dir(&h).with_file_name("Chapter 7_tmp").exists(), "no stray tmp folder");
    }

    #[tokio::test]
    async fn a_cbz_replaces_an_older_folder_of_the_same_chapter() {
        let h = harness();
        std::fs::create_dir_all(chapter_dir(&h)).unwrap();
        std::fs::write(chapter_dir(&h).join("stale.jpg"), b"old").unwrap();
        let mut ctx = ctx(&h);
        ctx.save_as_cbz = true;
        run_chapter(ctx, job(fake(1))).await.unwrap();
        assert!(chapter_dir(&h).with_extension("cbz").is_file());
        assert!(!chapter_dir(&h).exists(), "the older folder would shadow the archive");
    }

    #[tokio::test]
    async fn running_a_finished_chapter_again_is_a_no_op() {
        let h = harness();
        let first = fake(1);
        let done = run_chapter(ctx(&h), job(first.clone())).await.unwrap();
        // The actor records the finished unit in the index.
        h.index.add_unit(Kind::Manga, 1, "Source 1 (EN)", "Manga 1", &done);
        let second = fake(1);
        run_chapter(ctx(&h), job(second.clone())).await.unwrap();
        assert_eq!(second.calls(), 0, "RC-01: nothing is fetched twice");
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
        let result = run_chapter(ctx(&h), job(source.clone())).await;
        assert!(matches!(result, Err(DownloadError::PagesFailed(1))));
        assert_eq!(source.calls(), 0);
    }

    #[tokio::test]
    async fn a_failed_chapter_keeps_its_page_list_for_the_resume_and_a_finished_one_drops_it() {
        let h = harness();
        let source = Arc::new(FakeSource {
            page_count: 1,
            failures: std::sync::Mutex::new(vec![SourceError::Http(404)].into()),
            ..FakeSource::new(1)
        });
        let first = ctx(&h);
        let lists = first.pages.clone();
        assert!(run_chapter(first, job(source.clone())).await.is_err());
        assert_eq!(lists.lock().unwrap().get(&7).map(Vec::len), Some(1));
        let mut second = ctx(&h);
        second.pages = lists.clone();
        run_chapter(second, job(source.clone())).await.unwrap();
        assert!(lists.lock().unwrap().is_empty());
    }
}

#[cfg(test)]
mod cache_and_split_tests {
    use super::*;
    use crate::index::IndexRoots;
    use crate::unit::fixtures;
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

    fn chapter_job(source: Arc<FakeSource>) -> ChapterJob {
        let q = fixtures::queued(Kind::Manga, 1, 7, 1);
        ChapterJob {
            source,
            key: q.source,
            entry: EntryRef { title: "Manga 1".into(), ..q.entry },
            chapter: UnitRef { name: "Chapter 7".into(), ..q.unit },
        }
    }

    fn setup(root: &Path, cache: Option<Arc<dyn PageImageCache>>) -> PipelineCtx {
        let index_cache = tempfile::tempdir().unwrap();
        let roots = IndexRoots { manga: root.to_path_buf(), anime: index_cache.path().join("anime") };
        let ctx = PipelineCtx {
            layout: Layout::new(Kind::Manga, root),
            index: DownloadIndex::open(roots, index_cache.path()),
            progress: Arc::default(),
            pages: Arc::default(),
            page_cache: cache,
            split_tall_images: true,
            optimal_height: 2000,
            speed: Arc::new(SpeedLimiter::new(None)),
            save_as_cbz: false,
        };
        std::mem::forget(index_cache);
        ctx
    }

    #[tokio::test]
    async fn a_cache_hit_is_copied_and_the_entry_removed_without_a_fetch() {
        let root = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        let cached = cache_dir.path().join("entry.0");
        std::fs::write(&cached, jpeg_bytes(64)).unwrap();
        let url = "https://fake/img/0".to_owned();
        let cache = Arc::new(FakeCache { files: HashMap::from([(url.clone(), cached)]), removed: Mutex::new(Vec::new()) });
        let ctx = setup(root.path(), Some(cache.clone()));
        let source = Arc::new(FakeSource { page_count: 1, ..FakeSource::new(1) });
        run_chapter(ctx, chapter_job(source.clone())).await.unwrap();
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
        let ctx = setup(root.path(), Some(cache.clone()));
        let source = Arc::new(FakeSource { page_count: 1, ..FakeSource::new(1) });
        run_chapter(ctx, chapter_job(source.clone())).await.unwrap();
        assert_eq!(source.calls(), 1, "TST-27");
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
        let ctx = setup(root.path(), None);
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
        let ctx = setup(root.path(), None);
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
