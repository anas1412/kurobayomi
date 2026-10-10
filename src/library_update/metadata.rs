//! The manual metadata passes (T-26a-050 to T-26a-053; behaviour 9, "Refresh library covers"): one pass per kind that
//! asks every library entry's source for its details and writes the title, status and cover. They are only started by
//! hand (the settings action), never by the schedule, and they take no category or restriction filter.
//!
//! Deliberate fixes of the Kotlin jobs:
//! - Kotlin shares one work name for both kinds (P-3), so starting one refuses the other. Here each kind has its own
//!   lock name ([`MANGA_LOCK`], [`ANIME_LOCK`]), and neither is the lock of an update pass.
//! - Kotlin does not de-duplicate an entry that sits in several categories (P-13); the queue here is distinct by id.
//!
//! The anime pass calls the source with `fetch_details = true` and `fetch_items = false` ([`RemoteFlags::DETAILS_ONLY`]),
//! so the episode list endpoint is never called. A missing source is skipped without an error. A failure of one entry
//! is logged and ignored, and the pass completes.
//!
//! Decisions:
//! - The cover rule keeps `manualFetch = true`: a new cover url always refreshes the cover. The custom-cover check
//!   needs the cover cache, which this lane does not own, so a custom cover is not protected here.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use kuro_data::anime::{AnimeRepo, AnimeUpdate};
use kuro_data::manga::{MangaRepo, MangaUpdate};
use kuro_db::handle::Db;
use kuro_domain::kind::EntryKind;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::library_update::error::UpdateError;
use crate::library_update::pass::{MAX_SOURCES_IN_PARALLEL, group_by_source};
use crate::library_update::politeness;
use crate::library_update::queue::LibraryEntry;
use crate::library_update::source::{RemoteFlags, Source, SourceDetails, SourceRegistry};
use crate::library_update::store::LibraryStore;

/// The lock name of the manga metadata pass (`MetadataUpdate-manga`).
pub const MANGA_LOCK: &str = "MetadataUpdate-manga";
/// The lock name of the anime metadata pass (`MetadataUpdate-anime`).
pub const ANIME_LOCK: &str = "MetadataUpdate-anime";

/// The lock name of the metadata pass of `kind`.
pub fn lock_name(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Manga => MANGA_LOCK,
        EntryKind::Anime => ANIME_LOCK,
    }
}

/// The running metadata passes, by lock name. Separate from `PassLocks`, so a metadata pass never refuses an update
/// pass and an update pass never refuses a metadata pass.
#[derive(Default)]
pub struct MetadataLocks {
    held: Mutex<HashMap<&'static str, Arc<AtomicBool>>>,
}

impl MetadataLocks {
    fn lock(&self) -> MutexGuard<'_, HashMap<&'static str, Arc<AtomicBool>>> {
        self.held.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The guard of a new metadata pass of `kind`, or `None` when one of that kind is already running.
    pub fn acquire(this: &Arc<Self>, kind: EntryKind) -> Option<MetadataGuard> {
        let name = lock_name(kind);
        let mut held = this.lock();
        if held.contains_key(name) {
            return None;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        held.insert(name, Arc::clone(&cancel));
        Some(MetadataGuard { locks: Arc::clone(this), name, cancel })
    }

    pub fn is_running(&self, kind: EntryKind) -> bool {
        self.lock().contains_key(lock_name(kind))
    }

    /// Cancels the running pass of `kind`. A stopped metadata pass is not started again (SC-11). `false` when none runs.
    pub fn stop(&self, kind: EntryKind) -> bool {
        match self.lock().get(lock_name(kind)) {
            Some(cancel) => {
                cancel.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }
}

/// Held while a metadata pass runs. Dropping it frees the lock name.
pub struct MetadataGuard {
    locks: Arc<MetadataLocks>,
    name: &'static str,
    cancel: Arc<AtomicBool>,
}

impl Drop for MetadataGuard {
    fn drop(&mut self) {
        self.locks.lock().remove(self.name);
    }
}

/// The library entries once each, in first-seen order (P-13: an entry in several categories comes back once).
pub fn distinct_by_id(entries: Vec<LibraryEntry>) -> Vec<LibraryEntry> {
    let mut seen = HashSet::new();
    entries.into_iter().filter(|entry| seen.insert(entry.id)).collect()
}

/// What a pass writes for one entry (behaviour 5.3 over the fields a source's details carry here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataChange {
    /// The remote title, only when it is not empty and the entry is not a favourite.
    pub title: Option<String>,
    pub status: i64,
    /// The new cover url and the cover timestamp that makes the cover reload; both `None` without a cover url.
    pub thumbnail_url: Option<String>,
    pub cover_last_modified: Option<i64>,
}

/// The change `details` makes to `entry`.
pub fn plan_change(entry: &LibraryEntry, details: &SourceDetails, now_ms: i64) -> MetadataChange {
    let title = details.title.as_deref().filter(|t| !t.is_empty() && !entry.favorite).map(str::to_owned);
    let thumbnail_url = details.thumbnail_url.as_deref().filter(|u| !u.is_empty()).map(str::to_owned);
    let cover_last_modified = thumbnail_url.as_ref().map(|_| now_ms);
    MetadataChange { title, status: details.status, thumbnail_url, cover_last_modified }
}

/// Writes the details of one entry (`UpdateManga.await`, `UpdateAnimeFromRemote.awaitUpdateFromSource`).
pub trait MetadataStore: Send + Sync {
    fn apply(
        &self,
        entry: &LibraryEntry,
        details: &SourceDetails,
        now_ms: i64,
    ) -> impl Future<Output = Result<(), UpdateError>> + Send;
}

/// The writer over the two databases. The repository calls block, so they run on tokio's blocking pool.
pub struct DbMetadataStore {
    manga: Arc<Db>,
    anime: Arc<Db>,
}

impl DbMetadataStore {
    pub fn new(manga: Arc<Db>, anime: Arc<Db>) -> Self {
        Self { manga, anime }
    }
}

impl MetadataStore for DbMetadataStore {
    async fn apply(&self, entry: &LibraryEntry, details: &SourceDetails, now_ms: i64) -> Result<(), UpdateError> {
        let change = plan_change(entry, details, now_ms);
        let (id, kind) = (entry.id, entry.kind);
        let (manga, anime) = (Arc::clone(&self.manga), Arc::clone(&self.anime));
        tokio::task::spawn_blocking(move || match kind {
            EntryKind::Manga => MangaRepo::new(manga).update(&MangaUpdate {
                title: change.title,
                status: Some(change.status),
                thumbnail_url: change.thumbnail_url,
                cover_last_modified: change.cover_last_modified,
                initialized: Some(true),
                ..MangaUpdate::new(id)
            }),
            EntryKind::Anime => AnimeRepo::new(anime).update(&AnimeUpdate {
                title: change.title,
                status: Some(change.status),
                thumbnail_url: change.thumbnail_url,
                cover_last_modified: change.cover_last_modified,
                initialized: Some(true),
                ..AnimeUpdate::new(id)
            }),
        })
        .await
        .map_err(|e| UpdateError::Other(e.to_string()))?
        .map_err(|e| UpdateError::Other(e.to_string()))
    }
}

/// The collaborators of a metadata pass.
pub struct MetadataDeps<R, S, M> {
    pub registry: R,
    pub store: S,
    pub writer: M,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataStatus {
    /// A pass of this kind is already running; nothing started.
    Refused,
    Completed,
    Cancelled,
    /// The library could not be read.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataOutcome {
    pub status: MetadataStatus,
    /// Entries in the queue (distinct by id).
    pub queued: usize,
    pub updated: usize,
    /// Entries whose source is not installed: skipped, no error recorded.
    pub skipped: usize,
    /// Entries whose refresh or write failed: logged and ignored.
    pub failed: usize,
}

impl MetadataOutcome {
    fn ended(status: MetadataStatus) -> Self {
        Self { status, queued: 0, updated: 0, skipped: 0, failed: 0 }
    }
}

#[derive(Default)]
struct Counts {
    updated: AtomicUsize,
    skipped: AtomicUsize,
    failed: AtomicUsize,
}

/// The metadata passes of the app: one lock per kind, manual starts only.
pub struct MetadataRunner<R, S, M> {
    deps: Arc<MetadataDeps<R, S, M>>,
    locks: Arc<MetadataLocks>,
}

impl<R, S, M> MetadataRunner<R, S, M>
where
    R: SourceRegistry + 'static,
    S: LibraryStore + 'static,
    M: MetadataStore + 'static,
{
    pub fn new(deps: MetadataDeps<R, S, M>) -> Self {
        Self { deps: Arc::new(deps), locks: Arc::new(MetadataLocks::default()) }
    }

    pub fn locks(&self) -> &Arc<MetadataLocks> {
        &self.locks
    }

    /// Runs a pass of `kind` on the caller's task. Refused when a pass of that kind is running.
    pub async fn run(&self, kind: EntryKind, now_ms: i64) -> MetadataOutcome {
        match MetadataLocks::acquire(&self.locks, kind) {
            Some(guard) => self.run_with(guard, kind, now_ms).await,
            None => MetadataOutcome::ended(MetadataStatus::Refused),
        }
    }

    /// The manual start: `false` without starting when a pass of `kind` is running, otherwise the pass runs on a
    /// spawned task. Call from inside the runtime.
    pub fn start_now(self: &Arc<Self>, kind: EntryKind, now_ms: i64) -> bool {
        let Some(guard) = MetadataLocks::acquire(&self.locks, kind) else { return false };
        let this = Arc::clone(self);
        tokio::spawn(async move {
            this.run_with(guard, kind, now_ms).await;
        });
        true
    }

    pub fn stop(&self, kind: EntryKind) -> bool {
        self.locks.stop(kind)
    }

    pub fn is_running(&self, kind: EntryKind) -> bool {
        self.locks.is_running(kind)
    }

    async fn run_with(&self, guard: MetadataGuard, kind: EntryKind, now_ms: i64) -> MetadataOutcome {
        let deps = Arc::clone(&self.deps);
        let library = match deps.store.library(kind).await {
            Ok(library) => library,
            Err(error) => return MetadataOutcome::ended(MetadataStatus::Failed(error.user_message())),
        };
        let queue = distinct_by_id(library);
        let queued = queue.len();
        let counts = Arc::new(Counts::default());
        let permits = Arc::new(Semaphore::new(MAX_SOURCES_IN_PARALLEL));
        let mut tasks = JoinSet::new();
        for group in group_by_source(queue) {
            let (deps, counts, permits) = (Arc::clone(&deps), Arc::clone(&counts), Arc::clone(&permits));
            let cancel = Arc::clone(&guard.cancel);
            tasks.spawn(async move {
                let _permit = permits.acquire_owned().await.expect("metadata semaphore is never closed");
                for entry in group {
                    if cancel.load(Ordering::SeqCst) {
                        break;
                    }
                    refresh_entry(&deps, &counts, &entry, now_ms).await;
                }
            });
        }
        while let Some(joined) = tasks.join_next().await {
            if let Err(error) = joined {
                tracing::error!(%error, "metadata update entry task ended abnormally");
            }
        }
        let status = if guard.cancel.load(Ordering::SeqCst) { MetadataStatus::Cancelled } else { MetadataStatus::Completed };
        MetadataOutcome {
            status,
            queued,
            updated: counts.updated.load(Ordering::SeqCst),
            skipped: counts.skipped.load(Ordering::SeqCst),
            failed: counts.failed.load(Ordering::SeqCst),
        }
    }
}

/// One entry: a missing source is skipped without an error; any other failure is logged and ignored.
async fn refresh_entry<R, S, M>(deps: &MetadataDeps<R, S, M>, counts: &Counts, entry: &LibraryEntry, now_ms: i64)
where
    R: SourceRegistry,
    M: MetadataStore,
{
    let Some(source) = deps.registry.get(entry.source) else {
        counts.skipped.fetch_add(1, Ordering::SeqCst);
        return;
    };
    let details = {
        let _gate = politeness::acquire(entry.source).await;
        source.refresh(entry.kind, &entry.url, RemoteFlags::DETAILS_ONLY).await
    };
    let result = match details {
        Ok(Some(details)) => deps.writer.apply(entry, &details, now_ms).await.map(|()| true),
        Ok(None) => Ok(false),
        Err(error) => Err(error),
    };
    match result {
        Ok(true) => {
            counts.updated.fetch_add(1, Ordering::SeqCst);
        }
        Ok(false) => {}
        Err(UpdateError::Cancelled) => {}
        Err(error) => {
            tracing::warn!(entry = entry.id, message = %error.user_message(), "metadata update of an entry failed");
            counts.failed.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_update::fake::{FakeStore, entry};
    use crate::library_update::pass::{PassLocks, Trigger};
    use crate::library_update::source::fake::{FakeRegistry, FakeSource};

    #[derive(Default)]
    struct FakeWriter {
        applied: Mutex<Vec<i64>>,
    }

    impl MetadataStore for FakeWriter {
        async fn apply(&self, entry: &LibraryEntry, _details: &SourceDetails, _now_ms: i64) -> Result<(), UpdateError> {
            self.applied.lock().unwrap().push(entry.id);
            Ok(())
        }
    }

    type TestRunner = MetadataRunner<FakeRegistry, FakeStore, FakeWriter>;

    fn runner(store: FakeStore, registry: FakeRegistry) -> Arc<TestRunner> {
        Arc::new(MetadataRunner::new(MetadataDeps { registry, store, writer: FakeWriter::default() }))
    }

    fn anime(id: i64, source: i64, title: &str) -> LibraryEntry {
        LibraryEntry { kind: EntryKind::Anime, ..entry(id, source, title) }
    }

    fn details() -> SourceDetails {
        SourceDetails { title: Some("New".into()), status: 2, thumbnail_url: Some("https://x/c.jpg".into()) }
    }

    #[test]
    fn each_kind_has_its_own_lock_name_and_starting_one_does_not_refuse_the_other() {
        // T-26a-051
        assert_ne!(MANGA_LOCK, ANIME_LOCK);
        let locks = Arc::new(MetadataLocks::default());
        let manga = MetadataLocks::acquire(&locks, EntryKind::Manga).expect("manga metadata starts");
        assert!(MetadataLocks::acquire(&locks, EntryKind::Manga).is_none(), "a second manga pass is refused");
        let anime = MetadataLocks::acquire(&locks, EntryKind::Anime).expect("anime metadata is not refused by manga");
        drop(manga);
        assert!(!locks.is_running(EntryKind::Manga) && locks.is_running(EntryKind::Anime));
        drop(anime);
        assert!(MetadataLocks::acquire(&locks, EntryKind::Manga).is_some(), "dropping the guard frees the name");
    }

    #[test]
    fn a_metadata_pass_and_an_update_pass_never_refuse_each_other() {
        // T-26a-051: the metadata locks are not the update pass locks.
        let metadata = Arc::new(MetadataLocks::default());
        let updates = Arc::new(PassLocks::default());
        let _update = PassLocks::acquire(&updates, EntryKind::Manga, Trigger::Manual).expect("update pass");
        assert!(MetadataLocks::acquire(&metadata, EntryKind::Manga).is_some());
        let _meta = MetadataLocks::acquire(&metadata, EntryKind::Anime).expect("anime metadata");
        assert!(PassLocks::acquire(&updates, EntryKind::Anime, Trigger::Manual).is_ok());
    }

    #[test]
    fn the_queue_is_distinct_by_entry_id() {
        // P-13: an entry in two categories is queued once, in first-seen order.
        let mut twin = entry(1, 7, "A");
        twin.category = 4;
        let queue = distinct_by_id(vec![entry(1, 7, "A"), entry(2, 7, "B"), twin]);
        assert_eq!(queue.iter().map(|e| e.id).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(queue[0].category, 0, "the first row wins");
    }

    #[tokio::test]
    async fn a_duplicated_library_row_is_refreshed_once() {
        let mut twin = entry(1, 7, "A");
        twin.category = 4;
        let mut registry = FakeRegistry::default();
        let source = FakeSource::ok(vec![]).with_details(details());
        let refreshes = Arc::clone(&source.refreshes);
        registry.sources.insert(7, source);
        let runner = runner(FakeStore::with(vec![entry(1, 7, "A"), twin]), registry);
        let outcome = runner.run(EntryKind::Manga, 10).await;
        assert_eq!((outcome.queued, outcome.updated), (1, 1));
        assert_eq!(refreshes.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_missing_source_is_skipped_with_no_error() {
        // T-26a-053
        let runner = runner(FakeStore::with(vec![entry(1, 99, "A")]), FakeRegistry::default());
        let outcome = runner.run(EntryKind::Manga, 10).await;
        assert_eq!(outcome.status, MetadataStatus::Completed);
        assert_eq!((outcome.skipped, outcome.failed, outcome.updated), (1, 0, 0));
        assert!(runner.deps.writer.applied.lock().unwrap().is_empty(), "nothing is written");
    }

    #[tokio::test]
    async fn an_entry_error_is_ignored_and_the_pass_completes() {
        // T-26a-053
        let mut registry = FakeRegistry::default();
        registry.sources.insert(1, FakeSource::ok(vec![]).failing_refresh(UpdateError::Other("boom".into())));
        registry.sources.insert(2, FakeSource::ok(vec![]).with_details(details()));
        let runner = runner(FakeStore::with(vec![entry(1, 1, "A"), entry(2, 2, "B")]), registry);
        let outcome = runner.run(EntryKind::Manga, 10).await;
        assert_eq!(outcome.status, MetadataStatus::Completed);
        assert_eq!((outcome.failed, outcome.updated), (1, 1), "the failing entry did not stop the other");
        assert_eq!(*runner.deps.writer.applied.lock().unwrap(), vec![2]);
    }

    #[tokio::test]
    async fn the_anime_pass_asks_for_details_only_and_never_the_episode_list() {
        // T-26a-052
        let mut registry = FakeRegistry::default();
        let source = FakeSource::ok(vec![]).with_details(details());
        let (refreshes, list_calls) = (Arc::clone(&source.refreshes), Arc::clone(&source.calls));
        registry.sources.insert(5, source);
        let runner = runner(FakeStore::with(vec![anime(3, 5, "Frieren")]), registry);
        let outcome = runner.run(EntryKind::Anime, 10).await;
        assert_eq!(outcome.updated, 1);
        let flags = refreshes.lock().unwrap().clone();
        assert_eq!(flags, vec![RemoteFlags { fetch_details: true, fetch_items: false }]);
        assert_eq!(list_calls.load(Ordering::SeqCst), 0, "the episode list endpoint is not called");
    }

    #[tokio::test]
    async fn a_pass_is_refused_while_one_of_its_kind_runs_and_the_other_kind_still_starts() {
        // T-26a-050 and T-26a-051
        let runner = runner(FakeStore::with(vec![entry(1, 1, "A")]), FakeRegistry::default());
        let held = MetadataLocks::acquire(runner.locks(), EntryKind::Manga).expect("running manga pass");
        assert_eq!(runner.run(EntryKind::Manga, 1).await.status, MetadataStatus::Refused);
        assert!(!Arc::clone(&runner).start_now(EntryKind::Manga, 1), "the manual start is refused too");
        assert_eq!(runner.run(EntryKind::Anime, 1).await.status, MetadataStatus::Completed);
        drop(held);
        assert!(!runner.is_running(EntryKind::Manga));
    }

    #[tokio::test]
    async fn a_stopped_pass_ends_cancelled() {
        let mut registry = FakeRegistry::default();
        registry.sources.insert(1, FakeSource::ok(vec![]).with_details(details()));
        let runner = runner(FakeStore::with(vec![entry(1, 1, "A"), entry(2, 1, "B")]), registry);
        let guard = MetadataLocks::acquire(runner.locks(), EntryKind::Manga).expect("lock");
        assert!(runner.stop(EntryKind::Manga));
        let outcome = runner.run_with(guard, EntryKind::Manga, 1).await;
        assert_eq!(outcome.status, MetadataStatus::Cancelled);
        assert_eq!(outcome.updated, 0, "no entry runs after the stop");
    }

    #[test]
    fn the_title_changes_only_for_a_non_favourite_and_an_empty_cover_url_keeps_the_cover() {
        // behaviour 5.3
        let mut entry = entry(1, 1, "Old");
        let change = plan_change(&entry, &details(), 50);
        assert_eq!(change.title, None, "a favourite keeps its title");
        assert_eq!((change.thumbnail_url.as_deref(), change.cover_last_modified), (Some("https://x/c.jpg"), Some(50)));
        entry.favorite = false;
        assert_eq!(plan_change(&entry, &details(), 50).title.as_deref(), Some("New"));
        let blank = SourceDetails { title: Some(String::new()), status: 1, thumbnail_url: Some(String::new()) };
        let change = plan_change(&entry, &blank, 50);
        assert_eq!((change.title, change.thumbnail_url, change.cover_last_modified), (None, None, None));
    }

    fn open_dbs(dir: &std::path::Path) -> (Arc<Db>, Arc<Db>) {
        let manga_path = dir.join("manga.db");
        let manga = Arc::new(Db::new(&manga_path, kuro_db::schema_manga::open(&manga_path).expect("manga db")).expect("handle"));
        let anime_path = dir.join("anime.db");
        let anime = Arc::new(Db::new(&anime_path, kuro_db::schema_anime::open(&anime_path).expect("anime db")).expect("handle"));
        (manga, anime)
    }

    #[tokio::test]
    async fn the_database_writer_stores_status_and_cover_and_the_title_of_a_non_favourite() {
        use kuro_data::manga::Manga;
        let dir = tempfile::tempdir().expect("temp dir");
        let (manga_db, anime_db) = open_dbs(dir.path());
        let id = MangaRepo::new(manga_db.clone())
            .insert(&Manga { favorite: false, source: 7, url: "/m".into(), title: "Old".into(), ..Manga::create() })
            .expect("insert");
        let writer = DbMetadataStore::new(manga_db.clone(), anime_db);
        let mut target = entry(id, 7, "Old");
        target.favorite = false;
        writer.apply(&target, &details(), 777).await.expect("write");
        let stored = MangaRepo::new(manga_db).by_id(id).expect("row");
        assert_eq!(stored.title, "New");
        assert_eq!(stored.status, 2);
        assert_eq!(stored.thumbnail_url.as_deref(), Some("https://x/c.jpg"));
        assert_eq!(stored.cover_last_modified, 777);
    }
}
