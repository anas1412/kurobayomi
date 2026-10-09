//! The preference store: values in memory, change events, and writes through a [`Backend`].
//!
//! Writes are debounced without an async runtime. The first change starts a window of
//! [`FLUSH_DELAY`]; every change inside it shares one save made when the window closes. A thread
//! checks every [`FLUSH_POLL`] and exits when the last clone of the store is dropped. Dropping the
//! last clone saves any pending change first.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::thread;
use std::time::{Duration, Instant};

use crate::memory::MemoryBackend;
use crate::pref::Pref;
use crate::values::{PrefType, PrefValue};
use crate::{PrefError, lock, read, write};

/// Quiet period after the first unsaved change, before the store is written.
pub const FLUSH_DELAY: Duration = Duration::from_millis(250);
/// How often the flusher thread checks whether a save is due.
const FLUSH_POLL: Duration = Duration::from_millis(50);
/// Consecutive failed saves that send one notice to [`PrefStore::save_failures`].
pub const FAILURES_BEFORE_NOTICE: u32 = 3;

/// Persistence behind a store. `load` runs once when the store opens; `save` gets every value.
pub trait Backend: Send + Sync {
    fn load(&self) -> Result<BTreeMap<String, PrefValue>, PrefError>;
    fn save(&self, all: &BTreeMap<String, PrefValue>) -> Result<(), PrefError>;
    /// Whether the store runs a background flusher for this backend. A backend with nothing to write to (memory)
    /// answers false, so no thread is started.
    fn flushes(&self) -> bool {
        true
    }
}

/// A change notification. `key` is `None` when many keys changed at once.
#[derive(Clone, Debug, PartialEq)]
pub struct Changed {
    pub key: Option<String>,
}

/// Shared handle to the values. Clones are cheap.
#[derive(Clone)]
pub struct PrefStore(Arc<Shared>);

struct Shared {
    values: RwLock<BTreeMap<String, PrefValue>>,
    backend: Box<dyn Backend>,
    /// When the oldest unsaved change happened. `None` when nothing is pending.
    dirty_since: Mutex<Option<Instant>>,
    /// Held for a whole save, so an older snapshot never overwrites a newer one.
    save_lock: Mutex<()>,
    subscribers: Mutex<Vec<Sender<Changed>>>,
    /// Failed saves since the last successful one.
    failed_saves: Mutex<u32>,
    /// The error of the last save when it failed; cleared by the next successful save.
    save_error: Mutex<Option<(std::io::ErrorKind, String)>>,
    failure_subscribers: Mutex<Vec<Sender<()>>>,
}

impl PrefStore {
    /// Loads the values from `backend` and starts the flusher.
    pub fn new(backend: Box<dyn Backend>) -> Result<Self, PrefError> {
        let values = backend.load()?;
        let flushes = backend.flushes();
        let shared = Arc::new(Shared {
            values: RwLock::new(values),
            backend,
            dirty_since: Mutex::new(None),
            save_lock: Mutex::new(()),
            subscribers: Mutex::new(Vec::new()),
            failed_saves: Mutex::new(0),
            save_error: Mutex::new(None),
            failure_subscribers: Mutex::new(Vec::new()),
        });
        if flushes {
            spawn_flusher(Arc::downgrade(&shared));
        }
        Ok(Self(shared))
    }

    /// A store with no file behind it, for tests and previews.
    pub fn in_memory() -> Self {
        Self::new(Box::new(MemoryBackend::default()))
            .expect("the memory backend cannot fail to load")
    }

    pub fn get_raw(&self, key: &str) -> Option<PrefValue> {
        read(&self.0.values).get(key).cloned()
    }

    /// Stores `v`. Setting the value already stored does nothing: no event, no write.
    pub fn set_raw(&self, key: &str, v: PrefValue) {
        let changed = {
            let mut values = write(&self.0.values);
            if values.get(key) == Some(&v) {
                false
            } else {
                values.insert(key.to_owned(), v);
                true
            }
        };
        if changed {
            self.0.changed(Some(key));
        }
    }

    pub fn remove(&self, key: &str) {
        let removed = write(&self.0.values).remove(key).is_some();
        if removed {
            self.0.changed(Some(key));
        }
    }

    /// Every stored value, in key order.
    pub fn all(&self) -> Vec<(String, PrefValue)> {
        read(&self.0.values)
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Stores many values with one notification (`key: None`).
    pub fn replace_many(&self, entries: Vec<(String, PrefValue)>) {
        let mut any = false;
        {
            let mut values = write(&self.0.values);
            for (key, v) in entries {
                if values.get(&key) != Some(&v) {
                    values.insert(key, v);
                    any = true;
                }
            }
        }
        if any {
            self.0.changed(None);
        }
    }

    /// A receiver for every change. Dropping it ends the subscription.
    pub fn subscribe(&self) -> Receiver<Changed> {
        let (tx, rx) = mpsc::channel();
        lock(&self.0.subscribers).push(tx);
        rx
    }

    /// A receiver that gets one message after [`FAILURES_BEFORE_NOTICE`] failed saves in a row. No
    /// more messages come until a save succeeds. Values stay readable from memory meanwhile.
    pub fn save_failures(&self) -> Receiver<()> {
        let (tx, rx) = mpsc::channel();
        lock(&self.0.failure_subscribers).push(tx);
        rx
    }

    /// Whether the last save failed. The values stay in memory and the save is retried, so a
    /// write made while this is `Err` is not lost yet; [`Pref::set`] and [`Pref::delete`] return it.
    pub fn status(&self) -> Result<(), PrefError> {
        match &*lock(&self.0.save_error) {
            Some((kind, msg)) => Err(PrefError::Io(std::io::Error::new(*kind, msg.clone()))),
            None => Ok(()),
        }
    }

    /// Saves now, even when the debounce window has not closed.
    pub fn flush(&self) -> Result<(), PrefError> {
        self.0.save()
    }

    /// A typed handle for `key` with the value `default` when the key is not stored.
    pub fn pref<T: PrefType>(&self, key: impl Into<Cow<'static, str>>, default: T) -> Pref<T> {
        Pref::new(self.clone(), key, default)
    }
}

impl Shared {
    fn changed(&self, key: Option<&str>) {
        self.mark_dirty();
        lock(&self.subscribers).retain(|tx| {
            tx.send(Changed {
                key: key.map(str::to_owned),
            })
            .is_ok()
        });
    }

    fn mark_dirty(&self) {
        lock(&self.dirty_since).get_or_insert_with(Instant::now);
    }

    /// Clears the pending flag before the snapshot, so a change made during the save is
    /// marked dirty again and written later.
    fn save(&self) -> Result<(), PrefError> {
        let _saving = lock(&self.save_lock);
        *lock(&self.dirty_since) = None;
        let snapshot = read(&self.values).clone();
        let result = self.backend.save(&snapshot);
        match &result {
            Ok(()) => {
                *lock(&self.failed_saves) = 0;
                *lock(&self.save_error) = None;
            }
            Err(e) => {
                *lock(&self.save_error) = Some((save_error_kind(e), e.to_string()));
                tracing::error!(error = %e, "could not save preferences; retrying");
                // ponytail: fixed retry every FLUSH_DELAY; add backoff if the log gets noisy.
                self.mark_dirty();
                self.count_failure();
            }
        }
        result
    }

    /// Counts a failed save. The notice goes out only when the streak reaches the threshold.
    fn count_failure(&self) {
        let mut failed = lock(&self.failed_saves);
        *failed = failed.saturating_add(1);
        if *failed == FAILURES_BEFORE_NOTICE {
            lock(&self.failure_subscribers).retain(|tx| tx.send(()).is_ok());
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        if lock(&self.dirty_since).is_some() {
            let _ = self.save();
        }
    }
}

fn save_error_kind(e: &PrefError) -> std::io::ErrorKind {
    match e {
        PrefError::Io(io) | PrefError::Dir { source: io, .. } => io.kind(),
        PrefError::Locked => std::io::ErrorKind::WouldBlock,
    }
}

fn spawn_flusher(weak: Weak<Shared>) {
    thread::spawn(move || {
        loop {
            thread::sleep(FLUSH_POLL);
            let Some(shared) = weak.upgrade() else { return };
            let due = lock(&shared.dirty_since).is_some_and(|since| since.elapsed() >= FLUSH_DELAY);
            if due {
                let _ = shared.save();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    /// A memory backend that runs the flusher, so the debounce window can be observed.
    #[derive(Clone, Default)]
    struct Flushing(MemoryBackend);

    impl Backend for Flushing {
        fn load(&self) -> Result<BTreeMap<String, PrefValue>, PrefError> {
            self.0.load()
        }
        fn save(&self, all: &BTreeMap<String, PrefValue>) -> Result<(), PrefError> {
            self.0.save(all)
        }
    }

    fn counted() -> (PrefStore, MemoryBackend) {
        let backend = Flushing::default();
        let store = PrefStore::new(Box::new(backend.clone())).unwrap();
        (store, backend.0)
    }

    /// Backend whose saves fail while the flag is set.
    #[derive(Clone, Default)]
    struct Flaky(Arc<AtomicBool>);

    impl Flaky {
        fn set_failing(&self, failing: bool) {
            self.0.store(failing, Ordering::SeqCst);
        }
    }

    impl Backend for Flaky {
        fn load(&self) -> Result<BTreeMap<String, PrefValue>, PrefError> {
            Ok(BTreeMap::new())
        }

        fn save(&self, _all: &BTreeMap<String, PrefValue>) -> Result<(), PrefError> {
            if self.0.load(Ordering::SeqCst) {
                Err(PrefError::Io(std::io::Error::other("disk full")))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn one_notice_after_three_failed_saves_until_a_save_succeeds() {
        let flaky = Flaky::default();
        let store = PrefStore::new(Box::new(flaky.clone())).unwrap();
        let notices = store.save_failures();
        store.set_raw("k", PrefValue::Int(1));
        flaky.set_failing(true);
        store.flush().unwrap_err();
        store.flush().unwrap_err();
        assert!(notices.try_recv().is_err(), "two failures are not enough");
        store.flush().unwrap_err();
        assert_eq!(notices.try_iter().count(), 1, "the third failure sends one notice");
        store.flush().unwrap_err();
        store.flush().unwrap_err();
        assert_eq!(notices.try_iter().count(), 0, "no more notices until a save succeeds");
        assert_eq!(store.get_raw("k"), Some(PrefValue::Int(1)), "values stay readable");
        flaky.set_failing(false);
        store.flush().unwrap();
        flaky.set_failing(true);
        for _ in 0..FAILURES_BEFORE_NOTICE {
            store.flush().unwrap_err();
        }
        assert_eq!(notices.try_iter().count(), 1, "a new streak notifies again");
    }

    #[test]
    fn status_reports_a_failed_save_until_one_succeeds() {
        let flaky = Flaky::default();
        let store = PrefStore::new(Box::new(flaky.clone())).unwrap();
        store.status().unwrap();
        flaky.set_failing(true);
        store.flush().unwrap_err();
        assert!(store.status().unwrap_err().to_string().contains("disk full"));
        flaky.set_failing(false);
        store.flush().unwrap();
        store.status().unwrap();
    }

    #[test]
    fn set_then_get_returns_the_new_value() {
        let store = PrefStore::in_memory();
        store.set_raw("k", PrefValue::Int(3));
        assert_eq!(store.get_raw("k"), Some(PrefValue::Int(3)));
        store.set_raw("k", PrefValue::Int(4));
        assert_eq!(store.get_raw("k"), Some(PrefValue::Int(4)));
    }

    #[test]
    fn remove_and_all_follow_the_stored_map() {
        let store = PrefStore::in_memory();
        store.set_raw("b", PrefValue::Bool(true));
        store.set_raw("a", PrefValue::Long(1));
        assert_eq!(
            store.all(),
            vec![
                ("a".to_owned(), PrefValue::Long(1)),
                ("b".to_owned(), PrefValue::Bool(true))
            ]
        );
        store.remove("a");
        assert_eq!(store.get_raw("a"), None);
        assert_eq!(store.all().len(), 1);
    }

    #[test]
    fn equal_set_publishes_nothing_and_schedules_no_write() {
        let (store, backend) = counted();
        store.set_raw("k", PrefValue::Bool(true));
        let rx = store.subscribe();
        store.set_raw("k", PrefValue::Bool(true));
        assert!(rx.try_recv().is_err());
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(backend.saves(), 1, "only the first distinct set is written");
    }

    #[test]
    fn subscribers_get_key_events_and_bulk_events() {
        let store = PrefStore::in_memory();
        let rx = store.subscribe();
        store.set_raw("k", PrefValue::Bool(true));
        assert_eq!(
            rx.try_recv().unwrap(),
            Changed {
                key: Some("k".into())
            }
        );
        store.replace_many(vec![
            ("a".into(), PrefValue::Int(1)),
            ("b".into(), PrefValue::Int(2)),
        ]);
        assert_eq!(rx.try_recv().unwrap(), Changed { key: None });
        store.remove("missing");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn a_hundred_sets_in_a_burst_make_one_write_after_the_window() {
        let (store, backend) = counted();
        for i in 0..100 {
            store.set_raw("k", PrefValue::Int(i));
        }
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(backend.saves(), 0, "still inside the debounce window");
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(backend.saves(), 1);
        assert_eq!(backend.saved().get("k"), Some(&PrefValue::Int(99)));
    }

    #[test]
    fn flush_writes_immediately_and_drop_saves_pending_changes() {
        let (store, backend) = counted();
        store.set_raw("k", PrefValue::Int(1));
        store.flush().unwrap();
        assert_eq!(backend.saves(), 1);
        store.set_raw("k", PrefValue::Int(2));
        drop(store);
        // The last drop may run on the flusher thread if it held a clone at that moment.
        let deadline = Instant::now() + Duration::from_secs(2);
        while backend.saved().get("k") != Some(&PrefValue::Int(2)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(backend.saved().get("k"), Some(&PrefValue::Int(2)));
    }

    /// Backend that records the order and overlap of saves.
    #[derive(Clone, Default)]
    struct Slow {
        active: Arc<AtomicBool>,
        overlapped: Arc<AtomicBool>,
        last: Arc<Mutex<Option<PrefValue>>>,
    }

    impl Backend for Slow {
        fn load(&self) -> Result<BTreeMap<String, PrefValue>, PrefError> {
            Ok(BTreeMap::new())
        }

        fn save(&self, all: &BTreeMap<String, PrefValue>) -> Result<(), PrefError> {
            if self.active.swap(true, Ordering::SeqCst) {
                self.overlapped.store(true, Ordering::SeqCst);
            }
            std::thread::sleep(Duration::from_millis(20));
            *lock(&self.last) = all.get("k").cloned();
            self.active.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn concurrent_saves_never_overlap_and_the_newest_snapshot_lands_last() {
        let slow = Slow::default();
        let store = PrefStore::new(Box::new(slow.clone())).unwrap();
        let threads: Vec<_> = (0..8)
            .map(|i| {
                let store = store.clone();
                thread::spawn(move || {
                    store.set_raw("k", PrefValue::Int(i));
                    store.flush().unwrap();
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        store.flush().unwrap();
        assert!(!slow.overlapped.load(Ordering::SeqCst), "two saves ran at once");
        assert_eq!(*lock(&slow.last), store.get_raw("k"));
    }
}
