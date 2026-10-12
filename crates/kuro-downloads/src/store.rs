//! SQLite queue store (port of MangaDownloadStore.kt). Rows live in the unified `download_queue` table with `kind = 'manga'`.
//!
//! The downloader writes the whole queue after each membership or order change, inside one transaction, so
//! `position` values stay dense by construction. Rows are read back `ORDER BY position`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, params};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error("queue store lock was poisoned")]
    Poisoned,
    /// The last [`QueueStore::load`] failed, so the saved rows are unknown. Writing would delete them.
    #[error("the saved queue could not be read, so it is left as it is")]
    Unreadable,
}

/// One manga row of `download_queue` (`chapter_id` = `unit_id`, `manga_id` = `entry_id`, `source_id` = `source`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueRow {
    pub chapter_id: i64,
    pub manga_id: i64,
    pub source_id: i64,
    pub position: i64,
    /// `DownloadState` value at the last write. Informational: restore sets QUEUE.
    pub status: u8,
    pub enqueued_at: i64,
}

pub struct QueueStore {
    conn: Mutex<Connection>,
    /// Set when `load` fails and cleared when it succeeds. While set, `replace_all` refuses, because it deletes the
    /// whole table and would drop every saved row the load could not read (T-23a-035).
    unreadable: AtomicBool,
}

impl QueueStore {
    /// The tables belong to the unified database schema. The connection is used as given, so the caller owns the pragmas.
    pub fn open(conn: Connection) -> Result<Self, StoreError> {
        Ok(Self { conn: Mutex::new(conn), unreadable: AtomicBool::new(false) })
    }

    /// A private database with only the download tables. Foreign keys are off, so tests may use arbitrary ids.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::open(crate::memory_downloads_db()?)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn.lock().map_err(|_| StoreError::Poisoned)
    }

    /// Every row, ordered by `position`.
    pub fn load(&self) -> Result<Vec<QueueRow>, StoreError> {
        let rows = self.read_rows();
        self.unreadable.store(rows.is_err(), Ordering::SeqCst);
        rows
    }

    fn read_rows(&self) -> Result<Vec<QueueRow>, StoreError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT unit_id, entry_id, source, position, status, enqueued_at \
             FROM download_queue WHERE kind = 'manga' ORDER BY position",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(QueueRow {
                    chapter_id: r.get(0)?,
                    manga_id: r.get(1)?,
                    source_id: r.get(2)?,
                    position: r.get(3)?,
                    status: r.get::<_, i64>(4)? as u8,
                    enqueued_at: r.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Replaces the whole queue in one transaction. Positions are renumbered `0..n` in slice order. A failure in
    /// any row rolls the whole change back, so the table keeps its previous contents.
    pub fn replace_all(&self, rows: &[QueueRow]) -> Result<(), StoreError> {
        if self.unreadable.load(Ordering::SeqCst) {
            return Err(StoreError::Unreadable);
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM download_queue WHERE kind = 'manga'", [])?;
        for (position, row) in rows.iter().enumerate() {
            tx.execute(
                "INSERT INTO download_queue \
                 (unit_id, kind, entry_id, source, position, status, enqueued_at) VALUES (?1, 'manga', ?2, ?3, ?4, ?5, ?6)",
                params![row.chapter_id, row.manga_id, row.source_id, position as i64, row.status as i64, row.enqueued_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(chapter_id: i64, position: i64) -> QueueRow {
        QueueRow { chapter_id, manga_id: 1, source_id: 1, position, status: 1, enqueued_at: 0 }
    }

    fn store_with_chapters() -> QueueStore {
        QueueStore::open_in_memory().unwrap()
    }

    #[test]
    fn load_returns_rows_in_position_order() {
        let store = store_with_chapters();
        store.replace_all(&[row(10, 0), row(11, 1), row(12, 2)]).unwrap();
        let ids: Vec<i64> = store.load().unwrap().iter().map(|r| r.chapter_id).collect();
        assert_eq!(ids, vec![10, 11, 12]);
    }

    // TST-43
    #[test]
    fn positions_are_renumbered_densely_on_write() {
        let store = store_with_chapters();
        store.replace_all(&[row(10, 0), row(11, 5), row(12, 9)]).unwrap();
        let positions: Vec<i64> = store.load().unwrap().iter().map(|r| r.position).collect();
        assert_eq!(positions, vec![0, 1, 2]);
    }

    // TST-42
    #[test]
    fn a_failing_row_leaves_the_previous_contents_untouched() {
        let store = store_with_chapters();
        store.replace_all(&[row(10, 0), row(11, 1)]).unwrap();
        // Row 3 repeats chapter 12: the primary key rejects it and the transaction rolls back.
        let bad = [row(12, 0), row(13, 1), row(12, 2)];
        assert!(store.replace_all(&bad).is_err());
        let ids: Vec<i64> = store.load().unwrap().iter().map(|r| r.chapter_id).collect();
        assert_eq!(ids, vec![10, 11]);
    }

    #[test]
    fn replace_all_with_an_empty_slice_clears_the_queue() {
        let store = store_with_chapters();
        store.replace_all(&[row(10, 0)]).unwrap();
        store.replace_all(&[]).unwrap();
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn a_failed_load_stops_replace_all_from_wiping_the_table() {
        let store = store_with_chapters();
        store.replace_all(&[row(10, 0)]).unwrap();
        // A status the row mapper cannot read: the load fails, but a DELETE would still succeed.
        store.lock().unwrap().execute("UPDATE download_queue SET status = 'bad'", []).unwrap();
        assert!(store.load().is_err());
        assert!(matches!(store.replace_all(&[row(11, 0)]), Err(StoreError::Unreadable)));
        let kept: i64 =
            store.lock().unwrap().query_row("SELECT COUNT(*) FROM download_queue", [], |r| r.get(0)).unwrap();
        assert_eq!(kept, 1, "the unreadable row is still there");
        store.lock().unwrap().execute("UPDATE download_queue SET status = 1", []).unwrap();
        assert!(store.load().is_ok(), "a later good load lifts the guard");
        store.replace_all(&[row(11, 0)]).unwrap();
    }

    #[test]
    fn the_in_memory_store_has_the_download_tables() {
        let store = QueueStore::open_in_memory().unwrap();
        let conn = store.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN \
                 ('download_queue','download_pending_delete','download_pending_corrupt')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn the_anime_rows_of_the_shared_table_are_not_touched() {
        let store = QueueStore::open_in_memory().unwrap();
        store.lock().unwrap().execute(
            "INSERT INTO download_queue(unit_id, kind, entry_id, source, position, enqueued_at) VALUES (99, 'anime', 5, 1, 0, 0)",
            [],
        ).unwrap();
        store.replace_all(&[row(10, 0)]).unwrap();
        store.replace_all(&[]).unwrap();
        let kept: i64 = store.lock().unwrap().query_row("SELECT COUNT(*) FROM download_queue", [], |r| r.get(0)).unwrap();
        assert_eq!(kept, 1);
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn download_queue_rows_cascade_with_the_item_and_pending_rows_do_not() {
        let conn = kuro_db::schema_unified::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO entries(_id, kind, source, url, title, status, favorite, initialized, viewer, item_flags,
                                 cover_last_modified, date_added) VALUES (1, 'manga', 1, '/m', 'M', 0, 1, 1, 0, 0, 0, 0);
             INSERT INTO items(_id, entry_id, url, name, done, bookmark, progress, number, source_order, date_fetch, date_upload)
                 VALUES (10, 1, '/c', 'C', 0, 0, 0, 1.0, 0, 0, 0);
             INSERT INTO download_pending_delete(unit_id, kind, entry_id, entry_url, entry_title, source, unit_url, unit_name, seq)
                 VALUES (10, 'manga', 1, '/m', 'M', 1, '/c', 'C', 0);",
        )
        .unwrap();
        let store = QueueStore::open(conn).unwrap();
        store.replace_all(&[row(10, 0)]).unwrap();
        assert_eq!(store.load().unwrap().len(), 1);
        store.lock().unwrap().execute("DELETE FROM items WHERE _id = 10", []).unwrap();
        assert!(store.load().unwrap().is_empty(), "the queue row left with its item");
        let pending: i64 =
            store.lock().unwrap().query_row("SELECT COUNT(*) FROM download_pending_delete", [], |r| r.get(0)).unwrap();
        assert_eq!(pending, 1, "the pending row stays: the item may be gone when the flush runs");
    }
}
