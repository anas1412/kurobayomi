//! The download layer as the Updates and Storage screens see it (T-42a-029, T-42a-031, T-42a-051, T-42a-054, T-42b-002).
//!
//! The screens talk to a [`DownloadBackend`] seam: the queue state, the disk check, the source check and the write
//! actions. The shell installs a [`LiveBackend`] over the one download service when it starts the downloads; before that
//! the default [`IndexBackend`] answers the disk check, the queue state and the usage from the service's index and queue
//! (when it runs) and reports that nothing can be queued or deleted. The [`Hub`] carries the cache or queue change
//! signal that re-derives the visible rows.

use std::sync::{Arc, OnceLock, RwLock};

use kuro_data::item::ItemRepo;
use kuro_db::handle::Db;
use kuro_downloads::next::delete::DeleteRules;
use kuro_downloads::next::index::DownloadIndex;
use kuro_downloads::next::queue::{CancelTarget, Place, Queue};
use kuro_downloads::next::runner::SourceDirectory as _;
use kuro_downloads::next::service::Downloads;
use kuro_downloads::next::unit::Availability;
use kuro_platform::kind::Kind;

use crate::downloads::directory::AppDirectory;
use crate::updates::download_state::{DiskKey, QueueEntry};
use crate::updates::signals::CacheSignals;

/// What the Updates and Storage screens need from the downloads layer. Methods block, so callers run them off the GTK
/// thread when they write (see [`spawn_detached`]); the reads are in-memory lookups.
pub trait DownloadBackend: Send + Sync {
    /// The queue entry of one unit, when it is queued, running or failed.
    fn queue_entry(&self, kind: Kind, unit_id: i64) -> Option<QueueEntry>;
    /// Whether the unit's files are on disk (the disk check of the download cache).
    fn on_disk(&self, kind: Kind, key: &DiskKey) -> bool;
    /// Whether the source of an entry is installed (a download needs it).
    fn source_installed(&self, kind: Kind, source_id: i64) -> bool;
    /// Queues units of one entry. `external` is the anime "use external downloader" choice read at screen construction.
    /// `false`: nothing was queued.
    fn enqueue(&self, kind: Kind, entry_id: i64, unit_ids: &[i64], external: bool) -> bool;
    /// Starts the queue.
    fn start_queue(&self, kind: Kind);
    /// Starts one unit now.
    fn start_now(&self, kind: Kind, unit_id: i64);
    /// Removes one unit from the queue.
    fn cancel(&self, kind: Kind, unit_id: i64);
    /// Deletes the downloads of these units of one entry.
    fn delete(&self, kind: Kind, entry_id: i64, unit_ids: &[i64]);
    /// Deletes every download of an entry (the Storage delete). `false` when its source is not installed and nothing
    /// was deleted (EC-42-T-15).
    fn delete_entry(&self, kind: Kind, entry_id: i64) -> bool;
    /// The downloaded size in bytes and the unit count of one entry. Blocking; the Storage page calls it off the GTK thread.
    fn entry_usage(&self, kind: Kind, source_id: i64, title: &str) -> (u64, usize);
}

/// The announcements of the downloads layer: `cache` carries the download-cache and queue-membership changes and the
/// `initializing` flag.
pub struct Hub {
    pub cache: CacheSignals,
}

impl Hub {
    fn new() -> Self {
        let cache = CacheSignals::new();
        // The index loads from its snapshot (or starts empty) without a scan the page must wait for. The downloads
        // layer sets the flag while it renews the index, so the Storage page shows "Scanning downloads…" only then.
        cache.set_initializing(false);
        Self { cache }
    }
}

static HUB: OnceLock<Hub> = OnceLock::new();

/// The process-wide hub.
pub fn hub() -> &'static Hub {
    HUB.get_or_init(Hub::new)
}

static BACKEND: RwLock<Option<Arc<dyn DownloadBackend>>> = RwLock::new(None);

/// Installs the downloads layer's backend. The last call wins.
pub fn install(backend: Arc<dyn DownloadBackend>) {
    *BACKEND.write().unwrap_or_else(|e| e.into_inner()) = Some(backend);
    hub().cache.notify_changed();
}

/// The installed backend, or the default over the shell's download index.
pub fn current() -> Arc<dyn DownloadBackend> {
    if let Some(backend) = BACKEND.read().unwrap_or_else(|e| e.into_inner()).clone() {
        return backend;
    }
    let service = crate::shell::host::Shell::get().and_then(|shell| shell.download_service());
    Arc::new(IndexBackend {
        index: service.as_ref().map(|downloads| downloads.index().clone()),
        runtime: crate::shell::host::Shell::get().map(|shell| shell.runtime().clone()),
        queue: service.map(|downloads| downloads.queue().clone()),
    })
}

/// The unit's place in the queue as the screens show it: its state and percent.
fn queue_entry_of(queue: &Queue, kind: Kind, unit_id: i64) -> Option<QueueEntry> {
    let unit = queue.queued(kind, unit_id)?;
    Some(QueueEntry { status: unit.state, progress: i64::from(unit.progress.percent()) })
}

/// The default backend before the service is installed: the disk check, the queue state and the usage read the one
/// index and queue of both kinds, and nothing can be queued or deleted through it.
pub struct IndexBackend {
    index: Option<DownloadIndex>,
    runtime: Option<tokio::runtime::Handle>,
    queue: Option<Queue>,
}

impl DownloadBackend for IndexBackend {
    fn queue_entry(&self, kind: Kind, unit_id: i64) -> Option<QueueEntry> {
        queue_entry_of(self.queue.as_ref()?, kind, unit_id)
    }

    fn on_disk(&self, kind: Kind, key: &DiskKey) -> bool {
        self.index.as_ref().is_some_and(|i| i.is_downloaded(kind, key.source_id, &key.entry_title, &key.unit_name, key.scanlator.as_deref()))
    }

    fn source_installed(&self, _kind: Kind, _source_id: i64) -> bool {
        false
    }

    fn enqueue(&self, _kind: Kind, _entry_id: i64, _unit_ids: &[i64], _external: bool) -> bool {
        false
    }

    fn start_queue(&self, _kind: Kind) {}

    fn start_now(&self, _kind: Kind, _unit_id: i64) {}

    fn cancel(&self, _kind: Kind, _unit_id: i64) {}

    fn delete(&self, _kind: Kind, _entry_id: i64, _unit_ids: &[i64]) {}

    fn delete_entry(&self, _kind: Kind, _entry_id: i64) -> bool {
        false
    }

    fn entry_usage(&self, kind: Kind, source_id: i64, title: &str) -> (u64, usize) {
        let (Some(index), Some(runtime)) = (&self.index, &self.runtime) else { return (0, 0) };
        (runtime.block_on(index.size_entry(kind, source_id, title)), index.unit_count_entry(kind, source_id, title))
    }
}

/// The backend over the one download service, for both kinds. `external` of `enqueue` is not used: the app always passes
/// the diet constant.
pub struct LiveBackend {
    downloads: Downloads,
    directory: AppDirectory,
    db: Arc<Db>,
    runtime: tokio::runtime::Handle,
}

impl LiveBackend {
    pub fn new(downloads: Downloads, db: Arc<Db>, runtime: tokio::runtime::Handle) -> Self {
        Self { downloads, directory: AppDirectory::new(Arc::clone(&db)), db, runtime }
    }

    /// Waits for `work`. Callers run off the GTK thread (see [`spawn_detached`]); one that already runs on the runtime's
    /// pool is let to block there.
    fn block<T>(&self, work: impl std::future::Future<Output = T>) -> T {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.runtime.block_on(work))
        } else {
            self.runtime.block_on(work)
        }
    }
}

impl DownloadBackend for LiveBackend {
    fn queue_entry(&self, kind: Kind, unit_id: i64) -> Option<QueueEntry> {
        queue_entry_of(self.downloads.queue(), kind, unit_id)
    }

    fn on_disk(&self, kind: Kind, key: &DiskKey) -> bool {
        self.downloads.index().is_downloaded(kind, key.source_id, &key.entry_title, &key.unit_name, key.scanlator.as_deref())
    }

    fn source_installed(&self, kind: Kind, source_id: i64) -> bool {
        self.directory.source_key(kind, source_id).is_some_and(|source| source.availability == Availability::Live)
    }

    fn enqueue(&self, kind: Kind, entry_id: i64, unit_ids: &[i64], _external: bool) -> bool {
        // paused: the caller starts the queue once (a hand pick keeps its fillermarked units, D6)
        let Some(request) = self.directory.enqueue_request(kind, entry_id, unit_ids, Place::Back, false, false) else { return false };
        match self.block(self.downloads.enqueue(request)) {
            Ok(done) => !done.added.is_empty(),
            Err(error) => {
                tracing::warn!(%error, entry_id, "the units were not queued");
                false
            }
        }
    }

    fn start_queue(&self, kind: Kind) {
        self.downloads.start(kind);
    }

    fn start_now(&self, kind: Kind, unit_id: i64) {
        let entry_id = ItemRepo::new(Arc::clone(&self.db)).by_id(unit_id).ok().flatten().map(|item| item.entry_id);
        let request = entry_id.and_then(|id| self.directory.enqueue_request(kind, id, &[unit_id], Place::Front, true, false));
        let Some(request) = request else { return };
        if let Err(error) = self.block(self.downloads.start_now(request)) {
            tracing::debug!(%error, unit_id, "the unit was not started now");
        }
    }

    fn cancel(&self, kind: Kind, unit_id: i64) {
        if let Err(error) = self.block(self.downloads.cancel(kind, CancelTarget::Units(vec![unit_id]))) {
            tracing::warn!(%error, unit_id, "the unit was not cancelled");
        }
    }

    fn delete(&self, kind: Kind, entry_id: i64, unit_ids: &[i64]) {
        let Some((entry, source)) = self.directory.entry_context(kind, entry_id) else { return };
        let units = self.directory.units(entry_id, unit_ids);
        if let Err(error) = self.block(self.downloads.delete_units(&entry, &source, units, DeleteRules::EXPLICIT)) {
            tracing::warn!(%error, entry_id, "the downloads were not deleted");
        }
    }

    fn delete_entry(&self, kind: Kind, entry_id: i64) -> bool {
        let Some((entry, source)) = self.directory.entry_context(kind, entry_id) else { return false };
        match self.block(self.downloads.delete_entry(&entry, &source, true)) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, entry_id, "the entry's downloads were not deleted");
                false
            }
        }
    }

    fn entry_usage(&self, kind: Kind, source_id: i64, title: &str) -> (u64, usize) {
        self.block(self.downloads.entry_usage(kind, source_id, title, None))
    }
}

/// Runs `work` detached from the screen: on the shell's runtime when it exists, else on a thread. The work is not
/// cancelled when the screen goes away. It announces a cache change when it ends, so the rows re-derive.
pub fn spawn_detached(work: impl FnOnce() + Send + 'static) {
    let job = move || {
        work();
        hub().cache.notify_changed();
    };
    match crate::shell::host::Shell::get() {
        Some(shell) => {
            shell.runtime().spawn_blocking(job);
        }
        None => {
            std::thread::spawn(job);
        }
    }
}

/// Runs `work` off the GTK thread and yields its result (`None` when the task panicked). Without a shell (unit tests)
/// it runs inline. Call it on the GTK thread: the shell is looked up there.
pub async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    match crate::shell::host::Shell::get() {
        Some(shell) => shell.runtime().spawn_blocking(work).await.ok(),
        None => Some(work()),
    }
}

#[cfg(test)]
pub mod fake {
    //! A recording backend for the tests of the executor and the screens' pure parts.

    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;
    use crate::updates::download_state::DownloadState;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Call {
        Enqueue { entry_id: i64, unit_ids: Vec<i64>, external: bool },
        StartQueue,
        StartNow(i64),
        Cancel(i64),
        Delete { entry_id: i64, unit_ids: Vec<i64> },
        DeleteEntry(i64),
    }

    #[derive(Default)]
    pub struct Fake {
        pub calls: Mutex<Vec<Call>>,
        pub queue: Mutex<HashMap<i64, QueueEntry>>,
        pub disk: Mutex<Vec<i64>>,
        pub installed: bool,
    }

    impl Fake {
        pub fn installed() -> Self {
            Self { installed: true, ..Self::default() }
        }

        pub fn calls(&self) -> Vec<Call> {
            self.calls.lock().unwrap().clone()
        }

        pub fn queue(&self, unit_id: i64, status: DownloadState, progress: i64) {
            self.queue.lock().unwrap().insert(unit_id, QueueEntry { status, progress });
        }
    }

    impl DownloadBackend for Fake {
        fn queue_entry(&self, _: Kind, unit_id: i64) -> Option<QueueEntry> {
            self.queue.lock().unwrap().get(&unit_id).copied()
        }
        fn on_disk(&self, _: Kind, key: &DiskKey) -> bool {
            self.disk.lock().unwrap().iter().any(|id| key.unit_name == format!("c{id}"))
        }
        fn source_installed(&self, _: Kind, _: i64) -> bool {
            self.installed
        }
        fn enqueue(&self, _: Kind, entry_id: i64, unit_ids: &[i64], external: bool) -> bool {
            self.calls.lock().unwrap().push(Call::Enqueue { entry_id, unit_ids: unit_ids.to_vec(), external });
            true
        }
        fn start_queue(&self, _: Kind) {
            self.calls.lock().unwrap().push(Call::StartQueue);
        }
        fn start_now(&self, _: Kind, unit_id: i64) {
            self.calls.lock().unwrap().push(Call::StartNow(unit_id));
        }
        fn cancel(&self, _: Kind, unit_id: i64) {
            self.calls.lock().unwrap().push(Call::Cancel(unit_id));
        }
        fn delete(&self, _: Kind, entry_id: i64, unit_ids: &[i64]) {
            self.calls.lock().unwrap().push(Call::Delete { entry_id, unit_ids: unit_ids.to_vec() });
        }
        fn delete_entry(&self, _: Kind, entry_id: i64) -> bool {
            self.calls.lock().unwrap().push(Call::DeleteEntry(entry_id));
            self.installed
        }
        fn entry_usage(&self, _: Kind, _: i64, _: &str) -> (u64, usize) {
            (0, 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_downloads::next::index::IndexRoots;
    use kuro_downloads::next::runner::{RunCtx, UnitDone, UnitJob, UnitRunner};
    use kuro_downloads::next::unit::{SourceKey, UnitRef};
    use crate::updates::download_state::DownloadState;

    #[test]
    fn the_default_backend_has_no_queue_and_refuses_writes() {
        let backend = IndexBackend { index: None, runtime: None, queue: None };
        assert_eq!(backend.queue_entry(Kind::Manga, 1), None);
        assert!(!backend.enqueue(Kind::Manga, 1, &[2], false));
        assert!(!backend.delete_entry(Kind::Manga, 1));
        assert!(!backend.source_installed(Kind::Anime, 3));
        assert_eq!(backend.entry_usage(Kind::Manga, 1, "T"), (0, 0));
    }

    #[test]
    fn the_default_disk_check_reads_the_index_for_both_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let roots = IndexRoots { manga: dir.path().join("manga"), anime: dir.path().join("anime") };
        let index = DownloadIndex::open(roots, dir.path());
        index.add_unit(Kind::Manga, 7, "Source (EN)", "Title", "Ch 1");
        index.add_unit(Kind::Anime, 7, "Source (EN)", "Title", "Episode 1");
        let backend = IndexBackend { index: Some(index), runtime: None, queue: None };
        let key = DiskKey { source_id: 7, entry_title: "Title".into(), unit_name: "Ch 1".into(), scanlator: None };
        assert!(backend.on_disk(Kind::Manga, &key));
        assert!(!backend.on_disk(Kind::Anime, &key), "the other kind has no such unit");
        let episode = DiskKey { unit_name: "Episode 1".into(), ..key.clone() };
        assert!(backend.on_disk(Kind::Anime, &episode));
        let other = DiskKey { unit_name: "Ch 2".into(), ..key };
        assert!(!backend.on_disk(Kind::Manga, &other));
    }

    /// A runner that cannot run any source: its units queue and wait.
    struct Idle(Kind);

    #[async_trait::async_trait]
    impl UnitRunner for Idle {
        fn kind(&self) -> Kind {
            self.0
        }
        fn can_run(&self, _: &SourceKey) -> bool {
            false
        }
        fn staging_names(&self, _: &UnitRef) -> Vec<String> {
            Vec::new()
        }
        async fn run(&self, _: UnitJob, _: RunCtx) -> Result<UnitDone, kuro_downloads::DownloadError> {
            Err(kuro_downloads::DownloadError::Internal("idle".to_owned()))
        }
    }

    /// A service over runners that run nothing: units queue and wait, nothing downloads.
    fn live_backend(rt: &tokio::runtime::Runtime) -> (tempfile::TempDir, Arc<Db>, LiveBackend) {
        use kuro_downloads::next::index::IndexRoots;
        use kuro_downloads::next::layout::Layout;
        use kuro_downloads::next::pending::PendingStore;
        use kuro_downloads::next::queue::{Layouts, QueueParams};
        use kuro_downloads::next::store::QueueStore;

        let dir = tempfile::tempdir().unwrap();
        let path = kuro_db::schema_unified::db_path(dir.path());
        let db = Arc::new(Db::new(&path, kuro_db::schema_unified::open_file(&path).unwrap()).unwrap());
        let directory = Arc::new(AppDirectory::new(db.clone()));
        let layouts = || Layouts {
            manga: Layout::new(Kind::Manga, dir.path().join("manga")),
            anime: Layout::new(Kind::Anime, dir.path().join("anime")),
        };
        let roots = IndexRoots { manga: dir.path().join("manga"), anime: dir.path().join("anime") };
        let index = DownloadIndex::open(roots, dir.path());
        let (_tx, connectivity) = tokio::sync::watch::channel(kuro_downloads::next::gate::Connectivity { online: true, metered: false });
        let queue = {
            let _entered = rt.enter();
            Queue::spawn(QueueParams {
                db: db.clone(),
                store: Arc::new(QueueStore::new(db.clone())),
                runners: Kind::ALL.map(|kind| Arc::new(Idle(kind)) as Arc<dyn UnitRunner>).to_vec(),
                directory: directory.clone(),
                layouts: layouts(),
                index: index.clone(),
                free_space: |_| Some(u64::MAX),
                max_active: Arc::new(|| 1),
                connectivity,
                only_over_wifi: false,
            })
        };
        let downloads = Downloads::new(queue, index, layouts(), Arc::new(PendingStore::new(db.clone())), directory, db.clone());
        (dir, db.clone(), LiveBackend::new(downloads, db, rt.handle().clone()))
    }

    #[test]
    fn the_live_backend_queues_and_cancels_units_of_both_kinds() {
        use kuro_data::entry::{Entry, EntryRepo};
        use kuro_data::item::Item;
        use kuro_data::source::StubSourceRepo;

        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        let (_dir, db, backend) = live_backend(&rt);
        for kind in Kind::ALL {
            StubSourceRepo::new(db.clone(), kind).upsert(77, "en", "Site").unwrap();
            let entry = Entry { url: "/e".into(), title: "Title".into(), source: 77, status: 1, ..Entry::create(kind) };
            let entry_id = EntryRepo::new(db.clone()).insert(&entry).unwrap();
            let item = Item { entry_id, url: "/e/1".into(), name: "Unit 1".into(), ..Item::create(kind) };
            let unit_id = ItemRepo::new(db.clone()).add_all(&[item]).unwrap()[0].id;
            assert!(!backend.source_installed(kind, 77), "a stub is not installed");
            assert_eq!(backend.queue_entry(kind, unit_id), None);
            assert!(backend.enqueue(kind, entry_id, &[unit_id], false), "{kind:?} is queued");
            assert!(!backend.enqueue(kind, entry_id, &[unit_id], false), "a queued unit is not queued twice");
            let queued = backend.queue_entry(kind, unit_id).expect("the unit is in the queue");
            assert_eq!(queued.status, DownloadState::Queue);
            backend.cancel(kind, unit_id);
            assert_eq!(backend.queue_entry(kind, unit_id), None);
        }
    }

    #[test]
    fn the_hub_starts_not_initializing() {
        assert!(!*hub().cache.initializing().borrow());
    }

    #[test]
    fn detached_work_runs_and_announces_a_change() {
        let mut changes = hub().cache.subscribe();
        let (tx, rx) = std::sync::mpsc::channel();
        spawn_detached(move || tx.send(7).unwrap());
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), 7);
        for _ in 0..50 {
            if changes.try_recv().is_ok() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("no cache change was announced");
    }

    #[test]
    fn blocking_without_a_shell_runs_inline() {
        let value = glib_free_block_on(blocking(|| 41 + 1));
        assert_eq!(value, Some(42));
    }

    /// Polls a future that is ready on its first poll.
    fn glib_free_block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        let mut future = Box::pin(future);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("the future was not ready"),
        }
    }
}
