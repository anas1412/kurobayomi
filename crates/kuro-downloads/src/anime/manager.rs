//! Anime queue manager: the command channel and the one task that owns the queue (T-24a-010, T-24a-011; design 3 and 6,
//! behaviour 2.x).
//!
//! The actor is the only writer of the queue and of the store. Commands arrive on an mpsc channel, and each episode
//! runner reports back on a second channel. Every loop turn ends with `reconcile`, which starts a runner for the first
//! active item of each source, at most three sources at once (T-24a-011). A runner that fails leaves its item in ERROR,
//! which frees the slot for the next item of that source.
//!
//! Lookups that need the database (`startDownloadNow` in Kotlin) happen in the caller. The caller passes the full
//! [`QueueEntry`]. The real download pipeline is an [`EpisodeRunner`]: it is supplied by the engine and tests use fakes.

//!
//! # Kotlin `AnimeDownloadManager` public API, one by one (T-24a-010)
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `isRunning`, `isDownloaderRunning` | [`AnimeQueue::running`] |
//! | `queueState`, `statusFlow`, `progressFlow` | [`AnimeQueue::snapshot`], then each item's `state` and `progress` |
//! | `getQueuedDownloadOrNull` | [`AnimeQueue::queued`] |
//! | `startDownloads`, `pauseDownloads`, `clearQueue` | `Start`, `Pause`, `ClearQueue` |
//! | `startDownloadNow` | `StartNow` |
//! | `reorderQueue` (`updateQueue`) | `Reorder` |
//! | `downloadEpisodes` | `Enqueue` |
//! | `addDownloadsToStartOfQueue` | `AddToStart` |
//! | `cancelQueuedDownloads`, private `removeFromDownloadQueue` | `Cancel`, `CancelAnime` |
//! | `deleteEpisodes`, `deleteAnime` | `DeleteEpisodes`, `DeleteAnime` |
//! | `enqueueEpisodesToDelete` | `EnqueueDelete` (the bookmark and category filter is the caller's, T-24a-033) |
//! | `deletePendingEpisodes` | `FlushPendingDeletes` |
//! | `renameSource`, `renameEpisode` | `RenameSource`, `RenameEpisode` |
//! | private `getEpisodesToDownload` | `plan_enqueue` in `anime::enqueue` |
//!
//! Dropped, with the reason:
//!
//! | Kotlin | Why |
//! |---|---|
//! | `downloaderStart`, `downloaderStop(reason)` | `Start` and `Pause` already turn the runner on and off. The stop reason only fed the Android notification text, and errors now travel as [`DownloadEvent::Error`]. |
//! | `buildVideo` | Local playback is the player's (area 45). It finds the file with `AnimeLayout::find_episode_dir`. |
//! | `isEpisodeDownloaded`, `getDownloadCount`, `getDownloadSize` (both forms) | These read the downloads folder, not the queue. They belong to the download index (area 25), which the actor must not block on. |

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

use crate::anime::cache::AnimeIndex;
use crate::anime::delete_rules::{DeleteRules, episodes_to_delete};
use crate::anime::enqueue::{QueueWarning, needs_queue_warning, plan_enqueue, queue_warning};
use crate::anime::layout::AnimeLayout;
use crate::anime::layout::check_free_space;
use crate::anime::lifecycle::{RunnerState, runner_state};
use crate::anime::model::{AnimeDownload, AnimeRef, AnimeSource, EpisodeRef, Video};
use crate::anime::downloader::sweep_staging;
use crate::anime::ffmpeg::{FfmpegNotice, startup_notice};
use crate::anime::naming::{anime_dir_name, episode_dir_name, source_dir_name};
use crate::anime::network::{GateAction, NetworkGate, NetworkState};
use crate::anime::notify::WARNING_TIMEOUT_SECS;
use crate::anime::queue_view::{SortKey, sorted_order};
use crate::anime::pending::{AnimePendingStore, PendingGroup};
use crate::anime::store::AnimeQueueStore;
use crate::error::DownloadError;
use crate::queue_model::normalize_order;
use crate::events::{DownloadEvent, EVENT_CAPACITY};
use crate::model::DownloadState;
use crate::pipeline::FreeSpaceFn;
use crate::scheduler::pick_active;

/// Anime sources that download at once (behaviour 2.3, `take(3)`). The Kotlin comment says 5; the code says 3.
pub const ANIME_MAX_ACTIVE_SOURCES: usize = 3;

/// Anime settings the queue reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnimeSettings {
    /// `pref_download_fillermarked`, default off.
    pub download_fillermarked: bool,
    /// `pref_remove_bookmarked`, default off: bookmarked episodes are kept by deletes unless this is on.
    pub remove_bookmarked: bool,
    /// `remove_exclude_anime_categories`, default empty: watched episodes of these categories are kept.
    pub remove_exclude_categories: Vec<i64>,
}

/// Runs one episode to the end. Returns `Ok` when the episode is finished, `Err(Cancelled)` when cancelled, and any
/// other error when it failed (the item then goes to ERROR).
#[async_trait]
pub trait EpisodeRunner: Send + Sync {
    async fn run(&self, item: Arc<AnimeDownload>, cancel: CancellationToken) -> Result<(), DownloadError>;
}

/// An episode to queue, with everything the engine needs. The caller builds it from the database.
#[derive(Clone, Debug, PartialEq)]
pub struct QueueEntry {
    pub source: AnimeSource,
    pub anime: AnimeRef,
    pub episode: EpisodeRef,
    pub alt: bool,
    pub video: Option<Video>,
}

impl QueueEntry {
    fn into_download(self) -> AnimeDownload {
        AnimeDownload::new(self.source, self.anime, self.episode, self.alt, self.video)
    }
}

/// What an enqueue did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnqueueOutcome {
    /// Episode ids appended to the queue, in queue order.
    pub added: Vec<i64>,
    /// The runner was started by this call.
    pub started: bool,
    /// The queue-size warning to show, when the run starts over a large queue.
    pub warning: Option<QueueWarning>,
    /// Items were added to a queue that was not empty while the runner is stopped: show "Queued. Press Continue to
    /// start." (T-24a-069, E-Q07).
    pub continue_toast: bool,
}

pub enum Command {
    /// `queueEpisodes`: filter, sort and append. `auto_start` starts the run only when the queue was empty.
    Enqueue {
        source: AnimeSource,
        anime: AnimeRef,
        episodes: Vec<EpisodeRef>,
        auto_start: bool,
        alt: bool,
        video: Option<Video>,
        reply: oneshot::Sender<EnqueueOutcome>,
    },
    /// `addDownloadsToStartOfQueue`: put the entries first and start the run.
    AddToStart {
        entries: Vec<QueueEntry>,
    },
    /// `startDownloadNow`: move the episode to the front (or insert it) and start the run. An episode already in the
    /// downloads folder is refused with `AlreadyDownloaded` and nothing is queued (T-24a-068).
    StartNow {
        entry: QueueEntry,
        reply: oneshot::Sender<Result<(), DownloadError>>,
    },
    /// The network was observed at `at`. A run stopped by the network starts again when it returns (T-24a-050, E-N01).
    Network {
        at: Instant,
        state: NetworkState,
        wifi_only: bool,
    },
    /// `reorderQueue` / `updateQueue`: the listed episode ids in the new order. Unknown ids are ignored, unlisted items follow in their current order, and an empty
    /// list does nothing (E-15).
    Reorder {
        episode_ids: Vec<i64>,
    },
    /// `removeFromDownloadQueue` for these episodes.
    Cancel {
        episode_ids: Vec<i64>,
    },
    /// Removes every queued item of one anime.
    CancelAnime {
        anime_id: i64,
    },
    Start,
    Pause,
    /// Stops the run because of a failure, such as a network rule. The DOWNLOADING items go to ERROR and the warning is
    /// emitted (behaviour 2.5, step 3).
    StopWithFailure {
        text: String,
    },
    ClearQueue,
    /// Removes the episodes' queue items and their folders, after the delete rules (behaviour 6.2).
    DeleteEpisodes {
        source: AnimeSource,
        anime: AnimeRef,
        episodes: Vec<EpisodeRef>,
        categories: Vec<i64>,
        /// Replies with how many episodes the delete rules selected.
        reply: oneshot::Sender<Result<usize, DownloadError>>,
    },
    /// Removes the anime folder, and its queue items when `remove_queued`.
    DeleteAnime {
        source: AnimeSource,
        anime: AnimeRef,
        remove_queued: bool,
        reply: oneshot::Sender<Result<(), DownloadError>>,
    },
    /// `enqueueEpisodesToDelete`: stores the episodes that the delete rules allow, for the next flush. `categories` are the
    /// anime's categories. Replies with how many were new.
    EnqueueDelete {
        anime: AnimeRef,
        episodes: Vec<EpisodeRef>,
        categories: Vec<i64>,
        reply: oneshot::Sender<Result<usize, DownloadError>>,
    },
    /// `deletePendingEpisodes`: deletes the pending episodes of every anime whose source is in `installed`. Rows of other
    /// sources stay for the next flush (E-P01). The delete rules are checked again here (E-P07), with the categories
    /// of each anime, and the live bookmark flag of each episode (`bookmarks`, by episode id; an episode not listed
    /// keeps the flag stored at enqueue). Replies with the number of episodes handled.
    FlushPendingDeletes {
        installed: Vec<AnimeSource>,
        categories: HashMap<i64, Vec<i64>>,
        bookmarks: HashMap<i64, bool>,
        reply: oneshot::Sender<Result<usize, DownloadError>>,
    },
    RenameEpisode {
        source: AnimeSource,
        anime: AnimeRef,
        old: EpisodeRef,
        new: EpisodeRef,
        reply: oneshot::Sender<Result<Option<PathBuf>, DownloadError>>,
    },
    RenameSource {
        old: AnimeSource,
        new: AnimeSource,
        reply: oneshot::Sender<Result<bool, DownloadError>>,
    },
}

/// Everything the actor is started with.
pub struct AnimeQueueParams {
    pub layout: AnimeLayout,
    pub store: Arc<AnimeQueueStore>,
    pub pending: Arc<AnimePendingStore>,
    /// Free space of the filesystem holding a path, `None` when unknown. The GUI passes the platform implementation.
    pub free_space: FreeSpaceFn,
    pub runner: Arc<dyn EpisodeRunner>,
    pub settings: AnimeSettings,
    /// Rows kept by `AnimeQueueStore::restore`, in queue order. They come back as QUEUE and nothing starts until `Start`.
    pub restored: Vec<QueueEntry>,
    /// The downloaded-episode index, kept in step with deletes and finished downloads.
    pub index: AnimeIndex,
}

/// The caller's handle to the actor. Cheap to clone.
#[derive(Clone)]
pub struct AnimeQueue {
    tx: mpsc::UnboundedSender<Command>,
    queue: watch::Receiver<Arc<Vec<Arc<AnimeDownload>>>>,
    state: watch::Receiver<RunnerState>,
    events: broadcast::Sender<DownloadEvent>,
    ffmpeg_notice: Option<FfmpegNotice>,
}

impl AnimeQueue {
    /// The startup ffmpeg check (T-24a-067): `Some` when `ffmpeg` or `ffprobe` was not on `PATH` at spawn. Settings shows
    /// its message with the link.
    pub fn ffmpeg_notice(&self) -> Option<&FfmpegNotice> {
        self.ffmpeg_notice.as_ref()
    }

    pub fn send(&self, command: Command) -> Result<(), DownloadError> {
        self.tx.send(command).map_err(|_| DownloadError::Internal("anime queue manager stopped".into()))
    }

    /// The queue as of the last loop turn. Statuses and progress are read live from each item.
    pub fn snapshot(&self) -> Arc<Vec<Arc<AnimeDownload>>> {
        self.queue.borrow().clone()
    }

    /// `getQueuedDownloadOrNull`: the queued item of this episode, if any.
    pub fn queued(&self, episode_id: i64) -> Option<Arc<AnimeDownload>> {
        self.queue.borrow().iter().find(|d| d.episode.id == episode_id).cloned()
    }

    /// A receiver for failures: one [`DownloadEvent::Error`] per episode that went to ERROR. Feed it to
    /// `notify::spawn_notifier`.
    pub fn events(&self) -> broadcast::Receiver<DownloadEvent> {
        self.events.subscribe()
    }

    /// `true` while the runner is on.
    pub fn running(&self) -> bool {
        *self.state.borrow() == RunnerState::Running
    }

    /// Running, paused (off with items queued) or stopped, as the queue last published it.
    pub fn runner_state(&self) -> RunnerState {
        *self.state.borrow()
    }

    /// A receiver that changes with the runner state, for the UI.
    pub fn subscribe_state(&self) -> watch::Receiver<RunnerState> {
        self.state.clone()
    }

    pub async fn enqueue(
        &self,
        source: AnimeSource,
        anime: AnimeRef,
        episodes: Vec<EpisodeRef>,
        auto_start: bool,
        alt: bool,
        video: Option<Video>,
    ) -> Result<EnqueueOutcome, DownloadError> {
        let (reply, answer) = oneshot::channel();
        self.send(Command::Enqueue { source, anime, episodes, auto_start, alt, video, reply })?;
        answer.await.map_err(|_| DownloadError::Internal("anime queue manager stopped".into()))
    }

    /// `startDownloadNow` (T-24a-068): queues the episode at the front and starts the run, unless it is already on disk.
    pub async fn start_now(&self, entry: QueueEntry) -> Result<(), DownloadError> {
        let (reply, answer) = oneshot::channel();
        self.send(Command::StartNow { entry, reply })?;
        answer.await.map_err(|_| DownloadError::Internal("anime queue manager stopped".into()))?
    }

    /// Feeds one network observation to the runner (T-24a-050). The caller passes the time of the observation.
    pub fn network_changed(&self, at: Instant, state: NetworkState, wifi_only: bool) -> Result<(), DownloadError> {
        self.send(Command::Network { at, state, wifi_only })
    }

    /// Reorders the queue by the sort key inside each source group and sends `Reorder` (T-24a-045). An empty queue
    /// sends nothing.
    pub fn reorder_sorted(&self, key: SortKey, reverse: bool) -> Result<(), DownloadError> {
        let episode_ids = sorted_order(&self.snapshot(), key, reverse);
        if episode_ids.is_empty() {
            return Ok(());
        }
        self.send(Command::Reorder { episode_ids })
    }

    /// A receiver for the queue as the actor last published it (T-24a-046).
    pub fn subscribe_queue(&self) -> watch::Receiver<Arc<Vec<Arc<AnimeDownload>>>> {
        self.queue.clone()
    }

    pub async fn rename_source(&self, old: AnimeSource, new: AnimeSource) -> Result<bool, DownloadError> {
        let (reply, answer) = oneshot::channel();
        self.send(Command::RenameSource { old, new, reply })?;
        answer.await.map_err(|_| DownloadError::Internal("anime queue manager stopped".into()))?
    }

    pub async fn rename_episode(
        &self,
        source: AnimeSource,
        anime: AnimeRef,
        old: EpisodeRef,
        new: EpisodeRef,
    ) -> Result<Option<PathBuf>, DownloadError> {
        let (reply, answer) = oneshot::channel();
        self.send(Command::RenameEpisode { source, anime, old, new, reply })?;
        answer.await.map_err(|_| DownloadError::Internal("anime queue manager stopped".into()))?
    }
}

/// Starts the actor on the current tokio runtime and returns its handle.
pub fn spawn(params: AnimeQueueParams) -> AnimeQueue {
    let (tx, commands) = mpsc::unbounded_channel();
    let (done_tx, done_rx) = mpsc::unbounded_channel();
    // Startup sweep (T-24a-062): staging folders of rows that were not restored are removed.
    if let Some(root) = params.layout.root() {
        let keep: Vec<PathBuf> =
            params.restored.iter().flat_map(|e| params.layout.staging_dirs(&e.source, &e.anime, &e.episode)).collect();
        let removed = sweep_staging(root, &keep);
        if removed > 0 {
            debug!(target: "kuro_downloads", removed, "startup sweep removed orphaned staging entries");
        }
    }
    let restored: Vec<Arc<AnimeDownload>> = params.restored.into_iter().map(|e| Arc::new(e.into_download())).collect();
    let (queue_tx, queue_rx) = watch::channel(Arc::new(restored.clone()));
    let (state_tx, state_rx) = watch::channel(runner_state(false, restored.len()));
    let (events, _) = broadcast::channel(EVENT_CAPACITY);
    let actor = Actor {
        queue: restored,
        running: HashMap::new(),
        runner_on: false,
        layout: params.layout,
        store: params.store,
        pending: params.pending,
        free_space: params.free_space,
        events: events.clone(),
        runner: params.runner,
        settings: params.settings,
        next_generation: 0,
        series: Arc::new(crate::manager::SeriesLocks::default()),
        done_tx,
        queue_tx,
        state_tx,
        index: params.index,
        network: NetworkGate::default(),
    };
    tokio::spawn(actor.run(commands, done_rx));
    let ffmpeg_notice = startup_notice(&std::env::var_os("PATH").unwrap_or_default());
    if ffmpeg_notice.is_some() {
        warn!(target: "kuro_downloads", "ffmpeg or ffprobe is not on PATH: anime downloads will fail");
    }
    AnimeQueue { tx, queue: queue_rx, state: state_rx, events, ffmpeg_notice }
}

/// A runner's result, tagged with the generation it was started under.
struct Done {
    episode_id: i64,
    generation: u64,
    result: Result<(), DownloadError>,
}

struct Running {
    generation: u64,
    cancel: CancellationToken,
}

struct Actor {
    queue: Vec<Arc<AnimeDownload>>,
    running: HashMap<i64, Running>,
    /// `AnimeDownloadJob` is running.
    runner_on: bool,
    layout: AnimeLayout,
    store: Arc<AnimeQueueStore>,
    pending: Arc<AnimePendingStore>,
    free_space: FreeSpaceFn,
    events: broadcast::Sender<DownloadEvent>,
    runner: Arc<dyn EpisodeRunner>,
    settings: AnimeSettings,
    next_generation: u64,
    /// Per-series locks for the delete steps that run on the blocking pool (E-22).
    series: Arc<crate::manager::SeriesLocks>,
    done_tx: mpsc::UnboundedSender<Done>,
    queue_tx: watch::Sender<Arc<Vec<Arc<AnimeDownload>>>>,
    state_tx: watch::Sender<RunnerState>,
    index: AnimeIndex,
    network: NetworkGate,
}

impl Actor {
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>, mut done_rx: mpsc::UnboundedReceiver<Done>) {
        loop {
            tokio::select! {
                command = commands.recv() => match command {
                    Some(command) => self.handle(command),
                    None => break,
                },
                Some(done) = done_rx.recv() => self.finished(done),
            }
            self.reconcile();
            self.publish();
        }
        for (_, running) in self.running.drain() {
            running.cancel.cancel();
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Enqueue { source, anime, episodes, auto_start, alt, video, reply } => {
                let outcome = self.enqueue(source, anime, episodes, auto_start, alt, video);
                let _ = reply.send(outcome);
            }
            Command::AddToStart { entries } => {
                self.add_to_start(entries);
                if !self.runner_on {
                    self.start();
                }
            }
            Command::StartNow { entry, reply } => {
                let result = self.start_now(entry);
                if result.is_ok() {
                    self.start();
                }
                let _ = reply.send(result);
            }
            Command::Network { at, state, wifi_only } => {
                match self.network.observe(at, state, wifi_only, self.runner_on) {
                    GateAction::Stop(text) => self.stop_with_failure(text.to_owned()),
                    GateAction::Resume => {
                        self.start();
                    }
                    GateAction::Nothing => {}
                }
            }
            Command::Reorder { episode_ids } => self.reorder(&episode_ids),
            Command::Cancel { episode_ids } => self.remove_episodes(&episode_ids),
            Command::CancelAnime { anime_id } => {
                let ids: Vec<i64> =
                    self.queue.iter().filter(|d| d.anime.id == anime_id).map(|d| d.episode.id).collect();
                self.remove_episodes(&ids);
            }
            Command::Start => {
                self.start();
            }
            Command::Pause => {
                self.network.cancel_auto_resume();
                self.pause();
            }
            Command::StopWithFailure { text } => self.stop_with_failure(text),
            Command::ClearQueue => {
                self.network.cancel_auto_resume();
                self.clear();
            }
            Command::DeleteEpisodes { source, anime, episodes, categories, reply } => {
                let rules = self.delete_rules(categories);
                let episodes = episodes_to_delete(episodes, &rules);
                let ids: Vec<i64> = episodes.iter().map(|e| e.id).collect();
                self.remove_episodes(&ids);
                let selected = episodes.len();
                // The folders go on the blocking pool, so the actor keeps serving commands. The task owns the reply, so a
                // caller that gives up does not stop the delete half way (T-25a-016).
                let (layout, index) = (self.layout.clone(), self.index.clone());
                let (series, anime_id) = (self.series.clone(), anime.id);
                tokio::spawn(async move {
                    let _series = series.lock(crate::queue_model::EntryKind::Anime, anime_id).await;
                    let result =
                        tokio::task::spawn_blocking(move || delete_episodes_on_disk(&layout, &index, &source, &anime, &episodes))
                            .await
                            .unwrap_or_else(|e| Err(DownloadError::Internal(e.to_string())));
                    let _ = reply.send(result.map(|()| selected));
                });
            }
            Command::EnqueueDelete { anime, episodes, categories, reply } => {
                let rules = self.delete_rules(categories);
                let episodes = episodes_to_delete(episodes, &rules);
                let result = self.pending.add_episodes(&anime, &episodes);
                let _ = reply.send(result.map_err(|e| DownloadError::Internal(e.to_string())));
            }
            Command::FlushPendingDeletes { installed, categories, bookmarks, reply } => {
                let _ = reply.send(self.flush_pending(&installed, &categories, &bookmarks));
            }
            Command::DeleteAnime { source, anime, remove_queued, reply } => {
                // E-D02: pause before the queue items go, then resume if the run was on.
                let was_on = self.runner_on;
                if remove_queued {
                    self.pause();
                    let ids: Vec<i64> =
                        self.queue.iter().filter(|d| d.anime.id == anime.id).map(|d| d.episode.id).collect();
                    self.remove_episodes(&ids);
                }
                let result = self.delete_anime_folder(&source, &anime);
                if remove_queued && was_on {
                    self.start();
                }
                let _ = reply.send(result);
            }
            Command::RenameEpisode { source, anime, old, new, reply } => {
                let result = self.layout.rename_episode(&source, &anime, &old, &new);
                if let Err(err) = &result {
                    self.warn_rename(err);
                }
                if let Ok(Some(_)) = result {
                    let folder = episode_dir_name(&new.name, new.scanlator.as_deref());
                    self.index.remove_episodes(source.id, &anime, std::slice::from_ref(&old));
                    self.index.add_episode(source.id, &source_dir_name(&source.name, &source.lang), &anime_dir_name(&anime.title), &folder);
                }
                let _ = reply.send(result);
            }
            Command::RenameSource { old, new, reply } => {
                let result = self.layout.rename_source(&old, &new);
                if let Err(err) = &result {
                    self.warn_rename(err);
                }
                if let Ok(true) = result {
                    // The index keys sources by folder name, so it is rebuilt on the next renewal (proposal, 6.4).
                    self.index.invalidate();
                }
                let _ = reply.send(result);
            }
        }
    }

    /// The delete rules for one anime: the settings plus the anime's categories.
    fn delete_rules(&self, categories: Vec<i64>) -> DeleteRules {
        DeleteRules {
            exclude_categories: self.settings.remove_exclude_categories.clone(),
            item_categories: categories,
            remove_bookmarked: self.settings.remove_bookmarked,
        }
    }

    /// [`delete_episodes_on_disk`] on this actor's layout and index.
    fn delete_episodes_now(
        &self,
        source: &AnimeSource,
        anime: &AnimeRef,
        episodes: &[EpisodeRef],
    ) -> Result<(), DownloadError> {
        delete_episodes_on_disk(&self.layout, &self.index, source, anime, episodes)
    }

    /// `deleteAnime` on disk, on this actor's layout and index.
    fn delete_anime_folder(&self, source: &AnimeSource, anime: &AnimeRef) -> Result<(), DownloadError> {
        delete_anime_folder_on_disk(&self.layout, &self.index, source, anime)
    }

    /// Takes the pending rows of the installed sources and deletes their episodes like `DeleteEpisodes`, with the rules
    /// checked again (E-P07). The first failing group stops the flush; its rows are already gone from the table (Kotlin
    /// clears before it deletes).
    fn flush_pending(
        &mut self,
        installed: &[AnimeSource],
        categories: &HashMap<i64, Vec<i64>>,
        bookmarks: &HashMap<i64, bool>,
    ) -> Result<usize, DownloadError> {
        let groups = self
            .pending
            .take_for_installed(|id| installed.iter().any(|s| s.id == id))
            .map_err(|e| DownloadError::Internal(e.to_string()))?;
        let mut handled = 0;
        let mut first_error = None;
        for group in groups {
            let Some(source) = installed.iter().find(|s| s.id == group.anime.source_id).cloned() else {
                self.restore_pending(&group);
                continue;
            };
            let rules = self.delete_rules(categories.get(&group.anime.id).cloned().unwrap_or_default());
            let mut episodes = group.episodes.clone();
            for e in &mut episodes {
                e.bookmark = bookmarks.get(&e.id).copied().unwrap_or(e.bookmark);
            }
            let episodes = episodes_to_delete(episodes, &rules);
            let ids: Vec<i64> = episodes.iter().map(|e| e.id).collect();
            self.remove_episodes(&ids);
            match self.delete_episodes_now(&source, &group.anime, &episodes) {
                Ok(()) => handled += episodes.len(),
                Err(err) => {
                    // The rows were taken from the table before the delete; a failed group goes back so the next flush
                    // retries it instead of losing it (T-24a-053).
                    self.restore_pending(&group);
                    first_error.get_or_insert(err);
                }
            }
        }
        match first_error {
            Some(err) => Err(err),
            None => Ok(handled),
        }
    }

    /// Puts a taken group back into the pending table.
    fn restore_pending(&self, group: &PendingGroup) {
        if let Err(err) = self.pending.add_episodes(&group.anime, &group.episodes) {
            error!(target: "kuro_downloads", error = %err, "could not restore a pending delete group");
        }
    }

    fn enqueue(
        &mut self,
        source: AnimeSource,
        anime: AnimeRef,
        episodes: Vec<EpisodeRef>,
        auto_start: bool,
        alt: bool,
        video: Option<Video>,
    ) -> EnqueueOutcome {
        if episodes.is_empty() {
            return EnqueueOutcome::default();
        }
        if !source.is_http {
            debug!(target: "kuro_downloads", source = source.id, "anime enqueue ignored: source is not HTTP");
            return EnqueueOutcome::default();
        }
        let was_empty = self.queue.is_empty();
        let queued: HashSet<i64> = self.queue.iter().map(|d| d.episode.id).collect();
        let layout = &self.layout;
        let picks = plan_enqueue(
            &episodes,
            &queued,
            |e| layout.find_episode_dir(&source, &anime, e).is_some(),
            self.settings.download_fillermarked,
        );
        let added: Vec<i64> = picks.iter().map(|e| e.id).collect();
        for episode in picks {
            let item = AnimeDownload::new(source.clone(), anime.clone(), episode, alt, video.clone());
            self.queue.push(Arc::new(item));
        }
        self.persist();
        let continue_toast = !was_empty && !self.runner_on && !added.is_empty();
        let mut outcome = EnqueueOutcome { added, started: false, warning: None, continue_toast };
        if auto_start && was_empty {
            if needs_queue_warning(self.queue.iter().map(|d| (d.source.id, d.source.unmetered))) {
                let warning = queue_warning();
                let _ = self.events.send(DownloadEvent::Warning {
                    text: warning.text.to_owned(),
                    timeout_secs: Some(warning.timeout_secs),
                    help_url: Some(warning.help_url.to_owned()),
                });
                outcome.warning = Some(warning);
            }
            outcome.started = self.start();
        }
        outcome
    }

    /// `start()`: re-queues every item that is not QUEUE, then turns the runner on. Does nothing on an empty queue.
    fn start(&mut self) -> bool {
        if self.runner_on || self.queue.is_empty() {
            return false;
        }
        for item in &self.queue {
            if item.status() != DownloadState::Queue {
                item.set_status(DownloadState::Queue);
            }
        }
        self.runner_on = true;
        true
    }

    /// `pause()`: cancels every running episode, which goes back to QUEUE.
    fn pause(&mut self) {
        self.runner_on = false;
        for (_, running) in self.running.drain() {
            running.cancel.cancel();
        }
        for item in &self.queue {
            if item.status() == DownloadState::Downloading {
                item.set_status(DownloadState::Queue);
            }
        }
    }

    /// A failed rename is a warning for the user, not only a log line (T-24a-071, E-F23).
    fn warn_rename(&self, err: &DownloadError) {
        let _ = self.events.send(DownloadEvent::Warning {
            text: format!("Could not rename the download folder: {err}"),
            timeout_secs: Some(WARNING_TIMEOUT_SECS),
            help_url: None,
        });
    }

    /// `stop(reason)` with a reason: every running episode is cancelled and goes to ERROR, the run is off, and the reason
    /// is sent as a warning. Unlike `pause`, the items are not requeued here; the next `start` requeues them.
    fn stop_with_failure(&mut self, text: String) {
        self.runner_on = false;
        for (_, running) in self.running.drain() {
            running.cancel.cancel();
        }
        for item in &self.queue {
            if item.status() == DownloadState::Downloading {
                item.set_status(DownloadState::Error);
            }
        }
        let _ = self.events.send(DownloadEvent::Warning {
            text,
            timeout_secs: Some(WARNING_TIMEOUT_SECS),
            help_url: None,
        });
    }

    /// `clearQueue()`: pauses, sets every DOWNLOADING and QUEUE item to NOT_DOWNLOADED, and empties the queue.
    fn clear(&mut self) {
        self.pause();
        for item in self.queue.drain(..) {
            if matches!(item.status(), DownloadState::Downloading | DownloadState::Queue) {
                item.set_status(DownloadState::NotDownloaded);
            }
        }
        self.persist();
    }

    fn add_to_start(&mut self, entries: Vec<QueueEntry>) {
        if entries.is_empty() {
            return;
        }
        let ids: HashSet<i64> = entries.iter().map(|e| e.episode.id).collect();
        self.cancel_running_for(&ids);
        self.queue.retain(|d| !ids.contains(&d.episode.id));
        let mut front: Vec<Arc<AnimeDownload>> = entries.into_iter().map(|e| Arc::new(e.into_download())).collect();
        front.append(&mut self.queue);
        self.queue = front;
        self.persist();
    }

    /// Moves the episode to the front. An episode whose folder already exists is refused, as in Kotlin's
    /// `startDownloadNow` (E-Q06).
    fn start_now(&mut self, entry: QueueEntry) -> Result<(), DownloadError> {
        if self.layout.find_episode_dir(&entry.source, &entry.anime, &entry.episode).is_some() {
            return Err(DownloadError::AlreadyDownloaded);
        }
        let id = entry.episode.id;
        let item = match self.queue.iter().position(|d| d.episode.id == id) {
            Some(pos) => self.queue.remove(pos),
            None => Arc::new(entry.into_download()),
        };
        self.queue.insert(0, item);
        self.persist();
        Ok(())
    }

    /// `updateQueue`: pause, rebuild the queue in the given order with every item in QUEUE, then resume if it ran. Unknown
    /// ids are ignored, live items that are not listed follow in their current order, and an empty list does nothing
    /// (E-15).
    fn reorder(&mut self, episode_ids: &[i64]) {
        let current: Vec<i64> = self.queue.iter().map(|d| d.episode.id).collect();
        // An empty request, or one that leaves the order as it is, changes nothing: no pause, no restart (T-24a-052).
        if normalize_order(&current, episode_ids).is_none() {
            return;
        }
        let was_running = self.runner_on;
        self.pause();
        let current: Vec<Arc<AnimeDownload>> = std::mem::take(&mut self.queue);
        let mut by_id: HashMap<i64, Arc<AnimeDownload>> = current.iter().map(|d| (d.episode.id, d.clone())).collect();
        for id in episode_ids {
            if let Some(item) = by_id.remove(id) {
                item.set_status(DownloadState::Queue);
                self.queue.push(item);
            }
        }
        for item in current {
            if by_id.contains_key(&item.episode.id) {
                item.set_status(DownloadState::Queue);
                self.queue.push(item);
            }
        }
        self.persist();
        if was_running {
            self.start();
        }
    }

    /// `removeFromQueueIf`: removed items become NOT_DOWNLOADED, and their runners are cancelled.
    fn remove_episodes(&mut self, episode_ids: &[i64]) {
        if episode_ids.is_empty() {
            return;
        }
        let ids: HashSet<i64> = episode_ids.iter().copied().collect();
        self.cancel_running_for(&ids);
        self.queue.retain(|d| {
            if ids.contains(&d.episode.id) {
                // Only QUEUE and DOWNLOADING items become NOT_DOWNLOADED; an ERROR item keeps its status.
                d.set_status(DownloadState::NotDownloaded);
                false
            } else {
                true
            }
        });
        self.persist();
        if self.queue.is_empty() {
            self.runner_on = false;
        }
    }

    fn cancel_running_for(&mut self, ids: &HashSet<i64>) {
        for id in ids {
            if let Some(running) = self.running.remove(id) {
                running.cancel.cancel();
            }
        }
    }

    /// The scheduler (T-24a-011). Starts runners for the first active item of each source, at most three sources, and
    /// stops runners whose item is no longer in that set.
    fn reconcile(&mut self) {
        if !self.runner_on {
            return;
        }
        let wanted =
            pick_active(self.queue.iter().map(|d| (d.source.id, d.episode.id, d.status())), ANIME_MAX_ACTIVE_SOURCES);
        let stale: Vec<i64> = self.running.keys().copied().filter(|id| !wanted.contains(id)).collect();
        for id in stale {
            if let Some(running) = self.running.remove(&id) {
                running.cancel.cancel();
            }
            if let Some(item) = self.queue.iter().find(|d| d.episode.id == id) {
                if item.status() == DownloadState::Downloading {
                    item.set_status(DownloadState::Queue);
                }
            }
        }
        for id in wanted {
            if !self.running.contains_key(&id) {
                self.spawn_runner(id);
            }
        }
        if self.running.is_empty() && !self.queue.iter().any(|d| d.status().is_active()) {
            self.runner_on = false;
        }
    }

    fn spawn_runner(&mut self, episode_id: i64) {
        let Some(item) = self.queue.iter().find(|d| d.episode.id == episode_id).cloned() else {
            return;
        };
        item.set_status(DownloadState::Downloading);
        self.next_generation += 1;
        let generation = self.next_generation;
        let cancel = CancellationToken::new();
        self.running.insert(episode_id, Running { generation, cancel: cancel.clone() });
        // Free-space precheck on the anime folder (T-24a-017). An unknown size skips it.
        let precheck = self
            .layout
            .anime_dir(&item.source, &item.anime.title)
            .and_then(|dir| check_free_space((self.free_space)(&dir)));
        if let Err(err) = precheck {
            let _ = self.done_tx.send(Done { episode_id, generation, result: Err(err) });
            return;
        }
        let runner = self.runner.clone();
        let done = self.done_tx.clone();
        tokio::spawn(async move {
            let result = runner.run(item, cancel).await;
            let _ = done.send(Done { episode_id, generation, result });
        });
    }

    fn finished(&mut self, done: Done) {
        match self.running.get(&done.episode_id) {
            Some(running) if running.generation == done.generation => {}
            _ => return,
        }
        self.running.remove(&done.episode_id);
        let Some(pos) = self.queue.iter().position(|d| d.episode.id == done.episode_id) else {
            return;
        };
        let item = self.queue[pos].clone();
        match done.result {
            Ok(()) => {
                item.set_status(DownloadState::Downloaded);
                // The folder name is the one the engine finalised, which may carry the episode id (T-24a-066).
                let folder = self
                    .layout
                    .find_episode_dir(&item.source, &item.anime, &item.episode)
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| episode_dir_name(&item.episode.name, item.episode.scanlator.as_deref()));
                self.index.add_episode(
                    item.source.id,
                    &source_dir_name(&item.source.name, &item.source.lang),
                    &anime_dir_name(&item.anime.title),
                    &folder,
                );
                self.queue.remove(pos);
                self.persist();
            }
            Err(DownloadError::Cancelled) => {
                if item.status() == DownloadState::Downloading {
                    item.set_status(DownloadState::Queue);
                }
            }
            Err(err) => {
                warn!(target: "kuro_downloads", episode = done.episode_id, error = %err, "anime episode failed");
                item.set_status(DownloadState::Error);
                let _ = self.events.send(DownloadEvent::Error {
                    chapter_id: Some(item.episode.id),
                    manga_id: Some(item.anime.id),
                    manga_title: Some(item.anime.title.clone()),
                    chapter_name: Some(item.episode.name.clone()),
                    message: err.to_string(),
                });
            }
        }
    }

    fn persist(&self) {
        let rows: Vec<(i64, i64)> = self.queue.iter().map(|d| (d.episode.id, d.anime.id)).collect();
        if let Err(err) = self.store.replace_all(&rows) {
            error!(target: "kuro_downloads", error = %err, "anime queue store write failed");
        }
    }

    fn publish(&self) {
        self.queue_tx.send_replace(Arc::new(self.queue.clone()));
        self.state_tx.send_replace(runner_state(self.runner_on, self.queue.len()));
    }
}

/// `true` for a folder with no entries. A folder that cannot be read is not empty.
/// Deletes the episodes' folders, partial downloads included (E-D01), and updates the index. Then the anime folder goes
/// when it is empty, and so does the source folder when it is empty (behaviour 6.2 and 6.3). Blocking: call it from
/// `spawn_blocking`.
fn delete_episodes_on_disk(
    layout: &AnimeLayout,
    index: &AnimeIndex,
    source: &AnimeSource,
    anime: &AnimeRef,
    episodes: &[EpisodeRef],
) -> Result<(), DownloadError> {
    if episodes.is_empty() {
        return Ok(());
    }
    let (anime_dir, finished) = layout.find_episode_dirs(source, anime, episodes);
    let partial = layout.find_episode_tmp_dirs(source, anime, episodes);
    for path in finished.iter().chain(partial.iter()) {
        remove_path(path).map_err(DownloadError::from)?;
    }
    index.remove_episodes(source.id, anime, episodes);
    if let Some(dir) = anime_dir {
        if is_empty_dir(&dir) {
            delete_anime_folder_on_disk(layout, index, source, anime)?;
        }
    }
    Ok(())
}

/// The anime folder, its index entry, and the source folder when that is now empty. Blocking.
fn delete_anime_folder_on_disk(
    layout: &AnimeLayout,
    index: &AnimeIndex,
    source: &AnimeSource,
    anime: &AnimeRef,
) -> Result<(), DownloadError> {
    if let Some(path) = layout.find_anime_dir(source, &anime.title) {
        remove_path(&path).map_err(DownloadError::from)?;
    }
    index.remove_anime(source.id, &anime.title);
    if let Some(source_dir) = layout.find_source_dir(source) {
        if is_empty_dir(&source_dir) {
            remove_path(&source_dir).map_err(DownloadError::from)?;
            index.remove_source(source.id);
        }
    }
    Ok(())
}

fn is_empty_dir(path: &Path) -> bool {
    fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
}

/// Deletes a folder or a file.
fn remove_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::oneshot;
    use crate::anime::queue_view::SortKey;
    use crate::anime::network::NetworkState;

    /// Runs for `delay`, then fails for the ids in `fail` and succeeds otherwise. Records the peak number in flight.
    struct FakeRunner {
        delay: Duration,
        fail: Mutex<HashSet<i64>>,
        in_flight: AtomicUsize,
        peak: AtomicUsize,
    }

    #[async_trait]
    impl EpisodeRunner for FakeRunner {
        async fn run(&self, item: Arc<AnimeDownload>, cancel: CancellationToken) -> Result<(), DownloadError> {
            let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            let fails = self.fail.lock().unwrap().contains(&item.episode.id);
            let result = tokio::select! {
                _ = cancel.cancelled() => Err(DownloadError::Cancelled),
                _ = tokio::time::sleep(self.delay) => if fails { Err(DownloadError::NoVideo) } else { Ok(()) },
            };
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            result
        }
    }

    fn plenty(_: &Path) -> Option<u64> {
        Some(u64::MAX)
    }

    struct Env {
        queue: AnimeQueue,
        store: Arc<AnimeQueueStore>,
        pending: Arc<AnimePendingStore>,
        runner: Arc<FakeRunner>,
        index: AnimeIndex,
        root: tempfile::TempDir,
    }

    fn setup(delay: Duration, fail: &[i64], settings: AnimeSettings, restored: Vec<QueueEntry>) -> Env {
        setup_with(delay, fail, settings, restored, plenty)
    }

    fn setup_with(
        delay: Duration,
        fail: &[i64],
        settings: AnimeSettings,
        restored: Vec<QueueEntry>,
        free_space: FreeSpaceFn,
    ) -> Env {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(AnimeQueueStore::open_in_memory().unwrap());
        let pending = Arc::new(AnimePendingStore::open_in_memory().unwrap());
        let runner = Arc::new(FakeRunner {
            delay,
            fail: Mutex::new(fail.iter().copied().collect()),
            in_flight: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        });
        let index = AnimeIndex::open(root.path(), &root.path().join("cache"));
        let queue = spawn(AnimeQueueParams {
            layout: AnimeLayout::new(Some(root.path().to_path_buf())),
            store: store.clone(),
            pending: pending.clone(),
            free_space,
            runner: runner.clone(),
            settings,
            restored,
            index: index.clone(),
        });
        Env { queue, store, pending, runner, index, root }
    }

    fn src(id: i64) -> AnimeSource {
        AnimeSource { id, name: format!("S{id}"), lang: "en".into(), is_http: true, unmetered: false }
    }

    fn anime(id: i64, source_id: i64) -> AnimeRef {
        AnimeRef { id, source_id, url: format!("/a/{id}"), title: format!("Anime {id}") }
    }

    fn ep(id: i64, anime_id: i64, source_order: i64) -> EpisodeRef {
        EpisodeRef {
            id,
            anime_id,
            url: format!("/e/{id}"),
            name: format!("Ep {id}"),
            scanlator: None,
            seen: false,
            bookmark: false,
            fillermark: false,
            source_order,
            upload_date: 0,
            episode_number: 0.0,
        }
    }

    fn entry(source: &AnimeSource, anime: &AnimeRef, episode: EpisodeRef) -> QueueEntry {
        QueueEntry { source: source.clone(), anime: anime.clone(), episode, alt: false, video: None }
    }

    fn q_all_error_or_queue(q: &AnimeQueue) -> bool {
        q.snapshot().iter().all(|d| matches!(d.status(), DownloadState::Error | DownloadState::Queue))
    }

    fn ids(q: &AnimeQueue) -> Vec<i64> {
        q.snapshot().iter().map(|d| d.episode.id).collect()
    }

    async fn settle(q: &AnimeQueue, cond: impl Fn(&AnimeQueue) -> bool) {
        for _ in 0..500 {
            if cond(q) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("condition never held; queue {:?}", ids(q));
    }

    const LONG: Duration = Duration::from_secs(3600);

    #[test]
    fn four_sources_pick_the_first_three_in_queue_order() {
        use DownloadState::*;
        let items =
            vec![(4, 40, Queue), (1, 10, Error), (1, 11, Queue), (2, 20, Queue), (3, 30, Downloading), (2, 21, Queue)];
        assert_eq!(pick_active(items, ANIME_MAX_ACTIVE_SOURCES), vec![40, 11, 20]);
    }

    #[tokio::test(start_paused = true)]
    async fn five_sources_two_items_each_run_three_sources_at_a_time() {
        let env = setup(Duration::from_millis(100), &[], AnimeSettings::default(), Vec::new());
        for s in 1..=5 {
            let source = src(s);
            let show = anime(s, s);
            let eps = vec![ep(s * 10 + 1, s, 1), ep(s * 10 + 2, s, 2)];
            env.queue.enqueue(source, show, eps, true, false, None).await.unwrap();
        }
        settle(&env.queue, |q| q.snapshot().is_empty()).await;
        assert_eq!(env.runner.peak.load(Ordering::SeqCst), 3, "one per source, three sources");
        assert!(env.store.rows().unwrap().is_empty(), "finished items leave the store");
        assert!(!env.queue.running(), "the run stops when nothing is left");
    }

    #[tokio::test(start_paused = true)]
    async fn an_errored_item_frees_its_slot_for_the_next_item_of_its_source() {
        let env = setup(Duration::from_millis(100), &[11], AnimeSettings::default(), Vec::new());
        let source = src(1);
        let show = anime(1, 1);
        env.queue
            .enqueue(source.clone(), show.clone(), vec![ep(11, 1, 2), ep(12, 1, 1)], true, false, None)
            .await
            .unwrap();
        settle(&env.queue, |q| q.snapshot().len() == 1).await;
        let left = env.queue.snapshot();
        assert_eq!(left[0].episode.id, 11);
        assert_eq!(left[0].status(), DownloadState::Error, "the failed item stays in the queue");
        assert_eq!(env.store.rows().unwrap(), vec![(11, 1, 0)]);
    }

    #[tokio::test(start_paused = true)]
    async fn enqueue_skips_downloaded_queued_and_fillermarked_and_sorts_newest_first() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let source = src(1);
        let show = anime(1, 1);
        let folder = AnimeLayout::new(Some(env.root.path().to_path_buf())).anime_dir(&source, &show.title).unwrap();
        std::fs::create_dir(folder.join("Ep 1")).unwrap();
        let mut filler = ep(5, 1, 9);
        filler.fillermark = true;
        let eps = vec![ep(1, 1, 1), ep(3, 1, 3), ep(4, 1, 4), filler];
        let outcome = env.queue.enqueue(source.clone(), show.clone(), eps.clone(), false, false, None).await.unwrap();
        assert_eq!(outcome.added, vec![4, 3], "episode 1 is on disk, 5 is fillermarked");
        assert!(!outcome.started, "auto start was off");
        let again = env.queue.enqueue(source.clone(), show, eps, false, false, None).await.unwrap();
        assert!(again.added.is_empty(), "already queued by id");
    }

    #[tokio::test(start_paused = true)]
    async fn a_non_http_source_is_ignored() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let local = AnimeSource { is_http: false, ..src(9) };
        let outcome =
            env.queue.enqueue(local.clone(), anime(9, 9), vec![ep(90, 9, 1)], true, false, None).await.unwrap();
        assert_eq!(outcome, EnqueueOutcome::default());
        assert!(env.queue.snapshot().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn the_fillermark_setting_lets_fillermarked_episodes_in() {
        let env = setup(LONG, &[], AnimeSettings { download_fillermarked: true, ..Default::default() }, Vec::new());
        let mut filler = ep(5, 1, 1);
        filler.fillermark = true;
        let outcome = env.queue.enqueue(src(1), anime(1, 1), vec![filler], false, false, None).await.unwrap();
        assert_eq!(outcome.added, vec![5]);
    }

    #[tokio::test(start_paused = true)]
    async fn auto_start_only_starts_when_the_queue_was_empty_and_warns_over_twenty() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let big: Vec<EpisodeRef> = (1..=21).map(|i| ep(i, 1, i)).collect();
        let first = env.queue.enqueue(src(1), anime(1, 1), big, true, false, None).await.unwrap();
        assert!(first.started, "queue was empty");
        let warning = first.warning.expect("21 metered items warn");
        assert_eq!(warning.timeout_secs, 30);
        let second = env.queue.enqueue(src(2), anime(2, 2), vec![ep(100, 2, 1)], true, false, None).await.unwrap();
        assert!(!second.started, "queue was not empty before this call");
        assert!(second.warning.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn pause_returns_running_items_to_queue_and_start_resumes_them() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1), ep(2, 1, 2)], true, false, None).await.unwrap();
        settle(&env.queue, |_| env.runner.in_flight.load(Ordering::SeqCst) == 1).await;
        env.queue.send(Command::Pause).unwrap();
        settle(&env.queue, |q| !q.running() && env.runner.in_flight.load(Ordering::SeqCst) == 0).await;
        assert!(env.queue.snapshot().iter().all(|d| d.status() == DownloadState::Queue));
        env.queue.send(Command::Start).unwrap();
        settle(&env.queue, |_| env.runner.in_flight.load(Ordering::SeqCst) == 1).await;
    }

    #[tokio::test(start_paused = true)]
    async fn clear_empties_the_queue_and_the_store() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1), ep(2, 1, 2)], false, false, None).await.unwrap();
        let before = env.queue.snapshot();
        env.queue.send(Command::ClearQueue).unwrap();
        settle(&env.queue, |q| q.snapshot().is_empty()).await;
        assert!(before.iter().all(|d| d.status() == DownloadState::NotDownloaded));
        assert!(env.store.rows().unwrap().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn reorder_keeps_listed_items_in_the_new_order_and_appends_the_rest() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        env.queue
            .enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1), ep(2, 1, 2), ep(3, 1, 3)], false, false, None)
            .await
            .unwrap();
        env.queue.send(Command::Reorder { episode_ids: vec![3, 1] }).unwrap();
        settle(&env.queue, |q| ids(q) == vec![3, 1, 2]).await;
        assert_eq!(env.store.rows().unwrap(), vec![(3, 1, 0), (1, 1, 1), (2, 1, 2)]);
        assert!(!env.queue.running(), "it was not running before the reorder");
    }

    #[tokio::test(start_paused = true)]
    async fn adding_to_a_stopped_queue_asks_the_user_to_press_continue() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let first = env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], false, false, None).await.unwrap();
        assert!(!first.continue_toast, "an empty queue has nothing to continue");
        let second = env.queue.enqueue(src(1), anime(1, 1), vec![ep(2, 1, 2)], false, false, None).await.unwrap();
        assert!(second.continue_toast, "a stopped run over a non-empty queue");
    }

    #[tokio::test(start_paused = true)]
    async fn the_same_order_is_no_reorder() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1), ep(2, 1, 2)], false, false, None).await.unwrap();
        env.queue.send(Command::Reorder { episode_ids: vec![1, 2] }).unwrap();
        env.queue.send(Command::Reorder { episode_ids: vec![2, 1] }).unwrap();
        settle(&env.queue, |q| ids(q) == vec![2, 1]).await;
        assert_eq!(env.store.rows().unwrap(), vec![(2, 1, 0), (1, 1, 1)]);
    }

    #[tokio::test(start_paused = true)]
    async fn an_empty_or_unknown_reorder_changes_nothing() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1), ep(2, 1, 2)], false, false, None).await.unwrap();
        env.queue.send(Command::Reorder { episode_ids: Vec::new() }).unwrap();
        env.queue.send(Command::Reorder { episode_ids: vec![99, 2] }).unwrap();
        settle(&env.queue, |q| ids(q) == vec![2, 1]).await;
        assert_eq!(env.store.rows().unwrap(), vec![(2, 1, 0), (1, 1, 1)]);
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_removes_the_item_and_cancels_its_runner() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], true, false, None).await.unwrap();
        env.queue.enqueue(src(2), anime(2, 2), vec![ep(2, 2, 1)], false, false, None).await.unwrap();
        settle(&env.queue, |_| env.runner.in_flight.load(Ordering::SeqCst) == 2).await;
        let cancelled = env.queue.snapshot().iter().find(|d| d.episode.id == 2).cloned().unwrap();
        env.queue.send(Command::Cancel { episode_ids: vec![2] }).unwrap();
        settle(&env.queue, |q| ids(q) == vec![1]).await;
        assert_eq!(cancelled.status(), DownloadState::NotDownloaded);
        assert_eq!(env.store.rows().unwrap(), vec![(1, 1, 0)]);
        settle(&env.queue, |_| env.runner.in_flight.load(Ordering::SeqCst) == 1).await;
    }

    #[tokio::test(start_paused = true)]
    async fn start_now_refuses_an_episode_already_on_disk() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let source = src(1);
        let show = anime(1, 1);
        let folder = env.root.path().join("S1 (EN)").join("Anime 1").join("Ep 5");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("Ep 5.mkv"), b"x").unwrap();
        let (reply, answer) = oneshot::channel();
        env.queue.send(Command::StartNow { entry: entry(&source, &show, ep(5, 1, 5)), reply }).unwrap();
        assert!(matches!(answer.await.unwrap(), Err(DownloadError::AlreadyDownloaded)));
        assert!(env.queue.snapshot().is_empty(), "nothing is queued (E-Q06)");
        assert!(!env.queue.running());
    }

    #[tokio::test(start_paused = true)]
    async fn sorted_reorder_sends_the_new_order_inside_the_groups() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let (a, b) = (src(1), src(2));
        let show_a = anime(1, 1);
        let show_b = anime(2, 2);
        let mut e1 = ep(1, 1, 1);
        e1.episode_number = 2.0;
        let mut e2 = ep(2, 1, 2);
        e2.episode_number = 1.0;
        let mut e3 = ep(3, 2, 3);
        e3.episode_number = 9.0;
        env.queue.enqueue(a.clone(), show_a.clone(), vec![e1], false, false, None).await.unwrap();
        env.queue.enqueue(a.clone(), show_a.clone(), vec![e2], false, false, None).await.unwrap();
        env.queue.enqueue(b.clone(), show_b.clone(), vec![e3], false, false, None).await.unwrap();
        settle(&env.queue, |q| q.snapshot().len() == 3).await;
        env.queue.reorder_sorted(SortKey::EpisodeNumber, false).unwrap();
        settle(&env.queue, |q| ids(q) == vec![2, 1, 3]).await;
        env.queue.reorder_sorted(SortKey::EpisodeNumber, true).unwrap();
        settle(&env.queue, |q| ids(q) == vec![1, 2, 3]).await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_network_stop_resumes_when_the_network_returns() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let show = anime(1, 1);
        env.queue
            .enqueue(src(1), show, vec![ep(1, 1, 1), ep(2, 1, 2)], false, false, None)
            .await
            .unwrap();
        env.queue.send(Command::Start).unwrap();
        settle(&env.queue, |q| q.snapshot().iter().any(|d| d.status() == DownloadState::Downloading)).await;

        let base = Instant::now();
        let at = |secs: u64| base + Duration::from_secs(secs);
        let offline = NetworkState::Offline;
        let online = NetworkState::Online(crate::anime::network::Link::Wifi);
        env.queue.network_changed(at(0), offline, true).unwrap();
        env.queue.network_changed(at(3), offline, true).unwrap();
        settle(&env.queue, |q| !q.snapshot().is_empty() && !q.snapshot().iter().any(|d| d.status() == DownloadState::Downloading)).await;
        assert!(!env.queue.running(), "the run stopped with the network warning");
        assert!(q_all_error_or_queue(&env.queue));

        env.queue.network_changed(at(5), online, true).unwrap();
        settle(&env.queue, |q| q.snapshot().iter().any(|d| d.status() == DownloadState::Downloading)).await;
        assert!(env.queue.running(), "the run started again when the network came back (E-N01)");
    }

    #[tokio::test(start_paused = true)]
    async fn a_user_pause_stops_the_network_resume() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], false, false, None).await.unwrap();
        env.queue.send(Command::Start).unwrap();
        settle(&env.queue, |q| q.snapshot().iter().any(|d| d.status() == DownloadState::Downloading)).await;
        let base = Instant::now();
        env.queue.network_changed(base, NetworkState::Offline, true).unwrap();
        env.queue.network_changed(base + Duration::from_secs(3), NetworkState::Offline, true).unwrap();
        settle(&env.queue, |q| !q.snapshot().iter().any(|d| d.status() == DownloadState::Downloading)).await;
        env.queue.send(Command::Pause).unwrap();
        env.queue.network_changed(base + Duration::from_secs(5), NetworkState::Online(crate::anime::network::Link::Wifi), true).unwrap();
        settle(&env.queue, |q| !q.snapshot().is_empty()).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(!env.queue.running(), "a paused run is not started by the network");
    }

    #[tokio::test(start_paused = true)]
    async fn start_now_moves_the_episode_to_the_front_and_starts() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let source = src(1);
        let show = anime(1, 1);
        env.queue
            .enqueue(source.clone(), show.clone(), vec![ep(1, 1, 1), ep(2, 1, 2)], false, false, None)
            .await
            .unwrap();
        // Enqueue sorts newest first, so the queue is [2, 1] before StartNow.
        let (reply, _answer) = oneshot::channel();
        env.queue.send(Command::StartNow { entry: entry(&source, &show, ep(3, 1, 3)), reply }).unwrap();
        settle(&env.queue, |q| ids(q) == vec![3, 2, 1]).await;
        assert!(env.queue.running());
    }

    #[tokio::test(start_paused = true)]
    async fn restored_items_come_back_queued_and_wait_for_start() {
        let source = src(1);
        let show = anime(1, 1);
        let restored = vec![entry(&source, &show, ep(7, 1, 1)), entry(&source, &show, ep(8, 1, 2))];
        let env = setup(Duration::from_millis(10), &[], AnimeSettings::default(), restored);
        assert_eq!(ids(&env.queue), vec![7, 8]);
        assert!(env.queue.snapshot().iter().all(|d| d.status() == DownloadState::Queue));
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(env.runner.peak.load(Ordering::SeqCst), 0, "nothing starts without Start");
        env.queue.send(Command::Start).unwrap();
        settle(&env.queue, |q| q.snapshot().is_empty()).await;
    }

    #[tokio::test(start_paused = true)]
    async fn rename_commands_go_through_the_layout() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let source = src(1);
        let show = anime(1, 1);
        let folder = AnimeLayout::new(Some(env.root.path().to_path_buf())).anime_dir(&source, &show.title).unwrap();
        std::fs::create_dir(folder.join("Ep 1")).unwrap();
        let renamed = env
            .queue
            .rename_episode(source.clone(), show, ep(1, 1, 1), EpisodeRef { name: "Renamed".into(), ..ep(1, 1, 1) })
            .await
            .unwrap();
        assert_eq!(renamed, Some(folder.join("Renamed")));
        let moved = env.queue.rename_source(source, AnimeSource { name: "Other".into(), ..src(1) }).await.unwrap();
        assert!(moved);
        assert!(env.root.path().join("Other (EN)").is_dir());
    }

    #[tokio::test(start_paused = true)]
    async fn delete_episodes_removes_the_folder_and_the_queue_item() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let source = src(1);
        let show = anime(1, 1);
        env.queue.enqueue(source.clone(), show.clone(), vec![ep(1, 1, 1)], false, false, None).await.unwrap();
        let folder = AnimeLayout::new(Some(env.root.path().to_path_buf())).anime_dir(&source, &show.title).unwrap();
        std::fs::create_dir(folder.join("Ep 1")).unwrap();
        let (reply, answer) = oneshot::channel();
        env.queue
            .send(Command::DeleteEpisodes { source, anime: show, episodes: vec![ep(1, 1, 1)], categories: vec![], reply })
            .unwrap();
        answer.await.unwrap().unwrap();
        assert!(!folder.join("Ep 1").exists());
        settle(&env.queue, |q| q.snapshot().is_empty()).await;
    }

    fn low(_: &Path) -> Option<u64> {
        Some(crate::pipeline::MIN_FREE_BYTES - 1)
    }

    fn unknown(_: &Path) -> Option<u64> {
        None
    }

    #[tokio::test(start_paused = true)]
    async fn low_free_space_sets_error_without_running_and_notifies() {
        let env = setup_with(Duration::from_millis(10), &[], AnimeSettings::default(), Vec::new(), low);
        let mut events = env.queue.events();
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], true, false, None).await.unwrap();
        settle(&env.queue, |q| q.snapshot().first().is_some_and(|d| d.status() == DownloadState::Error)).await;
        assert_eq!(env.runner.peak.load(Ordering::SeqCst), 0, "the runner never started");
        let event = events.recv().await.unwrap();
        let DownloadEvent::Error { chapter_id, message, .. } = event else { panic!("expected an error event") };
        assert_eq!(chapter_id, Some(1));
        assert_eq!(message, "Couldn't download due to low storage space");
        assert_eq!(env.store.rows().unwrap(), vec![(1, 1, 0)], "the errored item stays queued");
    }

    #[tokio::test(start_paused = true)]
    async fn an_unknown_free_space_skips_the_check() {
        let env = setup_with(Duration::from_millis(10), &[], AnimeSettings::default(), Vec::new(), unknown);
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], true, false, None).await.unwrap();
        settle(&env.queue, |q| q.snapshot().is_empty()).await;
        assert_eq!(env.runner.peak.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_runner_emits_an_error_event() {
        let env = setup(Duration::from_millis(10), &[1], AnimeSettings::default(), Vec::new());
        let mut events = env.queue.events();
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], true, false, None).await.unwrap();
        let DownloadEvent::Error { message, manga_title, .. } = events.recv().await.unwrap() else {
            panic!("expected an error event")
        };
        assert_eq!(message, "No video found");
        assert_eq!(manga_title.as_deref(), Some("Anime 1"));
    }

    #[tokio::test(start_paused = true)]
    async fn flush_deletes_installed_sources_and_keeps_the_others() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let (one, two) = (src(1), src(2));
        let (a1, a2) = (anime(1, 1), anime(2, 2));
        let layout = AnimeLayout::new(Some(env.root.path().to_path_buf()));
        let dir1 = layout.anime_dir(&one, &a1.title).unwrap();
        let dir2 = layout.anime_dir(&two, &a2.title).unwrap();
        std::fs::create_dir(dir1.join("Ep 1")).unwrap();
        std::fs::create_dir(dir2.join("Ep 2")).unwrap();
        for (a, e) in [(&a1, ep(1, 1, 1)), (&a2, ep(2, 2, 1))] {
            let (reply, answer) = oneshot::channel();
            env.queue
                .send(Command::EnqueueDelete { anime: a.clone(), episodes: vec![e.clone(), e], categories: vec![], reply })
                .unwrap();
            assert_eq!(answer.await.unwrap().unwrap(), 1, "the duplicate in the batch is dropped");
        }
        let (reply, answer) = oneshot::channel();
        env.queue
            .send(Command::FlushPendingDeletes { installed: vec![one.clone()], categories: HashMap::new(), bookmarks: HashMap::new(), reply })
            .unwrap();
        assert_eq!(answer.await.unwrap().unwrap(), 1);
        assert!(!dir1.join("Ep 1").exists());
        assert!(dir2.join("Ep 2").exists(), "source 2 is not installed, so its file stays");
        assert_eq!(env.pending.len().unwrap(), 1, "and its row stays for the next flush");
        let (reply, answer) = oneshot::channel();
        env.queue
            .send(Command::FlushPendingDeletes { installed: vec![one, two], categories: HashMap::new(), bookmarks: HashMap::new(), reply })
            .unwrap();
        assert_eq!(answer.await.unwrap().unwrap(), 1);
        assert!(!dir2.join("Ep 2").exists());
        assert!(env.pending.is_empty().unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn queued_finds_an_item_by_episode_id() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], false, false, None).await.unwrap();
        assert!(env.queue.queued(1).is_some());
        assert!(env.queue.queued(2).is_none());
    }

    async fn delete_episodes_cmd(
        env: &Env,
        source: &AnimeSource,
        anime: &AnimeRef,
        episodes: Vec<EpisodeRef>,
    ) -> Result<usize, DownloadError> {
        let (reply, answer) = oneshot::channel();
        env.queue
            .send(Command::DeleteEpisodes { source: source.clone(), anime: anime.clone(), episodes, categories: vec![], reply })
            .unwrap();
        answer.await.unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn deleting_the_last_episode_also_removes_the_anime_and_source_folders() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let (source, show) = (src(1), anime(1, 1));
        let folder = AnimeLayout::new(Some(env.root.path().to_path_buf())).anime_dir(&source, &show.title).unwrap();
        std::fs::create_dir(folder.join("Ep 1")).unwrap();
        env.index.add_episode(1, "S1 (EN)", &anime_dir_name(&show.title), "Ep 1");
        let (reply, answer) = oneshot::channel();
        env.queue
            .send(Command::DeleteEpisodes { source: source.clone(), anime: show.clone(), episodes: vec![ep(1, 1, 1)], categories: vec![], reply })
            .unwrap();
        answer.await.unwrap().unwrap();
        assert!(!folder.exists(), "E-D03: the emptied anime folder goes");
        assert!(!env.root.path().join("S1 (EN)").exists(), "E-D04: the emptied source folder goes");
        assert_eq!(env.index.episode_count_total(), 0, "the index forgets the episode and the anime");
    }

    #[tokio::test(start_paused = true)]
    async fn deleting_an_episode_also_removes_its_partial_download() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let (source, show) = (src(1), anime(1, 1));
        let folder = AnimeLayout::new(Some(env.root.path().to_path_buf())).anime_dir(&source, &show.title).unwrap();
        std::fs::create_dir(folder.join("Ep 1_tmp")).unwrap();
        std::fs::create_dir(folder.join("Ep 2")).unwrap();
        delete_episodes_cmd(&env, &source, &show, vec![ep(1, 1, 1)]).await.unwrap();
        assert!(!folder.join("Ep 1_tmp").exists(), "E-D01: the partial download goes");
        assert!(folder.join("Ep 2").exists(), "other episodes stay");
    }

    #[tokio::test(start_paused = true)]
    async fn a_protected_episode_is_kept_and_the_delete_still_succeeds() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let (source, show) = (src(1), anime(1, 1));
        let folder = AnimeLayout::new(Some(env.root.path().to_path_buf())).anime_dir(&source, &show.title).unwrap();
        std::fs::create_dir(folder.join("Ep 1")).unwrap();
        let bookmarked = EpisodeRef { bookmark: true, ..ep(1, 1, 1) };
        delete_episodes_cmd(&env, &source, &show, vec![bookmarked]).await.unwrap();
        assert!(folder.join("Ep 1").exists(), "bookmarked episodes stay unless the setting allows it (behaviour 3.2)");
    }

    #[tokio::test(start_paused = true)]
    async fn deleting_an_anime_while_it_runs_pauses_then_resumes_the_other_items() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let (s1, a1, s2, a2) = (src(1), anime(1, 1), src(2), anime(2, 2));
        env.queue.enqueue(s1.clone(), a1.clone(), vec![ep(1, 1, 1)], true, false, None).await.unwrap();
        env.queue.enqueue(s2.clone(), a2.clone(), vec![ep(3, 2, 1)], false, false, None).await.unwrap();
        settle(&env.queue, |q| q.snapshot().first().is_some_and(|d| d.status() == DownloadState::Downloading)).await;
        let (reply, answer) = oneshot::channel();
        env.queue
            .send(Command::DeleteAnime { source: s1, anime: a1, remove_queued: true, reply })
            .unwrap();
        answer.await.unwrap().unwrap();
        assert_eq!(ids(&env.queue), vec![3], "E-D02: the deleted anime's item is gone");
        settle(&env.queue, |q| q.running() && q.snapshot().iter().any(|d| d.status() == DownloadState::Downloading)).await;
        assert_eq!(env.queue.runner_state(), RunnerState::Running, "the run resumes after the delete");
    }

    #[tokio::test(start_paused = true)]
    async fn a_failure_stop_puts_running_items_in_error_and_warns() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let mut events = env.queue.events();
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], true, false, None).await.unwrap();
        settle(&env.queue, |q| q.snapshot().first().is_some_and(|d| d.status() == DownloadState::Downloading)).await;
        env.queue.send(Command::StopWithFailure { text: "No network connection available".into() }).unwrap();
        settle(&env.queue, |q| q.snapshot().first().is_some_and(|d| d.status() == DownloadState::Error)).await;
        assert_eq!(env.queue.runner_state(), RunnerState::Paused, "off with an item still queued");
        let DownloadEvent::Warning { text, timeout_secs, .. } = events.recv().await.unwrap() else {
            panic!("expected a warning")
        };
        assert_eq!(text, "No network connection available");
        assert_eq!(timeout_secs, Some(30));
    }

    #[tokio::test(start_paused = true)]
    async fn the_runner_state_follows_start_pause_and_clear() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let mut state = env.queue.subscribe_state();
        assert_eq!(env.queue.runner_state(), RunnerState::Stopped);
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], false, false, None).await.unwrap();
        assert_eq!(env.queue.runner_state(), RunnerState::Paused, "queued but not started");
        env.queue.send(Command::Start).unwrap();
        settle(&env.queue, |q| q.running()).await;
        state.changed().await.unwrap();
        env.queue.send(Command::Pause).unwrap();
        settle(&env.queue, |q| q.runner_state() == RunnerState::Paused).await;
        env.queue.send(Command::ClearQueue).unwrap();
        settle(&env.queue, |q| q.runner_state() == RunnerState::Stopped).await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_finished_download_is_recorded_in_the_index() {
        let env = setup(Duration::from_millis(10), &[], AnimeSettings::default(), Vec::new());
        env.queue.enqueue(src(1), anime(1, 1), vec![ep(1, 1, 1)], true, false, None).await.unwrap();
        settle(&env.queue, |q| q.snapshot().is_empty()).await;
        assert!(env.index.is_episode_downloaded(1, "Anime 1", &ep(1, 1, 1)), "the finished episode is indexed");
    }

    #[tokio::test(start_paused = true)]
    async fn flush_checks_the_bookmark_again_before_deleting() {
        let env = setup(LONG, &[], AnimeSettings::default(), Vec::new());
        let (source, show) = (src(1), anime(1, 1));
        let folder = AnimeLayout::new(Some(env.root.path().to_path_buf())).anime_dir(&source, &show.title).unwrap();
        std::fs::create_dir(folder.join("Ep 1")).unwrap();
        std::fs::create_dir(folder.join("Ep 2")).unwrap();
        // Enqueued unbookmarked, bookmarked afterwards (E-P07): the live flag passed to the flush must keep it.
        env.pending.add_episodes(&show, &[ep(1, 1, 1), ep(2, 1, 2)]).unwrap();
        let (reply, answer) = oneshot::channel();
        env.queue
            .send(Command::FlushPendingDeletes {
                installed: vec![source],
                categories: HashMap::new(),
                bookmarks: HashMap::from([(1, true)]),
                reply,
            })
            .unwrap();
        assert_eq!(answer.await.unwrap().unwrap(), 1, "only the unprotected episode is handled");
        assert!(folder.join("Ep 1").exists());
        assert!(!folder.join("Ep 2").exists());
    }
}
