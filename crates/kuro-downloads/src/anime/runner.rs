//! `EpisodeEngine` as the anime `UnitRunner` (docs/design/unified-downloads.md 2.3). The engine keeps its resolver, its
//! ffmpeg programs and its copy fallback; this file only maps the shared job and context onto them.

use async_trait::async_trait;
use kuro_platform::kind::Kind;

use super::downloader::EpisodeEngine;
use super::model::{AnimeDownload, AnimeRef, AnimeSource, EpisodeRef};
use crate::error::DownloadError;
use crate::layout::Layout;
use crate::naming::{UnitName, staging_dir_names};
use crate::runner::{RunCtx, UnitDone, UnitJob, UnitRunner};
use crate::unit::{Availability, SourceKey, UnitRef};

/// The engine's view of a job: the old item type its resolver and ffmpeg helpers take.
fn download_of(job: &UnitJob) -> AnimeDownload {
    let (entry, unit, source) = (&job.entry, &job.unit, &job.source);
    AnimeDownload::new(
        AnimeSource {
            id: source.id,
            name: source.name.clone(),
            lang: source.lang.clone(),
            is_http: true,
            unmetered: source.unmetered,
        },
        AnimeRef { id: entry.id, source_id: source.id, url: entry.url.clone(), title: entry.title.clone() },
        EpisodeRef {
            id: unit.id,
            anime_id: unit.entry_id,
            url: unit.url.clone(),
            name: unit.name.clone(),
            scanlator: unit.scanlator.clone(),
            seen: unit.done,
            bookmark: unit.bookmark,
            fillermark: unit.fillermark,
            source_order: unit.source_order,
            upload_date: unit.date_upload,
            episode_number: unit.number,
        },
        false,
        None,
    )
}

#[async_trait]
impl UnitRunner for EpisodeEngine {
    fn kind(&self) -> Kind {
        Kind::Anime
    }

    /// The hoster of a source that is not installed cannot be asked.
    fn can_run(&self, source: &SourceKey) -> bool {
        source.availability == Availability::Live
    }

    /// `<name>_tmp` for the current and the legacy episode name.
    fn staging_names(&self, unit: &UnitRef) -> Vec<String> {
        staging_dir_names(Kind::Anime, UnitName::new(&unit.name, unit.scanlator.as_deref()))
    }

    async fn run(&self, job: UnitJob, ctx: RunCtx) -> Result<UnitDone, DownloadError> {
        let RunCtx { cancel, progress, layout, index } = ctx;
        let source_dir = Layout::source_dir_name(&job.source.name, &job.source.lang);
        let name = UnitName::new(&job.unit.name, job.unit.scanlator.as_deref());
        let anime_dir = layout.entry_dir(&source_dir, &job.entry.title)?;

        // Already on disk (a pause dropped the result of an episode that had just finished): nothing to do.
        if index.is_downloaded(Kind::Anime, job.source.id, &job.entry.title, &job.unit.name, job.unit.scanlator.as_deref()) {
            if let Some(found) = layout.find_unit(&source_dir, &job.entry.title, name) {
                let dir_name = found.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                return Ok(UnitDone { dir_name });
            }
        }

        let item = download_of(&job);
        // The engine reports a percent on the item's own cell; it is forwarded to the shared progress.
        let mut percent = item.progress.subscribe();
        let forward = async {
            while percent.changed().await.is_ok() {
                progress.set_percent(*percent.borrow_and_update() as u8);
            }
            std::future::pending::<()>().await
        };
        tokio::select! {
            biased;
            done = self.download(&item, &anime_dir, &cancel) => done.map(|dir_name| UnitDone { dir_name }),
            _ = forward => unreachable!("the percent cell lives as long as the item"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::resolve::HosterResolver;
    use crate::index::{DownloadIndex, IndexRoots};
    use crate::unit::{EntryRef, fixtures};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    use crate::anime::model::Video;

    static SPAWN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    struct Hoster(Option<Video>);

    #[async_trait]
    impl HosterResolver for Hoster {
        async fn best_video(&self, _: &AnimeRef, _: &EpisodeRef, _: &AnimeSource) -> Result<Option<Video>, DownloadError> {
            Ok(self.0.clone())
        }
    }

    fn engine(root: &Path, video: Option<Video>) -> EpisodeEngine {
        let ffprobe = script(root, "ffprobe", "echo 30.0");
        // The fake ffmpeg writes the partial file (the argument before the trailing `-y`), then ends.
        let ffmpeg = script(
            root,
            "ffmpeg",
            "prev=; last=; for a; do prev=$last; last=$a; done\nprintf video > \"$prev\"\n\
             printf 'out_time_us=30000000\\nprogress=end\\n'",
        );
        EpisodeEngine::new(Arc::new(Hoster(video)))
            .with_programs(ffmpeg.as_os_str(), ffprobe.as_os_str())
    }

    fn video() -> Video {
        Video { url: "/local/source.mkv".into(), title: "v".into(), ..Default::default() }
    }

    fn job() -> UnitJob {
        let q = fixtures::queued(Kind::Anime, 1, 5, 10);
        UnitJob {
            entry: EntryRef { title: "Show".into(), ..q.entry },
            unit: UnitRef { name: "Episode 1".into(), ..q.unit },
            source: SourceKey { name: "Src".into(), ..q.source },
        }
    }

    fn run_ctx(root: &Path, cancel: CancellationToken) -> RunCtx {
        let cache = tempfile::tempdir().unwrap();
        let roots = IndexRoots { manga: cache.path().join("manga"), anime: root.to_path_buf() };
        let ctx = RunCtx {
            cancel,
            progress: Arc::default(),
            layout: Layout::new(Kind::Anime, root),
            index: DownloadIndex::open(roots, cache.path()),
        };
        std::mem::forget(cache);
        ctx
    }

    fn folder(root: &Path) -> PathBuf {
        root.join("Src (EN)").join("Show")
    }

    #[tokio::test(start_paused = true)]
    async fn the_episode_lands_in_its_folder_and_the_folder_name_is_reported() {
        let _guard = SPAWN_LOCK.lock().await;
        let root = tempfile::tempdir().unwrap();
        let engine = engine(root.path(), Some(video()));
        let done = engine.run(job(), run_ctx(root.path(), CancellationToken::new())).await.unwrap();
        assert_eq!(done.dir_name, "Episode 1");
        assert!(folder(root.path()).join("Episode 1").join("Episode 1.mkv").is_file());
        assert!(!folder(root.path()).join("Episode 1_tmp").exists());
    }

    #[tokio::test(start_paused = true)]
    async fn the_finished_episode_replaces_an_existing_one() {
        let _guard = SPAWN_LOCK.lock().await;
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(folder(root.path()).join("Episode 1")).unwrap();
        fs::write(folder(root.path()).join("Episode 1").join("old.mkv"), b"old").unwrap();
        let engine = engine(root.path(), Some(video()));
        let done = engine.run(job(), run_ctx(root.path(), CancellationToken::new())).await.unwrap();
        assert_eq!(done.dir_name, "Episode 1", "the plain name, not `Episode 1 (5)`");
        assert!(folder(root.path()).join("Episode 1").join("Episode 1.mkv").is_file());
        assert!(!folder(root.path()).join("Episode 1").join("old.mkv").exists());
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_episode_reports_the_error() {
        let _guard = SPAWN_LOCK.lock().await;
        let root = tempfile::tempdir().unwrap();
        let engine = engine(root.path(), None);
        let result = engine.run(job(), run_ctx(root.path(), CancellationToken::new())).await;
        assert!(matches!(result, Err(DownloadError::NoVideo)), "{:?}", result.err());
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancelled_episode_is_cancelled() {
        let _guard = SPAWN_LOCK.lock().await;
        let root = tempfile::tempdir().unwrap();
        let engine = engine(root.path(), Some(video()));
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = engine.run(job(), run_ctx(root.path(), cancel)).await;
        assert!(matches!(result, Err(DownloadError::Cancelled)), "{:?}", result.err());
    }

    #[tokio::test(start_paused = true)]
    async fn an_episode_already_on_disk_is_not_downloaded_again() {
        let _guard = SPAWN_LOCK.lock().await;
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(folder(root.path()).join("Episode 1")).unwrap();
        let ctx = run_ctx(root.path(), CancellationToken::new());
        ctx.index.add_unit(Kind::Anime, 5, "Src (EN)", "Show", "Episode 1");
        // No video: a download would fail with NoVideo.
        let engine = engine(root.path(), None);
        let mut j = job();
        j.source.id = 5;
        let done = engine.run(j, ctx).await.unwrap();
        assert_eq!(done.dir_name, "Episode 1");
    }

    #[test]
    fn staging_names_cover_the_current_and_the_legacy_name_and_only_live_sources_run() {
        let root = tempfile::tempdir().unwrap();
        let engine = engine(root.path(), None);
        let q = fixtures::queued(Kind::Anime, 1, 5, 10);
        let unit = UnitRef { name: "Ep 1".into(), scanlator: Some("Scan".into()), ..q.unit };
        assert_eq!(engine.staging_names(&unit), vec!["Scan_Ep 1_tmp".to_owned()]);
        assert!(engine.can_run(&q.source));
        assert!(!engine.can_run(&SourceKey { availability: Availability::Stub, ..q.source }));
    }
}
