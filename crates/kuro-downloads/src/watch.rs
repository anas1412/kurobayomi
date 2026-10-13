//! Index invalidation on external changes to the downloads roots (T-23a-045), one watcher over both kinds. A `notify`
//! watcher on each root (not its subfolders, as Kotlin reacts to the storage root only) forwards each event to a
//! debouncer, and that kind's index is invalidated after [`QUIET_PERIOD`] without events. A root that cannot be watched
//! (it does not exist yet) relies on the hourly renew of the index (EC-89).

use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use kuro_platform::kind::Kind;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::index::DownloadIndex;

/// Quiet period after the last change before the index is invalidated (T-23a-045).
pub const QUIET_PERIOD: Duration = Duration::from_secs(2);

/// Keeps the watchers alive. Dropping it stops the watch and ends the debouncer threads.
pub struct RootWatch {
    _watchers: Vec<RecommendedWatcher>,
}

/// Watches both roots of `index`; a change under one root invalidates only that kind. `None` when no root can be
/// watched.
pub fn watch_roots(index: &DownloadIndex) -> Option<RootWatch> {
    let watchers: Vec<RecommendedWatcher> = Kind::ALL
        .into_iter()
        .filter_map(|kind| {
            let index = index.clone();
            let root = index.root(kind).to_path_buf();
            watch_dir(&root, QUIET_PERIOD, move || index.invalidate(Some(kind)))
        })
        .collect();
    (!watchers.is_empty()).then_some(RootWatch { _watchers: watchers })
}

/// Watches one folder and calls `fire` once per burst of events, `quiet` after the last one. `None` when the watcher
/// cannot start. Dropping the returned watcher ends the debouncer thread.
fn watch_dir(root: &Path, quiet: Duration, fire: impl FnMut() + Send + 'static) -> Option<RecommendedWatcher> {
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |_event: notify::Result<notify::Event>| {
        let _ = tx.send(());
    })
    .ok()?;
    watcher.watch(root, RecursiveMode::NonRecursive).ok()?;
    std::thread::Builder::new().name("kuro-downloads-watch".into()).spawn(move || debounce(rx, quiet, fire)).ok()?;
    Some(watcher)
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
    use crate::index::IndexRoots;
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
        let roots = IndexRoots { manga: base.path().join("nope"), anime: base.path().join("nope-either") };
        let missing = DownloadIndex::open(roots, base.path());
        assert!(missing.watch().is_none(), "the hourly renew is the fallback");

        let manga = base.path().join("manga");
        std::fs::create_dir_all(&manga).unwrap();
        let one = IndexRoots { manga, anime: base.path().join("nope") };
        assert!(DownloadIndex::open(one, base.path()).watch().is_some(), "one watchable root is enough");

        let both = IndexRoots { manga: base.path().join("manga"), anime: base.path().join("anime") };
        std::fs::create_dir_all(&both.anime).unwrap();
        assert!(DownloadIndex::open(both, base.path()).watch().is_some());
    }

    // Test 4.2 item 11: the watcher reacts only to the root that changed.
    #[test]
    fn a_change_under_one_root_fires_only_that_roots_watcher() {
        let base = tempfile::tempdir().unwrap();
        let (manga_root, anime_root) = (base.path().join("manga"), base.path().join("anime"));
        std::fs::create_dir_all(&manga_root).unwrap();
        std::fs::create_dir_all(&anime_root).unwrap();
        let (manga_hits, anime_hits) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (m, a) = (manga_hits.clone(), anime_hits.clone());
        let quiet = Duration::from_millis(50);
        let _manga = watch_dir(&manga_root, quiet, move || {
            m.fetch_add(1, Ordering::SeqCst);
        })
        .expect("manga root watched");
        let _anime = watch_dir(&anime_root, quiet, move || {
            a.fetch_add(1, Ordering::SeqCst);
        })
        .expect("anime root watched");

        std::fs::create_dir(anime_root.join("New Source (EN)")).unwrap();
        for _ in 0..100 {
            if anime_hits.load(Ordering::SeqCst) > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(anime_hits.load(Ordering::SeqCst), 1, "the anime root fired once for its change");
        assert_eq!(manga_hits.load(Ordering::SeqCst), 0, "the manga root saw nothing");
    }
}
