//! The queue store of both kinds (merges `store.rs` and `anime/store.rs`). Rows live in `download_queue`; the statements are
//! `kuro_db::tables::downloads`, so no SQL is here. The store sits on the one `Db` handle.
//!
//! The actor writes the whole lane after each membership or order change in one transaction, so `position` values stay
//! dense. Rows are read back `ORDER BY position`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use kuro_db::tables::downloads;
use kuro_db::{Db, DbError};
use kuro_platform::kind::Kind;

/// One `download_queue` row (`state` is the `DownloadState` value at the last write; restore sets QUEUE).
pub use kuro_db::tables::downloads::QueueRow as StoredRow;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Db(#[from] DbError),
    /// The last [`QueueStore::load`] of this kind failed, so the saved rows are unknown. Writing would delete them.
    #[error("the saved queue could not be read, so it is left as it is")]
    Unreadable,
}

pub struct QueueStore {
    db: Arc<Db>,
    /// Per kind (manga, anime). Set when `load` fails and cleared when it succeeds. While set, `replace` of that kind
    /// refuses, because it deletes the kind's rows and would drop every saved row the load could not read (T-23a-035, B4).
    unreadable: [AtomicBool; 2],
}

fn slot(kind: Kind) -> usize {
    match kind {
        Kind::Manga => 0,
        Kind::Anime => 1,
    }
}

impl QueueStore {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db, unreadable: [AtomicBool::new(false), AtomicBool::new(false)] }
    }

    /// Every row of `kind`, ordered by `position`. A failure sets the guard of `kind`; a success lifts it.
    pub fn load(&self, kind: Kind) -> Result<Vec<StoredRow>, StoreError> {
        let rows = self.db.read(|c| Ok(downloads::queue_load(c, kind)?));
        self.unreadable[slot(kind)].store(rows.is_err(), Ordering::SeqCst);
        Ok(rows?)
    }

    /// Sets the guard of `kind` by hand: restore calls it when it could not read the rows behind the queue, so the
    /// next `replace` cannot erase them.
    pub(crate) fn mark_unreadable(&self, kind: Kind) {
        self.unreadable[slot(kind)].store(true, Ordering::SeqCst);
    }

    /// Replaces the rows of `kind` in one transaction. Positions are renumbered `0..n` in slice order, the other kind is
    /// untouched, and a row whose item is gone is skipped (see `queue_replace`).
    pub fn replace(&self, kind: Kind, rows: &[StoredRow]) -> Result<(), StoreError> {
        if self.unreadable[slot(kind)].load(Ordering::SeqCst) {
            return Err(StoreError::Unreadable);
        }
        self.db.write(|tx| Ok(downloads::queue_replace(tx, kind, rows)?))?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use kuro_data::entry::{Entry, EntryRepo};
    use kuro_data::item::{Item, ItemRepo};
    use kuro_db::schema_unified;

    use super::*;

    /// A unified database in a file (a `Db` reads through its own connections, so it cannot be a memory database).
    pub fn test_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let path = schema_unified::db_path(dir.path());
        let writer = schema_unified::open_file(&path).unwrap();
        let db = Arc::new(Db::new(&path, writer).unwrap());
        (dir, db)
    }

    /// An entry of `kind` on `source` with `n` items; returns the entry id and the item ids.
    pub fn seed(db: &Arc<Db>, kind: Kind, source: i64, url: &str, n: usize) -> (i64, Vec<i64>) {
        let mut entry = Entry::create(kind);
        entry.source = source;
        entry.url = url.to_owned();
        entry.title = format!("Title {url}");
        let entry_id = EntryRepo::new(db.clone()).insert(&entry).unwrap();
        let new: Vec<Item> = (0..n)
            .map(|i| {
                let mut item = Item::create(kind);
                item.entry_id = entry_id;
                item.url = format!("{url}/{i}");
                item.name = format!("Unit {i}");
                item.number = i as f64;
                item
            })
            .collect();
        let ids = ItemRepo::new(db.clone()).add_all(&new).unwrap().iter().map(|i| i.id).collect();
        (entry_id, ids)
    }

    pub fn row(unit_id: i64, entry_id: i64, source: i64) -> StoredRow {
        StoredRow { unit_id, entry_id, source, position: 99, state: 1, enqueued_at: 0 }
    }

    pub fn count(db: &Db, table: &str) -> i64 {
        db.read(|c| Ok(c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{count, row, seed, test_db};
    use super::*;

    fn ids(rows: &[StoredRow]) -> Vec<i64> {
        rows.iter().map(|r| r.unit_id).collect()
    }

    #[test]
    fn load_returns_rows_in_position_order_and_renumbers_densely() {
        let (_dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        for kind in Kind::ALL {
            let (e, u) = seed(&db, kind, 1, "/a", 3);
            store.replace(kind, &[row(u[2], e, 1), row(u[0], e, 1), row(u[1], e, 1)]).unwrap();
            let back = store.load(kind).unwrap();
            assert_eq!(ids(&back), vec![u[2], u[0], u[1]]);
            assert_eq!(back.iter().map(|r| r.position).collect::<Vec<_>>(), vec![0, 1, 2], "TST-43");
        }
    }

    #[test]
    fn replace_with_an_empty_slice_clears_only_its_own_kind() {
        let (_dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        let (me, mu) = seed(&db, Kind::Manga, 1, "/m", 1);
        let (ae, au) = seed(&db, Kind::Anime, 1, "/a", 1);
        store.replace(Kind::Manga, &[row(mu[0], me, 1)]).unwrap();
        store.replace(Kind::Anime, &[row(au[0], ae, 1)]).unwrap();
        store.replace(Kind::Manga, &[]).unwrap();
        assert!(store.load(Kind::Manga).unwrap().is_empty());
        assert_eq!(ids(&store.load(Kind::Anime).unwrap()), vec![au[0]]);
    }

    #[test]
    fn a_failing_row_leaves_the_previous_contents_untouched() {
        let (_dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        let (e, u) = seed(&db, Kind::Manga, 1, "/a", 3);
        store.replace(Kind::Manga, &[row(u[0], e, 1)]).unwrap();
        // A repeated unit id breaks the primary key half way through: the transaction rolls back (TST-42).
        assert!(store.replace(Kind::Manga, &[row(u[1], e, 1), row(u[2], e, 1), row(u[1], e, 1)]).is_err());
        assert_eq!(ids(&store.load(Kind::Manga).unwrap()), vec![u[0]]);
    }

    #[test]
    fn a_row_whose_item_is_gone_is_skipped_not_fatal() {
        let (_dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        let (e, u) = seed(&db, Kind::Anime, 1, "/a", 1);
        store.replace(Kind::Anime, &[row(u[0], e, 1), row(9_999, e, 1)]).unwrap();
        assert_eq!(ids(&store.load(Kind::Anime).unwrap()), vec![u[0]]);
    }

    #[test]
    fn a_failed_load_stops_replace_from_wiping_that_kind_only() {
        let (_dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        let (ae, au) = seed(&db, Kind::Anime, 1, "/a", 2);
        let (me, mu) = seed(&db, Kind::Manga, 1, "/m", 2);
        store.replace(Kind::Anime, &[row(au[0], ae, 1)]).unwrap();
        store.replace(Kind::Manga, &[row(mu[0], me, 1)]).unwrap();
        // A status the row mapper cannot read: the load fails, but a DELETE would still succeed.
        db.write(|tx| Ok(tx.execute("UPDATE download_queue SET status = 'bad' WHERE kind = 'anime'", [])?)).unwrap();
        assert!(store.load(Kind::Anime).is_err());
        assert!(matches!(store.replace(Kind::Anime, &[row(au[1], ae, 1)]), Err(StoreError::Unreadable)));
        assert_eq!(count(&db, "download_queue WHERE kind = 'anime'"), 1, "the unreadable row is still there");
        store.load(Kind::Manga).unwrap();
        store.replace(Kind::Manga, &[row(mu[1], me, 1)]).unwrap();
        assert_eq!(ids(&store.load(Kind::Manga).unwrap()), vec![mu[1]], "the manga lane still saves");
        db.write(|tx| Ok(tx.execute("UPDATE download_queue SET status = 1", [])?)).unwrap();
        store.load(Kind::Anime).unwrap();
        store.replace(Kind::Anime, &[row(au[1], ae, 1)]).unwrap();
    }

    #[test]
    fn mark_unreadable_blocks_replace_until_a_good_load() {
        let (_dir, db) = test_db();
        let store = QueueStore::new(db);
        store.mark_unreadable(Kind::Manga);
        assert!(matches!(store.replace(Kind::Manga, &[]), Err(StoreError::Unreadable)));
        store.replace(Kind::Anime, &[]).unwrap();
        store.load(Kind::Manga).unwrap();
        store.replace(Kind::Manga, &[]).unwrap();
    }

    #[test]
    fn queue_rows_cascade_with_the_item_and_pending_rows_do_not() {
        let (_dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        let (e, u) = seed(&db, Kind::Manga, 1, "/m", 1);
        db.write(|tx| {
            Ok(tx.execute(
                "INSERT INTO download_pending_delete(unit_id, kind, entry_id, entry_url, entry_title, source, unit_url, unit_name, seq) \
                 VALUES (?1, 'manga', ?2, '/m', 'M', 1, '/c', 'C', 0)",
                rusqlite::params![u[0], e],
            )?)
        })
        .unwrap();
        store.replace(Kind::Manga, &[row(u[0], e, 1)]).unwrap();
        assert_eq!(store.load(Kind::Manga).unwrap().len(), 1);
        db.write(|tx| Ok(tx.execute("DELETE FROM items WHERE _id = ?1", rusqlite::params![u[0]])?)).unwrap();
        assert!(store.load(Kind::Manga).unwrap().is_empty(), "the queue row left with its item");
        assert_eq!(count(&db, "download_pending_delete"), 1, "the item may be gone when the flush runs");
    }
}
