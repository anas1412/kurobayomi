//! The anime store runner: one tokio task consumes `AnimeCmd`, every result goes back as an `AnimeEvent` through an
//! `async-channel`, and the GTK side drains it. `Shutdown` (the page was popped) aborts the live subscriptions and
//! cancels the page's `CancellationToken`, which ends a refresh that is still fetching. Write commands run in their
//! own task, on the blocking pool, and never look at the token, so a pop does not cancel them. No GTK here.
//! Upstream: `AnimeScreenModel.kt:204-283` (init), `fetchAllFromSource`.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use async_channel::{Receiver, Sender};
use kuro_data::anime::Anime;
use kuro_data::episode::Episode;
use kuro_data::source::{AnimeStubSourceRepo, StubSource};
use kuro_db::handle::Db;
use kuro_domain::download::filter::{CategoryDecision, should_download_new};
use kuro_domain::download::prefs::DownloadPrefs;
use kuro_prefs::store::PrefStore;
use tokio::runtime::Handle;
use tokio::task::AbortHandle;
use tokio_util::sync::CancellationToken;

use super::actions::Mark;
use super::entry_state::RefreshPlan;
use super::library::{RemoveOutcome, add_to_library_flag, remove_from_library};
use super::model::{AnimeEntryModel, AnimeLoadContext};
use super::refresh::{Begin, RefreshGate, Trigger, auto_download_targets, last_empty_open_fetch, record_empty_open_fetch};
use super::store::AnimeStore;
use crate::entry_core::detail::SourceInfo;
use crate::entry_core::errors::DetailError;

/// What a fetch hands back: the episodes the sync added, which a manual refresh queues for download (T-REF-4).
pub type AnimeFetch = Pin<Box<dyn Future<Output = Result<Vec<Episode>, DetailError>> + Send>>;

/// What the page needs from outside its own database: the source line and the source fetch.
pub trait AnimeBackend: Send + Sync + 'static {
    /// The source line of the info box. `stub` is true when the extension is not installed.
    fn source_info(&self, source_id: i64) -> SourceInfo;
    /// `fetchAllFromSource`: fetches what `plan` names and stores it, so the page sees it through the subscriptions.
    /// `trigger` says whether the user asked (`manualFetch`). It returns the episodes the sync added.
    fn fetch(&self, anime_id: i64, plan: RefreshPlan, trigger: Trigger) -> AnimeFetch;
    /// Queues new episodes for download. Only a manual refresh reaches this (E-REF-5). The app has no anime download
    /// queue wired yet, so the default does nothing.
    fn enqueue_new_episodes(&self, _anime_id: i64, _episodes: &[Episode]) {}
    /// Deletes the cover files of an entry that just left the library (`removeCovers`), after the write succeeded.
    /// `true` when a file was removed.
    fn remove_cover(&self, _anime_id: i64) -> bool {
        false
    }
    /// How many downloaded episodes the entry has.
    fn downloaded_count(&self, _anime_id: i64) -> usize {
        0
    }
    /// Deletes every download of the entry.
    fn delete_downloads(&self, _anime_id: i64) {}
    /// Plays an episode in an external player. `false` when it cannot (the app resolves no video URL yet): the page
    /// then opens the internal player.
    fn play_external(&self, _anime_id: i64, _episode_id: i64) -> bool {
        false
    }
    /// Whether the source serves torrents (`isSourceForTorrents`).
    fn is_torrent_source(&self, _source_id: i64) -> bool {
        false
    }
    /// Starts the torrent server before a torrent episode plays (17.5).
    fn start_torrent_server(&self) {}
}

/// The backend of an app with no anime source registry yet: every source reads as not installed, the local source
/// reads as installed, and every fetch reports that the source is missing.
pub struct StubAnimeBackend {
    stubs: AnimeStubSourceRepo,
}

impl StubAnimeBackend {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            stubs: AnimeStubSourceRepo::new(db),
        }
    }
}

impl AnimeBackend for StubAnimeBackend {
    fn source_info(&self, source_id: i64) -> SourceInfo {
        if source_id == kuro_source::local::LOCAL_SOURCE_ID {
            return SourceInfo {
                name: kuro_source::local::LOCAL_ANIME_NAME.to_owned(),
                stub: false,
            };
        }
        let stub = self
            .stubs
            .get(source_id)
            .ok()
            .flatten()
            .unwrap_or_else(|| StubSource::fallback(source_id));
        SourceInfo {
            name: stub.display_name(),
            stub: true,
        }
    }

    fn fetch(&self, _anime_id: i64, _plan: RefreshPlan, _trigger: Trigger) -> AnimeFetch {
        Box::pin(async { Err(DetailError::SourceNotInstalled) })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimeCmd {
    Load { anime_id: i64 },
    /// A manual refresh: every part of the entry is fetched again, and the new episodes may auto-download.
    Refresh { anime_id: i64 },
    SetFlagBits { anime_id: i64, flag: i64, mask: i64 },
    SortBy { anime_id: i64, mode: i64 },
    /// The library button: removes a favorite, or adds an entry that is not one, with the library defaults.
    ToggleFavorite { anime_id: i64 },
    /// Sets or clears the bookmark or fillermark of exactly these episodes (`actions::bulk_mark_targets`).
    SetMark { ids: Vec<i64>, mark: Mark, on: bool },
    /// Marks these episodes seen or unseen.
    SetSeen { ids: Vec<i64>, seen: bool },
    /// Deletes every download of the entry (the Delete button of the prompt).
    DeleteDownloads { anime_id: i64 },
    Shutdown,
}

#[derive(Debug)]
pub enum AnimeEvent {
    /// The load finished: the model, or `None` when the row does not exist.
    Loaded(Option<Box<AnimeEntryModel>>),
    /// The live subscription published a newer entry and episode list.
    Snapshot { anime: Box<Anime>, episodes: Vec<Episode> },
    /// The refresh started by the first open ended, with or without an error.
    RefreshEnded(Result<(), DetailError>),
    /// The entry joined the library, with the library defaults written to its flags.
    Added,
    /// The entry left the library. A failed write sends nothing (E-LIB-9).
    Removed(RemoveOutcome),
    /// A store read or write failed.
    Failed(String),
}

/// The page's handle on the runner. Dropping it stops the runner and the subscriptions.
pub struct AnimeRunner {
    cmds: Sender<AnimeCmd>,
    events: Receiver<AnimeEvent>,
    cancel: CancellationToken,
}

impl AnimeRunner {
    /// Queues a command. The queue is unbounded, so this never blocks the GTK thread.
    pub fn send(&self, cmd: AnimeCmd) {
        let _ = self.cmds.try_send(cmd);
    }

    pub fn events(&self) -> Receiver<AnimeEvent> {
        self.events.clone()
    }
}

impl Drop for AnimeRunner {
    fn drop(&mut self) {
        self.cancel.cancel();
        let _ = self.cmds.try_send(AnimeCmd::Shutdown);
    }
}

struct Shared {
    store: AnimeStore,
    prefs: PrefStore,
    backend: Arc<dyn AnimeBackend>,
    ctx: AnimeLoadContext,
    events: Sender<AnimeEvent>,
    cancel: CancellationToken,
    subscriptions: Mutex<Vec<AbortHandle>>,
    gate: Mutex<RefreshGate>,
}

/// Starts the runner on `handle`.
pub fn spawn(
    handle: &Handle,
    store: AnimeStore,
    prefs: PrefStore,
    backend: Arc<dyn AnimeBackend>,
    ctx: AnimeLoadContext,
) -> AnimeRunner {
    let (cmd_tx, cmd_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();
    let cancel = CancellationToken::new();
    let shared = Arc::new(Shared {
        store,
        prefs,
        backend,
        ctx,
        events: event_tx,
        cancel: cancel.clone(),
        subscriptions: Mutex::new(Vec::new()),
        gate: Mutex::default(),
    });
    handle.spawn(run(shared, cmd_rx));
    AnimeRunner {
        cmds: cmd_tx,
        events: event_rx,
        cancel,
    }
}

async fn run(shared: Arc<Shared>, cmds: Receiver<AnimeCmd>) {
    while let Ok(cmd) = cmds.recv().await {
        match cmd {
            AnimeCmd::Shutdown => {
                shared.cancel.cancel();
                break;
            }
            AnimeCmd::Load { anime_id } => shared.clone().load(anime_id).await,
            AnimeCmd::Refresh { anime_id } => {
                let all = RefreshPlan { info: true, episodes: true, seasons: true };
                tokio::spawn(shared.clone().refresh(anime_id, all, Trigger::Manual));
            }
            write => {
                tokio::spawn(shared.clone().write(write));
            }
        }
    }
    shared.drop_subscriptions();
}

/// Unix time in milliseconds, for the empty-list guard of the open refresh.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

impl Shared {
    fn emit(&self, event: AnimeEvent) {
        let _ = self.events.try_send(event);
    }

    fn drop_subscriptions(&self) {
        for handle in self.subscriptions.lock().unwrap_or_else(PoisonError::into_inner).drain(..) {
            handle.abort();
        }
    }

    /// Runs blocking store work on the blocking pool. The work keeps running if the awaiting task is aborted.
    async fn blocking<T: Send + 'static>(&self, work: impl FnOnce(AnimeStore) -> T + Send + 'static) -> T {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || work(store)).await.expect("store work does not panic")
    }

    /// The load sequence (`AnimeEntryModel::load`), then the live subscription, then the first-open refresh.
    async fn load(self: Arc<Self>, anime_id: i64) {
        let ctx = AnimeLoadContext {
            last_empty_open_fetch_ms: last_empty_open_fetch(anime_id),
            now_ms: now_ms(),
            ..self.ctx
        };
        let (prefs, backend) = (self.prefs.clone(), Arc::clone(&self.backend));
        let loaded = self
            .blocking(move |store| {
                AnimeEntryModel::load(&store, &prefs, anime_id, ctx, |source| backend.source_info(source))
            })
            .await;
        match loaded {
            Ok(Some(model)) => {
                let plan = model.plan;
                if plan.episodes {
                    record_empty_open_fetch(anime_id, ctx.now_ms);
                }
                self.emit(AnimeEvent::Loaded(Some(Box::new(model))));
                self.subscribe(anime_id);
                if plan.any() {
                    tokio::spawn(self.clone().refresh(anime_id, plan, Trigger::Open));
                }
            }
            Ok(None) => self.emit(AnimeEvent::Loaded(None)),
            Err(e) => {
                self.emit(AnimeEvent::Failed(e.to_string()));
                self.emit(AnimeEvent::Loaded(None));
            }
        }
    }

    /// Step 10 and 11 of the init block, or a manual refresh. Single-flight per entry: a request that arrives while one
    /// runs joins it and fetches nothing (E-CONC-1), and the running one reports the end. A cancelled refresh ends
    /// silently: nobody is left to read its event.
    async fn refresh(self: Arc<Self>, anime_id: i64, plan: RefreshPlan, trigger: Trigger) {
        if self.gate.lock().unwrap_or_else(PoisonError::into_inner).begin(anime_id) == Begin::Joined {
            return;
        }
        let fetched = tokio::select! {
            () = self.cancel.cancelled() => {
                self.gate.lock().unwrap_or_else(PoisonError::into_inner).end(anime_id);
                return;
            }
            result = self.backend.fetch(anime_id, plan, trigger) => result,
        };
        self.gate.lock().unwrap_or_else(PoisonError::into_inner).end(anime_id);
        let result = match fetched {
            Ok(new_episodes) => {
                self.auto_download(anime_id, trigger, new_episodes).await;
                Ok(())
            }
            Err(e) => Err(e),
        };
        self.emit(AnimeEvent::RefreshEnded(result));
    }

    /// Queues the new episodes of a manual refresh when the preferences and the entry's categories allow it (T-REF-4).
    /// The open-time refresh never gets here with episodes to queue (E-REF-5).
    async fn auto_download(&self, anime_id: i64, trigger: Trigger, new_episodes: Vec<Episode>) {
        if new_episodes.is_empty() {
            return;
        }
        let prefs = self.prefs.clone();
        let targets = self
            .blocking(move |store| -> kuro_data::DataResult<Vec<Episode>> {
                let downloads = DownloadPrefs::new(&prefs);
                let favorite = store.anime(anime_id)?.is_some_and(|a| a.favorite);
                let rule = CategoryDecision {
                    included: downloads.download_new_episode_categories.get().0,
                    excluded: downloads.download_new_episode_categories_exclude.get().0,
                };
                let eligible = should_download_new(favorite, &store.category_ids(anime_id)?, &rule);
                Ok(auto_download_targets(
                    trigger,
                    downloads.download_new_episodes.get(),
                    eligible,
                    downloads.download_new_unseen_episodes_only.get(),
                    &new_episodes,
                    &store.episodes(anime_id)?,
                ))
            })
            .await;
        match targets {
            Ok(targets) if !targets.is_empty() => {
                let backend = Arc::clone(&self.backend);
                let _ = tokio::task::spawn_blocking(move || backend.enqueue_new_episodes(anime_id, &targets)).await;
            }
            Ok(_) => {}
            Err(e) => self.emit(AnimeEvent::Failed(e.to_string())),
        }
    }

    /// The library button. A favorite is removed: the write first, then the cover and the download count. An entry
    /// that is not one is added with the library defaults written to its flags (T-39b-012). A failed write does
    /// nothing else (E-LIB-9).
    async fn toggle_favorite(&self, anime_id: i64) {
        let prefs = self.prefs.clone();
        let now = now_ms();
        let written = self
            .blocking(move |store| -> kuro_data::DataResult<Option<bool>> {
                let Some(anime) = store.anime(anime_id)? else { return Ok(None) };
                if anime.favorite {
                    Ok(store.set_favorite(anime_id, false, now)?.then_some(false))
                } else {
                    Ok(store.add_to_library(anime_id, &prefs, now)?.then_some(add_to_library_flag()))
                }
            })
            .await;
        match written {
            Ok(Some(true)) => self.emit(AnimeEvent::Added),
            Ok(Some(false)) => {
                let backend = Arc::clone(&self.backend);
                let (covers_changed, downloads) = tokio::task::spawn_blocking(move || (backend.remove_cover(anime_id), backend.downloaded_count(anime_id)))
                    .await
                    .expect("backend work does not panic");
                if let Some(outcome) = remove_from_library(true, covers_changed, downloads) {
                    self.emit(AnimeEvent::Removed(outcome));
                }
            }
            Ok(None) => {}
            Err(e) => {
                // the favorite write failed: no cover is deleted and no prompt shows
                self.emit(AnimeEvent::Failed(e.to_string()));
            }
        }
    }

    /// Two tasks, one per table, each publishing a snapshot of the pair when either side changes.
    fn subscribe(self: &Arc<Self>, anime_id: i64) {
        type Latest = (Option<Anime>, Option<Vec<Episode>>);
        let latest: Arc<Mutex<Latest>> = Arc::default();
        let mut anime_sub = self.store.watch_anime(anime_id);
        let mut episode_sub = self.store.watch_episodes(anime_id);
        let publish = {
            let (shared, latest) = (Arc::clone(self), Arc::clone(&latest));
            move || {
                let guard = latest.lock().unwrap_or_else(PoisonError::into_inner);
                if let (Some(anime), Some(episodes)) = (&guard.0, &guard.1) {
                    shared.emit(AnimeEvent::Snapshot {
                        anime: Box::new(anime.clone()),
                        episodes: episodes.clone(),
                    });
                }
            }
        };
        let anime_task = tokio::spawn({
            let (latest, publish) = (Arc::clone(&latest), publish.clone());
            async move {
                while let Some(published) = anime_sub.changed().await {
                    if let Ok(Some(anime)) = published.map(|p| p.as_ref().clone()) {
                        latest.lock().unwrap_or_else(PoisonError::into_inner).0 = Some(anime);
                        publish();
                    }
                }
            }
        });
        let episode_task = tokio::spawn(async move {
            while let Some(published) = episode_sub.changed().await {
                if let Ok(list) = published {
                    latest.lock().unwrap_or_else(PoisonError::into_inner).1 = Some(list.as_ref().clone());
                    publish();
                }
            }
        });
        let mut subs = self.subscriptions.lock().unwrap_or_else(PoisonError::into_inner);
        subs.push(anime_task.abort_handle());
        subs.push(episode_task.abort_handle());
    }

    /// One write command. Runs in its own task.
    async fn write(self: Arc<Self>, cmd: AnimeCmd) {
        let result = match cmd {
            AnimeCmd::SetFlagBits { anime_id, flag, mask } => {
                self.blocking(move |s| s.set_flag_bits(anime_id, flag, mask)).await
            }
            AnimeCmd::SortBy { anime_id, mode } => self.blocking(move |s| s.sort_by(anime_id, mode)).await,
            AnimeCmd::ToggleFavorite { anime_id } => return self.toggle_favorite(anime_id).await,
            AnimeCmd::SetMark { ids, mark, on } => self.blocking(move |s| s.set_marks(&ids, mark, on).map(|()| true)).await,
            AnimeCmd::SetSeen { ids, seen } => self.blocking(move |s| s.set_seen(&ids, seen).map(|()| true)).await,
            AnimeCmd::DeleteDownloads { anime_id } => {
                let backend = Arc::clone(&self.backend);
                let _ = tokio::task::spawn_blocking(move || backend.delete_downloads(anime_id)).await;
                return;
            }
            AnimeCmd::Load { .. } | AnimeCmd::Refresh { .. } | AnimeCmd::Shutdown => return,
        };
        if let Err(e) = result {
            self.emit(AnimeEvent::Failed(e.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use kuro_domain::entries::anime::episode_flags as ep;

    use super::super::entry_state::LoadState;
    use super::super::store::fixture::with_episodes;
    use super::*;

    /// A backend that counts fetches and holds them open. A cancelled fetch is dropped before it counts as finished.
    struct Fake {
        hold_ms: u64,
        started: AtomicUsize,
        finished: Arc<AtomicUsize>,
        /// The episodes every fetch reports as new.
        new_episodes: Vec<Episode>,
        triggers: Mutex<Vec<Trigger>>,
        queued: Mutex<Vec<Vec<f64>>>,
        covers: AtomicUsize,
        downloads: usize,
        deleted: AtomicUsize,
    }

    impl Fake {
        fn new(hold_ms: u64) -> Arc<Self> {
            Arc::new(Self {
                hold_ms,
                started: AtomicUsize::new(0),
                finished: Arc::default(),
                new_episodes: Vec::new(),
                triggers: Mutex::default(),
                queued: Mutex::default(),
                covers: AtomicUsize::new(0),
                downloads: 0,
                deleted: AtomicUsize::new(0),
            })
        }
    }

    impl AnimeBackend for Fake {
        fn source_info(&self, _: i64) -> SourceInfo {
            SourceInfo { name: "Fake".into(), stub: false }
        }

        fn fetch(&self, _: i64, _: RefreshPlan, trigger: Trigger) -> AnimeFetch {
            self.started.fetch_add(1, Ordering::SeqCst);
            self.triggers.lock().unwrap().push(trigger);
            let (hold, finished, new) = (self.hold_ms, Arc::clone(&self.finished), self.new_episodes.clone());
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(hold)).await;
                finished.fetch_add(1, Ordering::SeqCst);
                Ok(new)
            })
        }

        fn enqueue_new_episodes(&self, _: i64, episodes: &[Episode]) {
            self.queued.lock().unwrap().push(episodes.iter().map(|e| e.episode_number).collect());
        }

        fn remove_cover(&self, _: i64) -> bool {
            self.covers.fetch_add(1, Ordering::SeqCst);
            true
        }

        fn downloaded_count(&self, _: i64) -> usize {
            self.downloads
        }

        fn delete_downloads(&self, _: i64) {
            self.deleted.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap()
    }

    async fn until(rx: &Receiver<AnimeEvent>, mut pick: impl FnMut(&AnimeEvent) -> bool) -> AnimeEvent {
        loop {
            let event = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.expect("an event in time").expect("open");
            if pick(&event) {
                return event;
            }
        }
    }

    fn start(rt: &tokio::runtime::Runtime, store: AnimeStore, backend: Arc<Fake>) -> AnimeRunner {
        spawn(rt.handle(), store, PrefStore::in_memory(), backend, AnimeLoadContext::default())
    }

    #[test]
    fn a_fixture_entry_loads_to_success_as_the_first_event() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 3, 0);
        rt.block_on(async {
            let runner = start(&rt, store, Fake::new(0));
            runner.send(AnimeCmd::Load { anime_id: id });
            let rx = runner.events();
            let AnimeEvent::Loaded(Some(model)) = rx.recv().await.unwrap() else { panic!("the model first") };
            assert_eq!(model.flow.load, LoadState::Success);
            assert_eq!(model.episodes.len(), 3);
            assert_eq!(model.source.name, "Fake");
        });
    }

    #[test]
    fn a_missing_row_loads_as_none() {
        let rt = rt();
        let (_dir, store, _) = with_episodes(true, true, 0, 0);
        rt.block_on(async {
            let runner = start(&rt, store, Fake::new(0));
            runner.send(AnimeCmd::Load { anime_id: 4242 });
            assert!(matches!(runner.events().recv().await.unwrap(), AnimeEvent::Loaded(None)));
        });
    }

    #[test]
    fn the_first_open_refresh_runs_once_and_reports_its_end() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, false, 2, 0);
        let backend = Fake::new(20);
        rt.block_on(async {
            let runner = start(&rt, store, backend.clone());
            runner.send(AnimeCmd::Load { anime_id: id });
            let rx = runner.events();
            let AnimeEvent::Loaded(Some(model)) = until(&rx, |e| matches!(e, AnimeEvent::Loaded(_))).await else { panic!() };
            assert!(model.flow.refreshing);
            let AnimeEvent::RefreshEnded(result) = until(&rx, |e| matches!(e, AnimeEvent::RefreshEnded(_))).await else { panic!() };
            assert_eq!(result, Ok(()));
            assert_eq!(backend.started.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn a_refresh_requested_while_one_runs_joins_it() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 1, 0);
        let backend = Fake::new(300);
        rt.block_on(async {
            let runner = start(&rt, store, backend.clone());
            let rx = runner.events();
            runner.send(AnimeCmd::Refresh { anime_id: id });
            runner.send(AnimeCmd::Refresh { anime_id: id });
            until(&rx, |e| matches!(e, AnimeEvent::RefreshEnded(_))).await;
            assert_eq!(backend.started.load(Ordering::SeqCst), 1, "the second request joined the first");
        });
    }

    #[test]
    fn a_flag_write_reaches_the_page_as_a_snapshot_from_the_subscription() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 2, 0);
        rt.block_on(async {
            let runner = start(&rt, store, Fake::new(0));
            runner.send(AnimeCmd::Load { anime_id: id });
            let rx = runner.events();
            until(&rx, |e| matches!(e, AnimeEvent::Loaded(_))).await;
            runner.send(AnimeCmd::SetFlagBits { anime_id: id, flag: ep::SHOW_UNSEEN, mask: ep::UNSEEN_MASK });
            let event = until(&rx, |e| matches!(e, AnimeEvent::Snapshot { anime, .. } if anime.episode_flags & ep::UNSEEN_MASK == ep::SHOW_UNSEEN)).await;
            let AnimeEvent::Snapshot { episodes, .. } = event else { unreachable!() };
            assert_eq!(episodes.len(), 2);
        });
    }

    #[test]
    fn dispose_during_a_refresh_cancels_it_and_a_queued_write_still_lands() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, false, 1, 0);
        let backend = Fake::new(400);
        let finished = Arc::clone(&backend.finished);
        rt.block_on(async {
            let runner = start(&rt, store.clone(), backend.clone());
            let rx = runner.events();
            runner.send(AnimeCmd::Load { anime_id: id });
            until(&rx, |e| matches!(e, AnimeEvent::Loaded(_))).await;
            runner.send(AnimeCmd::SetFlagBits { anime_id: id, flag: ep::SHOW_BOOKMARKED, mask: ep::BOOKMARKED_MASK });
            drop(runner); // the page was popped while the fetch is held open
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while store.anime(id).unwrap().unwrap().episode_flags & ep::BOOKMARKED_MASK != ep::SHOW_BOOKMARKED {
                assert!(std::time::Instant::now() < deadline, "the queued write never landed");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            tokio::time::sleep(Duration::from_millis(700)).await;
            assert_eq!(backend.started.load(Ordering::SeqCst), 1, "the fetch had started");
            assert_eq!(finished.load(Ordering::SeqCst), 0, "dispose cancelled the fetch before it finished");
        });
    }

    fn episode(number: f64) -> Episode {
        Episode { episode_number: number, ..Episode::create() }
    }

    #[test]
    fn only_a_manual_refresh_queues_the_new_episodes_and_only_when_the_preference_is_on() {
        use kuro_domain::download::prefs::DownloadPrefs;
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 1, 0);
        let prefs = PrefStore::in_memory();
        DownloadPrefs::new(&prefs).download_new_episodes.set(true).unwrap();
        let backend = Arc::new(Fake { new_episodes: vec![episode(7.0), episode(8.0)], ..Arc::into_inner(Fake::new(0)).unwrap() });
        rt.block_on(async {
            let runner = spawn(rt.handle(), store.clone(), prefs, backend.clone(), AnimeLoadContext::default());
            let rx = runner.events();
            runner.send(AnimeCmd::Refresh { anime_id: id });
            until(&rx, |e| matches!(e, AnimeEvent::RefreshEnded(Ok(())))).await;
            assert_eq!(*backend.triggers.lock().unwrap(), vec![Trigger::Manual]);
            assert_eq!(*backend.queued.lock().unwrap(), vec![vec![7.0, 8.0]], "a manual refresh queues what the sync added");
        });
        // the open-time refresh reports the same episodes and queues none (E-REF-5)
        let (_dir2, store2, id2) = with_episodes(true, false, 1, 0);
        let prefs = PrefStore::in_memory();
        DownloadPrefs::new(&prefs).download_new_episodes.set(true).unwrap();
        let backend = Arc::new(Fake { new_episodes: vec![episode(7.0)], ..Arc::into_inner(Fake::new(0)).unwrap() });
        rt.block_on(async {
            let runner = spawn(rt.handle(), store2, prefs, backend.clone(), AnimeLoadContext::default());
            let rx = runner.events();
            runner.send(AnimeCmd::Load { anime_id: id2 });
            until(&rx, |e| matches!(e, AnimeEvent::RefreshEnded(_))).await;
            assert_eq!(*backend.triggers.lock().unwrap(), vec![Trigger::Open]);
            assert!(backend.queued.lock().unwrap().is_empty(), "the open refresh never auto-downloads");
        });
        // the preference is off: a manual refresh queues nothing
        let (_dir3, store3, id3) = with_episodes(true, true, 1, 0);
        let backend = Arc::new(Fake { new_episodes: vec![episode(7.0)], ..Arc::into_inner(Fake::new(0)).unwrap() });
        rt.block_on(async {
            let runner = spawn(rt.handle(), store3, PrefStore::in_memory(), backend.clone(), AnimeLoadContext::default());
            let rx = runner.events();
            runner.send(AnimeCmd::Refresh { anime_id: id3 });
            until(&rx, |e| matches!(e, AnimeEvent::RefreshEnded(_))).await;
            assert!(backend.queued.lock().unwrap().is_empty());
        });
    }

    #[test]
    fn removing_a_favorite_writes_first_then_deletes_the_cover_and_asks_about_downloads() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 1, 0);
        let backend = Arc::new(Fake { downloads: 2, ..Arc::into_inner(Fake::new(0)).unwrap() });
        rt.block_on(async {
            let runner = start(&rt, store.clone(), backend.clone());
            let rx = runner.events();
            runner.send(AnimeCmd::ToggleFavorite { anime_id: id });
            let AnimeEvent::Removed(outcome) = until(&rx, |e| matches!(e, AnimeEvent::Removed(_))).await else { unreachable!() };
            assert_eq!(outcome, RemoveOutcome { cover_removed: true, ask_delete_downloads: true });
            let row = store.anime(id).unwrap().unwrap();
            assert!(!row.favorite && row.date_added == 0);
            assert_eq!(backend.covers.load(Ordering::SeqCst), 1);
            runner.send(AnimeCmd::DeleteDownloads { anime_id: id });
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while backend.deleted.load(Ordering::SeqCst) == 0 {
                assert!(std::time::Instant::now() < deadline, "Delete never reached the downloads layer");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
    }

    #[test]
    fn adding_a_non_favorite_writes_favorite_and_the_library_defaults() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(false, true, 1, 0);
        let prefs = PrefStore::in_memory();
        kuro_domain::entries::defaults::set_episode_settings_default(&prefs, ep::SHOW_UNSEEN);
        rt.block_on(async {
            let runner = spawn(rt.handle(), store.clone(), prefs, Fake::new(0), AnimeLoadContext::default());
            let rx = runner.events();
            runner.send(AnimeCmd::ToggleFavorite { anime_id: id });
            until(&rx, |e| matches!(e, AnimeEvent::Added)).await;
            let row = store.anime(id).unwrap().unwrap();
            assert!(row.favorite);
            assert_eq!(row.episode_flags & ep::UNSEEN_MASK, ep::SHOW_UNSEEN, "defaults are written at add time");
        });
    }

    #[test]
    fn mark_and_seen_commands_write_what_they_are_given() {
        use super::super::actions::Mark;
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 2, 0);
        let ids: Vec<i64> = store.episodes(id).unwrap().iter().map(|e| e.id).collect();
        rt.block_on(async {
            let runner = start(&rt, store.clone(), Fake::new(0));
            runner.send(AnimeCmd::SetMark { ids: vec![ids[0]], mark: Mark::Fillermark, on: true });
            runner.send(AnimeCmd::SetSeen { ids: vec![ids[1]], seen: true });
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                let rows = store.episodes(id).unwrap();
                let a = rows.iter().find(|e| e.id == ids[0]).unwrap().fillermark;
                let b = rows.iter().find(|e| e.id == ids[1]).unwrap().seen;
                if a && b {
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "the writes never landed");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
    }

    #[test]
    fn the_stub_backend_reads_unknown_sources_as_missing_and_fetches_fail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anime.db");
        let writer = kuro_db::schema_anime::open(&path).unwrap();
        let db = Arc::new(Db::new(&path, writer).unwrap());
        AnimeStubSourceRepo::new(Arc::clone(&db)).upsert(7, "en", "Player").unwrap();
        let backend = StubAnimeBackend::new(db);
        assert_eq!(backend.source_info(7), SourceInfo { name: "Player (EN)".into(), stub: true });
        assert!(backend.source_info(99).stub);
        let local = backend.source_info(kuro_source::local::LOCAL_SOURCE_ID);
        assert!(!local.stub && !local.name.is_empty());
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let plan = RefreshPlan { info: true, episodes: false, seasons: false };
        assert_eq!(rt.block_on(backend.fetch(1, plan, Trigger::Manual)), Err(DetailError::SourceNotInstalled));
    }
}
