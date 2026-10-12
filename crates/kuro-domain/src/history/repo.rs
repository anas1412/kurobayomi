//! History repository for both kinds: the list, the latest row, resets, clear-all and the upsert. The SQL is in
//! `kuro-db` (manga `history`/`historyView`, anime `animehistory`/`animehistoryView`), except the by-entry query of
//! anime, which is written here.

use kuro_platform::kind::Kind;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kuro_db::handle::Db;
use kuro_db::schema_anime::Millis;
use kuro_db::statements_anime as anime;
use kuro_db::statements_manga::history as manga_history;
use kuro_db::statements_manga::views as manga_views;

use super::model::{CoverKey, HistoryRow, HistoryUpdate, HistoryWithRelations};
use crate::error::DomainError;

/// Reads and writes the history rows of either kind.
#[derive(Clone)]
pub struct HistoryRepo {
    db: Arc<Db>,
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

fn manga_row(r: kuro_db::statements_manga::HistoryViewRow) -> HistoryWithRelations {
    HistoryWithRelations {
        id: r.id,
        item_id: r.chapter_id,
        entry_id: r.manga_id,
        title: r.title,
        item_number: r.chapter_number,
        at: r.read_at,
        duration_ms: r.read_duration,
        cover: CoverKey {
            entry_id: r.manga_id,
            source_id: r.source,
            favorite: r.favorite,
            url: r.thumbnail_url,
            cover_last_modified: r.cover_last_modified,
        },
    }
}

fn anime_row(r: anime::HistoryEntry) -> HistoryWithRelations {
    HistoryWithRelations {
        id: r.id,
        item_id: r.episode_id,
        entry_id: r.anime_id,
        title: r.title,
        item_number: r.episode_number,
        at: r.seen_at.0,
        duration_ms: 0,
        cover: CoverKey {
            entry_id: r.anime_id,
            source_id: r.source,
            favorite: r.favorite,
            url: r.thumbnail_url,
            cover_last_modified: r.cover_last_modified,
        },
    }
}

impl HistoryRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// One row per entry (its newest row), only rows with a timestamp above 0, newest first. `query` is a
    /// case-insensitive substring of the title; `%`, `_` and `\` are literal.
    pub fn list(&self, kind: Kind, query: &str) -> Result<Vec<HistoryWithRelations>, DomainError> {
        Ok(match kind {
            Kind::Manga => {
                self.db.read(|c| Ok(manga_views::history(c, query)?))?.into_iter().map(manga_row).collect()
            }
            Kind::Anime => self.db.read(|c| Ok(anime::history(c, query)?))?.into_iter().map(anime_row).collect(),
        })
    }

    /// Live list of [`HistoryRepo::list`]: re-runs after a change to the history, item or entry tables.
    pub fn watch_list(
        &self,
        kind: Kind,
        query: String,
    ) -> kuro_db::subscribe::Subscription<Vec<HistoryWithRelations>> {
        let tables: &'static [&'static str] = match kind {
            Kind::Manga => &["history", "chapters", "mangas"],
            Kind::Anime => &["animehistory", "episodes", "animes"],
        };
        self.db.subscribe(tables, std::time::Duration::from_millis(30), move |c| match kind {
            Kind::Manga => Ok(manga_views::history(c, &query)?.into_iter().map(manga_row).collect()),
            Kind::Anime => Ok(anime::history(c, &query)?.into_iter().map(anime_row).collect()),
        })
    }

    /// The newest row with a timestamp above 0, or `None`.
    pub fn latest(&self, kind: Kind) -> Result<Option<HistoryWithRelations>, DomainError> {
        let rows = self.list(kind, "")?;
        Ok(rows.into_iter().max_by_key(|r| r.at))
    }

    /// Every history row of one entry, reset rows included.
    pub fn by_entry(&self, kind: Kind, entry_id: i64) -> Result<Vec<HistoryRow>, DomainError> {
        Ok(match kind {
            Kind::Manga => self
                .db
                .read(|c| Ok(manga_history::get_by_manga_id(c, entry_id)?))?
                .into_iter()
                .map(|r| HistoryRow { id: r.id, item_id: r.chapter_id, at: r.last_read, time_read: r.time_read })
                .collect(),
            Kind::Anime => self.db.read(|c| {
                let mut st = c.prepare(
                    "SELECT H._id, H.episode_id, H.last_seen FROM animehistory H \
                     JOIN episodes E ON H.episode_id = E._id WHERE E.anime_id = ?1",
                )?;
                let mut out = Vec::new();
                for row in st.query_map([entry_id], |r| {
                    Ok(HistoryRow { id: r.get(0)?, item_id: r.get(1)?, at: r.get(2)?, time_read: 0 })
                })? {
                    out.push(row?);
                }
                Ok(out)
            })?,
        })
    }

    /// Records a read or watch: sets the timestamp and adds the session to the duration (manga). A chapter or
    /// episode that is already recorded keeps one row.
    pub fn upsert(&self, kind: Kind, item_id: i64, at: i64, session_ms: i64) -> Result<(), DomainError> {
        self.db.write(|tx| {
            match kind {
                Kind::Manga => manga_history::upsert(tx, item_id, at, session_ms)?,
                Kind::Anime => anime::upsert_history(tx, item_id, Millis(at))?,
            };
            Ok(())
        })?;
        Ok(())
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
    pub fn reset_one(&self, kind: Kind, history_id: i64) -> Result<(), DomainError> {
        self.db.write(|tx| {
            match kind {
                Kind::Manga => manga_history::reset_by_id(tx, history_id)?,
                Kind::Anime => anime::reset_history(tx, history_id)?,
            };
            Ok(())
        })?;
        Ok(())
    }

    /// Resets every row of one entry.
    pub fn reset_entry(&self, kind: Kind, entry_id: i64) -> Result<(), DomainError> {
        self.db.write(|tx| {
            match kind {
                Kind::Manga => manga_history::reset_by_manga_id(tx, entry_id)?,
                Kind::Anime => anime::reset_history_by_anime(tx, entry_id)?,
            };
            Ok(())
        })?;
        Ok(())
    }

    /// Deletes every row. Returns `false` (after logging) when the delete fails.
    pub fn clear_all(&self, kind: Kind) -> bool {
        let result = self.db.write(|tx| {
            match kind {
                Kind::Manga => manga_history::remove_all(tx)?,
                Kind::Anime => anime::delete_all_history(tx)?,
            };
            Ok(())
        });
        match result {
            Ok(()) => true,
            Err(e) => {
                tracing::error!(error = %e, "cannot clear history");
                false
            }
        }
    }

    /// Deletes the rows that were reset ("Clear database" maintenance action).
    pub fn remove_reset(&self, kind: Kind) -> Result<usize, DomainError> {
        Ok(self.db.write(|tx| {
            Ok(match kind {
                Kind::Manga => manga_history::remove_resetted(tx)?,
                Kind::Anime => anime::remove_resetted_history(tx)?,
            })
        })?)
    }

    /// Total reading time of every manga row, reset rows included. Anime has no duration.
    pub fn total_duration(&self) -> Result<i64, DomainError> {
        Ok(self.db.read(|c| Ok(manga_history::get_read_duration(c)?))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::manga_db;
    use kuro_data::manga::{Manga, MangaRepo};

    /// A favourite manga with `n` chapters, `source_order` descending so the reading order is the insert order.
    fn manga_with_chapters(db: &Arc<Db>, title: &str, n: i64) -> (i64, Vec<i64>) {
        let manga = MangaRepo::new(db.clone())
            .insert(&Manga {
                favorite: true,
                source: 7,
                url: format!("u/{title}"),
                title: title.into(),
                ..Manga::create()
            })
            .unwrap();
        let ids = (0..n)
            .map(|i| {
                db.write(|tx| {
                    tx.execute(
                        "INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload) \
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
        let (_m, c) = manga_with_chapters(&db, "M", 4);
        let repo = HistoryRepo::new(db.clone());
        repo.upsert(Kind::Manga, c[0], 1, 0).unwrap();
        repo.upsert(Kind::Manga, c[1], 2, 5000).unwrap();
        repo.upsert(Kind::Manga, c[2], 3, 7000).unwrap();
        repo.upsert(Kind::Manga, c[3], 4, 3000).unwrap();
        let reset = repo.by_entry(Kind::Manga, _m).unwrap().into_iter().find(|r| r.item_id == c[3]).unwrap();
        repo.reset_one(Kind::Manga, reset.id).unwrap();
        assert_eq!(repo.total_duration().unwrap(), 15000);
        assert!(repo.clear_all(Kind::Manga));
        assert_eq!(repo.total_duration().unwrap(), 0);
        assert!(list_ids(&repo).is_empty());
    }

    #[test]
    fn a_detached_flush_is_recorded_before_shutdown_joins_it() {
        let (_d, db) = manga_db();
        let (_m, c) = manga_with_chapters(&db, "M", 1);
        let repo = HistoryRepo::new(db.clone());
        repo.record_detached(HistoryUpdate { kind: Kind::Manga, item_id: c[0], at: 1000, session_ms: 4000 });
        assert_eq!(join_pending_flushes(Duration::from_secs(2)), 0, "the flush finished inside the deadline");
        assert_eq!(repo.total_duration().unwrap(), 4000);
    }

    #[test]
    fn a_failing_record_is_swallowed_and_a_foreign_key_error_is_reported_by_upsert() {
        let (_d, db) = manga_db();
        let repo = HistoryRepo::new(db.clone());
        assert!(repo.upsert(Kind::Manga, 999_999, 1, 0).is_err(), "chapter does not exist");
        repo.record(HistoryUpdate { kind: Kind::Manga, item_id: 999_999, at: 1, session_ms: 0 });
    }

    #[test]
    fn next_items_follow_the_reading_order_of_a_real_manga() {
        let (_d, db) = manga_db();
        let (m, c) = manga_with_chapters(&db, "M", 6);
        let repo = HistoryRepo::new(db.clone());
        repo.upsert(Kind::Manga, c[1], 10, 0).unwrap();
        db.write(|tx| {
            tx.execute("UPDATE chapters SET read = 1 WHERE _id = ?1", [c[1]])?;
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
        use crate::testdb::anime_db;
        use kuro_data::anime::{Anime, AnimeRepo};
        let (_d, db) = anime_db();
        let anime = AnimeRepo::new(db.clone())
            .insert(&Anime { favorite: true, source: 3, url: "a".into(), title: "Show".into(), ..Anime::create() })
            .unwrap();
        let eps: Vec<i64> = (0..2)
            .map(|i| {
                db.write(|tx| {
                    tx.execute(
                        "INSERT INTO episodes(anime_id, url, name, seen, bookmark, last_second_seen, total_seconds, episode_number, source_order, date_fetch, date_upload, fillermark) \
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
}
