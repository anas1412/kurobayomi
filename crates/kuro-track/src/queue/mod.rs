//! Retry queue of progress updates that could not reach a tracker (`DelayedAnimeTrackingStore.kt`,
//! `DelayedMangaTrackingStore.kt`). Backed by `track_retry_queue` in the shared database.

pub mod retry;
pub mod store;

use std::sync::{Arc, Mutex};

use kuro_db::handle::Db;

use crate::error::TrackError;
use crate::model::{Kind, TrackerId};

/// One pending progress update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QueueItem {
    pub kind: Kind,
    pub entry_id: i64,
    pub tracker_id: TrackerId,
    pub progress: f64,
    pub queued_at_ms: i64,
    pub attempts: u32,
}

/// The queue. Each call is one statement in one transaction, so there is no read-modify-write race.
pub struct RetryQueue {
    db: Arc<Db>,
    /// The parked count last shown by [`RetryQueue::take_parked_notice`]. Session only (T-27a-071).
    announced: Mutex<usize>,
}

impl RetryQueue {
    /// Opens the queue, creating its table when missing.
    pub fn new(db: Arc<Db>) -> Result<Self, TrackError> {
        db.write(|tx| {
            tx.execute_batch(store::CREATE)?;
            Ok(())
        })?;
        Ok(Self {
            db,
            announced: Mutex::new(0),
        })
    }

    /// Queues `progress` for the binding, keeping the highest value already queued.
    pub fn add(
        &self,
        kind: Kind,
        entry_id: i64,
        tracker_id: TrackerId,
        progress: f64,
        now_ms: i64,
    ) -> Result<(), TrackError> {
        self.db.write(|tx| {
            tx.execute(
                store::UPSERT,
                (kind.as_str(), entry_id, tracker_id.0, progress, now_ms),
            )?;
            Ok(())
        })?;
        tracing::debug!(
            kind = kind.as_str(),
            entry = entry_id,
            tracker = tracker_id.0,
            "queue add"
        );
        Ok(())
    }

    /// Removes the pending update of the binding, if any.
    pub fn remove(
        &self,
        kind: Kind,
        entry_id: i64,
        tracker_id: TrackerId,
    ) -> Result<(), TrackError> {
        self.db.write(|tx| {
            tx.execute(store::DELETE, (kind.as_str(), entry_id, tracker_id.0))?;
            Ok(())
        })?;
        Ok(())
    }

    /// Removes the binding's item when its queued value is still `progress` or lower. Used after a
    /// successful run, so a higher value added while the run was in flight stays queued.
    pub fn remove_processed(
        &self,
        kind: Kind,
        entry_id: i64,
        tracker_id: TrackerId,
        progress: f64,
    ) -> Result<(), TrackError> {
        self.db.write(|tx| {
            tx.execute(
                store::REMOVE_PROCESSED,
                (kind.as_str(), entry_id, tracker_id.0, progress),
            )?;
            Ok(())
        })?;
        Ok(())
    }

    /// Counts one more failed run for the binding.
    pub fn bump_attempts(
        &self,
        kind: Kind,
        entry_id: i64,
        tracker_id: TrackerId,
    ) -> Result<(), TrackError> {
        self.db.write(|tx| {
            tx.execute(
                store::BUMP_ATTEMPTS,
                (kind.as_str(), entry_id, tracker_id.0),
            )?;
            Ok(())
        })?;
        Ok(())
    }

    /// Sets the attempts of every item back to zero.
    pub fn reset_attempts(&self) -> Result<(), TrackError> {
        self.db.write(|tx| {
            tx.execute(store::RESET_ATTEMPTS, [])?;
            Ok(())
        })?;
        Ok(())
    }

    /// The pending updates of `kind`, oldest first.
    pub fn list(&self, kind: Kind) -> Result<Vec<QueueItem>, TrackError> {
        let rows = self.db.read(|c| {
            let mut stmt = c.prepare(store::LIST)?;
            let rows = stmt
                .query_map([kind.as_str()], |r| {
                    Ok(QueueItem {
                        kind,
                        entry_id: r.get(0)?,
                        tracker_id: TrackerId(r.get(1)?),
                        progress: r.get(2)?,
                        queued_at_ms: r.get(3)?,
                        attempts: r.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })?;
        Ok(rows)
    }

    /// Pending updates of one entry (the sheet's badge, T-27a-071).
    pub fn pending_for_entry(&self, kind: Kind, entry_id: i64) -> Result<usize, TrackError> {
        Ok(self
            .list(kind)?
            .iter()
            .filter(|i| i.entry_id == entry_id)
            .count())
    }

    /// Items the scheduler has parked, meaning their attempts are used up (`retry::MAX_ATTEMPTS`).
    pub fn parked_count(&self) -> Result<usize, TrackError> {
        let mut parked = 0;
        for kind in [Kind::Anime, Kind::Manga] {
            parked += self
                .list(kind)?
                .iter()
                .filter(|i| i.attempts >= retry::MAX_ATTEMPTS)
                .count();
        }
        Ok(parked)
    }

    /// The count for the one parked toast of an entry open (T-27a-071). `Some(n)` when more items are parked than the
    /// last toast showed; `None` otherwise. Nothing is parked means the next parking shows again.
    pub fn take_parked_notice(&self) -> Result<Option<usize>, TrackError> {
        let parked = self.parked_count()?;
        let mut announced = self.announced.lock().unwrap_or_else(|e| e.into_inner());
        if parked == 0 {
            *announced = 0;
            return Ok(None);
        }
        if parked > *announced {
            *announced = parked;
            return Ok(Some(parked));
        }
        Ok(None)
    }

    /// True when nothing is queued for either kind.
    pub fn is_empty(&self) -> Result<bool, TrackError> {
        let count: i64 = self
            .db
            .read(|c| Ok(c.query_row(store::COUNT, [], |r| r.get(0))?))?;
        Ok(count == 0)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_testkit::{TempDir, TestDb};

    use super::*;

    fn queue() -> (TempDir, RetryQueue) {
        let dir = TempDir::new().unwrap();
        let db = TestDb::anime_handle(&dir);
        let q = RetryQueue::new(db).unwrap();
        (dir, q)
    }

    #[test]
    fn tr_27_025_create_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let db = TestDb::anime_handle(&dir);
        RetryQueue::new(db.clone()).unwrap();
        assert!(RetryQueue::new(db).unwrap().is_empty().unwrap());
    }

    #[test]
    fn tr_27_d03_anime_and_manga_with_the_same_numbers_stay_separate() {
        let (_dir, q) = queue();
        q.add(Kind::Anime, 42, TrackerId::MYANIMELIST, 3.0, 1)
            .unwrap();
        q.add(Kind::Manga, 42, TrackerId::MYANIMELIST, 9.0, 2)
            .unwrap();
        let anime = q.list(Kind::Anime).unwrap();
        assert_eq!(anime.len(), 1);
        assert_eq!(anime[0].progress, 3.0);
        assert_eq!(q.list(Kind::Manga).unwrap()[0].progress, 9.0);
    }

    #[test]
    fn t_27a_071_the_pending_count_is_per_entry() {
        let (_dir, q) = queue();
        q.add(Kind::Manga, 1, TrackerId::KITSU, 5.0, 10).unwrap();
        q.add(Kind::Manga, 2, TrackerId::KITSU, 5.0, 10).unwrap();
        assert_eq!(q.pending_for_entry(Kind::Manga, 1).unwrap(), 1);
        assert_eq!(q.pending_for_entry(Kind::Manga, 3).unwrap(), 0);
    }

    #[test]
    fn t_27a_071_a_parked_item_is_one_notice_until_the_count_changes() {
        let (_dir, q) = queue();
        q.add(Kind::Manga, 1, TrackerId::KITSU, 5.0, 10).unwrap();
        assert_eq!(q.take_parked_notice().unwrap(), None, "not parked yet");
        for _ in 0..retry::MAX_ATTEMPTS {
            q.bump_attempts(Kind::Manga, 1, TrackerId::KITSU).unwrap();
        }
        assert_eq!(q.parked_count().unwrap(), 1);
        assert_eq!(q.take_parked_notice().unwrap(), Some(1));
        assert_eq!(q.take_parked_notice().unwrap(), None, "shown once");
        q.add(Kind::Manga, 2, TrackerId::KITSU, 5.0, 10).unwrap();
        for _ in 0..retry::MAX_ATTEMPTS {
            q.bump_attempts(Kind::Manga, 2, TrackerId::KITSU).unwrap();
        }
        assert_eq!(q.take_parked_notice().unwrap(), Some(2));
    }

    #[test]
    fn progress_keeps_the_highest_value_queued() {
        let (_dir, q) = queue();
        q.add(Kind::Manga, 1, TrackerId::KITSU, 5.0, 10).unwrap();
        q.add(Kind::Manga, 1, TrackerId::KITSU, 3.0, 20).unwrap();
        let item = q.list(Kind::Manga).unwrap()[0];
        assert_eq!(item.progress, 5.0);
        assert_eq!(item.queued_at_ms, 10, "queued time is the first queueing");
        assert_eq!(item.attempts, 0);
    }

    #[test]
    fn tr_27_d07_fractional_progress_round_trips_exactly() {
        let (_dir, q) = queue();
        q.add(Kind::Anime, 7, TrackerId::SIMKL, 12.1, 0).unwrap();
        assert_eq!(q.list(Kind::Anime).unwrap()[0].progress, 12.1);
    }

    #[test]
    fn tr_27_d06_concurrent_adds_keep_the_maximum() {
        let (_dir, q) = queue();
        let q = Arc::new(q);
        let handles: Vec<_> = (0..100)
            .map(|i| {
                let q = Arc::clone(&q);
                std::thread::spawn(move || {
                    q.add(Kind::Anime, 3, TrackerId::ANILIST, (i * 7 % 100) as f64, i)
                        .unwrap()
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let items = q.list(Kind::Anime).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].progress, 99.0);
    }

    #[test]
    fn tr_27_026_conditional_removal_keeps_a_newer_add() {
        let (_dir, q) = queue();
        q.add(Kind::Anime, 1, TrackerId::ANILIST, 4.0, 0).unwrap();
        // The run read progress 4 and succeeded, but 6 was queued while it was in flight.
        q.add(Kind::Anime, 1, TrackerId::ANILIST, 6.0, 1).unwrap();
        q.remove_processed(Kind::Anime, 1, TrackerId::ANILIST, 4.0)
            .unwrap();
        assert_eq!(q.list(Kind::Anime).unwrap()[0].progress, 6.0);
        q.remove_processed(Kind::Anime, 1, TrackerId::ANILIST, 6.0)
            .unwrap();
        assert!(q.is_empty().unwrap());
    }

    #[test]
    fn bump_and_reset_change_only_the_attempt_count() {
        let (_dir, q) = queue();
        q.add(Kind::Manga, 5, TrackerId::KAVITA, 2.0, 7).unwrap();
        q.bump_attempts(Kind::Manga, 5, TrackerId::KAVITA).unwrap();
        q.bump_attempts(Kind::Manga, 5, TrackerId::KAVITA).unwrap();
        let item = q.list(Kind::Manga).unwrap()[0];
        assert_eq!(
            (item.attempts, item.progress, item.queued_at_ms),
            (2, 2.0, 7)
        );
        q.reset_attempts().unwrap();
        assert_eq!(q.list(Kind::Manga).unwrap()[0].attempts, 0);
    }

    #[test]
    fn remove_deletes_only_the_named_binding() {
        let (_dir, q) = queue();
        q.add(Kind::Anime, 1, TrackerId::ANILIST, 1.0, 0).unwrap();
        q.add(Kind::Anime, 2, TrackerId::ANILIST, 1.0, 0).unwrap();
        q.remove(Kind::Anime, 1, TrackerId::ANILIST).unwrap();
        let left = q.list(Kind::Anime).unwrap();
        assert_eq!(left.iter().map(|i| i.entry_id).collect::<Vec<_>>(), vec![2]);
        assert!(!q.is_empty().unwrap());
        q.remove(Kind::Anime, 2, TrackerId::ANILIST).unwrap();
        assert!(q.is_empty().unwrap());
    }
}

#[cfg(test)]
mod property_tests {
    use kuro_testkit::{TempDir, TestDb};

    use super::*;

    /// TT-27-302. Over a deterministic run of adds, the stored value of each key equals the largest value ever added
    /// for it, so it never goes down.
    #[test]
    #[ignore = "slow: run on demand"]
    fn tr_27_302_the_queued_value_never_decreases() {
        let dir = TempDir::new().unwrap();
        let q = RetryQueue::new(TestDb::anime_handle(&dir)).unwrap();
        let mut seed: u64 = 11;
        let mut largest = [f64::MIN; 3];
        for step in 0..300_i64 {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let entry = ((seed >> 33) % 3) as usize;
            let value = ((seed >> 40) % 500) as f64 / 10.0;
            q.add(Kind::Manga, entry as i64, TrackerId::KITSU, value, step)
                .unwrap();
            largest[entry] = largest[entry].max(value);
            let stored = q
                .list(Kind::Manga)
                .unwrap()
                .into_iter()
                .find(|i| i.entry_id == entry as i64)
                .unwrap()
                .progress;
            assert_eq!(stored, largest[entry]);
        }
    }
}
