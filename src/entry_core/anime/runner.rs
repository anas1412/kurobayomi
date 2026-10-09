//! The anime store runner: one tokio task consumes `AnimeCmd`, every result goes back as an `AnimeEvent` through an
//! `async-channel`, and the GTK side drains it. `Shutdown` (the page was popped) aborts the live subscriptions and
//! cancels the page's `CancellationToken`, which ends a refresh that is still fetching. Write commands run in their
//! own task, on the blocking pool, and never look at the token, so a pop does not cancel them. No GTK here.
//! Upstream: `AnimeScreenModel.kt:204-283` (init), `fetchAllFromSource`.

use std::sync::{Arc, Mutex, PoisonError};

use async_channel::{Receiver, Sender};
use kuro_data::anime::Anime;
use kuro_data::episode::Episode;
use kuro_data::source::{AnimeStubSourceRepo, StubSource};
use kuro_db::handle::Db;
use kuro_prefs::store::PrefStore;
use tokio::runtime::Handle;
use tokio::task::AbortHandle;
use tokio_util::sync::CancellationToken;

use super::entry_state::RefreshPlan;
use super::model::{AnimeEntryModel, AnimeLoadContext};
use super::store::AnimeStore;
use crate::entry_core::backend::Fetch;
use crate::entry_core::detail::SourceInfo;
use crate::entry_core::errors::DetailError;

/// What the page needs from outside its own database: the source line and the source fetch.
pub trait AnimeBackend: Send + Sync + 'static {
    /// The source line of the info box. `stub` is true when the extension is not installed.
    fn source_info(&self, source_id: i64) -> SourceInfo;
    /// `fetchAllFromSource`: fetches what `plan` names and stores it, so the page sees it through the subscriptions.
    fn fetch(&self, anime_id: i64, plan: RefreshPlan) -> Fetch;
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

    fn fetch(&self, _anime_id: i64, _plan: RefreshPlan) -> Fetch {
        Box::pin(async { Err(DetailError::SourceNotInstalled) })
    }
}

#[derive(Debug)]
pub enum AnimeCmd {
    Load { anime_id: i64 },
    SetFlagBits { anime_id: i64, flag: i64, mask: i64 },
    SortBy { anime_id: i64, mode: i64 },
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
            write => {
                tokio::spawn(shared.clone().write(write));
            }
        }
    }
    shared.drop_subscriptions();
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
        let (prefs, backend, ctx) = (self.prefs.clone(), Arc::clone(&self.backend), self.ctx);
        let loaded = self
            .blocking(move |store| {
                AnimeEntryModel::load(&store, &prefs, anime_id, ctx, |source| backend.source_info(source))
            })
            .await;
        match loaded {
            Ok(Some(model)) => {
                let plan = model.plan;
                self.emit(AnimeEvent::Loaded(Some(Box::new(model))));
                self.subscribe(anime_id);
                if plan.any() {
                    tokio::spawn(self.clone().refresh(anime_id, plan));
                }
            }
            Ok(None) => self.emit(AnimeEvent::Loaded(None)),
            Err(e) => {
                self.emit(AnimeEvent::Failed(e.to_string()));
                self.emit(AnimeEvent::Loaded(None));
            }
        }
    }

    /// Step 10 and 11 of the init block. A cancelled refresh ends silently: nobody is left to read its event.
    async fn refresh(self: Arc<Self>, anime_id: i64, plan: RefreshPlan) {
        let result = tokio::select! {
            () = self.cancel.cancelled() => return,
            result = self.backend.fetch(anime_id, plan) => result,
        };
        self.emit(AnimeEvent::RefreshEnded(result));
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
            AnimeCmd::Load { .. } | AnimeCmd::Shutdown => return,
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
    }

    impl Fake {
        fn new(hold_ms: u64) -> Arc<Self> {
            Arc::new(Self {
                hold_ms,
                started: AtomicUsize::new(0),
                finished: Arc::default(),
            })
        }
    }

    impl AnimeBackend for Fake {
        fn source_info(&self, _: i64) -> SourceInfo {
            SourceInfo { name: "Fake".into(), stub: false }
        }

        fn fetch(&self, _: i64, _: RefreshPlan) -> Fetch {
            self.started.fetch_add(1, Ordering::SeqCst);
            let (hold, finished) = (self.hold_ms, Arc::clone(&self.finished));
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(hold)).await;
                finished.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
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
        assert_eq!(rt.block_on(backend.fetch(1, plan)), Err(DetailError::SourceNotInstalled));
    }
}
