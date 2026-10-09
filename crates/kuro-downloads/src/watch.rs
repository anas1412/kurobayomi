//! Index invalidation on external changes to the downloads root (T-23a-045). A `notify` watcher on the root (not its
//! subfolders, as Kotlin reacts to the storage root only) forwards each event to a debouncer, and the index is
//! invalidated after [`QUIET_PERIOD`] without events. When the watcher cannot start, the hourly renew of the index stays
//! as the fallback (EC-89).

use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::index::DownloadIndex;

/// Quiet period after the last change before the index is invalidated (T-23a-045).
pub const QUIET_PERIOD: Duration = Duration::from_secs(2);

/// Keeps the watcher alive. Dropping it stops the watch and ends the debouncer thread.
pub struct RootWatch {
    _watcher: RecommendedWatcher,
}

/// Watches the root of `index`. `None` when the watcher cannot start.
pub fn watch_root(index: &DownloadIndex) -> Option<RootWatch> {
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |_event: notify::Result<notify::Event>| {
        let _ = tx.send(());
    })
    .ok()?;
    watcher.watch(index.root(), RecursiveMode::NonRecursive).ok()?;
    let index = index.clone();
    std::thread::Builder::new()
        .name("kuro-downloads-watch".into())
        .spawn(move || debounce(rx, QUIET_PERIOD, || index.invalidate()))
        .ok()?;
    Some(RootWatch { _watcher: watcher })
}

/// Calls `fire` once per burst of events: after the last one, when `quiet` passes with no further event. Returns when
/// the sender is gone, after a final `fire` if a burst was pending.
pub fn debounce(events: mpsc::Receiver<()>, quiet: Duration, mut fire: impl FnMut()) {
    while events.recv().is_ok() {
        loop {
            match events.recv_timeout(quiet) {
                Ok(()) => continue,
                Err(RecvTimeoutError::Timeout) => {
                    fire();
                    break;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    fire();
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn a_burst_fires_once_after_the_quiet_period() {
        let (tx, rx) = mpsc::channel();
        let fired = Arc::new(AtomicUsize::new(0));
        let counter = fired.clone();
        let worker = std::thread::spawn(move || {
            debounce(rx, Duration::from_millis(60), move || {
                counter.fetch_add(1, Ordering::SeqCst);
            })
        });
        for _ in 0..3 {
            tx.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(fired.load(Ordering::SeqCst), 1, "one burst, one invalidation");
        drop(tx);
        worker.join().unwrap();
        assert_eq!(fired.load(Ordering::SeqCst), 1, "no pending burst is left at the end");
    }

    #[test]
    fn a_missing_root_cannot_be_watched_and_an_existing_one_can() {
        let base = tempfile::tempdir().unwrap();
        let missing = DownloadIndex::open(base.path().join("nope"), base.path());
        assert!(watch_root(&missing).is_none(), "the hourly renew is the fallback");
        let root = base.path().join("downloads");
        std::fs::create_dir_all(&root).unwrap();
        let present = DownloadIndex::open(&root, base.path());
        assert!(watch_root(&present).is_some());
    }
}
