//! Per-key async locks: one holder per key, different keys never wait for each other (design 16, RC-3, T-14a-057).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// A lock per key. The map keeps every key it has seen; one entry per manga URL is cheap for a library-sized set.
// ponytail: the key map never shrinks; prune idle keys if a library ever holds very many entries.
#[derive(Debug, Default)]
pub struct KeyedLocks {
    cells: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl KeyedLocks {
    /// Waits for the lock of `key`. The guard releases it on drop.
    pub async fn lock(&self, key: &str) -> OwnedMutexGuard<()> {
        let cell = {
            let mut cells = self.cells.lock().unwrap_or_else(PoisonError::into_inner);
            Arc::clone(cells.entry(key.to_owned()).or_default())
        };
        cell.lock_owned().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn the_same_key_runs_one_holder_at_a_time() {
        let locks = Arc::new(KeyedLocks::default());
        let active = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let (locks, active, max_seen) = (Arc::clone(&locks), Arc::clone(&active), Arc::clone(&max_seen));
            tasks.push(tokio::spawn(async move {
                let _guard = locks.lock("manga/a").await;
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_seen.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for task in tasks {
            task.await.expect("task");
        }
        assert_eq!(max_seen.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn different_keys_do_not_wait_for_each_other() {
        let locks = KeyedLocks::default();
        let _held = locks.lock("manga/a").await;
        let other = tokio::time::timeout(Duration::from_secs(2), locks.lock("manga/b")).await;
        assert!(other.is_ok(), "a second key must not wait on the first");
    }
}
