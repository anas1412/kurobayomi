//! `QueueEngine`: the one command surface the download page uses for both kinds (T-25a-017), and the conversion of an
//! engine snapshot into a [`QueueModel`] (T-25a-020). Every command is a non-blocking send to the engine task.
//!
//! The anime engine stops with `Pause`, and the manga engine stops with `Pause` too (its pause already ends the run).
//! Kotlin's manga `pauseDownloads` is `pause(); stop()`, and the Rust actor does both in one step.

use kuro_platform::kind::Kind;
use std::sync::Arc;

use crate::anime::manager::{AnimeQueue, Command as AnimeCommand};
use crate::anime::model::AnimeDownload;
use crate::downloader::{Command as MangaCommand, RemoveTarget};
use crate::manager::DownloadManager;
use crate::model::QueuedDownload;
use crate::queue_model::{QueueItem, QueueModel};

/// The commands the page sends. Calls return at once; the result shows up in the next snapshot.
pub trait QueueEngine: Send + Sync {
    /// Starts the run. Does nothing while it runs (E-30).
    fn start(&self);
    /// Stops the run. Running items go back to the queue.
    fn stop(&self);
    /// Pauses, then stops (manga Pause, E-29).
    fn pause_and_stop(&self);
    /// Clears the queue, then stops (E-16).
    fn clear(&self);
    /// Puts the queue in the given order. Unknown ids are ignored, unlisted items follow, and an empty list does nothing
    /// (E-15).
    fn reorder(&self, item_ids: &[i64]);
    /// Removes items from the queue. Downloaded files are not touched.
    fn remove(&self, item_ids: &[i64]);
}

/// The anime queue adapter.
pub struct AnimeEngine {
    queue: AnimeQueue,
}

impl AnimeEngine {
    pub fn new(queue: AnimeQueue) -> Self {
        Self { queue }
    }

    fn send(&self, command: AnimeCommand) {
        if let Err(err) = self.queue.send(command) {
            tracing::warn!(target: "kuro_downloads", error = %err, "anime queue command dropped");
        }
    }
}

impl QueueEngine for AnimeEngine {
    fn start(&self) {
        self.send(AnimeCommand::Start);
    }

    fn stop(&self) {
        self.send(AnimeCommand::Pause);
    }

    fn pause_and_stop(&self) {
        self.stop();
    }

    fn clear(&self) {
        self.send(AnimeCommand::ClearQueue);
    }

    fn reorder(&self, item_ids: &[i64]) {
        // The actor ignores unknown ids and appends unlisted ones, so a stale snapshot cannot drop an item (E-15).
        if !item_ids.is_empty() {
            self.send(AnimeCommand::Reorder { episode_ids: item_ids.to_vec() });
        }
    }

    fn remove(&self, item_ids: &[i64]) {
        if !item_ids.is_empty() {
            self.send(AnimeCommand::Cancel { episode_ids: item_ids.to_vec() });
        }
    }
}

/// The manga queue adapter.
pub struct MangaEngine {
    manager: Arc<DownloadManager>,
}

impl MangaEngine {
    pub fn new(manager: Arc<DownloadManager>) -> Self {
        Self { manager }
    }

    fn post(&self, command: MangaCommand) {
        if let Err(err) = self.manager.post(command) {
            tracing::warn!(target: "kuro_downloads", error = %err, "manga queue command dropped");
        }
    }
}

impl QueueEngine for MangaEngine {
    fn start(&self) {
        self.post(MangaCommand::Start);
    }

    fn stop(&self) {
        self.post(MangaCommand::Pause);
    }

    fn pause_and_stop(&self) {
        self.post(MangaCommand::Pause);
    }

    fn clear(&self) {
        self.post(MangaCommand::Clear);
    }

    fn reorder(&self, item_ids: &[i64]) {
        if !item_ids.is_empty() {
            self.post(MangaCommand::Reorder { chapter_ids: item_ids.to_vec() });
        }
    }

    fn remove(&self, item_ids: &[i64]) {
        if item_ids.is_empty() {
            return;
        }
        // The reply is not needed here, so its receiver is dropped at once.
        let (reply, _answer) = tokio::sync::oneshot::channel();
        self.post(MangaCommand::Remove { target: RemoveTarget::Chapters(item_ids.to_vec()), reply });
    }
}

/// The flat queue of anime downloads as the page shows it.
pub fn anime_model(snapshot: &[Arc<AnimeDownload>]) -> QueueModel {
    QueueModel::from_flat(
        snapshot
            .iter()
            .map(|d| {
                let progress = i64::from(d.progress.get());
                QueueItem {
                    kind: Kind::Anime,
                    item_id: d.episode.id,
                    parent_id: d.anime.id,
                    source_id: d.source.id,
                    source_name: d.source.name.clone(),
                    title: d.anime.title.clone(),
                    item_name: d.episode.name.clone(),
                    date_upload: d.episode.upload_date,
                    number: d.episode.episode_number,
                    state: d.status(),
                    pages_done: 0,
                    pages_total: None,
                    progress,
                }
            })
            .collect(),
    )
}

/// The flat queue of manga downloads as the page shows it.
pub fn manga_model(snapshot: &[Arc<QueuedDownload>]) -> QueueModel {
    QueueModel::from_flat(
        snapshot
            .iter()
            .map(|d| QueueItem {
                kind: Kind::Manga,
                item_id: d.chapter.id,
                parent_id: d.manga.id,
                source_id: d.source.id(),
                source_name: d.source.display_name(),
                title: d.manga.title.clone(),
                item_name: d.chapter.name.clone(),
                date_upload: d.chapter.date_upload,
                number: d.chapter.chapter_number,
                state: d.status,
                pages_done: d.downloaded_images() as u32,
                pages_total: d.pages.as_ref().map(|p| p.len() as u32),
                progress: d.total_progress(),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DownloadState;
    use crate::anime::model::{AnimeRef, AnimeSource, EpisodeRef};
    use crate::testing::{chapter, fake_source, manga};
    use std::time::Duration;

    fn setup() -> (tempfile::TempDir, Arc<DownloadManager>) {
        let base = tempfile::tempdir().unwrap();
        let manager = crate::testing::manga_manager(base.path());
        (base, manager)
    }

    /// Polls the queue until it holds `expected`, or gives up after two seconds and returns what it holds.
    async fn settled(manager: &DownloadManager, expected: &[i64]) -> Vec<i64> {
        let mut ids = Vec::new();
        for _ in 0..400 {
            ids = manager.snapshot().iter().map(|d| d.chapter.id).collect();
            if ids == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        ids
    }

    async fn enqueue_three(manager: &Arc<DownloadManager>) {
        let source = fake_source(1);
        let series = manga(7, 1, "Series");
        let chapters = vec![chapter(1, 7, "One"), chapter(2, 7, "Two"), chapter(3, 7, "Three")];
        manager.enqueue(source, series, chapters, false).await.unwrap();
    }

    #[tokio::test]
    async fn reorder_sends_the_normalised_order_and_drops_nothing() {
        let (_base, manager) = setup();
        enqueue_three(&manager).await;
        let engine = MangaEngine::new(manager.clone());
        // Enqueue puts the highest source order first, so the queue starts as [3, 2, 1].
        engine.reorder(&[1, 99]);
        assert_eq!(
            settled(&manager, &[1, 3, 2]).await,
            vec![1, 3, 2],
            "E-15: unknown ids ignored, the rest follow in order"
        );
    }

    #[tokio::test]
    async fn an_empty_reorder_is_a_no_op_and_does_not_clear() {
        let (_base, manager) = setup();
        enqueue_three(&manager).await;
        let engine = MangaEngine::new(manager.clone());
        engine.reorder(&[]);
        assert_eq!(settled(&manager, &[3, 2, 1]).await, vec![3, 2, 1], "an empty list never clears the queue");
    }

    #[tokio::test]
    async fn remove_takes_the_items_out_of_the_queue() {
        let (_base, manager) = setup();
        enqueue_three(&manager).await;
        let engine = MangaEngine::new(manager.clone());
        engine.remove(&[2]);
        assert_eq!(settled(&manager, &[3, 1]).await, vec![3, 1]);
    }

    #[tokio::test]
    async fn clear_empties_the_queue() {
        let (_base, manager) = setup();
        enqueue_three(&manager).await;
        let engine = MangaEngine::new(manager.clone());
        engine.clear();
        assert!(settled(&manager, &[]).await.is_empty(), "E-16: clear then stop");
        assert!(!manager.running());
    }

    #[test]
    fn manga_model_maps_the_snapshot_fields() {
        let src = fake_source(4);
        let item = crate::testing::download(&src, 9, 5, DownloadState::Queue);
        let model = manga_model(&[Arc::new(item)]);
        let row = &model.flat()[0];
        assert_eq!((row.kind, row.item_id, row.parent_id, row.source_id), (Kind::Manga, 9, 5, 4));
        assert_eq!(row.state, DownloadState::Queue);
        assert_eq!(row.pages_total, None, "no page list yet");
    }

    fn anime_source() -> AnimeSource {
        AnimeSource { id: 1, name: "S".into(), lang: "en".into(), is_http: true, unmetered: false }
    }

    fn episode(id: i64) -> EpisodeRef {
        EpisodeRef {
            id,
            anime_id: 5,
            url: format!("/e/{id}"),
            name: format!("Ep {id}"),
            scanlator: None,
            seen: false,
            bookmark: false,
            fillermark: false,
            source_order: id,
            upload_date: 0,
            episode_number: id as f64,
        }
    }

    /// Queues episodes 1, 2 and 3 without starting. The queue sorts by source order, highest first: [3, 2, 1].
    async fn queue_three_episodes(queue: &AnimeQueue) {
        let anime = AnimeRef { id: 5, source_id: 1, url: "/a/5".into(), title: "Show".into() };
        let episodes = vec![episode(1), episode(2), episode(3)];
        queue.enqueue(anime_source(), anime, episodes, false, false, None).await.unwrap();
    }

    fn anime_ids(queue: &AnimeQueue) -> Vec<i64> {
        queue.snapshot().iter().map(|d| d.episode.id).collect()
    }

    /// Waits until `done` holds for the queue, for up to two seconds, then returns.
    async fn wait_for(queue: &AnimeQueue, done: impl Fn(&AnimeQueue) -> bool) {
        for _ in 0..400 {
            if done(queue) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    #[tokio::test]
    async fn anime_start_is_a_no_op_while_the_run_is_on() {
        let root = tempfile::tempdir().unwrap();
        let queue = crate::testing::anime_queue(root.path());
        queue_three_episodes(&queue).await;
        let engine = AnimeEngine::new(queue.clone());
        engine.start();
        wait_for(&queue, |q| q.running()).await;
        engine.start();
        assert!(queue.running(), "E-30: a second start does nothing");
        assert_eq!(anime_ids(&queue), vec![3, 2, 1], "nothing is re-queued or dropped");
    }

    #[tokio::test]
    async fn anime_stop_ends_the_run_and_keeps_the_items_queued() {
        let root = tempfile::tempdir().unwrap();
        let queue = crate::testing::anime_queue(root.path());
        queue_three_episodes(&queue).await;
        let engine = AnimeEngine::new(queue.clone());
        engine.start();
        wait_for(&queue, |q| q.running()).await;
        engine.stop();
        wait_for(&queue, |q| !q.running()).await;
        assert!(!queue.running());
        assert_eq!(anime_ids(&queue), vec![3, 2, 1], "a stop keeps the queue");
    }

    #[tokio::test]
    async fn anime_pause_and_stop_is_the_same_stop() {
        let root = tempfile::tempdir().unwrap();
        let queue = crate::testing::anime_queue(root.path());
        queue_three_episodes(&queue).await;
        let engine = AnimeEngine::new(queue.clone());
        engine.start();
        wait_for(&queue, |q| q.running()).await;
        engine.pause_and_stop();
        wait_for(&queue, |q| !q.running()).await;
        assert!(!queue.running(), "E-29: anime has no separate pause");
        assert_eq!(anime_ids(&queue), vec![3, 2, 1]);
    }

    #[tokio::test]
    async fn anime_clear_empties_the_queue_and_stops_the_run() {
        let root = tempfile::tempdir().unwrap();
        let queue = crate::testing::anime_queue(root.path());
        queue_three_episodes(&queue).await;
        let engine = AnimeEngine::new(queue.clone());
        engine.start();
        wait_for(&queue, |q| q.running()).await;
        engine.clear();
        wait_for(&queue, |q| q.snapshot().is_empty() && !q.running()).await;
        assert!(anime_ids(&queue).is_empty(), "E-16: clear empties the queue");
        assert!(!queue.running(), "E-16: and stops the run");
    }

    #[tokio::test]
    async fn anime_reorder_and_remove_follow_the_queue_rules() {
        let root = tempfile::tempdir().unwrap();
        let queue = crate::testing::anime_queue(root.path());
        queue_three_episodes(&queue).await;
        let engine = AnimeEngine::new(queue.clone());
        engine.reorder(&[1, 99]);
        wait_for(&queue, |q| anime_ids(q) == vec![1, 3, 2]).await;
        assert_eq!(anime_ids(&queue), vec![1, 3, 2], "E-15: unknown ids ignored, the rest follow in order");
        engine.reorder(&[]);
        assert_eq!(anime_ids(&queue), vec![1, 3, 2], "an empty reorder is a no-op");
        engine.remove(&[3]);
        wait_for(&queue, |q| anime_ids(q) == vec![1, 2]).await;
        assert_eq!(anime_ids(&queue), vec![1, 2], "remove takes the item out, the files stay");
    }
}
