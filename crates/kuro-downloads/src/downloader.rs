//! Downloader actor: owns the queue state, runs the scheduler and emits events (`p1-rust-design.md` sections 2, 4, 5).
//!
//! One task owns [`QueueState`]. Commands arrive on an mpsc channel, pipeline updates on another, and the
//! connectivity watch is a third input. Each loop turn ends with `reconcile`, which starts or aborts chapter tasks so
//! they match `pick_active`. With nothing running the loop sleeps on its inputs: no timer runs while idle (TST-17).

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::MissedTickBehavior;
use tokio::task::{AbortHandle, Id, JoinError, JoinSet};

use crate::error::{DownloadError, FailureKind};
use crate::events::DownloadEvent;
use crate::images::PageImageCache;
use crate::index::DownloadIndex;
use crate::layout::DownloadLayout;
use crate::manager::{EnqueueOutcome, ManagerParams};
use crate::model::{ChapterRef, DownloadState, MangaRef, QueuedDownload, SourceRef};
use crate::pipeline::{ChapterJob, ChapterOutcome, FreeSpaceFn, PipelineCtx, PipelineMsg, Update, run_chapter};
use crate::queue::QueueState;
use crate::scheduler::MAX_ACTIVE_SOURCES;
use crate::store::{QueueRow, QueueStore};

/// Progress events are coalesced to this interval per chapter (RC-05).
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);
pub const DOWNLOADS_QUEUED_WARNING_THRESHOLD: usize = 30;
pub const CHAPTERS_PER_SOURCE_QUEUE_WARNING_THRESHOLD: usize = 15;
pub const WARNING_TIMEOUT_SECS: u32 = 30;
pub const BULK_WARNING_TEXT: &str =
    "Warning: large bulk downloads may lead to sources becoming slower and/or blocking Aniyomi. Tap to learn more.";
pub const BULK_WARNING_HELP_URL: &str =
    "https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads";
pub const NO_NETWORK_TEXT: &str = "No network connection available";
pub const NO_WIFI_TEXT: &str = "No Wi-Fi connection available";
pub const COMMAND_CAPACITY: usize = 64;

/// Network state as the GUI reports it from `gio::NetworkMonitor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Connectivity {
    pub online: bool,
    pub metered: bool,
}

/// Keeps the process alive while downloads run. The returned guard releases the hold when dropped. On Linux the GUI
/// wraps `gio::Application::hold`.
pub trait ProcessHold: Send + Sync {
    fn hold(&self, reason: &str) -> Box<dyn Any + Send>;
}

/// A hold that does nothing, for tests and headless runs.
pub struct NoHold;

impl ProcessHold for NoHold {
    fn hold(&self, _reason: &str) -> Box<dyn Any + Send> {
        Box::new(())
    }
}

/// Settings read by the downloader. Changing one takes effect when the next run starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadSettings {
    /// Stop on a metered connection. Off: the settings diet cut the switch (hard-coded default).
    pub only_over_wifi: bool,
    /// Pack each finished chapter as a `.cbz`. On: the settings diet cut the switch (hard-coded default).
    pub save_as_cbz: bool,
    /// Split pages taller than 3x their width. On: the settings diet cut the switch (hard-coded default).
    pub split_tall_images: bool,
    /// Part height for the tall split: `2 * max(monitor width, height)` from GDK, set by the GUI. Not a user setting.
    pub optimal_image_height: u32,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            only_over_wifi: false,
            save_as_cbz: true,
            split_tall_images: true,
            optimal_image_height: crate::images::DEFAULT_OPTIMAL_HEIGHT,
        }
    }
}

/// What a removal applies to.
pub enum RemoveTarget {
    Chapters(Vec<i64>),
    Manga(i64),
}

pub enum Command {
    Enqueue {
        source: SourceRef,
        manga: MangaRef,
        chapters: Vec<ChapterRef>,
        auto_start: bool,
        reply: oneshot::Sender<Result<EnqueueOutcome, DownloadError>>,
    },
    Start,
    Pause,
    Clear,
    /// Puts the queue in this order. Unknown ids are ignored and unlisted chapters follow in their current order (E-15).
    Reorder {
        chapter_ids: Vec<i64>,
    },
    /// Adds chapters at the front of the queue (EC-09, EC-10, EC-11). `start` starts the run; a running queue is
    /// restarted so the new chapters are taken first.
    EnqueueFront {
        source: SourceRef,
        manga: MangaRef,
        chapters: Vec<ChapterRef>,
        start: bool,
        reply: oneshot::Sender<Result<EnqueueOutcome, DownloadError>>,
    },
    Remove {
        target: RemoveTarget,
        reply: oneshot::Sender<Vec<i64>>,
    },
    /// Stops a running queue that holds a chapter of `manga_id`, so the disk can change under it without a chapter
    /// writing into it. The reply is `true` when a run was stopped, and the caller then sends `ResumeAfterMutation`.
    PauseForMutation {
        manga_id: i64,
        reply: oneshot::Sender<bool>,
    },
    ResumeAfterMutation,
    Shutdown,
}

/// State the handle reads without a round trip: a snapshot of the queue, whether it runs, and a wakeup counter.
pub struct Shared {
    pub snapshot: RwLock<Arc<Vec<Arc<QueuedDownload>>>>,
    pub running: AtomicBool,
    pub wakeups: AtomicU64,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            snapshot: RwLock::new(Arc::new(Vec::new())),
            running: AtomicBool::new(false),
            wakeups: AtomicU64::new(0),
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

struct RunningTask {
    id: Id,
    abort: AbortHandle,
    generation: u64,
}

/// What a chapter task returns when it ends normally.
struct TaskDone {
    generation: u64,
    outcome: ChapterOutcome,
}

pub struct Downloader {
    queue: QueueState,
    running: bool,
    resume_on_network: bool,
    /// Chapters that stopped because of the network. They go back to QUEUE when it returns.
    network_failed: HashSet<i64>,
    tasks: JoinSet<TaskDone>,
    running_tasks: HashMap<i64, RunningTask>,
    task_chapter: HashMap<Id, i64>,
    next_gen: u64,
    hold: Option<Box<dyn Any + Send>>,
    hold_source: Arc<dyn ProcessHold>,
    store: Arc<QueueStore>,
    /// Saved rows whose source was missing at startup. They stay in the store after the queue rows (T-23a-035).
    kept_rows: Vec<QueueRow>,
    layout: Arc<DownloadLayout>,
    index: Arc<DownloadIndex>,
    free_space: FreeSpaceFn,
    settings: DownloadSettings,
    page_cache: Option<Arc<dyn PageImageCache>>,
    connectivity: watch::Receiver<Connectivity>,
    connectivity_open: bool,
    msg_tx: mpsc::UnboundedSender<PipelineMsg>,
    msg_rx: mpsc::UnboundedReceiver<PipelineMsg>,
    events: broadcast::Sender<DownloadEvent>,
    dirty_progress: HashSet<i64>,
    shared: Arc<Shared>,
    /// The shared speed limit. The settings diet cut `download_speed_limit`, so it stays unlimited.
    speed: Arc<crate::throttle::SpeedLimiter>,
}

impl Downloader {
    pub fn new(params: ManagerParams, events: broadcast::Sender<DownloadEvent>, shared: Arc<Shared>) -> Self {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        Self {
            queue: QueueState::restored(params.restored),
            running: false,
            resume_on_network: false,
            network_failed: HashSet::new(),
            tasks: JoinSet::new(),
            running_tasks: HashMap::new(),
            task_chapter: HashMap::new(),
            next_gen: 0,
            hold: None,
            hold_source: params.hold,
            store: params.store,
            kept_rows: params.kept_rows,
            layout: params.layout,
            index: params.index,
            free_space: params.free_space,
            settings: params.settings,
            page_cache: params.page_cache,
            connectivity: params.connectivity,
            connectivity_open: true,
            msg_tx,
            msg_rx,
            events,
            dirty_progress: HashSet::new(),
            shared,
            speed: Arc::new(crate::throttle::SpeedLimiter::new(None)),
        }
    }

    /// The actor loop. Returns on `Shutdown` or when every command sender is gone.
    pub async fn run(mut self, mut commands: mpsc::Receiver<Command>) {
        let mut flush = tokio::time::interval(PROGRESS_INTERVAL);
        flush.set_missed_tick_behavior(MissedTickBehavior::Delay);
        self.publish();
        loop {
            self.shared.wakeups.fetch_add(1, Ordering::Relaxed);
            tokio::select! {
                cmd = commands.recv() => match cmd {
                    None | Some(Command::Shutdown) => break,
                    Some(cmd) => self.handle(cmd).await,
                },
                Some(msg) = self.msg_rx.recv() => self.on_pipeline(msg),
                Some(joined) = self.tasks.join_next_with_id(), if !self.tasks.is_empty() => {
                    self.on_task_joined(joined).await;
                }
                changed = self.connectivity.changed(), if self.connectivity_open => match changed {
                    Ok(()) => self.on_connectivity().await,
                    Err(_) => self.connectivity_open = false,
                },
                _ = flush.tick(), if !self.dirty_progress.is_empty() => self.flush_progress(),
            }
            self.reconcile().await;
            self.publish();
        }
        self.abort_running_tasks();
        self.running = false;
        self.hold = None;
        self.publish();
    }

    async fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Enqueue { source, manga, chapters, auto_start, reply } => {
                let outcome = self.enqueue(source, manga, chapters, auto_start).await;
                let _ = reply.send(outcome);
            }
            Command::Start => self.start_run(true).await,
            Command::Pause => self.pause().await,
            Command::Clear => self.clear().await,
            Command::Reorder { chapter_ids } => self.reorder(&chapter_ids).await,
            Command::EnqueueFront { source, manga, chapters, start, reply } => {
                let outcome = self.enqueue_placed(source, manga, chapters, start, true).await;
                let _ = reply.send(outcome);
            }
            Command::Remove { target, reply } => {
                let ids = self.remove(target).await;
                let _ = reply.send(ids);
            }
            Command::PauseForMutation { manga_id, reply } => {
                let paused = self.queue.items().iter().any(|d| d.manga.id == manga_id) && self.pause_run();
                let _ = reply.send(paused);
            }
            Command::ResumeAfterMutation => self.resume_run().await,
            Command::Shutdown => {}
        }
    }

    async fn enqueue(
        &mut self,
        source: SourceRef,
        manga: MangaRef,
        chapters: Vec<ChapterRef>,
        auto_start: bool,
    ) -> Result<EnqueueOutcome, DownloadError> {
        self.enqueue_placed(source, manga, chapters, auto_start, false).await
    }

    async fn enqueue_placed(
        &mut self,
        source: SourceRef,
        manga: MangaRef,
        chapters: Vec<ChapterRef>,
        auto_start: bool,
        front: bool,
    ) -> Result<EnqueueOutcome, DownloadError> {
        if chapters.is_empty() {
            return Ok(EnqueueOutcome::NothingToDo);
        }
        if !source.is_http() {
            return Ok(EnqueueOutcome::SourceNotHttp);
        }
        let was_empty = self.queue.is_empty();
        let mut seen = HashSet::new();
        let mut fresh: Vec<ChapterRef> = chapters
            .into_iter()
            .filter(|c| seen.insert(c.id))
            .filter(|c| !self.queue.contains(c.id))
            .filter(|c| !self.index.is_chapter_downloaded(source.id(), &manga.title, &c.name, c.scanlator.as_deref()))
            .collect();
        // Highest source order first (`sortedByDescending { sourceOrder }`).
        fresh.sort_by_key(|c| std::cmp::Reverse(c.source_order));
        let downloads = fresh
            .into_iter()
            .map(|chapter| QueuedDownload {
                source: source.clone(),
                manga: manga.clone(),
                chapter,
                status: DownloadState::Queue,
                pages: None,
            })
            .collect();
        let added = if front { self.queue.add_front(downloads) } else { self.queue.add_all(downloads) }.len();
        if added == 0 {
            return Ok(EnqueueOutcome::NothingToDo);
        }
        self.persist().await;
        self.emit(DownloadEvent::QueueChanged);
        if auto_start && (was_empty || front) {
            // A running queue is restarted so the front chapters are taken first (EC-14).
            if front {
                self.pause_run();
            }
            self.warn_if_bulk();
            self.start_run(true).await;
        }
        Ok(EnqueueOutcome::Queued(added))
    }

    /// Warns when more than 30 non-unmetered chapters are queued, or more than 15 from one source.
    fn warn_if_bulk(&self) {
        let mut per_source: HashMap<i64, usize> = HashMap::new();
        for d in self.queue.items().iter().filter(|d| !d.source.is_unmetered()) {
            *per_source.entry(d.source.id()).or_default() += 1;
        }
        let total: usize = per_source.values().sum();
        let widest = per_source.values().copied().max().unwrap_or(0);
        if total > DOWNLOADS_QUEUED_WARNING_THRESHOLD || widest > CHAPTERS_PER_SOURCE_QUEUE_WARNING_THRESHOLD {
            self.emit(DownloadEvent::Warning {
                text: BULK_WARNING_TEXT.into(),
                timeout_secs: Some(WARNING_TIMEOUT_SECS),
                help_url: Some(BULK_WARNING_HELP_URL.into()),
            });
        }
    }

    /// `Dl.start`. An explicit start retries every ERROR chapter. An automatic resume (after the network returns)
    /// retries only the chapters the network stopped.
    async fn start_run(&mut self, explicit: bool) {
        if self.running || self.queue.is_empty() {
            return;
        }
        let changed = if explicit {
            self.queue.reset_for_start()
        } else {
            self.queue.requeue_errors(&self.network_failed)
        };
        self.emit_status_all(changed, DownloadState::Queue);
        self.network_failed.clear();
        self.queue.set_paused(false);
        self.resume_on_network = false;
        self.running = true;
        self.hold = Some(self.hold_source.hold("manga downloads"));
        self.emit(DownloadEvent::Started);
        let now = *self.connectivity.borrow();
        if let Err(reason) = gate(now, self.settings.only_over_wifi) {
            self.resume_on_network = true;
            self.stop(Some(reason)).await;
        }
    }

    async fn pause(&mut self) {
        self.abort_running_tasks();
        let changed = self.queue.pause_running();
        self.emit_status_all(changed, DownloadState::Queue);
        self.resume_on_network = false;
        self.stop(None).await;
    }

    /// `Dl.stop`. Running chapters become ERROR. With a reason, a warning is shown and the run ends. Without one,
    /// the run ends with `Paused` (user paused, items left) or `Complete`. The paused flag is cleared either way, so
    /// it only ever describes the stop that is ending now. A user pause also clears `resume_on_network`, which is
    /// what keeps a paused queue from starting when the network returns.
    async fn stop(&mut self, reason: Option<&str>) {
        self.abort_running_tasks();
        let errored = self.queue.stop_running();
        if reason.is_some() {
            // Stopped by the network: these chapters resume when it returns.
            self.network_failed.extend(errored.iter().copied());
        }
        self.emit_status_all(errored, DownloadState::Error);
        self.running = false;
        self.hold = None;
        match reason {
            Some(text) => self.emit(DownloadEvent::Warning { text: text.to_owned(), timeout_secs: None, help_url: None }),
            None if self.queue.is_paused() && !self.queue.is_empty() => self.emit(DownloadEvent::Paused),
            None => self.emit(DownloadEvent::Complete),
        }
        self.queue.set_paused(false);
    }

    async fn clear(&mut self) {
        self.abort_running_tasks();
        let _ = self.queue.clear();
        self.network_failed.clear();
        self.emit(DownloadEvent::Dismiss);
        self.persist().await;
        self.emit(DownloadEvent::QueueChanged);
        self.stop(None).await;
    }

    /// Puts the queue in the given order. Unknown ids are ignored and unlisted chapters follow (E-15).
    /// Reorders the queue. A running queue is paused, reordered and started again (EC-14).
    async fn reorder(&mut self, chapter_ids: &[i64]) {
        if chapter_ids.is_empty() {
            return;
        }
        let was_running = self.pause_run();
        self.queue.reorder(chapter_ids);
        self.persist().await;
        self.emit(DownloadEvent::QueueChanged);
        if was_running {
            self.resume_run().await;
        }
    }

    /// Removes chapters or a manga from the queue. A running queue is paused first and restarted afterwards, so a
    /// chapter is never written while its folder is being deleted (RC-04, RC-11).
    async fn remove(&mut self, target: RemoveTarget) -> Vec<i64> {
        let was_running = self.pause_run();
        let removed = match target {
            RemoveTarget::Chapters(ids) => self.queue.remove_chapters(&ids),
            RemoveTarget::Manga(manga_id) => self.queue.remove_manga(manga_id),
        };
        let ids: Vec<i64> = removed.iter().map(|d| d.chapter.id).collect();
        for id in &ids {
            self.network_failed.remove(id);
        }
        if !ids.is_empty() {
            self.persist().await;
            self.emit(DownloadEvent::QueueChanged);
        }
        if was_running {
            self.resume_run().await;
        }
        ids
    }

    /// Aborts the running chapters and puts them back in the queue, so nothing writes while the disk or the queue
    /// changes. Returns `false` when no run was going.
    fn pause_run(&mut self) -> bool {
        if !self.running {
            return false;
        }
        self.abort_running_tasks();
        let requeued = self.queue.requeue_running();
        self.emit_status_all(requeued, DownloadState::Queue);
        self.running = false;
        self.hold = None;
        true
    }

    /// Undoes [`Self::pause_run`]: ends the run when the queue is empty, otherwise starts it again.
    async fn resume_run(&mut self) {
        if self.queue.is_empty() {
            self.stop(None).await;
        } else {
            self.start_run(true).await;
        }
    }

    /// Starts and aborts chapter tasks so they match the scheduler's choice. Ends the run when nothing is left.
    async fn reconcile(&mut self) {
        if !self.running {
            return;
        }
        let wanted = self.queue.active_ids(MAX_ACTIVE_SOURCES);
        let stale: Vec<i64> = self.running_tasks.keys().copied().filter(|id| !wanted.contains(id)).collect();
        for chapter_id in stale {
            self.abort_task(chapter_id);
            if let Ok(true) = self.queue.set_status(chapter_id, DownloadState::Queue) {
                self.emit(DownloadEvent::Status { chapter_id, state: DownloadState::Queue });
            }
        }
        for chapter_id in &wanted {
            if !self.running_tasks.contains_key(chapter_id) {
                self.spawn_chapter(*chapter_id);
            }
        }
        if wanted.is_empty() && self.running_tasks.is_empty() {
            self.stop(None).await;
        }
    }

    fn spawn_chapter(&mut self, chapter_id: i64) {
        let Some(item) = self.queue.get(chapter_id).cloned() else { return };
        self.next_gen += 1;
        let generation = self.next_gen;
        let ctx = PipelineCtx {
            layout: self.layout.clone(),
            index: self.index.clone(),
            free_space: self.free_space,
            chapter_id,
            generation,
            tx: self.msg_tx.clone(),
            page_cache: self.page_cache.clone(),
            split_tall_images: self.settings.split_tall_images,
            optimal_height: self.settings.optimal_image_height,
            speed: self.speed.clone(),
            save_as_cbz: self.settings.save_as_cbz,
        };
        let job = ChapterJob {
            source: item.source.clone(),
            manga: item.manga.clone(),
            chapter: item.chapter.clone(),
            pages: item.pages.clone(),
        };
        let abort = self.tasks.spawn(async move {
            let outcome = run_chapter(ctx, job).await;
            TaskDone { generation, outcome }
        });
        let id = abort.id();
        self.task_chapter.insert(id, chapter_id);
        self.running_tasks.insert(chapter_id, RunningTask { id, abort, generation });
    }

    fn abort_task(&mut self, chapter_id: i64) {
        if let Some(task) = self.running_tasks.remove(&chapter_id) {
            task.abort.abort();
            self.task_chapter.remove(&task.id);
        }
    }

    fn abort_running_tasks(&mut self) {
        let ids: Vec<i64> = self.running_tasks.keys().copied().collect();
        for id in ids {
            self.abort_task(id);
        }
    }

    async fn on_task_joined(&mut self, joined: Result<(Id, TaskDone), JoinError>) {
        // Updates a task sent before it ended may still be queued behind this join: apply them first, while the
        // task's entry still matches, so its last page errors are reported.
        self.drain_pipeline();
        match joined {
            Ok((id, done)) => {
                // A task aborted by us has no entry here, so its late result is dropped.
                let Some(chapter_id) = self.task_chapter.remove(&id) else { return };
                if self.running_tasks.get(&chapter_id).is_none_or(|t| t.generation != done.generation) {
                    return;
                }
                self.running_tasks.remove(&chapter_id);
                self.finish(chapter_id, done.outcome).await;
            }
            Err(err) => {
                let Some(chapter_id) = self.task_chapter.remove(&err.id()) else { return };
                self.running_tasks.remove(&chapter_id);
                let message = if err.is_panic() {
                    panic_message(err.into_panic())
                } else {
                    "chapter task was cancelled".to_owned()
                };
                self.finish(chapter_id, ChapterOutcome::Errored(DownloadError::Internal(message))).await;
            }
        }
    }

    async fn finish(&mut self, chapter_id: i64, outcome: ChapterOutcome) {
        let Some(item) = self.queue.get(chapter_id).cloned() else { return };
        match outcome {
            ChapterOutcome::Downloaded => {
                self.network_failed.remove(&chapter_id);
                self.queue.mark_downloaded(chapter_id);
                self.emit(DownloadEvent::Status { chapter_id, state: DownloadState::Downloaded });
                self.emit(DownloadEvent::Finished { chapter_id });
                self.persist().await;
                self.emit(DownloadEvent::QueueChanged);
            }
            ChapterOutcome::Errored(err) => {
                if err.failure_kind() == FailureKind::Network {
                    self.network_failed.insert(chapter_id);
                }
                if let Ok(true) = self.queue.set_status(chapter_id, DownloadState::Error) {
                    self.emit(DownloadEvent::Status { chapter_id, state: DownloadState::Error });
                }
                // Per-page failures were already reported one by one.
                if !matches!(err, DownloadError::PagesFailed(_)) {
                    self.emit(DownloadEvent::Error {
                        chapter_id: Some(chapter_id),
                        manga_id: Some(item.manga.id),
                        manga_title: Some(item.manga.title.clone()),
                        chapter_name: Some(item.chapter.name.clone()),
                        message: err.to_string(),
                    });
                }
                // The folder cannot be written: stop the whole run, as Kotlin does (FM-01).
                if matches!(err, DownloadError::InvalidLocation(_)) {
                    self.stop(None).await;
                }
            }
        }
    }

    fn drain_pipeline(&mut self) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            self.on_pipeline(msg);
        }
    }

    fn on_pipeline(&mut self, msg: PipelineMsg) {
        let current = self.running_tasks.get(&msg.chapter_id).is_some_and(|t| t.generation == msg.generation);
        if !current {
            return;
        }
        let chapter_id = msg.chapter_id;
        match msg.update {
            Update::Status(state) => {
                if let Ok(true) = self.queue.set_status(chapter_id, state) {
                    self.emit(DownloadEvent::Status { chapter_id, state });
                }
            }
            Update::Pages(pages) => {
                self.queue.update(chapter_id, |d| d.pages = Some(pages));
                self.dirty_progress.insert(chapter_id);
            }
            Update::Page { index, state, progress } => {
                let changed = self.queue.update(chapter_id, |d| {
                    if let Some(page) = d.pages.as_mut().and_then(|p| p.get_mut(index)) {
                        page.state = state;
                        page.progress = progress;
                    }
                });
                if changed {
                    self.dirty_progress.insert(chapter_id);
                }
            }
            Update::PageFailed { message, .. } => {
                if let Some(item) = self.queue.get(chapter_id).cloned() {
                    self.emit(DownloadEvent::Error {
                        chapter_id: Some(chapter_id),
                        manga_id: Some(item.manga.id),
                        manga_title: Some(item.manga.title.clone()),
                        chapter_name: Some(item.chapter.name.clone()),
                        message,
                    });
                }
            }
        }
    }

    fn flush_progress(&mut self) {
        for chapter_id in std::mem::take(&mut self.dirty_progress) {
            if let Some(item) = self.queue.get(chapter_id) {
                let event = DownloadEvent::Progress {
                    chapter_id,
                    manga_title: item.manga.title.clone(),
                    chapter_name: item.chapter.name.clone(),
                    downloaded: item.downloaded_images(),
                    total: item.pages.as_ref().map_or(0, Vec::len),
                };
                self.emit(event);
            }
        }
    }

    /// Stops a run when the network is lost or the connection is metered with "Wi-Fi only" on. Resumes an automatic
    /// run when the network returns, unless the user paused.
    async fn on_connectivity(&mut self) {
        let now = *self.connectivity.borrow_and_update();
        match (self.running, gate(now, self.settings.only_over_wifi)) {
            (true, Err(reason)) => {
                self.resume_on_network = true;
                self.stop(Some(reason)).await;
            }
            (false, Ok(())) if self.resume_on_network => {
                self.start_run(false).await;
            }
            _ => {}
        }
    }

    async fn persist(&mut self) {
        let now = unix_millis();
        let rows: Vec<QueueRow> = self
            .queue
            .items()
            .iter()
            .enumerate()
            .map(|(position, d)| QueueRow {
                chapter_id: d.chapter.id,
                manga_id: d.manga.id,
                source_id: d.source.id(),
                position: position as i64,
                status: d.status as u8,
                // Written at save time: the queue does not keep the enqueue instant.
                enqueued_at: now,
            })
            .collect();
        let mut rows = rows;
        let base = rows.len() as i64;
        rows.extend(self.kept_rows.iter().enumerate().map(|(i, row)| QueueRow { position: base + i as i64, ..row.clone() }));
        let store = self.store.clone();
        match tokio::task::spawn_blocking(move || store.replace_all(&rows)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::error!(target: "kuro_downloads", "could not save the queue: {e}"),
            Err(e) => tracing::error!(target: "kuro_downloads", "queue save task failed: {e}"),
        }
    }

    fn publish(&self) {
        let snapshot = Arc::new(self.queue.snapshot());
        *self.shared.snapshot.write().unwrap_or_else(|e| e.into_inner()) = snapshot;
        self.shared.running.store(self.running, Ordering::Relaxed);
    }

    fn emit(&self, event: DownloadEvent) {
        let _ = self.events.send(event);
    }

    fn emit_status_all(&self, ids: Vec<i64>, state: DownloadState) {
        for chapter_id in ids {
            self.emit(DownloadEvent::Status { chapter_id, state });
        }
    }
}

/// The connectivity rule of `checkNetworkState`: offline stops with the network text, and a metered link stops
/// with the Wi-Fi text when "Wi-Fi only" is on.
pub fn gate(c: Connectivity, only_over_wifi: bool) -> Result<(), &'static str> {
    if !c.online {
        Err(NO_NETWORK_TEXT)
    } else if c.metered && only_over_wifi {
        Err(NO_WIFI_TEXT)
    } else {
        Ok(())
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "chapter task panicked".to_owned()
    }
}

pub(crate) fn unix_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::AtomicUsize;

    use crate::testing::{chapter, fake, manga};

    #[test]
    fn gate_follows_the_connectivity_table() {
        let offline = Connectivity { online: false, metered: false };
        let metered = Connectivity { online: true, metered: true };
        let free = Connectivity { online: true, metered: false };
        assert_eq!(gate(offline, true), Err(NO_NETWORK_TEXT));
        assert_eq!(gate(offline, false), Err(NO_NETWORK_TEXT));
        assert_eq!(gate(metered, true), Err(NO_WIFI_TEXT));
        assert_eq!(gate(metered, false), Ok(()));
        assert_eq!(gate(free, true), Ok(()));
    }

    #[test]
    fn default_settings_wait_for_an_unmetered_link() {
        assert!(!DownloadSettings::default().only_over_wifi, "the settings diet cut the switch; the default is off");
    }

    fn plenty(_: &Path) -> Option<u64> {
        Some(u64::MAX / 2)
    }

    /// Counts holds taken and released, standing in for the GUI's `gio::Application::hold`.
    #[derive(Default)]
    struct CountingHold {
        takes: AtomicUsize,
        active: Arc<AtomicUsize>,
    }

    struct HoldGuard(Arc<AtomicUsize>);

    impl Drop for HoldGuard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl ProcessHold for CountingHold {
        fn hold(&self, _reason: &str) -> Box<dyn Any + Send> {
            self.takes.fetch_add(1, Ordering::SeqCst);
            self.active.fetch_add(1, Ordering::SeqCst);
            Box::new(HoldGuard(self.active.clone()))
        }
    }

    /// A downloader with no actor loop around it, so a test drives each step itself.
    struct Harness {
        dl: Downloader,
        events: broadcast::Receiver<DownloadEvent>,
        conn: watch::Sender<Connectivity>,
        _base: tempfile::TempDir,
    }

    fn harness(hold: Arc<dyn ProcessHold>) -> Harness {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("downloads");
        std::fs::create_dir_all(&root).unwrap();
        let (conn, connectivity) = watch::channel(Connectivity { online: true, metered: false });
        let (events_tx, events) = broadcast::channel(256);
        let params = ManagerParams {
            layout: Arc::new(DownloadLayout::new(&root)),
            index: Arc::new(DownloadIndex::open(&root, &base.path().join("cache"))),
            store: Arc::new(QueueStore::open_in_memory().unwrap()),
            free_space: plenty,
            settings: DownloadSettings { save_as_cbz: false, ..DownloadSettings::default() },
            page_cache: None,
            connectivity,
            hold,
            restored: Vec::new(),
        kept_rows: Vec::new(),
        pending: Arc::new(crate::pending::MangaPendingStore::open_in_memory().unwrap()),
        };
        let dl = Downloader::new(params, events_tx, Arc::new(Shared::new()));
        Harness { dl, events, conn, _base: base }
    }

    async fn enqueue_chapters(dl: &mut Downloader, ids: &[i64]) {
        let source: SourceRef = fake(1);
        let chapters = ids.iter().map(|id| chapter(*id, 1, &format!("Chapter {id}"))).collect();
        let outcome = dl.enqueue(source, manga(1, 1, "Berserk"), chapters, false).await.unwrap();
        assert_eq!(outcome, EnqueueOutcome::Queued(ids.len()));
    }

    fn drain(events: &mut broadcast::Receiver<DownloadEvent>) -> Vec<DownloadEvent> {
        std::iter::from_fn(|| events.try_recv().ok()).collect()
    }


    #[tokio::test]
    async fn the_process_hold_is_taken_on_start_and_released_on_stop_or_pause() {
        let hold = Arc::new(CountingHold::default());
        let mut h = harness(hold.clone());
        enqueue_chapters(&mut h.dl, &[1, 2]).await;
        h.dl.start_run(true).await;
        assert_eq!(hold.active.load(Ordering::SeqCst), 1, "a running queue holds the process");
        h.dl.stop(None).await;
        assert_eq!(hold.active.load(Ordering::SeqCst), 0, "a stop releases the hold");
        h.dl.start_run(true).await;
        assert_eq!(hold.takes.load(Ordering::SeqCst), 2);
        h.dl.pause().await;
        assert_eq!(hold.active.load(Ordering::SeqCst), 0, "a pause releases the hold");
    }

    #[tokio::test]
    async fn stop_with_a_reason_clears_the_paused_flag_and_releases_the_hold() {
        let hold = Arc::new(CountingHold::default());
        let mut h = harness(hold.clone());
        enqueue_chapters(&mut h.dl, &[1]).await;
        h.dl.start_run(true).await;
        h.dl.queue.set_paused(true);
        h.conn.send(Connectivity { online: false, metered: false }).unwrap();
        h.dl.on_connectivity().await;
        assert!(!h.dl.running);
        assert!(!h.dl.queue.is_paused(), "EC-17: a stop with a reason clears the paused flag");
        assert_eq!(hold.active.load(Ordering::SeqCst), 0, "EC-17: the hold is released");
        let warned = drain(&mut h.events)
            .iter()
            .any(|e| matches!(e, DownloadEvent::Warning { text, .. } if text.as_str() == NO_NETWORK_TEXT));
        assert!(warned, "EC-17: the network warning is shown");
    }

    #[tokio::test]
    async fn reorder_while_running_pauses_reorders_and_restarts() {
        let mut h = harness(Arc::new(NoHold));
        enqueue_chapters(&mut h.dl, &[1, 2]).await;
        h.dl.start_run(true).await;
        assert!(h.dl.running);
        h.dl.reorder(&[2]).await;
        let order: Vec<i64> = h.dl.queue.items().iter().map(|d| d.chapter.id).collect();
        assert_eq!(order, vec![2, 1], "EC-14: the new order is applied");
        assert!(h.dl.running, "EC-14: the run starts again after the reorder");
    }

    #[tokio::test]
    async fn start_while_running_is_a_no_op() {
        let hold = Arc::new(CountingHold::default());
        let mut h = harness(hold.clone());
        enqueue_chapters(&mut h.dl, &[1, 2]).await;
        h.dl.start_run(true).await;
        h.dl.start_run(true).await;
        assert_eq!(hold.takes.load(Ordering::SeqCst), 1, "EC-19: a second start takes no new hold");
        let started = drain(&mut h.events).iter().filter(|e| matches!(e, DownloadEvent::Started)).count();
        assert_eq!(started, 1, "EC-19: a second start emits no Started");
        assert!(h.dl.running);
    }

    #[tokio::test]
    async fn pause_then_completion_race() {
        let mut h = harness(Arc::new(NoHold));
        enqueue_chapters(&mut h.dl, &[1]).await;
        h.dl.start_run(true).await;
        h.dl.reconcile().await;
        // The chapter task finishes, but its result is not joined yet when the pause lands.
        tokio::time::timeout(Duration::from_secs(20), async {
            while !h.dl.running_tasks[&1_i64].abort.is_finished() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the chapter task finishes");
        h.dl.pause().await;
        let joined = h.dl.tasks.join_next_with_id().await.expect("the finished task is still in the set");
        h.dl.on_task_joined(joined).await;
        let events = drain(&mut h.events);
        assert!(
            !events.iter().any(|e| matches!(e, DownloadEvent::Finished { .. })),
            "RC-01: no Finished after a pause"
        );
        assert!(
            !events.iter().any(|e| matches!(e, DownloadEvent::Status { state: DownloadState::Downloaded, .. })),
            "RC-01: no Downloaded status after a pause"
        );
        assert_eq!(h.dl.queue.get(1).map(|d| d.status), Some(DownloadState::Queue), "RC-01: the chapter stays queued");

        // The chapter was on disk before the pause, so the next start finishes it without an error or a stray tmp.
        h.dl.start_run(true).await;
        h.dl.reconcile().await;
        let joined = tokio::time::timeout(Duration::from_secs(20), h.dl.tasks.join_next_with_id())
            .await
            .expect("the second run finishes")
            .expect("the second task is in the set");
        h.dl.on_task_joined(joined).await;
        let events = drain(&mut h.events);
        assert!(events.iter().any(|e| matches!(e, DownloadEvent::Finished { chapter_id: 1 })), "RC-01: DOWNLOADED after the restart");
        assert!(!events.iter().any(|e| matches!(e, DownloadEvent::Error { .. })), "RC-01: no ERROR after the restart");
        assert!(h.dl.queue.get(1).is_none(), "the finished chapter leaves the queue");
        let manga_dir = h._base.path().join("downloads").join("Source 1 (EN)").join("Berserk");
        assert!(!manga_dir.join("Chapter 1_tmp").exists(), "RC-01: no stray tmp folder");
        assert!(manga_dir.join("Chapter 1").is_dir());
    }
}
