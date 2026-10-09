//! Retry scheduler of the progress queue (`DelayedAnimeTrackingUpdateJob.kt`, `DelayedMangaTrackingUpdateJob.kt`).
//! One loop serves both kinds. It waits for a kick, waits for the network, runs every eligible item, and backs off
//! between rounds while items keep failing.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::connectivity::Connectivity;
use crate::error::{TrackError, redact};
use crate::model::Kind;
use crate::queue::{QueueItem, RetryQueue};

/// Runs of an item before it is parked: the body runs for attempts 0 to 3 (`runAttemptCount > 3`).
pub const MAX_ATTEMPTS: u32 = 4;
/// Delay after the first failed run (`5.minutes`).
pub const INITIAL_BACKOFF: Duration = Duration::from_secs(5 * 60);
/// WorkManager's exponential cap.
pub const MAX_BACKOFF: Duration = Duration::from_secs(5 * 60 * 60);

/// Delay before the next round, after `attempts` failed runs: `INITIAL_BACKOFF * 2^(attempts - 1)`, capped.
pub fn backoff(attempts: u32) -> Duration {
    if attempts == 0 {
        return Duration::ZERO;
    }
    let factor = 1u64 << (attempts - 1).min(32);
    let secs = INITIAL_BACKOFF.as_secs().saturating_mul(factor);
    Duration::from_secs(secs.min(MAX_BACKOFF.as_secs()))
}

/// What a run did with its item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemOutcome {
    /// Updated, or no longer needed. The item leaves the queue.
    Done,
    /// Not runnable now (tracker logged out). The item stays and is not counted as a failure.
    Kept,
}

/// Runs one queued item. Errors count as a failed run.
#[async_trait]
pub trait RetryRunner: Send + Sync {
    async fn run_item(&self, item: &QueueItem) -> Result<ItemOutcome, TrackError>;
}

/// The single retry loop of the queue. Attempts live in the queue table, so they survive a restart.
pub struct RetryScheduler {
    queue: Arc<RetryQueue>,
    net: Arc<dyn Connectivity>,
    wake: Notify,
    started: AtomicBool,
}

impl RetryScheduler {
    pub fn new(queue: Arc<RetryQueue>, net: Arc<dyn Connectivity>) -> Self {
        Self {
            queue,
            net,
            wake: Notify::new(),
            started: AtomicBool::new(false),
        }
    }

    /// Resets every attempt counter and wakes the loop (`setupTask`, `ExistingWorkPolicy.REPLACE`). Kicks
    /// that arrive before the loop looks coalesce into one round.
    pub fn kick(&self) -> Result<(), TrackError> {
        self.queue.reset_attempts()?;
        self.wake.notify_one();
        Ok(())
    }

    /// Starts the loop. Returns `None` when this scheduler already has one, so there is only ever one.
    /// A non-empty queue at start is kicked once. Must run inside a tokio runtime.
    pub fn spawn(self: Arc<Self>, runner: Arc<dyn RetryRunner>) -> Option<JoinHandle<()>> {
        if self.started.swap(true, Ordering::SeqCst) {
            return None;
        }
        if matches!(self.queue.is_empty(), Ok(false)) {
            if let Err(e) = self.kick() {
                tracing::debug!(error = %redact(&e.to_string()), "retry queue kick at start failed");
            }
        }
        Some(tokio::spawn(async move {
            loop {
                self.wake.notified().await;
                self.drain(runner.as_ref()).await;
            }
        }))
    }

    /// Runs rounds until nothing is eligible, nothing failed, or every failing item is parked.
    async fn drain(&self, runner: &dyn RetryRunner) {
        loop {
            let eligible = match self.eligible() {
                Ok(v) => v,
                Err(e) => {
                    tracing::debug!(error = %redact(&e.to_string()), "retry queue unreadable");
                    return;
                }
            };
            if eligible.is_empty() {
                return;
            }
            self.net.wait_online().await;

            let mut failed = 0usize;
            for item in &eligible {
                match runner.run_item(item).await {
                    Ok(ItemOutcome::Done) => {
                        if let Err(e) = self.queue.remove_processed(
                            item.kind,
                            item.entry_id,
                            item.tracker_id,
                            item.progress,
                        ) {
                            tracing::debug!(error = %redact(&e.to_string()), "retry queue removal failed");
                        }
                    }
                    Ok(ItemOutcome::Kept) => {}
                    Err(e) => {
                        failed += 1;
                        tracing::info!(
                            entry = item.entry_id,
                            tracker = item.tracker_id.0,
                            error = %redact(&e.to_string()),
                            "queued tracker update failed"
                        );
                        if let Err(e) =
                            self.queue
                                .bump_attempts(item.kind, item.entry_id, item.tracker_id)
                        {
                            tracing::debug!(error = %redact(&e.to_string()), "retry attempt count not saved");
                        }
                    }
                }
            }
            if failed == 0 {
                return;
            }

            let remaining = match self.eligible() {
                Ok(v) => v,
                Err(_) => return,
            };
            let Some(attempts) = remaining.iter().map(|i| i.attempts).max() else {
                tracing::debug!("retry queue parked after {MAX_ATTEMPTS} runs");
                return;
            };
            tokio::select! {
                _ = tokio::time::sleep(backoff(attempts)) => {}
                _ = self.wake.notified() => {}
            }
        }
    }

    /// Items with attempts left, anime first, each kind in insertion order.
    fn eligible(&self) -> Result<Vec<QueueItem>, TrackError> {
        let mut items = self.queue.list(Kind::Anime)?;
        items.extend(self.queue.list(Kind::Manga)?);
        items.retain(|i| i.attempts < MAX_ATTEMPTS);
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    use kuro_testkit::{TempDir, TestDb};

    use super::*;
    use crate::model::TrackerId;

    /// Connectivity double whose state the test controls.
    struct Net(AtomicBool);

    #[async_trait]
    impl Connectivity for Net {
        fn is_online(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    /// Runner that counts calls and returns a scripted outcome.
    struct Scripted {
        calls: AtomicUsize,
        result: Mutex<Result<ItemOutcome, String>>,
    }

    impl Scripted {
        fn new(result: Result<ItemOutcome, String>) -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
                result: Mutex::new(result),
            })
        }
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl RetryRunner for Scripted {
        async fn run_item(&self, _item: &QueueItem) -> Result<ItemOutcome, TrackError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.result.lock().unwrap().clone() {
                Ok(o) => Ok(o),
                Err(m) => Err(TrackError::Message(m)),
            }
        }
    }

    fn setup(online: bool) -> (TempDir, Arc<RetryQueue>, Arc<Net>, Arc<RetryScheduler>) {
        let dir = TempDir::new().unwrap();
        let db = TestDb::anime_handle(&dir);
        let queue = Arc::new(RetryQueue::new(db).unwrap());
        let net = Arc::new(Net(AtomicBool::new(online)));
        let sched = Arc::new(RetryScheduler::new(queue.clone(), net.clone()));
        (dir, queue, net, sched)
    }

    #[test]
    fn tr_27_121_tr_27_122_backoff_doubles_from_five_minutes_and_caps_at_five_hours() {
        assert_eq!(backoff(0), Duration::ZERO);
        assert_eq!(backoff(1), Duration::from_secs(300));
        assert_eq!(backoff(2), Duration::from_secs(600));
        assert_eq!(backoff(3), Duration::from_secs(1200));
        assert_eq!(backoff(10), MAX_BACKOFF);
        assert_eq!(backoff(u32::MAX), MAX_BACKOFF);
    }

    #[tokio::test(start_paused = true)]
    async fn tr_27_d04_four_failed_runs_then_parked_with_items_kept() {
        let (_dir, queue, _net, sched) = setup(true);
        queue
            .add(Kind::Anime, 1, TrackerId::ANILIST, 5.0, 0)
            .unwrap();
        let runner = Scripted::new(Err("boom".into()));
        let handle = sched.clone().spawn(runner.clone()).unwrap();
        sched.kick().unwrap();

        // 0 + 5 + 10 + 20 minutes of backoff, then the queue parks.
        tokio::time::sleep(Duration::from_secs(60 * 60)).await;
        assert_eq!(runner.calls(), 4, "four runs, then parked");
        let item = queue.list(Kind::Anime).unwrap()[0];
        assert_eq!(item.attempts, MAX_ATTEMPTS);

        // TT-27-D04: a further time passes with no fifth run and the item is still queued.
        tokio::time::sleep(Duration::from_secs(10 * 60 * 60)).await;
        assert_eq!(runner.calls(), 4);
        assert!(!queue.is_empty().unwrap());
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn tr_27_123_kick_resets_attempts_after_failures() {
        let (_dir, queue, _net, sched) = setup(true);
        queue
            .add(Kind::Manga, 2, TrackerId::KAVITA, 3.0, 0)
            .unwrap();
        let runner = Scripted::new(Err("boom".into()));
        let handle = sched.clone().spawn(runner.clone()).unwrap();
        sched.kick().unwrap();
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert_eq!(queue.list(Kind::Manga).unwrap()[0].attempts, 1);

        sched.kick().unwrap();
        assert_eq!(queue.list(Kind::Manga).unwrap()[0].attempts, 0);
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn tr_27_124_nothing_runs_while_offline_and_runs_once_online() {
        let (_dir, queue, net, sched) = setup(false);
        queue.add(Kind::Anime, 3, TrackerId::SIMKL, 1.0, 0).unwrap();
        let runner = Scripted::new(Ok(ItemOutcome::Done));
        let handle = sched.clone().spawn(runner.clone()).unwrap();
        sched.kick().unwrap();

        tokio::time::sleep(Duration::from_secs(60 * 60)).await;
        assert_eq!(runner.calls(), 0);
        assert!(!queue.is_empty().unwrap());

        net.0.store(true, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(runner.calls(), 1);
        assert!(queue.is_empty().unwrap());
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn tr_27_126_ten_kicks_make_one_run() {
        let (_dir, queue, _net, sched) = setup(true);
        queue
            .add(Kind::Anime, 4, TrackerId::ANILIST, 2.0, 0)
            .unwrap();
        let runner = Scripted::new(Ok(ItemOutcome::Done));
        let handle = sched.clone().spawn(runner.clone()).unwrap();
        for _ in 0..10 {
            sched.kick().unwrap();
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(runner.calls(), 1);
        handle.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn tr_27_127_a_second_spawn_returns_none() {
        let (_dir, _queue, _net, sched) = setup(true);
        let runner = Scripted::new(Ok(ItemOutcome::Done));
        let first = sched.clone().spawn(runner.clone());
        assert!(first.is_some());
        assert!(sched.clone().spawn(runner).is_none());
        first.unwrap().abort();
    }

    #[tokio::test(start_paused = true)]
    async fn tr_27_029_kept_items_are_not_counted_as_failures() {
        let (_dir, queue, _net, sched) = setup(true);
        queue.add(Kind::Anime, 6, TrackerId::KITSU, 9.0, 0).unwrap();
        let runner = Scripted::new(Ok(ItemOutcome::Kept));
        let handle = sched.clone().spawn(runner.clone()).unwrap();
        sched.kick().unwrap();
        tokio::time::sleep(Duration::from_secs(60 * 60)).await;
        assert_eq!(runner.calls(), 1, "no retry for a kept item");
        let item = queue.list(Kind::Anime).unwrap()[0];
        assert_eq!(item.attempts, 0);
        handle.abort();
    }

    #[test]
    fn tr_27_125_queue_survives_a_restart() {
        let dir = TempDir::new().unwrap();
        {
            let db = TestDb::anime_handle(&dir);
            let queue = RetryQueue::new(db).unwrap();
            queue.add(Kind::Anime, 8, TrackerId::KITSU, 4.0, 0).unwrap();
        }
        let db = TestDb::anime_handle(&dir);
        let queue = RetryQueue::new(db).unwrap();
        let items = queue.list(Kind::Anime).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].progress, 4.0);
    }
}
