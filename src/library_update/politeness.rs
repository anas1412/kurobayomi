//! The process-wide politeness permit of a source (T-26a-022): one request at a time per source across the manga and
//! the anime pass, so a source never sees two concurrent requests from this app. Within one pass the groups of
//! different sources still overlap (at most five at a time, `pass::MAX_SOURCES_IN_PARALLEL`).
//!
//! Deliberate improvement over the Kotlin job, which limits only one pass: recorded in the decisions of the update
//! pipeline topic.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

fn gates() -> &'static Mutex<HashMap<i64, Arc<Semaphore>>> {
    static GATES: OnceLock<Mutex<HashMap<i64, Arc<Semaphore>>>> = OnceLock::new();
    GATES.get_or_init(Default::default)
}

/// Waits for the single permit of `source_id`. Dropping the returned permit lets the next request of that source go.
pub async fn acquire(source_id: i64) -> OwnedSemaphorePermit {
    let gate = {
        let mut map = gates().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        Arc::clone(map.entry(source_id).or_insert_with(|| Arc::new(Semaphore::new(1))))
    };
    gate.acquire_owned().await.expect("source gates are never closed")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn one_source_never_has_two_requests_in_flight() {
        // Ids in these tests are unique to each test so they cannot wait on one another.
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..4 {
            let (active, max_active) = (Arc::clone(&active), Arc::clone(&max_active));
            tasks.push(tokio::spawn(async move {
                let _permit = acquire(910_001).await;
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for task in tasks {
            task.await.expect("task");
        }
        assert_eq!(max_active.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_held_permit_does_not_block_another_source() {
        let _held = acquire(910_002).await;
        let other = tokio::time::timeout(Duration::from_secs(1), acquire(910_003)).await;
        assert!(other.is_ok(), "a different source has its own permit");
    }
}
