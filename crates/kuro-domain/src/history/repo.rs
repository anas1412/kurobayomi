//! History repository for both kinds: the list, the latest row, resets, clear-all and the upsert, over the shared
//! `kuro_data::history::HistoryRepo`. This layer adds the domain row shapes, the error type and the detached
//! writer; the SQL (`kuro_db::tables::history`) and the rules of the rows are the shared ones. The 12 h session clamp
//! and the incognito rules are in [`super::recorder`].

use kuro_platform::kind::Kind;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kuro_data::history::{History, HistoryItem, HistoryRepo as SharedHistory};
use kuro_db::handle::Db;
use kuro_db::tables::history as table;

use super::model::{CoverKey, HistoryRow, HistoryUpdate, HistoryWithRelations};
use crate::error::DomainError;

/// Reads and writes the history rows of either kind.
#[derive(Clone)]
pub struct HistoryRepo {
    db: Arc<Db>,
    shared: SharedHistory,
}

/// Detached flushes that have not been joined yet (see [`HistoryRepo::record_detached`]).
static PENDING_FLUSHES: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

fn pending_flushes() -> MutexGuard<'static, Vec<JoinHandle<()>>> {
    PENDING_FLUSHES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Waits up to `timeout` for the detached history flushes to finish, then joins the ones that did. Returns how many
/// are still running; they keep running detached, as an unjoined flush does when the process exits.
pub fn join_pending_flushes(timeout: Duration) -> usize {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline && !pending_flushes().iter().all(JoinHandle::is_finished) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let handles = std::mem::take(&mut *pending_flushes());
    let mut still_running = 0;
    for handle in handles {
        if handle.is_finished() {
            if handle.join().is_err() {
                tracing::error!("a history flush panicked");
            }
        } else {
            still_running += 1;
        }
    }
    still_running
}

fn view_row(r: HistoryItem) -> HistoryWithRelations {
    HistoryWithRelations {
        id: r.id,
        item_id: r.item_id,
        entry_id: r.entry_id,
        title: r.title,
        item_number: r.item_number,
        at: r.last_at,
        duration_ms: r.duration_ms,
        cover: CoverKey {
            entry_id: r.entry_id,
            source_id: r.source,
            favorite: r.favorite,
            url: r.thumbnail_url,
            cover_last_modified: r.cover_last_modified,
        },
    }
}

impl HistoryRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { shared: SharedHistory::new(db.clone()), db }
    }

    /// One row per entry (its newest row), only rows with a timestamp above 0, newest first. `query` is a
    /// case-insensitive substring of the title; `%`, `_` and `\` are literal.
    pub fn list(&self, kind: Kind, query: &str) -> Result<Vec<HistoryWithRelations>, DomainError> {
        Ok(self.shared.list(kind, query)?.into_iter().map(view_row).collect())
    }

    /// Live list of [`HistoryRepo::list`]: re-runs after a change to the history, item or entry tables.
    pub fn watch_list(&self, kind: Kind, query: String) -> kuro_db::subscribe::Subscription<Vec<HistoryWithRelations>> {
        self.db.subscribe(&["history", "items", "entries"], Duration::from_millis(30), move |c| {
            Ok(table::list(c, kind, &query)?.into_iter().map(|r| view_row(r.into())).collect())
        })
    }

    /// The newest row with a timestamp above 0, or `None`.
    pub fn latest(&self, kind: Kind) -> Result<Option<HistoryWithRelations>, DomainError> {
        Ok(self.shared.latest(kind)?.map(view_row))
    }

    /// Every history row of one entry, reset rows included. The rows of an entry are all of its kind, so `kind` does not
    /// select anything.
    pub fn by_entry(&self, _kind: Kind, entry_id: i64) -> Result<Vec<HistoryRow>, DomainError> {
        Ok(self
            .shared
            .by_entry_id(entry_id)?
            .into_iter()
            .map(|r: History| HistoryRow { id: r.id, item_id: r.item_id, at: r.last_at, time_read: r.duration_ms })
            .collect())
    }

    /// Records a read or watch: sets the timestamp and adds the session to the duration. An item that is already
    /// recorded keeps one row. `kind` does not select anything: an item belongs to one entry of one kind.
    pub fn upsert(&self, _kind: Kind, item_id: i64, at: i64, session_ms: i64) -> Result<(), DomainError> {
        Ok(self.shared.upsert(item_id, at, session_ms)?)
    }

    /// Writes one recording. A failure (for example a chapter deleted meanwhile) is logged and swallowed, as the
    /// upstream callers do.
    pub fn record(&self, update: HistoryUpdate) {
        if let Err(e) = self.upsert(update.kind, update.item_id, update.at, update.session_ms) {
            tracing::error!(error = %e, item = update.item_id, "cannot record history");
        }
    }

    /// Runs [`HistoryRepo::record`] on a detached thread, so closing a window cannot cancel it. The handle stays in
    /// the pending registry until [`join_pending_flushes`] runs at shutdown.
    pub fn record_detached(&self, update: HistoryUpdate) {
        let repo = self.clone();
        let handle = std::thread::spawn(move || repo.record(update));
        let mut pending = pending_flushes();
        pending.retain(|h| !h.is_finished());
        pending.push(handle);
    }

    /// Sets the timestamp of one row to 0. The row and its duration stay, so the total is unchanged.
    pub fn reset_one(&self, _kind: Kind, history_id: i64) -> Result<(), DomainError> {
        Ok(self.shared.reset_by_id(history_id)?)
    }

    /// Resets every row of one entry.
    pub fn reset_entry(&self, _kind: Kind, entry_id: i64) -> Result<(), DomainError> {
        Ok(self.shared.reset_by_entry_id(entry_id)?)
    }

    /// Deletes every row of the kind. Returns `false` (after logging) when the delete fails.
    pub fn clear_all(&self, kind: Kind) -> bool {
        match self.shared.delete_all(kind) {
            Ok(()) => true,
            Err(e) => {
                tracing::error!(error = %e, "cannot clear history");
                false
            }
        }
    }

    /// Deletes the rows that were reset ("Clear database" maintenance action).
    pub fn remove_reset(&self, kind: Kind) -> Result<usize, DomainError> {
        Ok(self.db.write(|tx| Ok(SharedHistory::remove_resetted(tx, kind)?))?)
    }

    /// Total time of every row of the kind, reset rows included.
    pub fn total_duration_of(&self, kind: Kind) -> Result<i64, DomainError> {
        Ok(self.shared.total_duration(kind)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::test_db;
    use kuro_data::entry::{Entry, EntryRepo};

    /// A favourite manga with `n` chapters, `source_order` descending so the reading order is the insert order.
    fn manga_with_chapters(db: &Arc<Db>, title: &str, n: i64) -> (i64, Vec<i64>) {
        let manga = EntryRepo::new(db.clone())
            .insert(&Entry {
                favorite: true,
                source: 7,
                url: format!("u/{title}"),
                title: title.into(),
                ..Entry::create(Kind::Manga)
            })
            .unwrap();
        let ids = (0..n)
            .map(|i| {
                db.write(|tx| {
                    tx.execute(
                        "INSERT INTO items(entry_id, url, name, scanlator, done, bookmark, progress, number, source_order, date_fetch, date_upload) \
                         VALUES (?1, ?2, ?3, NULL, 0, 0, 0, ?4, ?5, 0, 0)",
                        (manga, format!("{title}/{i}"), format!("c{i}"), (i + 1) as f64, n - 1 - i),
                    )?;
                    Ok(tx.last_insert_rowid())
                })
                .unwrap()
            })
            .collect();
        (manga, ids)
    }

    fn list_ids(repo: &HistoryRepo) -> Vec<i64> {
        repo.list(Kind::Manga, "").unwrap().iter().map(|r| r.item_id).collect()
    }

    #[test]
    fn two_upserts_for_one_chapter_leave_one_row_with_the_sum_of_sessions() {
        let (_d, db) = test_db();
        let (_m, c) = manga_with_chapters(&db, "M", 1);
        let repo = HistoryRepo::new(db.clone());
        repo.upsert(Kind::Manga, c[0], 1000, 5000).unwrap();
        repo.upsert(Kind::Manga, c[0], 2000, 3000).unwrap();
        let rows = repo.by_entry(Kind::Manga, _m).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].at, rows[0].time_read), (2000, 8000));
    }

    #[test]
    fn list_shows_the_newest_row_per_entry_and_reveals_the_previous_after_a_reset() {
        let (_d, db) = test_db();
        let (m, mc) = manga_with_chapters(&db, "M", 3);
        let (n, nc) = manga_with_chapters(&db, "N", 1);
        let repo = HistoryRepo::new(db.clone());
        repo.upsert(Kind::Manga, mc[0], 1000, 0).unwrap();
        repo.upsert(Kind::Manga, mc[1], 3000, 0).unwrap();
        repo.upsert(Kind::Manga, nc[0], 2000, 0).unwrap();
        assert_eq!(list_ids(&repo), vec![mc[1], nc[0]]);
        let row2 = repo.by_entry(Kind::Manga, m).unwrap().into_iter().find(|r| r.item_id == mc[1]).unwrap();
        repo.reset_one(Kind::Manga, row2.id).unwrap();
        assert_eq!(list_ids(&repo), vec![nc[0], mc[0]]);
        assert_eq!(repo.latest(Kind::Manga).unwrap().map(|r| r.item_id), Some(nc[0]));
        repo.reset_entry(Kind::Manga, n).unwrap();
        assert_eq!(list_ids(&repo), vec![mc[0]]);
    }

    #[test]
    fn search_is_a_literal_case_insensitive_substring() {
        let (_d, db) = test_db();
        let (_a, ca) = manga_with_chapters(&db, "100% Real", 1);
        let (_b, cb) = manga_with_chapters(&db, "Other", 1);
        let repo = HistoryRepo::new(db.clone());
        repo.upsert(Kind::Manga, ca[0], 1, 0).unwrap();
        repo.upsert(Kind::Manga, cb[0], 2, 0).unwrap();
        let hits = |q: &str| repo.list(Kind::Manga, q).unwrap().into_iter().map(|r| r.title).collect::<Vec<_>>();
        assert_eq!(hits("100%"), vec!["100% Real"]);
        assert_eq!(hits("%"), vec!["100% Real"]);
        assert_eq!(hits("REAL"), vec!["100% Real"]);
        assert!(hits("_").is_empty(), "underscore is literal");
    }

    #[test]
    fn total_duration_keeps_reset_rows_and_clear_all_zeroes_it() {
        let (_d, db) = test_db();
        let (_m, c) = manga_with_chapters(&db, "M", 4);
        let repo = HistoryRepo::new(db.clone());
        repo.upsert(Kind::Manga, c[0], 1, 0).unwrap();
        repo.upsert(Kind::Manga, c[1], 2, 5000).unwrap();
        repo.upsert(Kind::Manga, c[2], 3, 7000).unwrap();
        repo.upsert(Kind::Manga, c[3], 4, 3000).unwrap();
        let reset = repo.by_entry(Kind::Manga, _m).unwrap().into_iter().find(|r| r.item_id == c[3]).unwrap();
        repo.reset_one(Kind::Manga, reset.id).unwrap();
        assert_eq!(repo.total_duration_of(Kind::Manga).unwrap(), 15000);
        assert!(repo.clear_all(Kind::Manga));
        assert_eq!(repo.total_duration_of(Kind::Manga).unwrap(), 0);
        assert!(list_ids(&repo).is_empty());
    }

    #[test]
    fn a_detached_flush_is_recorded_before_shutdown_joins_it() {
        let (_d, db) = test_db();
        let (_m, c) = manga_with_chapters(&db, "M", 1);
        let repo = HistoryRepo::new(db.clone());
        repo.record_detached(HistoryUpdate { kind: Kind::Manga, item_id: c[0], at: 1000, session_ms: 4000 });
        assert_eq!(join_pending_flushes(Duration::from_secs(2)), 0, "the flush finished inside the deadline");
        assert_eq!(repo.total_duration_of(Kind::Manga).unwrap(), 4000);
    }

    #[test]
    fn a_failing_record_is_swallowed_and_a_foreign_key_error_is_reported_by_upsert() {
        let (_d, db) = test_db();
        let repo = HistoryRepo::new(db.clone());
        assert!(repo.upsert(Kind::Manga, 999_999, 1, 0).is_err(), "chapter does not exist");
        repo.record(HistoryUpdate { kind: Kind::Manga, item_id: 999_999, at: 1, session_ms: 0 });
    }

    #[test]
    fn next_items_follow_the_reading_order_of_a_real_manga() {
        let (_d, db) = test_db();
        let (m, c) = manga_with_chapters(&db, "M", 6);
        let repo = HistoryRepo::new(db.clone());
        repo.upsert(Kind::Manga, c[1], 10, 0).unwrap();
        db.write(|tx| {
            tx.execute("UPDATE items SET done = 1 WHERE _id = ?1", [c[1]])?;
            Ok(())
        })
        .unwrap();
        let after = crate::history::next_item::next_items(&db, Kind::Manga, m, Some(c[1]), false);
        let ids: Vec<i64> = after.iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![c[2], c[3], c[4], c[5]], "the read anchor is dropped");
        let pending = crate::history::next_item::next_items(&db, Kind::Manga, m, None, true);
        assert_eq!(pending.len(), 5, "c1 is read");
        let unknown = crate::history::next_item::next_items(&db, Kind::Manga, 999_999, None, true);
        assert!(unknown.is_empty());
        let last = crate::history::next_item::next_after_last_history(&db, Kind::Manga, false);
        assert_eq!(last.first().map(|i| i.id), Some(c[2]));
    }

    #[test]
    fn anime_history_keeps_one_row_per_episode_and_lists_the_newest_per_anime() {
        use crate::testdb::test_db;
        use kuro_data::entry::{Entry, EntryRepo};
        let (_d, db) = test_db();
        let anime = EntryRepo::new(db.clone())
            .insert(&Entry { favorite: true, source: 3, url: "a".into(), title: "Show".into(), ..Entry::create(Kind::Anime) })
            .unwrap();
        let eps: Vec<i64> = (0..2)
            .map(|i| {
                db.write(|tx| {
                    tx.execute(
                        "INSERT INTO items(entry_id, url, name, done, bookmark, progress, total_seconds, number, source_order, date_fetch, date_upload, fillermark) \
                         VALUES (?1, ?2, ?3, 0, 0, 0, 0, ?4, ?5, 0, 0, 0)",
                        (anime, format!("e{i}"), format!("ep{i}"), (i + 1) as f64, 2 - i),
                    )?;
                    Ok(tx.last_insert_rowid())
                })
                .unwrap()
            })
            .collect();
        let repo = HistoryRepo::new(db.clone());
        repo.upsert(Kind::Anime, eps[0], 1000, 0).unwrap();
        repo.upsert(Kind::Anime, eps[0], 2000, 0).unwrap();
        repo.upsert(Kind::Anime, eps[1], 1500, 0).unwrap();
        let rows = repo.list(Kind::Anime, "sho").unwrap();
        assert_eq!(rows.iter().map(|r| (r.item_id, r.at)).collect::<Vec<_>>(), vec![(eps[0], 2000)]);
        assert_eq!(repo.by_entry(Kind::Anime, anime).unwrap().len(), 2);
        assert_eq!(repo.latest(Kind::Anime).unwrap().map(|r| r.item_id), Some(eps[0]));
        let newest = repo.by_entry(Kind::Anime, anime).unwrap().into_iter().find(|r| r.item_id == eps[0]).unwrap();
        repo.reset_one(Kind::Anime, newest.id).unwrap();
        assert_eq!(repo.list(Kind::Anime, "").unwrap().first().map(|r| r.item_id), Some(eps[1]));
        repo.reset_entry(Kind::Anime, anime).unwrap();
        assert!(repo.list(Kind::Anime, "").unwrap().is_empty());
        assert!(repo.clear_all(Kind::Anime));
    }

    /// The shared upsert adds the delta for both kinds (an anime keeps its watched time, as a manga its reading time).
    #[test]
    fn history_upsert_adds_the_delta_for_both_kinds() {
        use kuro_data::entry::{Entry, EntryRepo};
        use kuro_data::item::{Item, ItemRepo};
        let (_d, db) = test_db();
        let repo = HistoryRepo::new(db.clone());
        for kind in [Kind::Manga, Kind::Anime] {
            let entries = EntryRepo::new(db.clone());
            let id = entries.insert(&Entry { url: format!("/{kind}"), title: "T".into(), favorite: true, ..Entry::create(kind) }).unwrap();
            let item = ItemRepo::new(db.clone()).add_all(&[Item { entry_id: id, url: "i".into(), ..Item::create(kind) }]).unwrap()[0].id;
            repo.upsert(kind, item, 1000, 5000).unwrap();
            repo.upsert(kind, item, 2000, 3000).unwrap();
            let rows = repo.by_entry(kind, id).unwrap();
            assert_eq!((rows.len(), rows[0].at, rows[0].time_read), (1, 2000, 8000), "{kind}");
            assert_eq!(repo.total_duration_of(kind).unwrap(), 8000, "{kind}: the total is scoped to the kind");
        }
    }
}
