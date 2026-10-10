//! Per-key serialisation for the multi-request writes of a tracker (T-32a-066, TST-60, TST-61).
//!
//! The write runs on a task of its own, so a caller that is dropped (a closed window) cannot stop the second request of
//! a pair. Two writes with the same key run one after the other.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinError;

// ponytail: the table keeps one entry per key the session has written and never shrinks. Prune it if it ever shows up
// in a profile; a user's tracked titles are in the hundreds.
#[derive(Default)]
pub struct KeyedLocks(Mutex<HashMap<i64, Arc<AsyncMutex<()>>>>);

impl KeyedLocks {
    /// Runs `work` while holding the lock of `key`, on a spawned task. The task finishes even if the caller is dropped.
    /// A panic inside `work` comes back as the `JoinError`.
    pub async fn run<T, F>(self: &Arc<Self>, key: i64, work: F) -> Result<T, JoinError>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let lock = {
            let mut map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            Arc::clone(
                map.entry(key)
                    .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
            )
        };
        tokio::spawn(async move {
            let _held = lock.lock().await;
            work.await
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn t_32a_066_two_writes_on_one_key_do_not_interleave() {
        let locks = Arc::new(KeyedLocks::default());
        let busy = Arc::new(AtomicBool::new(false));
        let mut runs = Vec::new();
        for _ in 0..3 {
            let busy = Arc::clone(&busy);
            let locks2 = Arc::clone(&locks);
            runs.push(tokio::spawn(async move {
                locks2
                    .run(7, async move {
                        assert!(!busy.swap(true, Ordering::SeqCst), "two writes overlapped");
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        busy.store(false, Ordering::SeqCst);
                    })
                    .await
            }));
        }
        for r in runs {
            r.await.unwrap().unwrap();
        }
    }

    #[tokio::test]
    async fn t_32a_066_a_dropped_caller_does_not_stop_the_second_write() {
        let locks = Arc::new(KeyedLocks::default());
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        // Give up on the caller while the write is still sleeping. The write must still complete.
        let caller = tokio::time::timeout(
            Duration::from_millis(5),
            locks.run(1, async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                flag.store(true, Ordering::SeqCst);
            }),
        )
        .await;
        assert!(caller.is_err(), "the caller should have been dropped");
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(
            done.load(Ordering::SeqCst),
            "the spawned write was cancelled"
        );
    }

    #[tokio::test]
    async fn t_32a_066_different_keys_do_not_wait_for_each_other() {
        let locks = Arc::new(KeyedLocks::default());
        let held = locks.run(1, std::future::pending::<()>());
        let other = tokio::time::timeout(Duration::from_millis(200), locks.run(2, async {}));
        // The first write never ends, so only its own task is stuck. Key 2 still finishes.
        let (_, got) = tokio::join!(tokio::time::timeout(Duration::from_millis(50), held), other);
        assert!(got.is_ok_and(|r| r.is_ok()));
    }
}
