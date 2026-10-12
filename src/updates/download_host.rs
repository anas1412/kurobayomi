//! The download layer as the Updates and Storage screens see it (T-42a-029, T-42a-031, T-42a-051, T-42a-054, T-42b-002).
//!
//! The app has no live download manager object yet, so the screens talk to a [`DownloadBackend`] seam: the queue
//! snapshot, the disk check, the source check and the four write actions. [`install`] replaces the default backend
//! when the downloads layer is wired; until then the default answers the disk check from the shell's download index
//! (manga), its queue state from the running queues (`downloads::live`), and reports that nothing can be queued or
//! deleted. The [`Hub`] carries the cache or queue change signal that re-derives the visible rows; the live status and
//! progress of each unit come from `downloads::live`.

use std::sync::{Arc, OnceLock, RwLock};

use kuro_platform::kind::Kind;
use kuro_downloads::index::DownloadIndex;

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
    Arc::new(IndexBackend {
        index: crate::shell::host::Shell::get().map(|shell| shell.downloads().clone()),
        runtime: crate::shell::host::Shell::get().map(|shell| shell.runtime().clone()),
        queues: crate::downloads::live::queues(),
    })
}

/// The default backend: the disk check reads the shell's download index (manga only), the queue state reads the
/// running queues, and nothing can be queued or deleted through it.
pub struct IndexBackend {
    index: Option<DownloadIndex>,
    runtime: Option<tokio::runtime::Handle>,
    queues: Option<crate::downloads::Queues>,
}

impl DownloadBackend for IndexBackend {
    fn queue_entry(&self, kind: Kind, unit_id: i64) -> Option<QueueEntry> {
        let (status, percent) = crate::downloads::live::queued(self.queues.as_ref()?, kind, unit_id)?;
        Some(QueueEntry { status, progress: i64::from(percent) })
    }

    fn on_disk(&self, kind: Kind, key: &DiskKey) -> bool {
        kind == Kind::Manga
            && self.index.as_ref().is_some_and(|i| i.is_chapter_downloaded(key.source_id, &key.entry_title, &key.unit_name, key.scanlator.as_deref()))
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
        let (Kind::Manga, Some(index), Some(runtime)) = (kind, &self.index, &self.runtime) else { return (0, 0) };
        (runtime.block_on(index.size_manga(source_id, title)), index.chapter_count_manga(source_id, title))
    }
}

/// The anime download index (T-42a-045): over the anime downloads folder of `storage`, with its own snapshot file in the
/// platform cache folder. Opened on the first call and shared (later calls get the same index); `None` when the
/// platform folders cannot be resolved. It rescans after each load of the repository sources, so their downloads are
/// recognised; open it inside the app's runtime.
pub fn anime_index(storage: &kuro_domain::storage::StorageManager) -> Option<kuro_downloads::anime::cache::AnimeIndex> {
    static INDEX: OnceLock<Option<kuro_downloads::anime::cache::AnimeIndex>> = OnceLock::new();
    INDEX
        .get_or_init(|| {
            use kuro_platform::paths::Paths as _;
            let paths = kuro_platform::linux::paths::LinuxPaths::system().ok()?;
            let cache = paths.cache_dir();
            if let Err(e) = std::fs::create_dir_all(&cache) {
                tracing::warn!("the anime download index snapshot folder was not created: {e}");
            }
            let index = kuro_downloads::anime::cache::AnimeIndex::open(storage.downloads_path(Kind::Anime), &cache);
            let follower = index.clone();
            crate::recipe_sources::follow_loads(move || {
                let index = follower.clone();
                async move {
                    index.rescan(crate::recipe_sources::ready(), || crate::recipe_sources::folder_names(Kind::Anime)).await;
                }
            });
            Some(index)
        })
        .clone()
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

    #[test]
    fn the_default_backend_has_no_queue_and_refuses_writes() {
        let backend = IndexBackend { index: None, runtime: None, queues: None };
        assert_eq!(backend.queue_entry(Kind::Manga, 1), None);
        assert!(!backend.enqueue(Kind::Manga, 1, &[2], false));
        assert!(!backend.delete_entry(Kind::Manga, 1));
        assert!(!backend.source_installed(Kind::Anime, 3));
        assert_eq!(backend.entry_usage(Kind::Manga, 1, "T"), (0, 0));
    }

    #[test]
    fn the_default_disk_check_reads_the_index_for_manga_only() {
        let dir = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(dir.path(), dir.path());
        index.add_chapter(7, "Source", "Title", "Ch 1");
        let backend = IndexBackend { index: Some(index), runtime: None, queues: None };
        let key = DiskKey { source_id: 7, entry_title: "Title".into(), unit_name: "Ch 1".into(), scanlator: None };
        assert!(backend.on_disk(Kind::Manga, &key));
        assert!(!backend.on_disk(Kind::Anime, &key), "the anime cache is not in the shell yet");
        let other = DiskKey { unit_name: "Ch 2".into(), ..key };
        assert!(!backend.on_disk(Kind::Manga, &other));
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
