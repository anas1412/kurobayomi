//! The per-entry lock of the chapter and episode syncs (race 2 of `SyncChaptersWithSource.kt`). The planner is
//! [`crate::items::sync_plan`] and the applier is [`crate::items::chapter_sync`] / [`crate::items::episode_sync`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// RC-08: the syncs of one manga run one at a time, and syncs of different manga do not wait for each other.
// ponytail: one lock entry is kept per manga id synced this session and never evicted; drop idle entries if that grows.
#[derive(Default)]
pub struct MangaSyncLocks {
    locks: Mutex<HashMap<i64, Arc<AsyncMutex<()>>>>,
}

impl MangaSyncLocks {
    /// Waits until no other sync of `manga_id` holds the lock, then holds it until the guard is dropped.
    pub async fn lock(&self, manga_id: i64) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
            Arc::clone(locks.entry(manga_id).or_default())
        };
        lock.lock_owned().await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn syncs_of_one_manga_run_one_at_a_time() {
        let locks = Arc::new(MangaSyncLocks::default());
        let inside = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let (locks, inside, peak) = (locks.clone(), inside.clone(), peak.clone());
            tasks.push(tokio::spawn(async move {
                let _guard = locks.lock(7).await;
                let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                tokio::task::yield_now().await;
                inside.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_sync_of_another_manga_does_not_wait_for_the_held_lock() {
        let locks = MangaSyncLocks::default();
        let _held = locks.lock(1).await;
        let other = tokio::time::timeout(Duration::from_secs(1), locks.lock(2)).await;
        assert!(other.is_ok(), "a different manga must not wait");
    }

    #[tokio::test]
    async fn a_second_sync_of_the_same_manga_waits_until_the_first_guard_drops() {
        let locks = MangaSyncLocks::default();
        let first = locks.lock(3).await;
        let blocked = tokio::time::timeout(Duration::from_millis(50), locks.lock(3)).await;
        assert!(blocked.is_err(), "the same manga must wait");
        drop(first);
        let second = tokio::time::timeout(Duration::from_secs(1), locks.lock(3)).await;
        assert!(second.is_ok());
    }
}
