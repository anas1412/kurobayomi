//! Public facade for the other areas (port of MangaDownloadManager.kt). Queue commands go to the downloader actor.
//! Disk work (delete, folder cleanup) runs on the blocking pool, and the index is updated on the calling task.

use kuro_platform::kind::Kind;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_archive::ArchiveReader;
use kuro_core::image::is_image_name;
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::delete::ChapterSourceResolver;
use crate::downloader::{
    COMMAND_CAPACITY, Command, Connectivity, DownloadSettings, Downloader, ProcessHold, RemoveTarget, Shared,
    unix_millis,
};
use crate::error::DownloadError;
use crate::events::{DownloadEvent, EVENT_CAPACITY};
use crate::index::DownloadIndex;
use crate::layout::{CBZ_EXTENSION, DownloadLayout, TMP_DIR_SUFFIX, chapter_dir_name, sweep_stray_chapters};
use crate::model::{ChapterRef, DownloadState, MangaRef, QueuedDownload, SourceRef};
use crate::pending::{MangaPendingStore, flush_plan};
use crate::pipeline::FreeSpaceFn;
use crate::source::ChapterSource;
use crate::store::{QueueRow, QueueStore};
use crate::watch::{RootWatch, watch_root};

/// Result of an enqueue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueOutcome {
    /// Chapters added to the queue (the count may be lower than requested: duplicates and downloaded chapters are
    /// left out).
    Queued(usize),
    NothingToDo,
    /// Local sources cannot be queued.
    SourceNotHttp,
}

/// One page of a downloaded chapter, as the reader opens it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalPage {
    pub index: u32,
    /// The image file, or the `.cbz` archive that holds the page.
    pub path: PathBuf,
    /// The entry name inside the `.cbz`. `None` for a file in a chapter folder.
    pub is_cbz_entry: Option<String>,
}

/// Everything the downloader needs from the host. The GUI supplies the platform parts (free space, network state,
/// process hold).
pub struct ManagerParams {
    pub layout: Arc<DownloadLayout>,
    pub index: Arc<DownloadIndex>,
    pub store: Arc<QueueStore>,
    pub free_space: FreeSpaceFn,
    pub settings: DownloadSettings,
    /// Chapters whose delete waits for the reader to exit (T-23a-028).
    pub pending: Arc<MangaPendingStore>,
    /// Reader page cache (area 34). `None` when the reader does not share its cache.
    pub page_cache: Option<Arc<dyn crate::images::PageImageCache>>,
    pub connectivity: watch::Receiver<Connectivity>,
    pub hold: Arc<dyn ProcessHold>,
    /// The queue read back at startup, in order ([`DownloadManager::restore`]). They come back as QUEUE, nothing starts until `Start`,
    /// and their staging folders survive the startup sweep so a resume continues them (T-23a-044).
    pub restored: Vec<QueuedDownload>,
    /// Saved rows whose source was not registered at startup (T-23a-035). They are written back unchanged.
    pub kept_rows: Vec<QueueRow>,
}

/// Which chapters a delete applies to, from the category and bookmark settings (`getChaptersToDelete`).
#[derive(Clone, Debug, Default)]
pub struct DeleteRules {
    /// Categories whose read chapters are kept (`remove_exclude_categories`).
    pub exclude_categories: Vec<i64>,
    /// Categories of the manga. Empty means category 0.
    pub manga_categories: Vec<i64>,
    /// Allow deleting bookmarked chapters (`pref_remove_bookmarked`).
    pub remove_bookmarked: bool,
}

/// The chapters a delete keeps out: read chapters in an excluded category, and bookmarks unless allowed.
pub fn chapters_to_delete(chapters: Vec<ChapterRef>, rules: &DeleteRules) -> Vec<ChapterRef> {
    crate::delete::select_for_delete(chapters, &rules.exclude_categories, &rules.manga_categories, rules.remove_bookmarked)
}

/// One async lock per series, keyed by kind and entry id (E-22). Deletes of the same series run one at a time, so two
/// concurrent deletes never race on the same folder or index entry.
#[derive(Default)]
pub struct SeriesLocks {
    map: std::sync::Mutex<HashMap<(Kind, i64), SeriesGate>>,
}

type SeriesGate = Arc<tokio::sync::Mutex<()>>;

impl SeriesLocks {
    pub async fn lock(&self, kind: Kind, id: i64) -> tokio::sync::OwnedMutexGuard<()> {
        let gate = {
            let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
            map.entry((kind, id)).or_default().clone()
        };
        gate.lock_owned().await
    }
}

#[derive(Clone)]
pub struct DownloadManager {
    commands: mpsc::Sender<Command>,
    layout: Arc<DownloadLayout>,
    index: Arc<DownloadIndex>,
    events: broadcast::Sender<DownloadEvent>,
    shared: Arc<Shared>,
    series: Arc<SeriesLocks>,
    pending: Arc<MangaPendingStore>,
    /// Keeps the watch on the downloads root alive while any handle exists (T-23a-045). `None` when it cannot start.
    _watch: Arc<Option<std::sync::Mutex<RootWatch>>>,
}

impl DownloadManager {
    /// Starts the downloader on the current Tokio runtime. Must be called inside a runtime.
    pub fn spawn(params: ManagerParams) -> Self {
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (commands, command_rx) = mpsc::channel(COMMAND_CAPACITY);
        let shared = Arc::new(Shared::new());
        let layout = params.layout.clone();
        let index = params.index.clone();
        let pending = params.pending.clone();
        // A change under the downloads root marks the index stale after a quiet period (T-23a-045).
        let watch = Arc::new(watch_root(&index).map(std::sync::Mutex::new));
        // Startup sweep (T-23a-044): staging folders of restored chapters are kept for resume, the rest go.
        let keep: Vec<PathBuf> = params
            .restored
            .iter()
            .map(|d| layout.staging_dir(&d.source.display_name(), &d.manga.title, &d.chapter.name, d.chapter.scanlator.as_deref()))
            .collect();
        let removed = sweep_stray_chapters(layout.root(), &keep);
        tracing::info!(
            target: "kuro_downloads",
            removed,
            kept = keep.len(),
            "startup sweep of stray chapter folders and archive parts"
        );
        let downloader = Downloader::new(params, events.clone(), shared.clone());
        tokio::spawn(downloader.run(command_rx));
        Self { commands, layout, index, events, shared, series: Arc::new(SeriesLocks::default()), pending, _watch: watch }
    }

    async fn send(&self, cmd: Command) -> Result<(), DownloadError> {
        self.commands.send(cmd).await.map_err(|_| stopped())
    }

    /// Queues chapters of one manga. Filters out downloaded and queued chapters, and starts the run when the queue
    /// was empty and `auto_start` is set.
    pub async fn enqueue(
        &self,
        source: SourceRef,
        manga: MangaRef,
        chapters: Vec<ChapterRef>,
        auto_start: bool,
    ) -> Result<EnqueueOutcome, DownloadError> {
        let (reply, rx) = oneshot::channel();
        self.send(Command::Enqueue { source, manga, chapters, auto_start, reply }).await?;
        rx.await.map_err(|_| stopped())?
    }

    /// Sends a command without waiting for the actor, for callers on the GTK main thread. Fails only when the actor is
    /// gone or its queue of 64 commands is full (the command is then dropped and the error says so).
    pub fn post(&self, cmd: Command) -> Result<(), DownloadError> {
        self.commands.try_send(cmd).map_err(|_| stopped())
    }

    /// Startup restore (see [`Command::Restore`]): queues `downloads` and keeps `kept_rows` as the saved rows that stay out.
    pub async fn restore(&self, downloads: Vec<QueuedDownload>, kept_rows: Vec<QueueRow>) -> Result<(), DownloadError> {
        self.send(Command::Restore { downloads, kept_rows }).await
    }

    pub async fn start(&self) -> Result<(), DownloadError> {
        self.send(Command::Start).await
    }

    /// Pauses the run. Running chapters go back to the queue.
    pub async fn pause(&self) -> Result<(), DownloadError> {
        self.send(Command::Pause).await
    }

    /// Empties the queue and stops the run.
    pub async fn clear(&self) -> Result<(), DownloadError> {
        self.send(Command::Clear).await
    }

    /// Removes chapters from the queue. Returns the ids that were queued. Removed running chapters are not written
    /// further.
    pub async fn cancel(&self, chapter_ids: Vec<i64>) -> Result<Vec<i64>, DownloadError> {
        self.remove_from_queue(RemoveTarget::Chapters(chapter_ids)).await
    }

    /// Removes every queued chapter of one manga.
    pub async fn cancel_manga(&self, manga_id: i64) -> Result<Vec<i64>, DownloadError> {
        self.remove_from_queue(RemoveTarget::Manga(manga_id)).await
    }

    /// Adds chapters at the front of the queue and starts the run when it is not running (EC-11).
    pub async fn add_to_front(
        &self,
        source: SourceRef,
        manga: MangaRef,
        chapters: Vec<ChapterRef>,
    ) -> Result<EnqueueOutcome, DownloadError> {
        self.enqueue_front(source, manga, chapters, true).await
    }

    /// Starts one chapter now. A queued chapter moves to the front; an unqueued one is added at the front (EC-09, EC-10).
    /// A chapter already queued gives `NothingToDo`, because nothing new was queued.
    pub async fn start_now(
        &self,
        source: SourceRef,
        manga: MangaRef,
        chapter: ChapterRef,
    ) -> Result<EnqueueOutcome, DownloadError> {
        if self.queued(chapter.id).is_some() {
            self.reorder(vec![chapter.id]).await?;
            self.start().await?;
            return Ok(EnqueueOutcome::NothingToDo);
        }
        self.enqueue_front(source, manga, vec![chapter], true).await
    }

    /// Puts the queue in this order. Unknown ids are ignored; unlisted chapters follow in their current order (E-15).
    /// A running queue is paused, reordered and restarted (EC-14).
    pub async fn reorder(&self, chapter_ids: Vec<i64>) -> Result<(), DownloadError> {
        self.send(Command::Reorder { chapter_ids }).await
    }

    async fn enqueue_front(
        &self,
        source: SourceRef,
        manga: MangaRef,
        chapters: Vec<ChapterRef>,
        start: bool,
    ) -> Result<EnqueueOutcome, DownloadError> {
        let (reply, rx) = oneshot::channel();
        self.send(Command::EnqueueFront { source, manga, chapters, start, reply }).await?;
        rx.await.map_err(|_| stopped())?
    }

    async fn remove_from_queue(&self, target: RemoveTarget) -> Result<Vec<i64>, DownloadError> {
        let (reply, rx) = oneshot::channel();
        self.send(Command::Remove { target, reply }).await?;
        rx.await.map_err(|_| stopped())
    }

    /// The queued item for a chapter, without waiting for the actor.
    pub fn queued(&self, chapter_id: i64) -> Option<Arc<QueuedDownload>> {
        self.snapshot().into_iter().find(|d| d.chapter.id == chapter_id)
    }

    /// The queue in order, as of the last actor step.
    pub fn snapshot(&self) -> Vec<Arc<QueuedDownload>> {
        self.shared.snapshot.read().unwrap_or_else(|e| e.into_inner()).as_ref().clone()
    }

    pub fn running(&self) -> bool {
        self.shared.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// A receiver for downloader events. A lagging receiver gets `Lagged` and should resync from [`Self::snapshot`].
    pub fn events(&self) -> broadcast::Receiver<DownloadEvent> {
        self.events.subscribe()
    }

    pub fn index(&self) -> &DownloadIndex {
        &self.index
    }

    /// How many times the downloader loop has woken up. Used to check that an idle queue does not poll.
    pub fn wakeups(&self) -> u64 {
        self.shared.wakeups.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn is_chapter_downloaded(&self, source: &dyn ChapterSource, manga_title: &str, chapter: &ChapterRef) -> bool {
        self.index.is_chapter_downloaded(source.id(), manga_title, &chapter.name, chapter.scanlator.as_deref())
    }

    pub fn chapter_count(&self) -> usize {
        self.index.chapter_count_total()
    }

    /// Chapters of one manga. The local source counts the folders and archives of its manga folder, every other source
    /// uses the index (T-23a-042).
    pub fn chapter_count_for_manga(&self, source: &dyn ChapterSource, manga_title: &str) -> usize {
        match source.local_manga_dir(manga_title) {
            Some(dir) => crate::local::chapter_count(&dir).unwrap_or(0),
            None => self.index.chapter_count_manga(source.id(), manga_title),
        }
    }

    pub async fn size_total(&self) -> u64 {
        self.index.size_total().await
    }

    /// Bytes of one manga. The local source sums its manga folder recursively, every other source uses the index.
    pub async fn size_for_manga(&self, source: &dyn ChapterSource, manga_title: &str) -> u64 {
        match source.local_manga_dir(manga_title) {
            Some(dir) => tokio::task::spawn_blocking(move || crate::local::size(&dir)).await.unwrap_or(0),
            None => self.index.size_manga(source.id(), manga_title).await,
        }
    }

    /// Remove-after-read when the reader is open: the chapters the rules pick are written to the pending table, and
    /// [`Self::flush_pending_deletes`] deletes them on reader exit (T-23a-028). Returns how many rows were new.
    pub fn defer_delete(&self, manga: &MangaRef, chapters: &[ChapterRef], rules: &DeleteRules) -> Result<usize, DownloadError> {
        self.pending
            .add_chapters(manga, chapters, rules, unix_millis())
            .map_err(|e| DownloadError::Internal(e.to_string()))
    }

    /// The reader calls this on exit. Takes the pending rows, drops those of a manga `live_rules` no longer knows
    /// (EC-93), applies the live exclusions again (EC-106) and deletes the rest with [`Self::delete_chapters`].
    /// A manga whose source is not installed, or whose delete failed, goes back to the table for the next flush.
    /// Returns how many chapters were deleted.
    pub async fn flush_pending_deletes(
        &self,
        sources: &dyn ChapterSourceResolver,
        live_rules: impl Fn(i64) -> Option<DeleteRules>,
    ) -> Result<usize, DownloadError> {
        let store = self.pending.clone();
        let taken = tokio::task::spawn_blocking(move || store.take_all())
            .await
            .map_err(|e| DownloadError::Internal(e.to_string()))?
            .map_err(|e| DownloadError::Internal(e.to_string()))?;
        let keep_all = DeleteRules { remove_bookmarked: true, ..DeleteRules::default() };
        let mut deleted = 0;
        for group in flush_plan(taken, &live_rules) {
            let Some(rules) = live_rules(group.manga.id) else { continue };
            let outcome = match sources.chapter_source(group.manga.source_id) {
                Some(source) => Some(self.delete_chapters(group.chapters.clone(), &group.manga, source.as_ref(), &rules).await),
                None => None,
            };
            match outcome {
                Some(Ok(n)) => deleted += n,
                other => {
                    if let Some(Err(err)) = &other {
                        tracing::warn!(target: "kuro_downloads", manga_id = group.manga.id, error = %err, "pending delete failed; kept for the next flush");
                    }
                    if let Err(err) = self.pending.add_chapters(&group.manga, &group.chapters, &keep_all, unix_millis()) {
                        tracing::error!(target: "kuro_downloads", error = %err, "could not keep a pending delete group");
                    }
                }
            }
        }
        Ok(deleted)
    }

    /// Deletes downloaded chapters the rules pick. The chapters leave the queue first (pause, remove, restart). An
    /// emptied manga folder is removed too. Returns how many chapters were selected for deletion.
    pub async fn delete_chapters(
        &self,
        chapters: Vec<ChapterRef>,
        manga: &MangaRef,
        source: &dyn ChapterSource,
        rules: &DeleteRules,
    ) -> Result<usize, DownloadError> {
        let selected = chapters_to_delete(chapters, rules);
        if selected.is_empty() {
            return Ok(0);
        }
        let ids: Vec<i64> = selected.iter().map(|c| c.id).collect();
        self.remove_from_queue(RemoveTarget::Chapters(ids)).await?;
        // Steps after the queue removal run under the series lock (E-22).
        let _series = self.series.lock(Kind::Manga, manga.id).await;

        let display = source.display_name();
        let layout = self.layout.clone();
        let title = manga.title.clone();
        let targets = selected.clone();
        let emptied = tokio::task::spawn_blocking(move || -> io::Result<bool> {
            let (manga_dir, found) = layout.find_chapter_dirs(&display, &title, &targets);
            for path in found {
                remove_path(&path)?;
            }
            match manga_dir {
                Some(dir) => Ok(is_empty_dir(&dir)),
                None => Ok(false),
            }
        })
        .await
        .map_err(|e| DownloadError::Internal(e.to_string()))??;

        let names: Vec<String> = selected.iter().map(|c| chapter_dir_name(&c.name, c.scanlator.as_deref())).collect();
        self.index.remove_chapter(source.id(), &manga.title, &names);
        if emptied {
            self.delete_manga_locked(manga, source, false).await?;
        }
        Ok(selected.len())
    }

    /// Deletes a manga folder and updates the index. The source folder goes too when it is then empty. With
    /// `remove_queued`, the manga's queued chapters are removed first. Chapters of the manga that stay queued are
    /// paused while the folder is deleted and start again afterwards (RC-04, RC-11).
    pub async fn delete_manga(&self, manga: &MangaRef, source: &dyn ChapterSource, remove_queued: bool) -> Result<(), DownloadError> {
        let _series = self.series.lock(Kind::Manga, manga.id).await;
        self.delete_manga_locked(manga, source, remove_queued).await
    }

    /// The body of [`Self::delete_manga`], for callers that already hold the series lock.
    async fn delete_manga_locked(&self, manga: &MangaRef, source: &dyn ChapterSource, remove_queued: bool) -> Result<(), DownloadError> {
        if remove_queued {
            self.remove_from_queue(RemoveTarget::Manga(manga.id)).await?;
        }
        let (reply, rx) = oneshot::channel();
        self.send(Command::PauseForMutation { manga_id: manga.id, reply }).await?;
        let paused = rx.await.map_err(|_| stopped())?;
        let display = source.display_name();
        let layout = self.layout.clone();
        let title = manga.title.clone();
        let disk = tokio::task::spawn_blocking(move || -> io::Result<bool> {
            if let Some(dir) = layout.find_manga_dir(&display, &title) {
                remove_path(&dir)?;
            }
            match layout.find_source_dir(&display) {
                Some(dir) if is_empty_dir(&dir) => {
                    fs::remove_dir(&dir)?;
                    Ok(true)
                }
                _ => Ok(false),
            }
        })
        .await;
        if paused {
            self.send(Command::ResumeAfterMutation).await?;
        }
        let source_removed = disk.map_err(|e| DownloadError::Internal(e.to_string()))??;

        self.index.remove_manga(source.id(), &manga.title);
        if source_removed {
            self.index.remove_source(source.id());
        }
        Ok(())
    }

    /// Renames the source folder after the source changed its name. A case-only change goes through `<name>_tmp`
    /// (EC-102). A target that exists fails the rename and keeps the old folder.
    pub async fn rename_source(&self, old: &dyn ChapterSource, new: &dyn ChapterSource) -> Result<(), DownloadError> {
        let Some(old_dir) = self.layout.find_source_dir(&old.display_name()) else { return Ok(()) };
        let new_name = DownloadLayout::source_dir_name(&new.display_name());
        let old_name = old_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if old_name == new_name {
            return Ok(());
        }
        let new_dir = old_dir.with_file_name(&new_name);
        let case_only = old_name.to_lowercase() == new_name.to_lowercase();
        let result = tokio::task::spawn_blocking(move || rename_folder(&old_dir, &new_dir, case_only))
            .await
            .map_err(|e| DownloadError::Internal(e.to_string()))?;
        if let Err(e) = result {
            tracing::warn!(target: "kuro_downloads", "Failed to rename source download folder: {old_name}: {e}");
            return Err(e.into());
        }
        self.index.rename_source_dir(old.id(), &new_name);
        Ok(())
    }

    /// Renames the downloaded folder or `.cbz` of a chapter whose name or scanlator changed. Does nothing when the
    /// chapter is not downloaded (EC-105). A target that exists fails the rename and keeps the old one (EC-104).
    pub async fn rename_chapter(
        &self,
        source: &dyn ChapterSource,
        manga: &MangaRef,
        old: &ChapterRef,
        new: &ChapterRef,
    ) -> Result<(), DownloadError> {
        let display = source.display_name();
        let new_dir_name = chapter_dir_name(&new.name, new.scanlator.as_deref());
        let layout = self.layout.clone();
        let (lookup_display, title, old_chapter) = (display.clone(), manga.title.clone(), old.clone());
        let renamed =
            tokio::task::spawn_blocking(move || rename_chapter_dir(&layout, &lookup_display, &title, &old_chapter, new_dir_name))
                .await
                .map_err(|e| DownloadError::Internal(e.to_string()))?;
        match renamed {
            Ok(Some(new_name)) => {
                let old_dir_name = chapter_dir_name(&old.name, old.scanlator.as_deref());
                self.index.remove_chapter(source.id(), &manga.title, &[old_dir_name]);
                self.index.add_chapter(source.id(), &DownloadLayout::source_dir_name(&display), &manga.title, &new_name);
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(e) => {
                tracing::warn!(target: "kuro_downloads", "Could not rename downloaded chapter: {}: {e}", old.name);
                Err(e.into())
            }
        }
    }

    /// The pages of a downloaded chapter in reading order: image files of the folder sorted by name, or the image
    /// entries of the `.cbz`. No pages is an error, as when the chapter is not downloaded (behaviour 9).
    pub async fn build_page_list(
        &self,
        source: &dyn ChapterSource,
        manga: &MangaRef,
        chapter: &ChapterRef,
    ) -> Result<Vec<LocalPage>, DownloadError> {
        let layout = self.layout.clone();
        let (display, title, chapter) = (source.display_name(), manga.title.clone(), chapter.clone());
        tokio::task::spawn_blocking(move || {
            let path = layout
                .find_chapter(&display, &title, &chapter.name, chapter.scanlator.as_deref())
                .ok_or(DownloadError::NoPages)?;
            let pages = if path.is_dir() { folder_pages(&path)? } else { cbz_pages(&path)? };
            if pages.is_empty() { Err(DownloadError::NoPages) } else { Ok(pages) }
        })
        .await
        .map_err(|e| DownloadError::Internal(e.to_string()))?
    }

    /// Rescans the downloads folder when the index is older than an hour, so chapters already on disk are not queued
    /// again (EC-80). The GUI calls this once the source registry is ready and again after
    /// `index().invalidate()` when the storage location changes. `ready` is the source gate, `installed` lists
    /// `(source id, display name)`. Returns `true` when a scan replaced the index.
    pub async fn renew_index<F>(&self, ready: watch::Receiver<bool>, installed: F) -> bool
    where
        F: FnOnce() -> Vec<(i64, String)> + Send,
    {
        self.index.renew_if_stale(unix_millis(), ready, installed).await
    }
}

fn stopped() -> DownloadError {
    DownloadError::Internal("the download queue has stopped".to_owned())
}

fn remove_path(path: &Path) -> io::Result<()> {
    let result = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
    match result {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Renames a folder or file and fails when the target exists, where `rename(2)` would replace an empty folder.
fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} already exists", to.display())));
    }
    fs::rename(from, to)
}

/// A case-only rename goes through `<to>_tmp`, which a case-insensitive disk needs. If the second step fails, the
/// folder goes back to its old name.
fn rename_folder(from: &Path, to: &Path, case_only: bool) -> io::Result<()> {
    if !case_only {
        return rename_new(from, to);
    }
    let mut staging = to.as_os_str().to_owned();
    staging.push(TMP_DIR_SUFFIX);
    let staging = PathBuf::from(staging);
    rename_new(from, &staging)?;
    rename_new(&staging, to).inspect_err(|_| {
        let _ = fs::rename(&staging, from);
    })
}

/// Finds the old chapter folder or `.cbz` and renames it. Returns the new name as the index keeps it (the caller
/// strips `.cbz`), or `None` when nothing was downloaded or the name is unchanged. A `.cbz` keeps its extension.
fn rename_chapter_dir(
    layout: &DownloadLayout,
    display: &str,
    title: &str,
    old: &ChapterRef,
    new_dir_name: String,
) -> io::Result<Option<String>> {
    let Some(found) = layout.find_chapter(display, title, &old.name, old.scanlator.as_deref()) else { return Ok(None) };
    let mut name = new_dir_name;
    if found.is_file() && found.extension().is_some_and(|e| e == CBZ_EXTENSION) {
        name = format!("{name}.{CBZ_EXTENSION}");
    }
    if found.file_name().is_some_and(|n| n == name.as_str()) {
        return Ok(None);
    }
    rename_new(&found, &found.with_file_name(&name))?;
    Ok(Some(name))
}

/// Up to 32 bytes of a file, for the image sniff.
fn head(path: &Path) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    fs::File::open(path).ok()?.take(32).read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn folder_pages(dir: &Path) -> io::Result<Vec<LocalPage>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| is_image_name(n, || head(p))))
        .collect();
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files.into_iter().enumerate().map(|(i, path)| LocalPage { index: i as u32, path, is_cbz_entry: None }).collect())
}

fn cbz_pages(cbz: &Path) -> Result<Vec<LocalPage>, DownloadError> {
    let entries = ArchiveReader::open(cbz)
        .and_then(|reader| reader.page_entries())
        .map_err(|e| DownloadError::Io(io::Error::other(e)))?;
    Ok(entries
        .into_iter()
        .enumerate()
        .map(|(i, entry)| LocalPage { index: i as u32, path: cbz.to_owned(), is_cbz_entry: Some(entry.name) })
        .collect())
}

fn is_empty_dir(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|mut rd| rd.next().is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::chapter;

    fn with(read: bool, bookmark: bool) -> ChapterRef {
        ChapterRef { read, bookmark, ..chapter(1, 1, "Ch") }
    }

    #[test]
    fn default_rules_keep_bookmarked_chapters() {
        let rules = DeleteRules::default();
        let kept = chapters_to_delete(vec![with(true, false), with(false, true)], &rules);
        assert_eq!(kept.len(), 1);
        assert!(!kept[0].bookmark);
    }

    #[test]
    fn excluded_category_keeps_only_unread_chapters() {
        let rules = DeleteRules { exclude_categories: vec![5], manga_categories: vec![5, 6], remove_bookmarked: true };
        let kept = chapters_to_delete(vec![with(true, false), with(false, false)], &rules);
        assert_eq!(kept.len(), 1);
        assert!(!kept[0].read, "EC-95: a read chapter in an excluded category stays");
    }

    #[test]
    fn an_uncategorised_manga_counts_as_category_zero() {
        let rules = DeleteRules { exclude_categories: vec![0], manga_categories: vec![], remove_bookmarked: true };
        assert!(chapters_to_delete(vec![with(true, false)], &rules).is_empty());
    }

    #[test]
    fn a_manga_outside_the_excluded_categories_deletes_read_chapters() {
        let rules = DeleteRules { exclude_categories: vec![5], manga_categories: vec![7], remove_bookmarked: true };
        assert_eq!(chapters_to_delete(vec![with(true, false)], &rules).len(), 1);
    }
}

#[cfg(test)]
mod run_tests {
    use super::*;
    use crate::downloader::{BULK_WARNING_TEXT, NO_NETWORK_TEXT, NO_WIFI_TEXT};
    use crate::model::{DownloadState, SourceRef};
    use crate::testing::{FakeSource, chapter, fake, manga};
    use kuro_source::SourceError;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Duration;

    fn plenty(_: &Path) -> Option<u64> {
        Some(u64::MAX / 2)
    }

    fn tiny(_: &Path) -> Option<u64> {
        Some(1024)
    }

    struct Env {
        mgr: DownloadManager,
        conn: watch::Sender<Connectivity>,
        root: PathBuf,
        _base: tempfile::TempDir,
    }

    fn setup_with(connectivity: Connectivity, only_over_wifi: bool, free: FreeSpaceFn) -> Env {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("downloads");
        std::fs::create_dir_all(&root).unwrap();
        let index = Arc::new(DownloadIndex::open(&root, &base.path().join("cache")));
        let store = Arc::new(QueueStore::open_in_memory().unwrap());
        let (conn, rx) = watch::channel(connectivity);
        let mgr = DownloadManager::spawn(ManagerParams {
            layout: Arc::new(DownloadLayout::new(&root)),
            index,
            store,
            free_space: free,
            settings: DownloadSettings { only_over_wifi, save_as_cbz: false, ..DownloadSettings::default() },
            page_cache: None,
            connectivity: rx,
            hold: Arc::new(crate::downloader::NoHold),
            restored: Vec::new(),
            kept_rows: Vec::new(),
            pending: Arc::new(crate::pending::MangaPendingStore::open_in_memory().unwrap()),
        });
        Env { mgr, conn, root, _base: base }
    }

    fn online() -> Connectivity {
        Connectivity { online: true, metered: false }
    }

    fn setup() -> Env {
        setup_with(online(), true, plenty)
    }

    /// Collects every event on a background task, so a busy run cannot lag the subscriber.
    #[derive(Clone)]
    struct Log(Arc<Mutex<Vec<DownloadEvent>>>);

    fn record(mgr: &DownloadManager) -> Log {
        let mut rx = mgr.events();
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = log.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => sink.lock().unwrap().push(event),
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Log(log)
    }

    impl Log {
        fn all(&self) -> Vec<DownloadEvent> {
            self.0.lock().unwrap().clone()
        }

        fn count(&self, pred: impl Fn(&DownloadEvent) -> bool) -> usize {
            self.0.lock().unwrap().iter().filter(|e| pred(e)).count()
        }
    }

    async fn wait_for(log: &Log, what: &str, pred: impl Fn(&[DownloadEvent]) -> bool) {
        let result = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if pred(&log.0.lock().unwrap()) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(result.is_ok(), "timed out waiting for {what}; events: {:?}", log.all());
    }

    fn is_complete(e: &DownloadEvent) -> bool {
        matches!(e, DownloadEvent::Complete)
    }

    fn source_ref(src: &Arc<FakeSource>) -> SourceRef {
        src.clone()
    }

    fn chapters(ids: &[i64]) -> Vec<ChapterRef> {
        ids.iter().map(|id| chapter(*id, 1, &format!("Chapter {id}"))).collect()
    }

    fn manga_title(source_id: i64) -> MangaRef {
        manga(1, source_id, "Berserk")
    }

    #[tokio::test]
    async fn a_queued_run_downloads_every_chapter_and_completes_once() {
        let env = setup();
        let log = record(&env.mgr);
        let src = fake(1);
        let outcome = env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1, 2]), true).await.unwrap();
        assert_eq!(outcome, EnqueueOutcome::Queued(2));
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        let events = log.all();
        assert_eq!(events.iter().filter(|e| matches!(e, DownloadEvent::Finished { .. })).count(), 2);
        assert_eq!(events.iter().filter(|e| is_complete(e)).count(), 1, "Complete is emitted once per run");
        assert!(events.iter().any(|e| matches!(e, DownloadEvent::Started)));
        assert!(env.mgr.snapshot().is_empty());
        assert!(!env.mgr.running());
        let chapter_dir = env.root.join("Source 1 (EN)").join("Berserk").join("Chapter 1");
        assert_eq!(std::fs::read_dir(&chapter_dir).unwrap().count(), 3, "three pages in {}", chapter_dir.display());
        assert!(env.mgr.is_chapter_downloaded(src.as_ref(), "Berserk", &chapter(2, 1, "Chapter 2")));
        assert_eq!(env.mgr.chapter_count(), 2);
    }

    // TST-11, TST-12
    #[tokio::test]
    async fn enqueue_filters_downloaded_and_queued_chapters_and_sorts_by_source_order() {
        let env = setup();
        let src = fake(1);
        env.mgr.index().add_chapter(1, "Source 1 (EN)", "Berserk", "Chapter 1");
        let mut list = chapters(&[1, 2, 3]);
        list.push(chapter(2, 1, "Chapter 2"));
        let outcome = env.mgr.enqueue(source_ref(&src), manga_title(1), list, false).await.unwrap();
        assert_eq!(outcome, EnqueueOutcome::Queued(2));
        let order: Vec<i64> = env.mgr.snapshot().iter().map(|d| d.chapter.id).collect();
        assert_eq!(order, vec![3, 2], "EC-02, EC-03, TST-12");
        assert!(!env.mgr.running(), "auto_start false does not start the run (EC-05)");
    }

    #[tokio::test]
    async fn local_sources_and_empty_lists_are_refused_quietly() {
        let env = setup();
        let local = Arc::new(FakeSource { http: false, ..FakeSource::new(9) });
        assert_eq!(
            env.mgr.enqueue(source_ref(&local), manga_title(9), chapters(&[1]), true).await.unwrap(),
            EnqueueOutcome::SourceNotHttp
        );
        let src = fake(1);
        assert_eq!(
            env.mgr.enqueue(source_ref(&src), manga_title(1), Vec::new(), true).await.unwrap(),
            EnqueueOutcome::NothingToDo
        );
        assert!(env.mgr.snapshot().is_empty());
    }

    #[tokio::test]
    async fn bulk_warning_needs_more_than_fifteen_from_one_source_or_thirty_queued() {
        let env = setup_with(Connectivity { online: false, metered: false }, true, plenty);
        let log = record(&env.mgr);
        let bulk = |ev: &[DownloadEvent]| ev.iter().any(|e| matches!(e, DownloadEvent::Warning { text, .. } if text == BULK_WARNING_TEXT));
        let range = |from: i64, to: i64| -> Vec<ChapterRef> { (from..=to).map(|i| chapter(i, 1, &format!("Chapter {i}"))).collect() };
        let offline = |ev: &[DownloadEvent]| ev.iter().any(|e| matches!(e, DownloadEvent::Warning { text, .. } if text == NO_NETWORK_TEXT));

        // TST-13: exactly 15 from one source is at the limit, so no warning.
        let src = fake(1);
        env.mgr.enqueue(source_ref(&src), manga(1, 1, "A"), range(1, 15), true).await.unwrap();
        wait_for(&log, "the offline stop", offline).await;
        assert!(!bulk(&log.all()), "15 from one source does not warn");
        env.mgr.clear().await.unwrap();

        // 16 from one source is over the per-source limit.
        env.mgr.enqueue(source_ref(&src), manga(1, 1, "A"), range(1, 16), true).await.unwrap();
        wait_for(&log, "the per-source warning", bulk).await;
        let warning = log.all().into_iter().find(|e| matches!(e, DownloadEvent::Warning { text, .. } if text == BULK_WARNING_TEXT)).unwrap();
        assert!(matches!(warning, DownloadEvent::Warning { timeout_secs: Some(30), help_url: Some(_), .. }), "30 s timeout and help URL");
        env.mgr.clear().await.unwrap();

        // Unmetered sources are left out of the counts (EC-06).
        let local = Arc::new(FakeSource { unmetered: true, ..FakeSource::new(5) });
        let before = log.count(|e| matches!(e, DownloadEvent::Warning { text, .. } if text == BULK_WARNING_TEXT));
        env.mgr.enqueue(source_ref(&local), manga(5, 5, "L"), range(100, 130), true).await.unwrap();
        wait_for(&log, "the offline stop for the local source", offline).await;
        let after = log.count(|e| matches!(e, DownloadEvent::Warning { text, .. } if text == BULK_WARNING_TEXT));
        assert_eq!(before, after, "31 unmetered chapters do not warn");
    }

    #[tokio::test]
    async fn pause_while_idle_reports_paused_and_keeps_the_items() {
        let env = setup();
        let log = record(&env.mgr);
        let src = fake(1);
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1, 2]), false).await.unwrap();
        env.mgr.pause().await.unwrap();
        wait_for(&log, "Paused", |ev| ev.iter().any(|e| matches!(e, DownloadEvent::Paused))).await;
        assert_eq!(env.mgr.snapshot().len(), 2, "EC-16: the queue keeps its items");
    }

    #[tokio::test]
    async fn start_on_an_empty_queue_is_a_no_op() {
        let env = setup();
        let log = record(&env.mgr);
        env.mgr.start().await.unwrap();
        // A clear is a round trip: once it reports, the earlier start has been handled.
        env.mgr.clear().await.unwrap();
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        assert_eq!(log.count(|e| matches!(e, DownloadEvent::Started)), 0, "EC-18");
    }

    #[tokio::test]
    async fn clear_dismisses_the_progress_and_completes() {
        let env = setup();
        let log = record(&env.mgr);
        let src = fake(1);
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), false).await.unwrap();
        env.mgr.clear().await.unwrap();
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        let events = log.all();
        let dismiss = events.iter().position(|e| matches!(e, DownloadEvent::Dismiss)).unwrap();
        let complete = events.iter().position(is_complete).unwrap();
        assert!(dismiss < complete, "EC-15: Dismiss then Complete");
        assert!(env.mgr.snapshot().is_empty());
    }

    // TST-15
    #[tokio::test]
    async fn a_failed_chapter_is_skipped_and_the_source_moves_on() {
        let env = setup();
        let log = record(&env.mgr);
        let src = Arc::new(FakeSource {
            page_count: 1,
            failures: Mutex::new(vec![SourceError::Http(404)].into()),
            ..FakeSource::new(1)
        });
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1, 2]), true).await.unwrap();
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        // Highest source order runs first, so chapter 2 takes the injected 404 and chapter 1 runs after it.
        let events = log.all();
        assert!(events.iter().any(|e| matches!(e, DownloadEvent::Status { chapter_id: 2, state: DownloadState::Error })));
        assert!(events.iter().any(|e| matches!(e, DownloadEvent::Finished { chapter_id: 1 })), "EC-27: the next chapter runs");
        let failed = env.mgr.snapshot();
        assert_eq!(failed.len(), 1, "the ERROR chapter stays in the queue");
        assert_eq!(failed[0].chapter.id, 2);
        assert_eq!(failed[0].status, DownloadState::Error);
        assert!(events.iter().any(|e| matches!(e, DownloadEvent::Error { chapter_id: Some(2), .. })));
    }

    // TST-14
    #[tokio::test]
    async fn at_most_five_sources_run_and_one_chapter_per_source() {
        let env = setup();
        let log = record(&env.mgr);
        for id in 1..=6 {
            let src = Arc::new(FakeSource { delay: Duration::from_millis(40), ..FakeSource::new(id) });
            let ids = [id * 10, id * 10 + 1];
            env.mgr.enqueue(source_ref(&src), manga(id, id, &format!("M{id}")), chapters(&ids), false).await.unwrap();
        }
        env.mgr.start().await.unwrap();
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        let mut downloading: HashSet<i64> = HashSet::new();
        let mut peak = 0usize;
        for e in log.all() {
            if let DownloadEvent::Status { chapter_id, state } = e {
                match state {
                    DownloadState::Downloading => {
                        downloading.insert(chapter_id);
                        peak = peak.max(downloading.len());
                    }
                    DownloadState::Downloaded | DownloadState::Error | DownloadState::Queue => {
                        downloading.remove(&chapter_id);
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(peak, 5, "EC-25: five sources at once, the sixth waits");
        assert_eq!(log.count(|e| matches!(e, DownloadEvent::Finished { .. })), 12);
    }

    #[tokio::test]
    async fn offline_stops_the_run_and_regaining_the_network_resumes_it() {
        let env = setup();
        let log = record(&env.mgr);
        let src = Arc::new(FakeSource { delay: Duration::from_millis(300), ..FakeSource::new(1) });
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), true).await.unwrap();
        wait_for(&log, "the first page", |ev| ev.iter().any(|e| matches!(e, DownloadEvent::Status { state: DownloadState::Downloading, .. }))).await;
        env.conn.send_replace(Connectivity { online: false, metered: false });
        wait_for(&log, "the network warning", |ev| ev.iter().any(|e| matches!(e, DownloadEvent::Warning { text, .. } if text == NO_NETWORK_TEXT))).await;
        assert!(!env.mgr.running(), "TST-16: the run stops");
        assert_eq!(env.mgr.snapshot()[0].status, DownloadState::Error);

        env.conn.send_replace(online());
        wait_for(&log, "Complete after resume", |ev| ev.iter().any(is_complete)).await;
        assert!(env.mgr.snapshot().is_empty(), "EC-31: the chapter downloaded after the network came back");
    }

    #[tokio::test]
    async fn a_paused_queue_does_not_resume_when_the_network_returns() {
        let env = setup();
        let log = record(&env.mgr);
        let src = Arc::new(FakeSource { delay: Duration::from_millis(300), ..FakeSource::new(1) });
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), true).await.unwrap();
        wait_for(&log, "the first page", |ev| ev.iter().any(|e| matches!(e, DownloadEvent::Status { state: DownloadState::Downloading, .. }))).await;
        env.conn.send_replace(Connectivity { online: false, metered: false });
        wait_for(&log, "the network warning", |ev| ev.iter().any(|e| matches!(e, DownloadEvent::Warning { .. }))).await;
        env.mgr.pause().await.unwrap();
        // Commands are queued, not run inline: wait until the pause has been applied before the network returns.
        wait_for(&log, "Paused", |ev| ev.iter().any(|e| matches!(e, DownloadEvent::Paused))).await;
        env.conn.send_replace(online());
        env.mgr.clear().await.unwrap();
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        assert_eq!(log.count(|e| matches!(e, DownloadEvent::Started)), 1, "EC-32: no second start after the pause");
    }

    #[tokio::test]
    async fn metered_link_with_wifi_only_stops_and_names_the_wifi_warning() {
        let env = setup_with(Connectivity { online: true, metered: true }, true, plenty);
        let log = record(&env.mgr);
        let src = fake(1);
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), true).await.unwrap();
        wait_for(&log, "the Wi-Fi warning", |ev| ev.iter().any(|e| matches!(e, DownloadEvent::Warning { text, .. } if text == NO_WIFI_TEXT))).await;
        assert_eq!(src.calls(), 0, "EC-33: nothing is fetched");
    }

    #[tokio::test]
    async fn metered_link_with_wifi_only_off_keeps_downloading() {
        let env = setup_with(Connectivity { online: true, metered: true }, false, plenty);
        let log = record(&env.mgr);
        let src = fake(1);
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), true).await.unwrap();
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        assert_eq!(log.count(|e| matches!(e, DownloadEvent::Warning { .. })), 0, "EC-34");
    }

    #[tokio::test]
    async fn low_storage_fails_each_chapter_with_the_storage_message() {
        let env = setup_with(online(), true, tiny);
        let log = record(&env.mgr);
        let src = fake(1);
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), true).await.unwrap();
        wait_for(&log, "the storage error", |ev| {
            ev.iter().any(|e| matches!(e, DownloadEvent::Error { message, .. } if message == "Couldn't download due to low storage space"))
        })
        .await;
        assert_eq!(src.calls(), 0, "FM-02: nothing is fetched");
    }

    #[tokio::test]
    async fn an_unwritable_root_halts_the_run_with_an_invalid_location_error() {
        let base = tempfile::tempdir().unwrap();
        let root_file = base.path().join("not-a-dir");
        std::fs::write(&root_file, b"x").unwrap();
        let index = Arc::new(DownloadIndex::open(&root_file, &base.path().join("cache")));
        let (_conn, rx) = watch::channel(online());
        let mgr = DownloadManager::spawn(ManagerParams {
            layout: Arc::new(DownloadLayout::new(&root_file)),
            index,
            store: Arc::new(QueueStore::open_in_memory().unwrap()),
            free_space: plenty,
            settings: DownloadSettings { save_as_cbz: false, ..DownloadSettings::default() },
            page_cache: None,
            connectivity: rx,
            hold: Arc::new(crate::downloader::NoHold),
            restored: Vec::new(),
            kept_rows: Vec::new(),
            pending: Arc::new(crate::pending::MangaPendingStore::open_in_memory().unwrap()),
        });
        let log = record(&mgr);
        let src = fake(1);
        mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), true).await.unwrap();
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        let message = log.all().into_iter().find_map(|e| match e {
            DownloadEvent::Error { message, .. } => Some(message),
            _ => None,
        });
        assert!(message.unwrap().starts_with("Invalid location: "), "FM-01");
        assert_eq!(src.calls(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn an_idle_queue_never_wakes_the_loop() {
        let env = setup();
        let src = fake(1);
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), false).await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        let before = env.mgr.wakeups();
        tokio::time::sleep(Duration::from_secs(600)).await;
        assert_eq!(env.mgr.wakeups(), before, "TST-17: no polling while idle");
    }

    #[tokio::test]
    async fn start_now_moves_a_queued_chapter_to_the_front_and_adds_a_new_one_there() {
        let env = setup();
        let src = fake(1);
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1, 2, 3]), false).await.unwrap();
        assert_eq!(env.mgr.snapshot().iter().map(|d| d.chapter.id).collect::<Vec<_>>(), vec![3, 2, 1]);
        env.mgr.reorder(vec![1]).await.unwrap();
        let order = |mgr: &DownloadManager| mgr.snapshot().iter().map(|d| d.chapter.id).collect::<Vec<i64>>();
        for _ in 0..200 {
            if order(&env.mgr) == vec![1, 3, 2] {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(order(&env.mgr), vec![1, 3, 2], "EC-09: a queued chapter moves to index 0");
        let outcome = env.mgr.start_now(source_ref(&src), manga_title(1), chapter(9, 1, "Chapter 9")).await.unwrap();
        assert_eq!(outcome, EnqueueOutcome::Queued(1));
        assert!(env.mgr.running(), "EC-10: the new chapter starts the run");
        assert!(env.mgr.queued(9).is_some());
    }

    #[tokio::test]
    async fn cancel_removes_only_the_named_chapters() {
        let env = setup();
        let src = fake(1);
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1, 2, 3]), false).await.unwrap();
        let removed = env.mgr.cancel(vec![2]).await.unwrap();
        assert_eq!(removed, vec![2]);
        let left: Vec<i64> = env.mgr.snapshot().iter().map(|d| d.chapter.id).collect();
        assert_eq!(left, vec![3, 1]);
        let other = manga(7, 1, "Other");
        env.mgr.enqueue(source_ref(&src), other, vec![chapter(9, 7, "C9")], false).await.unwrap();
        let removed = env.mgr.cancel_manga(7).await.unwrap();
        assert_eq!(removed, vec![9]);
        assert!(env.mgr.queued(9).is_none());
        assert!(env.mgr.queued(1).is_some());
    }

    // TST-47
    #[tokio::test]
    async fn delete_chapters_removes_the_folder_index_entry_and_empty_manga_and_source_dirs() {
        let env = setup();
        let src = fake(1);
        let manga_dir = env.root.join("Source 1 (EN)").join("Berserk");
        std::fs::create_dir_all(manga_dir.join("Chapter 1")).unwrap();
        env.mgr.index().add_chapter(1, "Source 1 (EN)", "Berserk", "Chapter 1");
        let deleted = env
            .mgr
            .delete_chapters(vec![chapter(1, 1, "Chapter 1")], &manga_title(1), src.as_ref(), &DeleteRules::default())
            .await
            .unwrap();
        assert_eq!(deleted, 1);
        assert!(!manga_dir.exists(), "EC-98: the emptied manga folder goes");
        assert!(!env.root.join("Source 1 (EN)").exists(), "EC-98: the emptied source folder goes");
        assert_eq!(env.mgr.chapter_count(), 0);
    }

    #[tokio::test]
    async fn a_source_that_is_not_installed_deletes_through_the_folder_the_index_knows() {
        let env = setup();
        let manga_dir = env.root.join("Gone Source (EN)").join("Berserk");
        std::fs::create_dir_all(manga_dir.join("Chapter 1")).unwrap();
        std::fs::create_dir_all(manga_dir.join("Chapter 2")).unwrap();
        env.mgr.index().add_chapter(1, "Gone Source (EN)", "Berserk", "Chapter 1");
        env.mgr.index().add_chapter(1, "Gone Source (EN)", "Berserk", "Chapter 2");
        let folder = env.mgr.index().manga_source_dir(1, "Berserk").expect("the index knows the folder");
        let src = crate::source::FolderOnlySource { id: 1, folder };
        let deleted = env
            .mgr
            .delete_chapters(vec![chapter(1, 1, "Chapter 1")], &manga_title(1), &src, &DeleteRules::default())
            .await
            .unwrap();
        assert_eq!(deleted, 1);
        assert!(!manga_dir.join("Chapter 1").exists() && manga_dir.join("Chapter 2").is_dir());
        assert_eq!(env.mgr.index().manga_source_dir(2, "Berserk"), None, "another source has no folder");
    }

    #[tokio::test]
    async fn delete_chapters_keeps_bookmarked_chapters_by_default() {
        let env = setup();
        let src = fake(1);
        let manga_dir = env.root.join("Source 1 (EN)").join("Berserk");
        std::fs::create_dir_all(manga_dir.join("Chapter 1")).unwrap();
        env.mgr.index().add_chapter(1, "Source 1 (EN)", "Berserk", "Chapter 1");
        let bookmarked = ChapterRef { bookmark: true, ..chapter(1, 1, "Chapter 1") };
        let deleted = env
            .mgr
            .delete_chapters(vec![bookmarked], &manga_title(1), src.as_ref(), &DeleteRules::default())
            .await
            .unwrap();
        assert_eq!(deleted, 0);
        assert!(manga_dir.join("Chapter 1").is_dir());
    }

    #[tokio::test]
    async fn delete_manga_removes_the_manga_folder_and_the_source_when_empty() {
        let env = setup();
        let src = fake(1);
        let manga_dir = env.root.join("Source 1 (EN)").join("Berserk");
        std::fs::create_dir_all(manga_dir.join("Chapter 1")).unwrap();
        env.mgr.index().add_chapter(1, "Source 1 (EN)", "Berserk", "Chapter 1");
        env.mgr.delete_manga(&manga_title(1), src.as_ref(), true).await.unwrap();
        assert!(!env.root.join("Source 1 (EN)").exists());
        assert_eq!(env.mgr.chapter_count_for_manga(fake(1).as_ref(), "Berserk"), 0);
    }

    #[tokio::test]
    async fn delete_manga_keeps_a_source_folder_that_has_other_manga() {
        let env = setup();
        let src = fake(1);
        let source_dir = env.root.join("Source 1 (EN)");
        std::fs::create_dir_all(source_dir.join("Berserk").join("Chapter 1")).unwrap();
        std::fs::create_dir_all(source_dir.join("Vagabond")).unwrap();
        env.mgr.delete_manga(&manga_title(1), src.as_ref(), true).await.unwrap();
        assert!(source_dir.join("Vagabond").is_dir(), "EC-101");
    }

    // TST-50
    #[tokio::test]
    async fn deleting_a_running_chapter_pauses_restarts_and_leaves_the_others() {
        let env = setup();
        let log = record(&env.mgr);
        let src = Arc::new(FakeSource { delay: Duration::from_millis(100), ..FakeSource::new(1) });
        let title = manga_title(1);
        env.mgr.enqueue(source_ref(&src), title.clone(), chapters(&[1, 2]), true).await.unwrap();
        wait_for(&log, "chapter 1 downloading", |ev| ev.iter().any(|e| matches!(e, DownloadEvent::Status { chapter_id: 1, state: DownloadState::Downloading }))).await;
        let selected = env
            .mgr
            .delete_chapters(vec![chapter(1, 1, "Chapter 1")], &title, src.as_ref(), &DeleteRules::default())
            .await
            .unwrap();
        assert_eq!(selected, 1);
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        let finished: Vec<i64> = log
            .all()
            .iter()
            .filter_map(|e| match e {
                DownloadEvent::Finished { chapter_id } => Some(*chapter_id),
                _ => None,
            })
            .collect();
        assert_eq!(finished, vec![2], "RC-04: the removed chapter is not written afterwards");
    }

    #[tokio::test]
    async fn progress_updates_are_coalesced_to_the_fifty_millisecond_interval() {
        let env = setup();
        let log = record(&env.mgr);
        let src = Arc::new(FakeSource { page_count: 40, delay: Duration::from_millis(10), ..FakeSource::new(1) });
        let started = std::time::Instant::now();
        env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), true).await.unwrap();
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        let elapsed = started.elapsed().as_secs_f64();
        let progress = log.count(|e| matches!(e, DownloadEvent::Progress { .. })) as f64;
        assert!(progress <= 20.0 * elapsed + 2.0, "RC-05: {progress} progress events in {elapsed:.2} s");
    }
    #[tokio::test]
    async fn delete_manga_without_remove_queued_pauses_the_running_chapter_and_keeps_the_queue() {
        let env = setup();
        let log = record(&env.mgr);
        let src = Arc::new(FakeSource { delay: Duration::from_millis(100), ..FakeSource::new(1) });
        let title = manga_title(1);
        env.mgr.enqueue(source_ref(&src), title.clone(), chapters(&[1, 2]), true).await.unwrap();
        wait_for(&log, "a chapter downloading", |ev| {
            ev.iter().any(|e| matches!(e, DownloadEvent::Status { state: DownloadState::Downloading, .. }))
        })
        .await;
        env.mgr.delete_manga(&title, src.as_ref(), false).await.unwrap();
        wait_for(&log, "Complete", |ev| ev.iter().any(is_complete)).await;
        let events = log.all();
        let downloading = events.iter().position(|e| matches!(e, DownloadEvent::Status { state: DownloadState::Downloading, .. })).unwrap();
        assert!(
            events[downloading..].iter().any(|e| matches!(e, DownloadEvent::Status { state: DownloadState::Queue, .. })),
            "RC-11: the running chapter goes back to the queue before the folder is deleted"
        );
        assert_eq!(log.count(|e| matches!(e, DownloadEvent::Finished { .. })), 2, "the chapters stayed queued and ran after the delete");
        assert_eq!(log.count(|e| matches!(e, DownloadEvent::Error { .. })), 0, "RC-11: no chapter wrote into the deleted folder");
    }

    // TST-48
    #[tokio::test]
    async fn rename_source_case_only_two_step() {
        let env = setup();
        let old = Arc::new(FakeSource { name: Some("source 1 (en)".into()), ..FakeSource::new(1) });
        let new = fake(1);
        let chapter_dir = env.root.join("source 1 (en)").join("Berserk").join("Chapter 1");
        std::fs::create_dir_all(&chapter_dir).unwrap();
        std::fs::write(chapter_dir.join("001.jpg"), b"1234").unwrap();
        env.mgr.index().add_chapter(1, "source 1 (en)", "Berserk", "Chapter 1");
        env.mgr.rename_source(old.as_ref(), new.as_ref()).await.unwrap();
        assert!(env.root.join("Source 1 (EN)").join("Berserk").join("Chapter 1").join("001.jpg").is_file(), "EC-102");
        assert!(!env.root.join("source 1 (en)").exists());
        assert!(!env.root.join("Source 1 (EN)_tmp").exists(), "the staging name does not stay");
        assert_eq!(env.mgr.size_for_manga(fake(1).as_ref(), "Berserk").await, 4, "the index points at the renamed folder");
    }

    #[tokio::test]
    async fn rename_source_onto_an_existing_folder_fails_and_keeps_the_old_one() {
        let env = setup();
        let old = Arc::new(FakeSource { name: Some("Old (EN)".into()), ..FakeSource::new(1) });
        std::fs::create_dir_all(env.root.join("Old (EN)")).unwrap();
        std::fs::create_dir_all(env.root.join("Source 1 (EN)")).unwrap();
        let result = env.mgr.rename_source(old.as_ref(), fake(1).as_ref()).await;
        assert!(result.is_err());
        assert!(env.root.join("Old (EN)").is_dir(), "the old folder stays");
    }

    #[tokio::test]
    async fn rename_chapter_cbz_keeps_the_extension_and_stores_the_bare_name() {
        let env = setup();
        let src = fake(1);
        let manga_dir = env.root.join("Source 1 (EN)").join("Berserk");
        std::fs::create_dir_all(&manga_dir).unwrap();
        std::fs::write(manga_dir.join("Old.cbz"), b"x").unwrap();
        env.mgr.index().add_chapter(1, "Source 1 (EN)", "Berserk", "Old");
        env.mgr
            .rename_chapter(src.as_ref(), &manga_title(1), &chapter(1, 1, "Old"), &chapter(1, 1, "New"))
            .await
            .unwrap();
        assert!(manga_dir.join("New.cbz").is_file(), "EC-103: the new name ends in .cbz");
        assert!(!manga_dir.join("Old.cbz").exists());
        assert!(env.mgr.is_chapter_downloaded(src.as_ref(), "Berserk", &chapter(1, 1, "New")));
        assert!(!env.mgr.is_chapter_downloaded(src.as_ref(), "Berserk", &chapter(1, 1, "Old")));
        assert_eq!(env.mgr.chapter_count_for_manga(fake(1).as_ref(), "Berserk"), 1, "the index holds the bare name once");
    }

    #[tokio::test]
    async fn rename_chapter_fails_on_an_existing_target_and_ignores_a_chapter_that_is_not_downloaded() {
        let env = setup();
        let src = fake(1);
        let manga_dir = env.root.join("Source 1 (EN)").join("Berserk");
        std::fs::create_dir_all(manga_dir.join("A")).unwrap();
        std::fs::create_dir_all(manga_dir.join("B")).unwrap();
        let clash = env.mgr.rename_chapter(src.as_ref(), &manga_title(1), &chapter(1, 1, "A"), &chapter(1, 1, "B")).await;
        assert!(clash.is_err(), "EC-104");
        assert!(manga_dir.join("A").is_dir() && manga_dir.join("B").is_dir(), "EC-104: the old folder stays");
        let other = manga(2, 1, "Vagabond");
        env.mgr.rename_chapter(src.as_ref(), &other, &chapter(2, 2, "A"), &chapter(2, 2, "C")).await.unwrap();
        assert!(!env.root.join("Source 1 (EN)").join("Vagabond").exists(), "EC-105: nothing is created");
    }

    /// A stored zip with the given entries, written by hand so the test needs no zip crate.
    fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        fn crc32(data: &[u8]) -> u32 {
            let mut crc = !0u32;
            for &b in data {
                crc ^= u32::from(b);
                for _ in 0..8 {
                    crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
                }
            }
            !crc
        }
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in entries {
            let offset = out.len() as u32;
            let (crc, len, name_len) = (crc32(data), data.len() as u32, name.len() as u16);
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
            for n in [crc, len, len] {
                out.extend_from_slice(&n.to_le_bytes());
            }
            out.extend_from_slice(&name_len.to_le_bytes());
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
            for n in [crc, len, len] {
                central.extend_from_slice(&n.to_le_bytes());
            }
            central.extend_from_slice(&name_len.to_le_bytes());
            central.extend_from_slice(&[0; 12]);
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_offset = out.len() as u32;
        let count = entries.len() as u16;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    #[tokio::test]
    async fn build_page_list_sorts_the_images_of_a_folder_and_skips_other_files() {
        let env = setup();
        let src = fake(1);
        let manga_dir = env.root.join("Source 1 (EN)").join("Berserk");
        let dir = manga_dir.join("Chapter 1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("002.jpg"), crate::testing::jpeg_bytes(16)).unwrap();
        std::fs::write(dir.join("001.jpg"), crate::testing::jpeg_bytes(16)).unwrap();
        std::fs::write(dir.join("ComicInfo.xml"), b"<ComicInfo/>").unwrap();
        std::fs::create_dir_all(manga_dir.join("Chapter 2")).unwrap();
        std::fs::write(manga_dir.join("Chapter 2").join("notes.txt"), b"text").unwrap();
        let pages = env.mgr.build_page_list(src.as_ref(), &manga_title(1), &chapter(1, 1, "Chapter 1")).await.unwrap();
        let names: Vec<_> = pages.iter().map(|p| p.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["001.jpg", "002.jpg"]);
        assert_eq!(pages.iter().map(|p| p.index).collect::<Vec<_>>(), vec![0, 1]);
        assert!(pages.iter().all(|p| p.is_cbz_entry.is_none()));
        for missing in ["Chapter 2", "Chapter 3"] {
            let result = env.mgr.build_page_list(src.as_ref(), &manga_title(1), &chapter(2, 1, missing)).await;
            assert!(matches!(result, Err(DownloadError::NoPages)), "EC-37: {missing}: {result:?}");
        }
    }

    #[tokio::test]
    async fn build_page_list_reads_the_image_entries_of_a_cbz_in_natural_order() {
        let env = setup();
        let src = fake(1);
        let manga_dir = env.root.join("Source 1 (EN)").join("Berserk");
        std::fs::create_dir_all(&manga_dir).unwrap();
        let jpeg = crate::testing::jpeg_bytes(16);
        let zip = stored_zip(&[("10.jpg", &jpeg), ("2.jpg", &jpeg), ("readme.txt", b"hello")]);
        std::fs::write(manga_dir.join("Chapter 1.cbz"), zip).unwrap();
        let pages = env.mgr.build_page_list(src.as_ref(), &manga_title(1), &chapter(1, 1, "Chapter 1")).await.unwrap();
        let entries: Vec<_> = pages.iter().map(|p| p.is_cbz_entry.clone().unwrap()).collect();
        assert_eq!(entries, vec!["2.jpg", "10.jpg"]);
        assert!(pages.iter().all(|p| p.path == manga_dir.join("Chapter 1.cbz")));
    }

    #[tokio::test]
    async fn renew_index_finds_chapters_on_disk_so_they_are_not_queued_again() {
        let env = setup();
        let src = fake(1);
        let dir = env.root.join("Source 1 (EN)").join("Berserk").join("Chapter 1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("001.jpg"), crate::testing::jpeg_bytes(16)).unwrap();
        let (_gate, ready) = watch::channel(true);
        assert!(env.mgr.renew_index(ready, || vec![(1, "Source 1 (EN)".to_owned())]).await, "EC-80: the first start scans");
        assert_eq!(env.mgr.chapter_count(), 1);
        let outcome = env.mgr.enqueue(source_ref(&src), manga_title(1), chapters(&[1]), false).await.unwrap();
        assert_eq!(outcome, EnqueueOutcome::NothingToDo, "a chapter on disk is not queued again");
        assert!(env.mgr.snapshot().is_empty());
    }

    #[tokio::test]
    async fn the_series_lock_serialises_one_series_and_leaves_the_others_free() {
        let locks = SeriesLocks::default();
        let held = locks.lock(Kind::Manga, 1).await;
        let other = tokio::time::timeout(Duration::from_millis(50), locks.lock(Kind::Manga, 2)).await;
        assert!(other.is_ok(), "another series is not blocked");
        let same = tokio::time::timeout(Duration::from_millis(50), locks.lock(Kind::Manga, 1)).await;
        assert!(same.is_err(), "E-22: the same series waits for the first delete");
        drop(held);
        let again = tokio::time::timeout(Duration::from_millis(500), locks.lock(Kind::Manga, 1)).await;
        assert!(again.is_ok(), "the lock is released when the guard drops");
    }

    // TST-19
    #[tokio::test]
    async fn tst_19_a_cancel_mid_write_leaves_the_tmp_folder_and_no_archive() {
        let env = setup();
        let src = fake(1);
        let manga = manga_title(1);
        let dir = env
            .root
            .join(DownloadLayout::source_dir_name(&src.display_name()))
            .join(DownloadLayout::manga_dir_name(&manga.title))
            .join("Chapter 1_tmp");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("001.jpg"), b"partial").unwrap();
        env.mgr.enqueue(source_ref(&src), manga, chapters(&[1]), false).await.unwrap();
        assert_eq!(env.mgr.cancel(vec![1]).await.unwrap(), vec![1]);
        assert!(env.mgr.queued(1).is_none(), "EC-21: the chapter leaves the queue");
        assert!(dir.join("001.jpg").is_file(), "EC-21: the _tmp folder stays");
        assert!(!dir.with_file_name("Chapter 1.cbz").exists(), "no archive is written");
    }

    #[tokio::test]
    async fn a_change_under_the_root_marks_the_index_stale_after_the_quiet_period() {
        let env = setup();
        let (_ready, rx) = watch::channel(true);
        env.mgr.renew_index(rx, Vec::new).await;
        assert!(env.mgr.index().last_renew_ms().is_some());
        std::fs::create_dir(env.root.join("New Source")).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(env.mgr.index().last_renew_ms().is_some(), "T-23a-045: the quiet period has not passed yet");
        let stale = tokio::time::timeout(Duration::from_secs(10), async {
            while env.mgr.index().last_renew_ms().is_some() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        assert!(stale.is_ok(), "T-23a-045: the watcher invalidated the index");
    }

    #[tokio::test]
    async fn the_local_source_counts_and_sizes_its_folder_and_other_sources_use_the_index() {
        let env = setup();
        let local_root = env._base.path().join("local");
        let manga_dir = local_root.join("Berserk");
        std::fs::create_dir_all(manga_dir.join("Chapter 1")).unwrap();
        std::fs::write(manga_dir.join("Chapter 1").join("001.jpg"), vec![0u8; 100]).unwrap();
        std::fs::write(manga_dir.join("Chapter 2.cbz"), vec![0u8; 20]).unwrap();
        std::fs::write(manga_dir.join("notes.txt"), vec![0u8; 5]).unwrap();
        let local = FakeSource { http: false, local_dir: Some(local_root), ..FakeSource::new(0) };
        assert_eq!(env.mgr.chapter_count_for_manga(&local, "Berserk"), 2, "a folder and an archive");
        assert_eq!(env.mgr.size_for_manga(&local, "Berserk").await, 125, "recursive, every file");

        env.mgr.index().add_chapter(1, "Source 1 (EN)", "Berserk", "Chapter 1");
        assert_eq!(env.mgr.chapter_count_for_manga(fake(1).as_ref(), "Berserk"), 1, "an http source reads the index");
    }

    struct OneSource(Option<SourceRef>);

    impl ChapterSourceResolver for OneSource {
        fn chapter_source(&self, source_id: i64) -> Option<SourceRef> {
            self.0.clone().filter(|s| s.id() == source_id)
        }
    }

    #[tokio::test]
    async fn pending_deletes_wait_for_the_flush_and_then_remove_the_folder() {
        let env = setup();
        let src = fake(1);
        let manga_dir = env.root.join("Source 1 (EN)").join("Berserk");
        for name in ["Chapter 1", "Chapter 2"] {
            std::fs::create_dir_all(manga_dir.join(name)).unwrap();
            env.mgr.index().add_chapter(1, "Source 1 (EN)", "Berserk", name);
        }
        let rules = DeleteRules::default();
        let live = |id: i64| (id == 1).then(DeleteRules::default);
        let m = manga_title(1);
        assert_eq!(env.mgr.defer_delete(&m, &chapters(&[1]), &rules).unwrap(), 1);
        assert_eq!(env.mgr.defer_delete(&m, &chapters(&[1]), &rules).unwrap(), 0, "EC-90: one row per chapter");
        assert!(manga_dir.join("Chapter 1").is_dir(), "nothing is deleted while the reader is open");

        let gone = OneSource(None);
        assert_eq!(env.mgr.flush_pending_deletes(&gone, live).await.unwrap(), 0);
        assert!(manga_dir.join("Chapter 1").is_dir(), "a source that is not installed keeps the row for later");

        let sources = OneSource(Some(src.clone()));
        assert_eq!(env.mgr.flush_pending_deletes(&sources, live).await.unwrap(), 1);
        assert!(!manga_dir.join("Chapter 1").exists(), "the flush deletes the chapter folder");
        assert!(manga_dir.join("Chapter 2").is_dir(), "other chapters stay");
        assert_eq!(env.mgr.flush_pending_deletes(&sources, live).await.unwrap(), 0, "the rows were taken");
    }

    #[tokio::test]
    async fn the_flush_drops_the_rows_of_a_manga_that_is_gone() {
        let env = setup();
        let src = fake(1);
        let manga_dir = env.root.join("Source 1 (EN)").join("Berserk");
        std::fs::create_dir_all(manga_dir.join("Chapter 1")).unwrap();
        env.mgr.defer_delete(&manga_title(1), &chapters(&[1]), &DeleteRules::default()).unwrap();
        let sources = OneSource(Some(src.clone()));
        assert_eq!(env.mgr.flush_pending_deletes(&sources, |_| None).await.unwrap(), 0);
        assert!(manga_dir.join("Chapter 1").is_dir(), "EC-93: a missing manga is not deleted");
        assert_eq!(env.mgr.flush_pending_deletes(&sources, |_| Some(DeleteRules::default())).await.unwrap(), 0, "EC-93: and its rows are gone");
        assert!(manga_dir.join("Chapter 1").is_dir());
    }
}
