//! Tracker sync for a seen episode (area 45 `PlayerViewModel.updateTrackEpisodeSeen`, T-45a-066,
//! T-45a-067). The decisions are `progress::seen_tracker_updates` and `completion::should_sync_tracker`;
//! this module runs them against two ports, so playback never waits on a tracker: an online update is
//! handed to the tracker queue, an offline one goes to the delayed-tracking store and its replay
//! worker is scheduled. Every failure is logged at info and swallowed.

use std::io;
use std::sync::Arc;

use kuro_domain::kind::EntryKind;
use kuro_domain::track::delayed::{DelayedTrackingStore, DelayedTrackingWorker};

use crate::completion::should_sync_tracker;
use crate::progress::{TrackUpdate, TrackerEntry, seen_tracker_updates};

/// The tracker queue. `send` hands the update over without waiting for the tracker, and an `Err`
/// means it was not accepted.
pub trait TrackerPort {
    fn online(&self) -> bool;
    fn send(&self, track_id: i64, episode: f64) -> Result<(), String>;
}

/// The delayed-tracking store and the job that replays it.
pub trait DelayedPort {
    fn add(&self, track_id: i64, episode: f64) -> io::Result<()>;
    fn schedule(&self);
}

/// `DelayedTrackingStore` and `DelayedTrackingWorker` of `kuro-domain`, for the anime kind. The
/// store is keyed by track row id, as `DelayedPort::add` is. The worker needs a tokio runtime, so
/// `schedule` enters `runtime` first. Without a worker (`Domain::start` not called) nothing is
/// scheduled, and the next start replays the queue.
pub struct DomainDelayed {
    pub store: Arc<DelayedTrackingStore>,
    pub worker: Option<Arc<DelayedTrackingWorker>>,
    pub runtime: tokio::runtime::Handle,
}

impl DelayedPort for DomainDelayed {
    fn add(&self, track_id: i64, episode: f64) -> io::Result<()> {
        self.store.add(EntryKind::Anime, track_id, episode)
    }

    fn schedule(&self) {
        match &self.worker {
            Some(worker) => {
                let _entered = self.runtime.enter();
                worker.schedule();
            }
            None => tracing::info!("delayed tracking worker is not started; replay waits for the next start"),
        }
    }
}

/// The preferences the sync reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeenPolicy {
    /// `pref_progress_preference`: the completion fraction.
    pub threshold: f64,
    /// The per-source incognito state of the playing anime.
    pub incognito_source: bool,
    /// The app-wide incognito flag, which decides tracker sync (E-17).
    pub incognito_global: bool,
    /// `pref_auto_update_manga_sync_key`.
    pub auto_update: bool,
}

impl Default for SeenPolicy {
    fn default() -> Self {
        Self {
            threshold: 0.85,
            incognito_source: false,
            incognito_global: false,
            auto_update: true,
        }
    }
}

pub struct Ports {
    pub tracker: Box<dyn TrackerPort>,
    pub delayed: Box<dyn DelayedPort>,
}

/// The trackers of the playing anime and what a seen episode does for them. Without `ports` the
/// sync does nothing.
#[derive(Default)]
pub struct Tracking {
    pub policy: SeenPolicy,
    pub trackers: Vec<TrackerEntry>,
    pub ports: Option<Ports>,
}

impl Tracking {
    pub fn has_trackers(&self) -> bool {
        !self.trackers.is_empty()
    }

    /// The episode was seen: updates every logged-in tracker that is behind, and returns how many
    /// updates were sent or delayed. A send the queue refuses is delayed instead (the Kotlin
    /// `TrackEpisode` queues a failed update for the retry job).
    pub fn on_seen(&self, episode_number: f64) -> usize {
        let Some(ports) = &self.ports else {
            return 0;
        };
        let policy = &self.policy;
        if !should_sync_tracker(policy.incognito_global, self.has_trackers(), policy.auto_update) {
            return 0;
        }
        let updates = seen_tracker_updates(
            policy.auto_update,
            true,
            episode_number,
            ports.tracker.online(),
            &self.trackers,
        );
        let mut delayed = false;
        for update in &updates {
            let (track_id, episode) = match *update {
                TrackUpdate::Send { track_id, episode } => {
                    match ports.tracker.send(track_id, episode) {
                        Ok(()) => continue,
                        Err(e) => {
                            tracing::info!(track_id, "tracker update failed, delaying it: {e}");
                            (track_id, episode)
                        }
                    }
                }
                TrackUpdate::Delay { track_id, episode } => (track_id, episode),
            };
            match ports.delayed.add(track_id, episode) {
                Ok(()) => delayed = true,
                Err(e) => tracing::info!(track_id, "cannot store the delayed tracker update: {e}"),
            }
        }
        if delayed {
            ports.delayed.schedule();
        }
        updates.len()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    #[derive(Default)]
    struct Log(Rc<RefCell<Vec<String>>>);

    struct Tracker {
        log: Rc<RefCell<Vec<String>>>,
        online: bool,
        accept: bool,
    }

    impl TrackerPort for Tracker {
        fn online(&self) -> bool {
            self.online
        }
        fn send(&self, track_id: i64, episode: f64) -> Result<(), String> {
            self.log.borrow_mut().push(format!("send {track_id} {episode}"));
            self.accept.then_some(()).ok_or_else(|| "queue closed".to_owned())
        }
    }

    struct Delayed {
        log: Rc<RefCell<Vec<String>>>,
        fail: bool,
    }

    impl DelayedPort for Delayed {
        fn add(&self, track_id: i64, episode: f64) -> io::Result<()> {
            self.log.borrow_mut().push(format!("add {track_id} {episode}"));
            if self.fail {
                return Err(io::Error::other("disk full"));
            }
            Ok(())
        }
        fn schedule(&self) {
            self.log.borrow_mut().push("schedule".to_owned());
        }
    }

    fn entry(track_id: i64, logged_in: bool, last: f64) -> TrackerEntry {
        TrackerEntry {
            track_id,
            logged_in,
            last_episode_seen: last,
        }
    }

    fn tracking(online: bool, accept: bool, fail_store: bool, policy: SeenPolicy) -> (Tracking, Log) {
        let log = Log::default();
        let tracking = Tracking {
            policy,
            trackers: vec![entry(1, true, 2.0), entry(2, false, 0.0), entry(3, true, 5.0)],
            ports: Some(Ports {
                tracker: Box::new(Tracker { log: log.0.clone(), online, accept }),
                delayed: Box::new(Delayed { log: log.0.clone(), fail: fail_store }),
            }),
        };
        (tracking, log)
    }

    #[test]
    fn online_sends_to_each_logged_in_tracker_that_is_behind() {
        let (t, log) = tracking(true, true, false, SeenPolicy::default());
        assert_eq!(t.on_seen(3.0), 1);
        assert_eq!(*log.0.borrow(), vec!["send 1 3"]);
    }

    #[test]
    fn offline_stores_the_update_and_schedules_the_job_once() {
        let mut policy = SeenPolicy::default();
        policy.auto_update = true;
        let (mut t, log) = tracking(false, true, false, policy);
        t.trackers.push(entry(4, true, 0.0));
        assert_eq!(t.on_seen(3.0), 2);
        assert_eq!(*log.0.borrow(), vec!["add 1 3", "add 4 3", "schedule"]);
    }

    #[test]
    fn a_refused_send_is_delayed_and_never_panics() {
        let (t, log) = tracking(true, false, false, SeenPolicy::default());
        assert_eq!(t.on_seen(3.0), 1);
        assert_eq!(*log.0.borrow(), vec!["send 1 3", "add 1 3", "schedule"]);
    }

    #[test]
    fn a_failed_store_write_schedules_nothing() {
        let (t, log) = tracking(false, true, true, SeenPolicy::default());
        assert_eq!(t.on_seen(3.0), 1);
        assert_eq!(*log.0.borrow(), vec!["add 1 3"]);
    }

    #[test]
    fn auto_update_off_or_incognito_does_nothing() {
        let off = SeenPolicy { auto_update: false, ..SeenPolicy::default() };
        let incognito = SeenPolicy { incognito_global: true, ..SeenPolicy::default() };
        for policy in [off, incognito] {
            let (t, log) = tracking(true, true, false, policy);
            assert_eq!(t.on_seen(3.0), 0);
            assert!(log.0.borrow().is_empty());
        }
        assert_eq!(Tracking::default().on_seen(3.0), 0);
    }

    #[test]
    fn the_domain_store_takes_anime_progress_by_track_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(DelayedTrackingStore::new(dir.path().join("q.json")));
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let delayed = DomainDelayed {
            store: Arc::clone(&store),
            worker: None,
            runtime: rt.handle().clone(),
        };
        delayed.add(7, 4.0).unwrap();
        delayed.schedule();
        assert_eq!(store.items(EntryKind::Anime), vec![(7, 4.0)]);
        assert!(store.items(EntryKind::Manga).is_empty());
    }
}
