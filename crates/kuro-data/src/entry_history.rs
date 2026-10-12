//! History of both kinds: one `History`, one `HistoryItem` and one `HistoryRepo` over `kuro_db::tables::history`.
//! (It sits beside `history`, which keeps the legacy per-kind names and the session recorder until phase E; this module
//! then becomes `history`.)

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::subscribe::Subscription;
use kuro_db::tables::history as t;
use kuro_db::tables::{HistoryRow, HistoryViewRow};
use kuro_platform::kind::Kind;
use rusqlite::Connection;

use crate::{DataResult, ENTRY_DEBOUNCE};

/// Tables a history list reads.
const HISTORY_TABLES: &[&str] = &["history", "items", "entries"];

/// One `history` row of an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct History {
    pub id: i64,
    pub item_id: i64,
    /// Epoch milliseconds of the last read or view; 0 after a reset (the row and its duration stay).
    pub last_at: i64,
    /// Accumulated time in milliseconds.
    pub duration_ms: i64,
}

impl From<HistoryRow> for History {
    fn from(r: HistoryRow) -> Self {
        Self { id: r.id, item_id: r.item_id, last_at: r.last_at, duration_ms: r.duration_ms }
    }
}

/// One line of the history list: the newest history row of an entry, with the entry's cover data.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryItem {
    pub id: i64,
    pub kind: Kind,
    pub entry_id: i64,
    pub item_id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub source: i64,
    pub favorite: bool,
    pub cover_last_modified: i64,
    pub item_number: f64,
    pub last_at: i64,
    pub duration_ms: i64,
}

impl From<HistoryViewRow> for HistoryItem {
    fn from(r: HistoryViewRow) -> Self {
        Self {
            id: r.id,
            kind: r.kind,
            entry_id: r.entry_id,
            item_id: r.item_id,
            title: r.title,
            thumbnail_url: r.thumbnail_url,
            source: r.source,
            favorite: r.favorite,
            cover_last_modified: r.cover_last_modified,
            item_number: r.item_number,
            last_at: r.last_at,
            duration_ms: r.duration_ms,
        }
    }
}

/// History access for both kinds.
#[derive(Clone)]
pub struct HistoryRepo {
    db: Arc<Db>,
}

impl HistoryRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// One row per entry of the kind (its newest history row) with `last_at > 0`, newest first, ties by the higher id.
    /// `query` is a case-insensitive substring of the title (Unicode fold); `%`, `_` and `\` are literal.
    pub fn list(&self, kind: Kind, query: &str) -> DataResult<Vec<HistoryItem>> {
        let rows = self.db.read(|c| Ok(t::list(c, kind, query)?))?;
        Ok(rows.into_iter().map(HistoryItem::from).collect())
    }

    /// The list, published again after a change to history, items or entries.
    pub fn watch(&self, kind: Kind, query: &str) -> Subscription<Vec<HistoryItem>> {
        self.watch_mapped(kind, query, |item| item)
    }

    /// [`Self::watch`] with each row turned into `T` inside the query, so the legacy per-kind row shapes publish too.
    pub(crate) fn watch_mapped<T: PartialEq + Send + Sync + 'static>(
        &self,
        kind: Kind,
        query: &str,
        map: fn(HistoryItem) -> T,
    ) -> Subscription<Vec<T>> {
        let query = query.to_owned();
        self.db.subscribe(HISTORY_TABLES, ENTRY_DEBOUNCE, move |c| {
            Ok(t::list(c, kind, &query)?.into_iter().map(|r| map(r.into())).collect::<Vec<_>>())
        })
    }

    /// The newest history row of the kind with `last_at > 0`; equals the first row of [`Self::list`] with no query.
    pub fn latest(&self, kind: Kind) -> DataResult<Option<HistoryItem>> {
        Ok(self.db.read(|c| Ok(t::latest(c, kind)?))?.map(HistoryItem::from))
    }

    /// Accumulated time over every history row of the kind, reset rows included.
    pub fn total_duration(&self, kind: Kind) -> DataResult<i64> {
        self.db.read(|c| Ok(t::total_duration(c, kind)?))
    }

    /// Every history row of one entry, reset rows included.
    pub fn by_entry_id(&self, entry_id: i64) -> DataResult<Vec<History>> {
        let rows = self.db.read(|c| Ok(t::by_entry(c, entry_id)?))?;
        Ok(rows.into_iter().map(History::from).collect())
    }

    /// The history row of an item of the kind with this url, lowest id first (the backup restore reads it).
    pub fn by_item_url(c: &Connection, kind: Kind, url: &str) -> DataResult<Option<History>> {
        Ok(t::by_item_url(c, kind, url)?.map(History::from))
    }

    /// Sets the time of the last read to `at_ms` and ADDS `delta_ms` to the duration (a new row starts at `delta_ms`).
    pub fn upsert(&self, item_id: i64, at_ms: i64, delta_ms: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(t::upsert(tx, item_id, at_ms, delta_ms).map(drop)?))
    }

    /// `last_at = 0` on one row; the row and its duration stay.
    pub fn reset_by_id(&self, history_id: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(t::reset_by_id(tx, history_id).map(drop)?))
    }

    pub fn reset_by_entry_id(&self, entry_id: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(t::reset_by_entry(tx, entry_id).map(drop)?))
    }

    /// Deletes every history row of the kind, time included.
    pub fn delete_all(&self, kind: Kind) -> DataResult<()> {
        self.db.write(|tx| Ok(t::remove_all(tx, kind).map(drop)?))
    }

    /// Deletes the reset rows of the kind inside the caller's transaction (restore). Returns the rows removed.
    pub fn remove_resetted(c: &Connection, kind: Kind) -> DataResult<usize> {
        Ok(t::remove_resetted(c, kind)?)
    }
}
