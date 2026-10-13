//! The anime store runner: one tokio task consumes `AnimeCmd`, every result goes back as an `AnimeEvent` through an
//! `async-channel`, and the GTK side drains it. `Shutdown` (the page was popped) aborts the live subscriptions and
//! cancels the page's `CancellationToken`, which ends a refresh that is still fetching. Write commands run in their
//! own task, on the blocking pool, and never look at the token, so a pop does not cancel them. No GTK here.
//! Upstream: `AnimeScreenModel.kt:204-283` (init), `fetchAllFromSource`.

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use async_channel::{Receiver, Sender};
use kuro_data::DataResult;
use kuro_data::anime_relation::AnimeRelationGroup;
use kuro_data::entry::Entry;
use kuro_data::item::Item;
use kuro_data::source::StubSourceRepo;
use kuro_db::handle::Db;
use kuro_db::codec::FetchType;
use kuro_platform::kind::Kind;
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::store::PrefStore;
use tokio::runtime::Handle;
use tokio::task::AbortHandle;
use tokio_util::sync::CancellationToken;

use super::actions::Mark;
use super::details_error::{DetailsError, join_failed};
use super::entry_state::RefreshPlan;
use super::model::{AnimeEntryModel, AnimeLoadContext};
use crate::entry_core::refresh_rules::{RefreshGate, Trigger, last_empty_open_fetch, record_empty_open_fetch};
use super::season_tile::SeasonExtra;
use super::set_default::Scope;
use super::store::AnimeStore;
use crate::entry_core::backend::stub_source_info;
use crate::entry_core::detail::SourceInfo;
use crate::entry_core::errors::DetailError;
use crate::entry_core::library_flow::{self, Hooks, RemoveOutcome, Toggled};
use crate::entry_core::runner_kit::{RunnerHandle, abort_all, watch_pair};

/// What a fetch hands back: the episodes the sync added, which a manual refresh queues for download (T-REF-4).
pub type AnimeFetch = Pin<Box<dyn Future<Output = Result<Vec<Item>, DetailError>> + Send>>;

/// What the page needs from outside its own database: the source line and the source fetch.
pub trait AnimeBackend: Send + Sync + 'static {
    /// The source line of the info box. `stub` is true when the extension is not installed.
    fn source_info(&self, source_id: i64) -> SourceInfo;
    /// `fetchAllFromSource`: fetches what `plan` names and stores it, so the page sees it through the subscriptions.
    /// `trigger` says whether the user asked (`manualFetch`). It returns the episodes the sync added.
    fn fetch(&self, anime_id: i64, plan: RefreshPlan, trigger: Trigger) -> AnimeFetch;
    /// Queues new episodes for download. Only a manual refresh reaches this (E-REF-5). The app has no anime download
    /// queue wired yet, so the default does nothing.
    fn enqueue_new_episodes(&self, _anime_id: i64, _episodes: &[Item]) {}
    /// Binds the enhanced trackers of an entry that just joined the library (`bindEnhancedTrackers`).
    fn bind_enhanced(&self, _anime_id: i64) {}
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
    /// Whether the app can queue and delete episode downloads (`onDownloadEpisode != null`): the Download menu and the
    /// download rows of the selection bar show only then.
    fn has_download_handler(&self) -> bool {
        false
    }
    /// The ids of the entry's episodes that are on disk.
    fn downloaded_episodes(&self, _anime_id: i64) -> HashSet<i64> {
        HashSet::new()
    }
    /// Queues these episodes. With `start_now` the one episode goes to the front and the queue starts. `false` when
    /// the downloads layer is not there: nothing was queued.
    fn enqueue_episodes(&self, _anime_id: i64, _episode_ids: &[i64], _start_now: bool) -> bool {
        false
    }
    /// Deletes the downloads of these episodes (the Delete row, and a mark seen with "remove after seen").
    fn delete_episodes(&self, _anime_id: i64, _episode_ids: &[i64]) {}
    /// Whether a tracker is logged in: the tracking button opens the tracking sheet then, the tracking settings
    /// otherwise (E-25). The default is no; the app's backends read the published tracker session.
    fn has_logged_in_tracker(&self) -> bool {
        false
    }
    /// Whether the source is an HTTP source: only then does the action row offer WebView (E-26).
    fn is_http_source(&self, _source_id: i64) -> bool {
        false
    }
    /// Whether the source is configurable: only then does the overflow menu offer Settings (T-39a-057).
    fn is_configurable_source(&self, _source_id: i64) -> bool {
        false
    }
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
    /// The language code of a source (`source.lang`), for the language badge of a season. Empty when it is not known.
    fn source_language(&self, _source_id: i64) -> String {
        String::new()
    }
}

/// The backend of an app with no anime source registry yet: every source reads as not installed, the local source
/// reads as installed, and every fetch reports that the source is missing.
pub struct StubAnimeBackend {
    stubs: StubSourceRepo,
}

impl StubAnimeBackend {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            stubs: StubSourceRepo::new(db, Kind::Anime),
        }
    }
}

impl AnimeBackend for StubAnimeBackend {
    fn has_logged_in_tracker(&self) -> bool {
        crate::entry_core::tracking::session::has_logged_in()
    }
    fn source_info(&self, source_id: i64) -> SourceInfo {
        stub_source_info(&self.stubs, Kind::Anime, source_id)
    }

    fn fetch(&self, _anime_id: i64, _plan: RefreshPlan, _trigger: Trigger) -> AnimeFetch {
        Box::pin(async { Err(DetailError::SourceNotInstalled) })
    }

    fn source_language(&self, source_id: i64) -> String {
        self.stubs.get(source_id).ok().flatten().map(|stub| stub.lang).unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimeCmd {
    Load { anime_id: i64 },
    /// A manual refresh: every part of the entry is fetched again, and the new episodes may auto-download.
    Refresh { anime_id: i64 },
    SetFlagBits { anime_id: i64, flag: i64, mask: i64 },
    /// The skip-intro length in seconds; 0 turns the skip off (`setSkipIntroLength`).
    SetSkipIntroLength { anime_id: i64, length: i32 },
    /// A masked write of the season flag word (a pick of the season list options).
    SetSeasonFlagBits { anime_id: i64, flag: i64, mask: i64 },
    /// "Use as default for all titles": the default of this scope only goes to every favorite. Answered by
    /// `AnimeEvent::DefaultsApplied`.
    ApplyDefaultsToLibrary { scope: Scope },
    /// The library button: remove an entry that is in the library, or add one that is not. With `check_duplicates` a
    /// library entry with the same title stops the add and the page is asked what to do; Add anyway sends it false.
    ToggleLibrary { anime_id: i64, check_duplicates: bool },
    /// Sets or clears the bookmark or fillermark of exactly these episodes (`actions::bulk_mark_targets`).
    SetMark { ids: Vec<i64>, mark: Mark, on: bool },
    /// Marks these episodes seen or unseen.
    SetSeen { ids: Vec<i64>, seen: bool },
    /// `setFetchInterval`: the wheel value; the stored value is its negative (0 is automatic).
    SetFetchInterval { anime_id: i64, wheel: i32 },
    /// Deletes every download of the entry (the Delete button of the prompt).
    DeleteDownloads { anime_id: i64 },
    /// Queues these episodes for download. With `start_now` the one episode goes to the front.
    Download { anime_id: i64, ids: Vec<i64>, start_now: bool },
    /// Deletes the downloads of these episodes.
    DeleteEpisodeDownloads { anime_id: i64, ids: Vec<i64> },
    /// Reads the categories and the entry's links, for the category editor: answered by `AnimeEvent::Categories`.
    LoadCategories { anime_id: i64 },
    /// Reads the tracking count again, after the tracking page bound, edited or removed a tracker. Answered by
    /// `AnimeEvent::Tracking`.
    ReadTracking { anime_id: i64, source_id: i64 },
    /// The category editor's OK: replaces the entry's category links.
    SetCategories { anime_id: i64, category_ids: Vec<i64> },
    /// The answer of the add dialog: the entry joins the library in these categories.
    AddWithCategories { anime_id: i64, category_ids: Vec<i64> },
    Shutdown,
}

#[derive(Debug)]
pub enum AnimeEvent {
    /// The load finished: the model, or `None` when the row does not exist.
    Loaded(Option<Box<AnimeEntryModel>>),
    /// The live subscription published a newer entry and episode list.
    Snapshot { anime: Box<Entry>, episodes: Vec<Item> },
    /// The refresh started by the first open ended, with or without an error.
    RefreshEnded(Result<(), DetailError>),
    /// The entry joined the library, with the library defaults written to its flags.
    Added,
    /// The entry left the library. A failed write sends nothing (E-LIB-9).
    Removed(RemoveOutcome),
    /// A store read or write failed. The text is what the user sees (`DetailsError::user_message`); the detail is logged.
    Failed(String),
    /// The related groups changed (or were read for the first time).
    Related(Vec<AnimeRelationGroup>),
    /// The result of `ApplyDefaultsToLibrary`: how many entries took the defaults.
    DefaultsApplied { scope: Scope, updated: usize },
    /// The entry being added has the same title as these library entries.
    Duplicate { existing: Vec<Entry> },
    /// The download count and source language of each season, for the tiles of a `Seasons` entry.
    SeasonFacts(Vec<SeasonExtra>),
    /// The ids of the episodes on disk changed (a download finished or a delete ran).
    Downloads(HashSet<i64>),
    /// The answer of `LoadCategories`: every category and the ids the entry is linked to.
    Categories { all: Vec<(i64, String)>, linked: Vec<i64> },
    /// The library button of an add with no default category: the category dialog asks which ones (T-39b-026).
    AddCategories { all: Vec<(i64, String)>, linked: Vec<i64> },
    /// The tracking count, read again (`AnimeCmd::ReadTracking`).
    Tracking(usize),
}

/// The page's handle on the runner. Dropping it stops the runner and the subscriptions.
pub type AnimeRunner = RunnerHandle<AnimeCmd, AnimeEvent>;

struct Shared {
    store: AnimeStore,
    prefs: PrefStore,
    backend: Arc<dyn AnimeBackend>,
    ctx: AnimeLoadContext,
    events: Sender<AnimeEvent>,
    cancel: CancellationToken,
    subscriptions: Mutex<Vec<AbortHandle>>,
    gate: RefreshGate,
}

/// Starts the runner on `handle`.
pub fn spawn(
    handle: &Handle,
    store: AnimeStore,
    prefs: PrefStore,
    backend: Arc<dyn AnimeBackend>,
    ctx: AnimeLoadContext,
) -> AnimeRunner {
    let (runner, cmd_rx, event_tx, cancel) = AnimeRunner::new(|| AnimeCmd::Shutdown);
    let shared = Arc::new(Shared {
        store,
        prefs,
        backend,
        ctx,
        events: event_tx,
        cancel,
        subscriptions: Mutex::new(Vec::new()),
        gate: RefreshGate::default(),
    });
    handle.spawn(run(shared, cmd_rx));
    runner
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
        abort_all(&self.subscriptions);
    }

    /// Tells the page a store read or write failed: the detail goes to the log, the user message to the page.
    fn failed(&self, error: impl Into<DetailsError>) {
        self.emit(AnimeEvent::Failed(error.into().record().user_message()));
    }

    /// Runs blocking store work on the blocking pool. The work keeps running if the awaiting task is aborted. A task
    /// that did not finish is a typed error, not a panic.
    async fn blocking<T: Send + 'static>(&self, work: impl FnOnce(AnimeStore) -> DataResult<T> + Send + 'static) -> Result<T, DetailsError> {
        let store = self.store.clone();
        match tokio::task::spawn_blocking(move || work(store)).await {
            Ok(result) => result.map_err(DetailsError::from),
            Err(e) => Err(join_failed(&e)),
        }
    }

    /// The load sequence (`AnimeEntryModel::load`), then the live subscription, then the first-open refresh.
    async fn load(self: Arc<Self>, anime_id: i64) {
        let ctx = AnimeLoadContext {
            last_empty_open_fetch_ms: last_empty_open_fetch(Kind::Anime, anime_id),
            now_ms: now_ms(),
            ..self.ctx
        };
        let (prefs, backend) = (self.prefs.clone(), Arc::clone(&self.backend));
        let loaded = self
            .blocking(move |store| -> DataResult<Option<AnimeEntryModel>> {
                let mut model = AnimeEntryModel::load(&store, &prefs, anime_id, ctx, |source| backend.source_info(source))?;
                if let Some(m) = &mut model {
                    m.downloaded = backend.downloaded_episodes(anime_id);
                }
                Ok(model)
            })
            .await;
        match loaded {
            Ok(Some(model)) => {
                let plan = model.plan;
                if plan.episodes {
                    record_empty_open_fetch(Kind::Anime, anime_id, ctx.now_ms);
                }
                let seasons = model.anime.fetch_type() == FetchType::Seasons;
                self.emit(AnimeEvent::Loaded(Some(Box::new(model))));
                self.subscribe(anime_id);
                if seasons {
                    self.emit_season_facts(anime_id).await;
                }
                if plan.any() {
                    tokio::spawn(self.clone().refresh(anime_id, plan, Trigger::Open));
                }
            }
            Ok(None) => self.emit(AnimeEvent::Loaded(None)),
            Err(e) => {
                self.failed(e);
                self.emit(AnimeEvent::Loaded(None));
            }
        }
    }

    /// Step 10 and 11 of the init block, or a manual refresh. Single-flight per entry: a request that arrives while one
    /// runs joins it and fetches nothing (E-CONC-1), and the running one reports the end. A cancelled refresh ends
    /// silently: nobody is left to read its event.
    async fn refresh(self: Arc<Self>, anime_id: i64, plan: RefreshPlan, trigger: Trigger) {
        let Some(running) = self.gate.begin(anime_id) else { return };
        let fetched = tokio::select! {
            () = self.cancel.cancelled() => return,
            result = self.backend.fetch(anime_id, plan, trigger) => result,
        };
        drop(running);
        let result = match fetched {
            Ok(new_episodes) => {
                self.auto_download(anime_id, trigger, new_episodes).await;
                Ok(())
            }
            Err(e) => Err(e),
        };
        let refreshed = result.is_ok();
        self.emit(AnimeEvent::RefreshEnded(result));
        // a refresh of a `Seasons` entry may have added seasons
        if refreshed {
            self.emit_season_facts(anime_id).await;
        }
    }

    /// The download count and source language of each season of the entry, read off the GTK thread. Nothing is sent
    /// for an entry with no seasons.
    async fn emit_season_facts(&self, anime_id: i64) {
        let backend = Arc::clone(&self.backend);
        let facts = self
            .blocking(move |store| {
                Ok(store
                    .seasons(anime_id)?
                    .iter()
                    .map(|season| SeasonExtra {
                        id: season.entry.id,
                        download_count: i64::try_from(backend.downloaded_count(season.entry.id)).unwrap_or(i64::MAX),
                        source_language: backend.source_language(season.entry.source),
                    })
                    .collect::<Vec<_>>())
            })
            .await;
        match facts {
            Ok(facts) if !facts.is_empty() => self.emit(AnimeEvent::SeasonFacts(facts)),
            Ok(_) => {}
            Err(e) => self.failed(e),
        }
    }

    /// Queues the new episodes of a manual refresh when the preferences and the entry's categories allow it (T-REF-4).
    /// The open-time refresh never gets here with episodes to queue (E-REF-5).
    async fn auto_download(&self, anime_id: i64, trigger: Trigger, new_episodes: Vec<Item>) {
        if new_episodes.is_empty() {
            return;
        }
        let prefs = self.prefs.clone();
        let targets = self.blocking(move |store| store.new_item_targets(&prefs, anime_id, trigger, &new_episodes)).await;
        match targets {
            Ok(targets) if !targets.is_empty() => {
                let backend = Arc::clone(&self.backend);
                let _ = tokio::task::spawn_blocking(move || backend.enqueue_new_episodes(anime_id, &targets)).await;
            }
            Ok(_) => {}
            Err(e) => self.failed(e),
        }
    }

    /// The library button (`library_flow`). A favorite is removed: the write first, then the cover and the download count.
    /// An entry that is not one is checked for a same-named library entry (with `check_duplicates`, T-39a-047), routed by
    /// the default category (T-39b-026), and added with the library defaults written to its flags (T-39b-012). A failed
    /// write does nothing else (E-LIB-9).
    async fn toggle_library(&self, anime_id: i64, check_duplicates: bool) {
        let prefs = self.prefs.clone();
        let default_category = i64::from(LibraryPrefs::new(prefs.clone()).default_category(Kind::Anime).get());
        let (backend, now) = (Arc::clone(&self.backend), now_ms());
        let toggled = self
            .blocking(move |store| {
                Self::with_hooks(&backend, anime_id, |hooks| {
                    library_flow::toggle(&store.library(&prefs), hooks, anime_id, default_category, check_duplicates, now)
                })
            })
            .await;
        match toggled {
            Ok(Toggled::Nothing) => {}
            Ok(Toggled::Removed(outcome)) => self.emit(AnimeEvent::Removed(outcome)),
            Ok(Toggled::Duplicate(existing)) => self.emit(AnimeEvent::Duplicate { existing }),
            Ok(Toggled::ChooseCategory { all, linked }) => self.emit(AnimeEvent::AddCategories { all, linked }),
            Ok(Toggled::Added) => self.emit(AnimeEvent::Added),
            Err(e) => self.failed(e),
        }
    }

    /// Adds the entry in these categories: the links and the favorite in order (`moveAnimeToCategoriesAndAddToLibrary`).
    async fn add_with_categories(&self, anime_id: i64, category_ids: Vec<i64>) {
        let (prefs, backend, now) = (self.prefs.clone(), Arc::clone(&self.backend), now_ms());
        let added = self
            .blocking(move |store| {
                Self::with_hooks(&backend, anime_id, |hooks| library_flow::add(&store.library(&prefs), hooks, anime_id, &category_ids, now))
            })
            .await;
        match added {
            Ok(true) => self.emit(AnimeEvent::Added),
            Ok(false) => {}
            Err(e) => self.failed(e),
        }
    }

    /// The library button's backend hooks for one entry (`library_flow::Hooks`).
    fn with_hooks<T>(backend: &Arc<dyn AnimeBackend>, anime_id: i64, f: impl FnOnce(&Hooks<'_>) -> T) -> T {
        let remove_cover = || backend.remove_cover(anime_id);
        let downloaded_count = || backend.downloaded_count(anime_id);
        let bind_enhanced = || backend.bind_enhanced(anime_id);
        f(&Hooks { remove_cover: &remove_cover, downloaded_count: &downloaded_count, bind_enhanced: &bind_enhanced })
    }

    /// Reads the episodes on disk again and tells the page.
    async fn emit_downloads(&self, anime_id: i64) {
        let backend = Arc::clone(&self.backend);
        if let Ok(on_disk) = tokio::task::spawn_blocking(move || backend.downloaded_episodes(anime_id)).await {
            self.emit(AnimeEvent::Downloads(on_disk));
        }
    }

    /// The entry and its episodes as one snapshot (`watch_pair`), and the related groups.
    fn subscribe(self: &Arc<Self>, anime_id: i64) {
        let shared = Arc::clone(self);
        let pair = watch_pair(self.store.watch_anime(anime_id), self.store.watch_episodes(anime_id), move |anime: &Entry, episodes: &[Item]| {
            shared.emit(AnimeEvent::Snapshot { anime: Box::new(anime.clone()), episodes: episodes.to_vec() });
        });
        let mut related_sub = self.store.watch_related(anime_id);
        let shared = Arc::clone(self);
        let related_task = tokio::spawn(async move {
            while let Some(published) = related_sub.changed().await {
                if let Ok(groups) = published {
                    shared.emit(AnimeEvent::Related(groups.as_ref().clone()));
                }
            }
        });
        let mut subs = self.subscriptions.lock().unwrap_or_else(PoisonError::into_inner);
        subs.extend(pair);
        subs.push(related_task.abort_handle());
    }

    /// One write command. Runs in its own task.
    async fn write(self: Arc<Self>, cmd: AnimeCmd) {
        let result = match cmd {
            AnimeCmd::SetFlagBits { anime_id, flag, mask } => {
                self.blocking(move |s| s.set_flag_bits(anime_id, flag, mask)).await
            }
            AnimeCmd::SetSkipIntroLength { anime_id, length } => {
                self.blocking(move |s| s.set_skip_intro_length(anime_id, length)).await
            }
            AnimeCmd::SetSeasonFlagBits { anime_id, flag, mask } => {
                self.blocking(move |s| s.set_season_flag_bits(anime_id, flag, mask)).await
            }
            AnimeCmd::ApplyDefaultsToLibrary { scope } => {
                let prefs = self.prefs.clone();
                let applied = self
                    .blocking(move |s| match scope {
                        Scope::Episode => s.apply_episode_defaults_to_library(&prefs),
                        Scope::Season => s.apply_season_defaults_to_library(&prefs),
                    })
                    .await;
                match applied {
                    Ok(updated) => return self.emit(AnimeEvent::DefaultsApplied { scope, updated }),
                    Err(e) => return self.failed(e),
                }
            }
            AnimeCmd::AddWithCategories { anime_id, category_ids } => return self.add_with_categories(anime_id, category_ids).await,
            AnimeCmd::ToggleLibrary { anime_id, check_duplicates } => return self.toggle_library(anime_id, check_duplicates).await,
            AnimeCmd::SetMark { ids, mark, on } => self.blocking(move |s| s.set_marks(&ids, mark, on).map(|()| true)).await,
            AnimeCmd::SetSeen { ids, seen } => self.blocking(move |s| s.set_seen(&ids, seen).map(|()| true)).await,
            AnimeCmd::SetFetchInterval { anime_id, wheel } => {
                self.blocking(move |s| s.set_fetch_interval(anime_id, wheel).map(|()| true)).await
            }
            AnimeCmd::DeleteDownloads { anime_id } => {
                let backend = Arc::clone(&self.backend);
                let _ = tokio::task::spawn_blocking(move || backend.delete_downloads(anime_id)).await;
                return self.emit_downloads(anime_id).await;
            }
            AnimeCmd::Download { anime_id, ids, start_now } => {
                let backend = Arc::clone(&self.backend);
                let _ = tokio::task::spawn_blocking(move || backend.enqueue_episodes(anime_id, &ids, start_now)).await;
                return self.emit_downloads(anime_id).await;
            }
            AnimeCmd::DeleteEpisodeDownloads { anime_id, ids } => {
                let backend = Arc::clone(&self.backend);
                let _ = tokio::task::spawn_blocking(move || backend.delete_episodes(anime_id, &ids)).await;
                return self.emit_downloads(anime_id).await;
            }
            AnimeCmd::LoadCategories { anime_id } => match self.blocking(move |s| s.categories(anime_id)).await {
                Ok((all, linked)) => return self.emit(AnimeEvent::Categories { all, linked }),
                Err(e) => Err(e),
            },
            AnimeCmd::SetCategories { anime_id, category_ids } => {
                self.blocking(move |s| s.set_categories(anime_id, &category_ids).map(|()| true)).await
            }
            AnimeCmd::ReadTracking { anime_id, source_id } => match self.blocking(move |s| s.track_summary(anime_id, source_id)).await {
                Ok(summary) => return self.emit(AnimeEvent::Tracking(summary.count as usize)),
                Err(e) => Err(e),
            },
            AnimeCmd::Load { .. } | AnimeCmd::Refresh { .. } | AnimeCmd::Shutdown => return,
        };
        if let Err(e) = result {
            self.failed(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use kuro_db::flags::item as ep;

    use super::super::entry_state::LoadState;
    use super::super::store::fixture::with_episodes;
    use super::*;

    /// A backend that counts fetches and holds them open. A cancelled fetch is dropped before it counts as finished.
    struct Fake {
        hold_ms: u64,
        started: AtomicUsize,
        finished: Arc<AtomicUsize>,
        /// The episodes every fetch reports as new.
        new_episodes: Vec<Item>,
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

        fn enqueue_new_episodes(&self, _: i64, episodes: &[Item]) {
            self.queued.lock().unwrap().push(episodes.iter().map(|e| e.number).collect());
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

        fn source_language(&self, _: i64) -> String {
            "ja".to_owned()
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
            runner.send(AnimeCmd::SetFlagBits { anime_id: id, flag: ep::SHOW_UNDONE, mask: ep::UNDONE_MASK });
            let event = until(&rx, |e| matches!(e, AnimeEvent::Snapshot { anime, .. } if anime.item_flags & ep::UNDONE_MASK == ep::SHOW_UNDONE)).await;
            let AnimeEvent::Snapshot { episodes, .. } = event else { unreachable!() };
            assert_eq!(episodes.len(), 2);
        });
    }

    #[test]
    #[ignore = "slow: run on demand"]
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
            // the refresh starts after `Loaded`: wait until the fetch has really started, or a dispose that wins the
            // race would cancel it before it began
            let started = std::time::Instant::now() + Duration::from_secs(5);
            while backend.started.load(Ordering::SeqCst) == 0 {
                assert!(std::time::Instant::now() < started, "the fetch never started");
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            runner.send(AnimeCmd::SetFlagBits { anime_id: id, flag: ep::SHOW_BOOKMARKED, mask: ep::BOOKMARKED_MASK });
            drop(runner); // the page was popped while the fetch is held open
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while store.anime(id).unwrap().unwrap().item_flags & ep::BOOKMARKED_MASK != ep::SHOW_BOOKMARKED {
                assert!(std::time::Instant::now() < deadline, "the queued write never landed");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            tokio::time::sleep(Duration::from_millis(700)).await;
            assert_eq!(backend.started.load(Ordering::SeqCst), 1, "the fetch had started");
            assert_eq!(finished.load(Ordering::SeqCst), 0, "dispose cancelled the fetch before it finished");
        });
    }

    fn episode(number: f64) -> Item {
        Item { number, ..Item::create(Kind::Anime) }
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
            runner.send(AnimeCmd::ToggleLibrary { anime_id: id, check_duplicates: true });
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
        kuro_domain::entries::defaults::set_item_settings_default(&prefs, Kind::Anime, ep::SHOW_UNDONE);
        rt.block_on(async {
            let runner = spawn(rt.handle(), store.clone(), prefs, Fake::new(0), AnimeLoadContext::default());
            let rx = runner.events();
            runner.send(AnimeCmd::ToggleLibrary { anime_id: id, check_duplicates: true });
            until(&rx, |e| matches!(e, AnimeEvent::Added)).await;
            let row = store.anime(id).unwrap().unwrap();
            assert!(row.favorite);
            assert_eq!(row.item_flags & ep::UNDONE_MASK, ep::SHOW_UNDONE, "defaults are written at add time");
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
                let b = rows.iter().find(|e| e.id == ids[1]).unwrap().done;
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
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).unwrap();
        let db = Arc::new(Db::new(&path, writer).unwrap());
        StubSourceRepo::new(Arc::clone(&db), Kind::Anime).upsert(7, "en", "Player").unwrap();
        let backend = StubAnimeBackend::new(db);
        assert_eq!(backend.source_info(7), SourceInfo { name: "Player (EN)".into(), stub: true });
        assert!(backend.source_info(99).stub);
        let local = backend.source_info(kuro_source::local::LOCAL_SOURCE_ID);
        assert!(!local.stub && !local.name.is_empty());
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let plan = RefreshPlan { info: true, episodes: false, seasons: false };
        assert_eq!(rt.block_on(backend.fetch(1, plan, Trigger::Manual)), Err(DetailError::SourceNotInstalled));
    }

    #[test]
    fn set_as_default_with_the_checkbox_reports_how_many_favorites_took_that_scope_only() {
        use kuro_db::flags::season as sf;
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 1, 0);
        store.set_season_flag_bits(id, sf::SHOW_STARTED, sf::STARTED_MASK).unwrap();
        let prefs = PrefStore::in_memory();
        kuro_domain::entries::defaults::set_item_settings_default(&prefs, Kind::Anime, ep::SHOW_DONE | ep::SORT_ASC | ep::SORTING_NUMBER);
        rt.block_on(async {
            let runner = spawn(rt.handle(), store.clone(), prefs, Fake::new(0), AnimeLoadContext::default());
            runner.send(AnimeCmd::ApplyDefaultsToLibrary { scope: Scope::Episode });
            let event = until(&runner.events(), |e| matches!(e, AnimeEvent::DefaultsApplied { .. })).await;
            let AnimeEvent::DefaultsApplied { scope, updated } = event else { unreachable!() };
            assert_eq!((scope, updated), (Scope::Episode, 1));
            let row = store.anime(id).unwrap().unwrap();
            assert_eq!(row.item_flags & ep::UNDONE_MASK, ep::SHOW_DONE);
            assert_eq!(row.season_flags() & sf::STARTED_MASK, sf::SHOW_STARTED, "the episode scope leaves the season flags alone");
        });
    }

    #[test]
    fn an_add_with_a_same_named_library_entry_waits_for_the_answer_and_add_anyway_adds() {
        use kuro_data::entry::EntryRepo;
        let rt = rt();
        let (_dir, store, id) = with_episodes(false, true, 0, 0);
        let twin = EntryRepo::new(store.db()).insert(&Entry { source: 8, url: "/b".into(), title: "frieren".into(), favorite: true, ..Entry::create(Kind::Anime) }).unwrap();
        rt.block_on(async {
            let runner = start(&rt, store.clone(), Fake::new(0));
            let rx = runner.events();
            runner.send(AnimeCmd::ToggleLibrary { anime_id: id, check_duplicates: true });
            let AnimeEvent::Duplicate { existing } = until(&rx, |e| matches!(e, AnimeEvent::Duplicate { .. })).await else { unreachable!() };
            assert_eq!(existing.iter().map(|a| a.id).collect::<Vec<_>>(), vec![twin]);
            assert!(!store.anime(id).unwrap().unwrap().favorite, "nothing is written until the user answers");
            runner.send(AnimeCmd::ToggleLibrary { anime_id: id, check_duplicates: false });
            until(&rx, |e| matches!(e, AnimeEvent::Added)).await;
            assert!(store.anime(id).unwrap().unwrap().favorite);
        });
    }

    #[test]
    fn an_add_without_a_duplicate_adds_at_once() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(false, true, 0, 0);
        rt.block_on(async {
            let runner = start(&rt, store.clone(), Fake::new(0));
            runner.send(AnimeCmd::ToggleLibrary { anime_id: id, check_duplicates: true });
            until(&runner.events(), |e| matches!(e, AnimeEvent::Added)).await;
            assert!(store.anime(id).unwrap().unwrap().favorite);
        });
    }

    #[test]
    fn related_groups_are_published_after_the_load_and_again_when_they_change() {
        use kuro_data::anime_relation::AnimeRelationRepo;
        use kuro_data::entry::EntryRepo;
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 0, 0);
        let other = EntryRepo::new(store.db()).insert(&Entry { source: 8, url: "/b".into(), title: "Other".into(), ..Entry::create(Kind::Anime) }).unwrap();
        rt.block_on(async {
            let runner = start(&rt, store.clone(), Fake::new(0));
            let rx = runner.events();
            runner.send(AnimeCmd::Load { anime_id: id });
            until(&rx, |e| matches!(e, AnimeEvent::Related(groups) if groups.is_empty())).await;
            let group = AnimeRelationGroup { name: "Sequel".into(), anime: vec![Entry { id: other, ..Entry::create(Kind::Anime) }] };
            AnimeRelationRepo::new(store.db()).replace_relations(id, &[group], 1).unwrap();
            let AnimeEvent::Related(groups) = until(&rx, |e| matches!(e, AnimeEvent::Related(g) if !g.is_empty())).await else { unreachable!() };
            assert_eq!((groups[0].name.as_str(), groups[0].anime[0].id), ("Sequel", other));
        });
    }

    #[test]
    fn a_seasons_entry_gets_the_download_count_and_language_of_each_season() {
        use kuro_data::entry::EntryRepo;
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 0, 0);
        store.set_fetch_type_for_test(id, FetchType::Seasons);
        let season = EntryRepo::new(store.db())
            .insert(&Entry { source: 7, url: "/s1".into(), title: "S1".into(), parent_id: Some(id), season_number: Some(1.0), ..Entry::create(Kind::Anime) })
            .unwrap();
        rt.block_on(async {
            let mut fake = Fake::new(0);
            Arc::get_mut(&mut fake).unwrap().downloads = 3;
            let runner = start(&rt, store.clone(), fake);
            let rx = runner.events();
            runner.send(AnimeCmd::Load { anime_id: id });
            let AnimeEvent::SeasonFacts(facts) = until(&rx, |e| matches!(e, AnimeEvent::SeasonFacts(_))).await else { unreachable!() };
            assert_eq!(facts, vec![SeasonExtra { id: season, download_count: 3, source_language: "ja".into() }]);
        });
    }
}
