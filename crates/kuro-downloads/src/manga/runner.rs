//! `ChapterPipeline`: the manga `UnitRunner` (docs/design/unified-downloads.md 2.3). It holds what every chapter run
//! shares (the live sources, the page cache, the splitter settings, the speed limit) and the page lists of chapters that
//! were started in this process (B37); the page work itself is `pipeline::run_chapter`.

use std::sync::Arc;

use async_trait::async_trait;
use kuro_platform::kind::Kind;
use kuro_source::SourceError;

use super::images::{DEFAULT_OPTIMAL_HEIGHT, PageImageCache};
use super::pipeline::{ChapterJob, PageLists, PipelineCtx, run_chapter};
use crate::error::DownloadError;
use crate::next::naming::{CBZ_PART_SUFFIX, UnitName, staging_dir_names, unit_dir_name};
use crate::next::runner::{ChapterSources, RunCtx, UnitDone, UnitJob, UnitRunner};
use crate::next::unit::{Availability, SourceKey, UnitRef};
use crate::throttle::SpeedLimiter;

pub struct ChapterPipeline {
    sources: Arc<dyn ChapterSources>,
    /// Reader page cache, checked before the network. `None` when the reader cache is not wired.
    pub page_cache: Option<Arc<dyn PageImageCache>>,
    /// Pack each finished chapter as a `.cbz`. On: the settings diet cut the switch (hard-coded default).
    pub save_as_cbz: bool,
    /// Split pages taller than 3x their width. On: the settings diet cut the switch (hard-coded default).
    pub split_tall_images: bool,
    /// Part height for the tall split: `2 * max(monitor width, height)` from GDK, set by the GUI. Not a user setting.
    pub optimal_height: u32,
    /// The settings diet cut `download_speed_limit`, so it stays unlimited.
    speed: Arc<SpeedLimiter>,
    // ponytail: an entry stays until its chapter finishes, so a chapter cancelled from the queue leaves its page list
    // behind for the session; evict on cancel if that ever shows in memory.
    pages: Arc<PageLists>,
}

impl ChapterPipeline {
    pub fn new(sources: Arc<dyn ChapterSources>) -> Self {
        Self {
            sources,
            page_cache: None,
            save_as_cbz: true,
            split_tall_images: true,
            optimal_height: DEFAULT_OPTIMAL_HEIGHT,
            speed: Arc::new(SpeedLimiter::new(None)),
            pages: Arc::default(),
        }
    }
}

#[async_trait]
impl UnitRunner for ChapterPipeline {
    fn kind(&self) -> Kind {
        Kind::Manga
    }

    fn can_run(&self, source: &SourceKey) -> bool {
        source.availability == Availability::Live && self.sources.chapter_source(source.id).is_some()
    }

    /// `<name>_tmp` and `<name>.cbz.part`.
    fn staging_names(&self, unit: &UnitRef) -> Vec<String> {
        let name = UnitName::new(&unit.name, unit.scanlator.as_deref());
        let mut names = staging_dir_names(Kind::Manga, name);
        names.push(format!("{}{CBZ_PART_SUFFIX}", unit_dir_name(Kind::Manga, name)));
        names
    }

    async fn run(&self, job: UnitJob, ctx: RunCtx) -> Result<UnitDone, DownloadError> {
        let UnitJob { entry, unit, source } = job;
        let RunCtx { cancel, progress, layout, index } = ctx;
        let live = self
            .sources
            .chapter_source(source.id)
            .ok_or(DownloadError::Source(SourceError::Unsupported("the source is not installed")))?;
        let pipeline = PipelineCtx {
            layout,
            index,
            progress,
            pages: self.pages.clone(),
            page_cache: self.page_cache.clone(),
            split_tall_images: self.split_tall_images,
            optimal_height: self.optimal_height,
            speed: self.speed.clone(),
            save_as_cbz: self.save_as_cbz,
        };
        let chapter = ChapterJob { source: live, key: source, entry, chapter: unit };
        // The actor used to abort the task; dropping the run here stops it at the same awaits.
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(DownloadError::Cancelled),
            done = run_chapter(pipeline, chapter) => done.map(|dir_name| UnitDone { dir_name }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manga::source::ChapterSource;
    use crate::next::index::{DownloadIndex, IndexRoots};
    use crate::next::layout::Layout;
    use crate::next::unit::{EntryRef, fixtures};
    use crate::testing::{FakeSource, fake, jpeg_bytes};
    use std::path::{Path, PathBuf};
    use tokio_util::sync::CancellationToken;

    struct Sources(Option<Arc<FakeSource>>);

    impl ChapterSources for Sources {
        fn chapter_source(&self, _source_id: i64) -> Option<Arc<dyn ChapterSource>> {
            self.0.clone().map(|s| s as Arc<dyn ChapterSource>)
        }
    }

    fn pipeline(source: &Arc<FakeSource>) -> ChapterPipeline {
        let mut p = ChapterPipeline::new(Arc::new(Sources(Some(source.clone()))));
        p.save_as_cbz = false;
        p
    }

    fn job() -> UnitJob {
        let q = fixtures::queued(Kind::Manga, 1, 7, 1);
        UnitJob {
            entry: EntryRef { title: "Manga 1".into(), ..q.entry },
            unit: UnitRef { name: "Chapter 7".into(), ..q.unit },
            source: q.source,
        }
    }

    fn run_ctx(root: &Path, cancel: CancellationToken) -> RunCtx {
        let cache = tempfile::tempdir().unwrap();
        let roots = IndexRoots { manga: root.to_path_buf(), anime: cache.path().join("anime") };
        let ctx = RunCtx {
            cancel,
            progress: Arc::default(),
            layout: Layout::new(Kind::Manga, root),
            index: DownloadIndex::open(roots, cache.path()),
        };
        std::mem::forget(cache);
        ctx
    }

    fn chapter_dir(root: &Path) -> PathBuf {
        root.join("Source 1 (EN)").join("Manga 1").join("Chapter 7")
    }

    #[tokio::test]
    async fn a_chapter_reports_the_folder_name_and_its_pages() {
        let root = tempfile::tempdir().unwrap();
        let source = fake(1);
        let ctx = run_ctx(root.path(), CancellationToken::new());
        let progress = ctx.progress.clone();
        let done = pipeline(&source).run(job(), ctx).await.unwrap();
        assert_eq!(done.dir_name, "Chapter 7");
        assert!(chapter_dir(root.path()).join("003.jpg").is_file());
        assert_eq!(progress.pages(), Some((3, 3)));
    }

    #[tokio::test]
    async fn a_restarted_chapter_reuses_its_staged_pages() {
        let root = tempfile::tempdir().unwrap();
        let tmp = chapter_dir(root.path()).with_file_name("Chapter 7_tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("001.jpg"), jpeg_bytes(10)).unwrap();
        std::fs::write(tmp.join("002.jpg"), jpeg_bytes(10)).unwrap();
        // A new pipeline is what a restart gives: it has no page list, so it asks the source again.
        let source = fake(1);
        let done = pipeline(&source).run(job(), run_ctx(root.path(), CancellationToken::new())).await.unwrap();
        assert_eq!(done.dir_name, "Chapter 7");
        assert_eq!(source.calls(), 1, "pages 1 and 2 came from the staging folder, only page 3 was fetched");
        assert!(chapter_dir(root.path()).join("001.jpg").is_file());
        assert!(!tmp.exists(), "the staging folder is gone after the finish");
    }

    #[tokio::test]
    async fn a_failing_chapter_reports_the_error_and_keeps_its_staging_folder() {
        let root = tempfile::tempdir().unwrap();
        let source = Arc::new(FakeSource { image_url_error: Some(SourceError::Http(500)), page_count: 2, ..FakeSource::new(1) });
        let result = pipeline(&source).run(job(), run_ctx(root.path(), CancellationToken::new())).await;
        assert!(matches!(result, Err(DownloadError::PagesFailed(2))), "{:?}", result.err());
        assert!(chapter_dir(root.path()).with_file_name("Chapter 7_tmp").is_dir(), "kept for the resume");
        let source = Arc::new(FakeSource {
            page_list_error: Some(SourceError::Transport("reset".into())),
            ..FakeSource::new(1)
        });
        let err = pipeline(&source).run(job(), run_ctx(root.path(), CancellationToken::new())).await.err().unwrap();
        assert_eq!(err.failure_kind(), crate::error::FailureKind::Network);
    }

    #[tokio::test]
    async fn a_cancelled_chapter_is_cancelled_and_fetches_nothing() {
        let root = tempfile::tempdir().unwrap();
        let source = fake(1);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = pipeline(&source).run(job(), run_ctx(root.path(), cancel)).await;
        assert!(matches!(result, Err(DownloadError::Cancelled)));
        assert_eq!(source.calls(), 0);
    }

    #[tokio::test]
    async fn the_finished_chapter_replaces_an_existing_one() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(chapter_dir(root.path())).unwrap();
        std::fs::write(chapter_dir(root.path()).join("stale.jpg"), b"old").unwrap();
        let source = fake(1);
        pipeline(&source).run(job(), run_ctx(root.path(), CancellationToken::new())).await.unwrap();
        assert!(chapter_dir(root.path()).join("001.jpg").is_file());
        assert!(!chapter_dir(root.path()).join("stale.jpg").exists());
    }

    #[test]
    fn a_source_that_is_not_installed_cannot_run_and_the_staging_names_are_listed() {
        let source = fake(1);
        let live = pipeline(&source);
        let q = fixtures::queued(Kind::Manga, 1, 7, 1);
        assert!(live.can_run(&q.source));
        assert!(!ChapterPipeline::new(Arc::new(Sources(None))).can_run(&q.source));
        let stub = SourceKey { availability: Availability::Stub, ..q.source.clone() };
        assert!(!live.can_run(&stub));
        let unit = UnitRef { name: "Ch 1".into(), ..q.unit };
        assert_eq!(live.staging_names(&unit), vec!["Ch 1_tmp".to_owned(), "Ch 1.cbz.part".to_owned()]);
    }
}
