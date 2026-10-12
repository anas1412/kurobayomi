//! `history` statements for both kinds. `upsert` sets the time and ADDS the delta (anime passes 0 until the player
//! writes durations).

use kuro_platform::kind::Kind;
use rusqlite::{Connection, Result};

use super::{escape_like, exec, query_all, query_opt, rows::*};

const ITEM_IDS: &str = "(SELECT i._id FROM items i JOIN entries e ON e._id = i.entry_id WHERE e.kind = :kind)";
const VIEW_COLS: &str = "id, kind, entryId, itemId, title, thumbnailUrl, source, favorite, cover_last_modified, itemNumber, lastAt, durationMs";

pub fn by_entry(c: &Connection, entry_id: i64) -> Result<Vec<HistoryRow>> {
    query_all(
        c,
        "SELECT H._id, H.item_id, H.last_at, H.duration_ms FROM history H JOIN items I ON H.item_id = I._id WHERE I.entry_id = :entryId ORDER BY H._id",
        &[(":entryId", &entry_id)],
        HistoryRow::from_row,
    )
}

/// The history row of an item of the kind with this url, lowest id first (restore reads it).
pub fn by_item_url(c: &Connection, kind: Kind, url: &str) -> Result<Option<HistoryRow>> {
    query_opt(
        c,
        "SELECT H._id, H.item_id, H.last_at, H.duration_ms FROM history H JOIN items I ON H.item_id = I._id JOIN entries E ON E._id = I.entry_id WHERE E.kind = :kind AND I.url = :url ORDER BY H._id LIMIT 1",
        &[(":kind", &kind.as_str()), (":url", &url)],
        HistoryRow::from_row,
    )
}

/// `last_at = 0` on one row; the row and its duration stay.
pub fn reset_by_id(c: &Connection, history_id: i64) -> Result<usize> {
    exec(c, "UPDATE history SET last_at = 0 WHERE _id = :id", &[(":id", &history_id)])
}

pub fn reset_by_entry(c: &Connection, entry_id: i64) -> Result<usize> {
    exec(
        c,
        "UPDATE history SET last_at = 0 WHERE item_id IN (SELECT _id FROM items WHERE entry_id = :entryId)",
        &[(":entryId", &entry_id)],
    )
}

/// Deletes every history row of the kind.
pub fn remove_all(c: &Connection, kind: Kind) -> Result<usize> {
    exec(c, concat!("DELETE FROM history WHERE item_id IN ", "(SELECT i._id FROM items i JOIN entries e ON e._id = i.entry_id WHERE e.kind = :kind)"), &[(":kind", &kind.as_str())])
}

/// Deletes the rows of the kind that were reset (`last_at = 0`).
pub fn remove_resetted(c: &Connection, kind: Kind) -> Result<usize> {
    let sql = format!("DELETE FROM history WHERE last_at = 0 AND item_id IN {ITEM_IDS}");
    exec(c, &sql, &[(":kind", &kind.as_str())])
}

/// Sets `last_at` and ADDS `delta_ms` to the duration (a new row starts at `delta_ms`).
pub fn upsert(c: &Connection, item_id: i64, at_ms: i64, delta_ms: i64) -> Result<usize> {
    exec(
        c,
        "INSERT INTO history(item_id, last_at, duration_ms) VALUES (:itemId, :at, :delta) ON CONFLICT(item_id) DO UPDATE SET last_at = :at, duration_ms = duration_ms + :delta",
        &[(":itemId", &item_id), (":at", &at_ms), (":delta", &delta_ms)],
    )
}

/// Sum of the durations of the kind's history, all rows (reset ones included).
pub fn total_duration(c: &Connection, kind: Kind) -> Result<i64> {
    let sql = format!("SELECT coalesce(sum(duration_ms), 0) FROM history WHERE item_id IN {ITEM_IDS}");
    c.prepare_cached(&sql)?.query_row(&[(":kind", &kind.as_str())][..], |r| r.get(0))
}

/// One row per entry (its newest history row) with `last_at > 0`, newest first, ties by higher id. `query` is a
/// substring of the title, case-insensitive for every Unicode letter (`kuro_lower`); `%`, `_` and `\` are literal.
pub fn list(c: &Connection, kind: Kind, query: &str) -> Result<Vec<HistoryViewRow>> {
    let sql = format!(
        "SELECT {VIEW_COLS} FROM historyView WHERE kind = :kind AND lastAt > 0 AND maxLastAtItemId = itemId AND kuro_lower(title) LIKE ('%' || kuro_lower(:query) || '%') ESCAPE '\\' ORDER BY lastAt DESC, id DESC"
    );
    query_all(c, &sql, &[(":kind", &kind.as_str()), (":query", &escape_like(query))], HistoryViewRow::from_row)
}

/// The newest history row of the kind with `last_at > 0`.
pub fn latest(c: &Connection, kind: Kind) -> Result<Option<HistoryViewRow>> {
    let sql = format!("SELECT {VIEW_COLS} FROM historyView WHERE kind = :kind AND lastAt > 0 ORDER BY lastAt DESC, id DESC LIMIT 1");
    query_opt(c, &sql, &[(":kind", &kind.as_str())], HistoryViewRow::from_row)
}
