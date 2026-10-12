//! Image loader wiring at startup (area:34 design section 2, behavior section 2, T-34a-055).
//!
//! The app calls `start` once, after the tokio runtime and the network client exist, and hands the result to the
//! cover widgets, the full-cover viewer and the reader. One service means one set of fetch and decode slots and
//! one decoded-image cache for all three.

use std::io;
use std::sync::Arc;

use kuro_net::NetworkClient;
use kuro_platform::paths::Paths;

use crate::chapter_cache::ChapterCache;
use crate::fetch::{ImageService, SourceHttp};
use crate::memory_cache::DEFAULT_BUDGET_BYTES;
use crate::platform::chapter_cache_dir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageConfig {
    /// Budget of the decoded-image cache (design section 6.1).
    pub memory_budget_bytes: u64,
    /// The `verbose_logging` preference (behavior section 2.5).
    pub verbose: bool,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            memory_budget_bytes: DEFAULT_BUDGET_BYTES,
            verbose: false,
        }
    }
}

/// What the app keeps for the lifetime of the process.
pub struct ImageStack {
    /// Covers, backgrounds and reader pages all decode through this one service.
    pub images: Arc<ImageService>,
    /// The reader's page-list and page-image cache.
    pub chapters: Arc<ChapterCache>,
    /// The tracing directive for this crate, to add to the app's filter (see `log_directive`).
    pub log_directive: &'static str,
}

/// The tracing filter directive for `kuro_images`: `debug` when `verbose_logging` is on, otherwise warnings and
/// errors only. Re-apply it when the preference changes.
pub fn log_directive(verbose: bool) -> &'static str {
    if verbose {
        "kuro_images=debug"
    } else {
        "kuro_images=warn"
    }
}

/// Creates the shared image service and the chapter cache.
pub async fn start(
    shared: NetworkClient,
    sources: Arc<dyn SourceHttp>,
    paths: Arc<dyn Paths + Send + Sync>,
    config: &ImageConfig,
) -> io::Result<ImageStack> {
    let chapters =
        ChapterCache::open_at(chapter_cache_dir(&*paths), crate::chapter_cache::CACHE_SIZE).await?;
    let images = ImageService::new(shared, sources, paths)
        .await?
        .with_memory_budget(config.memory_budget_bytes);
    Ok(ImageStack {
        images: Arc::new(images),
        chapters: Arc::new(chapters),
        log_directive: log_directive(config.verbose),
    })
}

#[cfg(test)]
mod tests {
    use kuro_platform::kind::Kind;
    use super::*;
    use crate::art::{ArtKind, ArtRef};
    use crate::fetch::{DataSource, LoadRequest};
    use crate::options::LoadOptions;
    use crate::testing::{NoSources, TestPaths, encode, gradient, test_client};
    use tokio_util::sync::CancellationToken;

    #[test]
    fn verbose_logging_selects_debug() {
        assert_eq!(log_directive(true), "kuro_images=debug");
        assert_eq!(log_directive(false), "kuro_images=warn");
    }

    #[test]
    fn default_config_is_64_mib_and_quiet() {
        let config = ImageConfig::default();
        assert_eq!(config.memory_budget_bytes, 64 * 1024 * 1024);
        assert!(!config.verbose);
    }

    #[tokio::test]
    async fn start_creates_one_service_and_the_chapter_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let paths: Arc<dyn Paths + Send + Sync> = Arc::new(TestPaths::new(tmp.path()));
        let config = ImageConfig {
            memory_budget_bytes: 1_000_000,
            verbose: true,
        };
        let stack = start(test_client(tmp.path()), Arc::new(NoSources), paths, &config)
            .await
            .unwrap();
        assert_eq!(stack.log_directive, "kuro_images=debug");
        assert!(tmp.path().join("cache/chapter_disk_cache").is_dir());
        assert!(tmp.path().join("cache/image_cache").is_dir());

        // The same service answers a cover request and a reader page, and shares one decoded-image cache.
        let file = tmp.path().join("cover.png");
        std::fs::write(&file, encode(&gradient(10, 10), image::ImageFormat::Png)).unwrap();
        let req = LoadRequest {
            art: ArtRef {
                owner: Kind::Manga,
                entry_id: 1,
                source_id: 1,
                kind: ArtKind::Cover,
                url: Some(file.to_str().unwrap().to_owned()),
                favorite: false,
                last_modified: 1,
            },
            options: LoadOptions::default(),
            cancel: CancellationToken::new(),
        };
        assert_eq!(
            stack.images.load(&req).await.unwrap().source,
            DataSource::Disk
        );
        assert_eq!(
            stack.images.load(&req).await.unwrap().source,
            DataSource::Memory
        );
        stack.chapters.put_image("u", b"page").await.unwrap();
        assert!(stack.chapters.is_image_cached("u").await);
    }
}
