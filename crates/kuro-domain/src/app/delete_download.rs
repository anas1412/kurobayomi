//! Download deletion for a list of items (`DeleteChapterDownload.kt`, `DeleteEpisodeDownload.kt`; T-10a-015). The
//! interactor resolves the entry's source in the registry and hands the whole list to the download manager in one call.
//! A source that is not installed is not an error: the interactor logs a warning and deletes nothing (EC-27). The
//! deletion runs under [`non_cancellable`], so a closed screen does not stop it (EC-29).
//!
//! The download manager is a port ([`ChapterDownloadFacade`], [`EpisodeDownloadFacade`]) because `kuro-downloads` sits
//! above this crate. The interactors also implement the synchronous [`ChapterDownloads`] and [`EpisodeDownloads`] ports
//! that `SetReadStatus` and `SetSeenStatus` call, so they are the production adapters of those two traits. The adapters
//! hand the work to the current tokio runtime and do not wait for it.

use std::sync::Arc;

use async_trait::async_trait;
use kuro_data::anime::Anime;
use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;
use kuro_data::manga::Manga;

use crate::app::rx::non_cancellable;
use crate::app::status::{ChapterDownloads, EpisodeDownloads};
use crate::source::registry::{InstalledSource, SourceRegistry};

/// What the interactor needs from the manga download manager (`DownloadManager.deleteChapters`).
#[async_trait]
pub trait ChapterDownloadFacade<S: ?Sized>: Send + Sync {
    /// Deletes the downloaded files of `chapters` of `manga`, which `source` serves.
    async fn delete_chapters(&self, chapters: &[Chapter], manga: &Manga, source: Arc<S>);
}

/// What the interactor needs from the anime download manager (`AnimeDownloadManager.deleteEpisodes`).
#[async_trait]
pub trait EpisodeDownloadFacade<S: ?Sized>: Send + Sync {
    /// Deletes the downloaded files of `episodes` of `anime`, which `source` serves.
    async fn delete_episodes(&self, episodes: &[Episode], anime: &Anime, source: Arc<S>);
}

/// `DeleteChapterDownload`.
pub struct DeleteChapterDownload<S: ?Sized> {
    sources: Arc<SourceRegistry<S>>,
    downloads: Arc<dyn ChapterDownloadFacade<S>>,
}

impl<S: ?Sized> Clone for DeleteChapterDownload<S> {
    fn clone(&self) -> Self {
        Self { sources: Arc::clone(&self.sources), downloads: Arc::clone(&self.downloads) }
    }
}

impl<S: InstalledSource + ?Sized + 'static> DeleteChapterDownload<S> {
    pub fn new(sources: Arc<SourceRegistry<S>>, downloads: Arc<dyn ChapterDownloadFacade<S>>) -> Self {
        Self { sources, downloads }
    }

    /// `awaitAll(manga, chapters)`: one `delete_chapters` call with the full slice. An absent source logs a warning and
    /// returns without deleting (EC-27).
    pub async fn await_all(&self, manga: &Manga, chapters: &[Chapter]) {
        let Some(source) = self.sources.get(manga.source) else {
            tracing::warn!(manga_id = manga.id, source_id = manga.source, "no source for the manga; downloads are kept");
            return;
        };
        let downloads = Arc::clone(&self.downloads);
        let manga = manga.clone();
        let chapters = chapters.to_vec();
        non_cancellable(async move { downloads.delete_chapters(&chapters, &manga, source).await }).await;
    }
}

impl<S: InstalledSource + ?Sized + 'static> ChapterDownloads for DeleteChapterDownload<S> {
    fn delete_chapters(&self, manga: &Manga, chapters: &[Chapter]) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::warn!(manga_id = manga.id, "no runtime for the download deletion; downloads are kept");
            return;
        };
        let this = self.clone();
        let manga = manga.clone();
        let chapters = chapters.to_vec();
        runtime.spawn(async move { this.await_all(&manga, &chapters).await });
    }
}

/// `DeleteEpisodeDownload`.
pub struct DeleteEpisodeDownload<S: ?Sized> {
    sources: Arc<SourceRegistry<S>>,
    downloads: Arc<dyn EpisodeDownloadFacade<S>>,
}

impl<S: ?Sized> Clone for DeleteEpisodeDownload<S> {
    fn clone(&self) -> Self {
        Self { sources: Arc::clone(&self.sources), downloads: Arc::clone(&self.downloads) }
    }
}

impl<S: InstalledSource + ?Sized + 'static> DeleteEpisodeDownload<S> {
    pub fn new(sources: Arc<SourceRegistry<S>>, downloads: Arc<dyn EpisodeDownloadFacade<S>>) -> Self {
        Self { sources, downloads }
    }

    /// `awaitAll(anime, episodes)`: one `delete_episodes` call with the full slice. An absent source logs a warning and
    /// returns without deleting (EC-27).
    pub async fn await_all(&self, anime: &Anime, episodes: &[Episode]) {
        let Some(source) = self.sources.get(anime.source) else {
            tracing::warn!(anime_id = anime.id, source_id = anime.source, "no source for the anime; downloads are kept");
            return;
        };
        let downloads = Arc::clone(&self.downloads);
        let anime = anime.clone();
        let episodes = episodes.to_vec();
        non_cancellable(async move { downloads.delete_episodes(&episodes, &anime, source).await }).await;
    }
}

impl<S: InstalledSource + ?Sized + 'static> EpisodeDownloads for DeleteEpisodeDownload<S> {
    fn delete_episodes(&self, anime: &Anime, episodes: &[Episode]) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::warn!(anime_id = anime.id, "no runtime for the download deletion; downloads are kept");
            return;
        };
        let this = self.clone();
        let anime = anime.clone();
        let episodes = episodes.to_vec();
        runtime.spawn(async move { this.await_all(&anime, &episodes).await });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::source::fakes::{FakeCatalog, Installed, MemStubStore};
    use crate::source::model::{Id, Source, SourceKind};

    /// Records `(entry id, item ids, source id)` of every deletion call.
    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<(i64, Vec<i64>, Id)>>,
    }

    #[async_trait]
    impl ChapterDownloadFacade<Installed> for Recorder {
        async fn delete_chapters(&self, chapters: &[Chapter], manga: &Manga, source: Arc<Installed>) {
            // A yield point, so a dropped caller would cancel an interrupted deletion.
            tokio::task::yield_now().await;
            self.calls.lock().unwrap().push((manga.id, chapters.iter().map(|c| c.id).collect(), source.id));
        }
    }

    #[async_trait]
    impl EpisodeDownloadFacade<Installed> for Recorder {
        async fn delete_episodes(&self, episodes: &[Episode], anime: &Anime, source: Arc<Installed>) {
            tokio::task::yield_now().await;
            self.calls.lock().unwrap().push((anime.id, episodes.iter().map(|e| e.id).collect(), source.id));
        }
    }

    fn registry(kind: SourceKind, installed: &[Id]) -> Arc<SourceRegistry<Installed>> {
        let registry =
            SourceRegistry::new(kind, Arc::new(MemStubStore::default()), Arc::new(FakeCatalog::default()));
        registry.finish_loading(
            installed
                .iter()
                .map(|&id| {
                    Arc::new(Installed {
                        id,
                        http: true,
                        catalogue: true,
                        source: Source::new(id, &format!("S{id}"), "en"),
                    })
                })
                .collect(),
        );
        Arc::new(registry)
    }

    fn manga(id: i64, source: i64) -> Manga {
        Manga { id, source, ..Manga::create() }
    }

    fn chapters(ids: &[i64]) -> Vec<Chapter> {
        ids.iter().map(|&id| Chapter { id, ..Chapter::create() }).collect()
    }

    /// Captures what the subscriber logs on this thread.
    #[derive(Clone, Default)]
    struct Logs(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Logs {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Logs {
        type Writer = Logs;
        fn make_writer(&'a self) -> Logs {
            self.clone()
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime")
    }

    #[test]
    fn the_whole_slice_goes_to_one_call_with_the_resolved_source() {
        let downloads = Arc::new(Recorder::default());
        let interactor = DeleteChapterDownload::new(registry(SourceKind::Manga, &[7]), downloads.clone());
        runtime().block_on(interactor.await_all(&manga(3, 7), &chapters(&[1, 2, 3])));
        assert_eq!(*downloads.calls.lock().unwrap(), vec![(3, vec![1, 2, 3], 7)]);
    }

    #[test]
    fn an_absent_source_logs_a_warning_and_deletes_nothing() {
        let downloads = Arc::new(Recorder::default());
        let interactor = DeleteChapterDownload::new(registry(SourceKind::Manga, &[7]), downloads.clone());
        let logs = Logs::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(logs.clone())
            .with_ansi(false)
            .without_time()
            .with_max_level(tracing::Level::WARN)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            runtime().block_on(interactor.await_all(&manga(3, 99), &chapters(&[1])));
        });
        assert!(downloads.calls.lock().unwrap().is_empty(), "EC-27");
        let text = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
        assert!(text.contains("WARN") && text.contains("no source for the manga"), "{text}");
    }

    #[test]
    fn the_deletion_finishes_when_the_caller_stops_waiting() {
        let downloads = Arc::new(Recorder::default());
        let interactor = DeleteChapterDownload::new(registry(SourceKind::Manga, &[7]), downloads.clone());
        runtime().block_on(async {
            let (entry, rows) = (manga(3, 7), chapters(&[1]));
            let work = interactor.await_all(&entry, &rows);
            // The caller gives up after one poll; the spawned deletion still lands.
            let _ = tokio::time::timeout(std::time::Duration::ZERO, work).await;
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        });
        assert_eq!(downloads.calls.lock().unwrap().len(), 1, "EC-29");
    }

    #[test]
    fn the_status_port_adapter_hands_the_chapters_to_the_interactor() {
        let downloads = Arc::new(Recorder::default());
        let interactor = DeleteChapterDownload::new(registry(SourceKind::Manga, &[7]), downloads.clone());
        runtime().block_on(async {
            ChapterDownloads::delete_chapters(&interactor, &manga(4, 7), &chapters(&[5, 6]));
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        });
        assert_eq!(*downloads.calls.lock().unwrap(), vec![(4, vec![5, 6], 7)]);
    }

    #[test]
    fn the_episode_interactor_has_the_same_contract() {
        let downloads = Arc::new(Recorder::default());
        let interactor = DeleteEpisodeDownload::new(registry(SourceKind::Anime, &[9]), downloads.clone());
        let anime = Anime { id: 2, source: 9, ..Anime::create() };
        let episodes: Vec<Episode> = [10, 11].iter().map(|&id| Episode { id, ..Episode::create() }).collect();
        runtime().block_on(interactor.await_all(&anime, &episodes));
        assert_eq!(*downloads.calls.lock().unwrap(), vec![(2, vec![10, 11], 9)]);

        let lost = Anime { id: 3, source: 1, ..Anime::create() };
        runtime().block_on(interactor.await_all(&lost, &episodes));
        assert_eq!(downloads.calls.lock().unwrap().len(), 1, "an absent source deletes nothing");
    }
}
