//! One async lock per series, keyed by kind and entry id. Deletes, pending flushes, renames and prunes of the same
//! series run one at a time, so two of them never race on the same folder or index entry (both kinds).

use std::collections::HashMap;
use std::sync::Arc;

use kuro_platform::kind::Kind;

#[derive(Default)]
pub struct SeriesLocks {
    map: std::sync::Mutex<HashMap<(Kind, i64), SeriesGate>>,
}

type SeriesGate = Arc<tokio::sync::Mutex<()>>;

impl SeriesLocks {
    fn gate(&self, kind: Kind, id: i64) -> SeriesGate {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.entry((kind, id)).or_default().clone()
    }

    pub async fn lock(&self, kind: Kind, id: i64) -> tokio::sync::OwnedMutexGuard<()> {
        self.gate(kind, id).lock_owned().await
    }

    /// For the one synchronous caller (renames). Panics when called on a runtime worker thread, like tokio's own
    /// `blocking_lock`; call it from `spawn_blocking` or a plain thread.
    pub fn lock_blocking(&self, kind: Kind, id: i64) -> tokio::sync::OwnedMutexGuard<()> {
        self.gate(kind, id).blocking_lock_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn the_same_series_is_locked_and_other_series_are_free() {
        let locks = SeriesLocks::default();
        let held = locks.lock(Kind::Manga, 1).await;
        let waited = tokio::time::timeout(Duration::from_millis(50), locks.lock(Kind::Manga, 1)).await;
        assert!(waited.is_err(), "the same series waits");
        let other_entry = tokio::time::timeout(Duration::from_millis(50), locks.lock(Kind::Manga, 2)).await;
        assert!(other_entry.is_ok(), "another entry is free");
        let other_kind = tokio::time::timeout(Duration::from_millis(50), locks.lock(Kind::Anime, 1)).await;
        assert!(other_kind.is_ok(), "the same id of the other kind is a different series");
        drop(held);
        let again = tokio::time::timeout(Duration::from_millis(50), locks.lock(Kind::Manga, 1)).await;
        assert!(again.is_ok(), "released on drop");
    }

    #[test]
    fn lock_blocking_works_outside_a_runtime_and_releases() {
        let locks = SeriesLocks::default();
        drop(locks.lock_blocking(Kind::Anime, 5));
        drop(locks.lock_blocking(Kind::Anime, 5));
    }
}
