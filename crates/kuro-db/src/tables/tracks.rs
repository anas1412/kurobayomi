//! `tracks` statements for both kinds. `insert` is an upsert keyed by `(entry_id, tracker_id)` that keeps the row id.

use kuro_platform::kind::Kind;
use rusqlite::{Connection, Result};

use super::{exec, query_all, query_opt, rows::*};
use crate::error::{DbError, DbResult};

const COLS: &str = "t._id, t.entry_id, e.kind, t.tracker_id, t.remote_id, t.library_id, t.title, t.last_item, t.total_items, t.status, t.score, t.remote_url, t.start_date, t.finish_date, t.private";
const UPSERT: &str = "INSERT INTO tracks(entry_id,tracker_id,remote_id,library_id,title,last_item,total_items,status,score,remote_url,start_date,finish_date,private) VALUES (:entryId,:trackerId,:remoteId,:libraryId,:title,:lastItem,:totalItems,:status,:score,:remoteUrl,:startDate,:finishDate,:private) ON CONFLICT(entry_id, tracker_id) DO UPDATE SET remote_id = excluded.remote_id, library_id = excluded.library_id, title = excluded.title, last_item = excluded.last_item, total_items = excluded.total_items, status = excluded.status, score = excluded.score, remote_url = excluded.remote_url, start_date = excluded.start_date, finish_date = excluded.finish_date, private = excluded.private RETURNING _id";
const UPDATE: &str = "UPDATE tracks SET entry_id = coalesce(:entryId, entry_id), tracker_id = coalesce(:trackerId, tracker_id), remote_id = coalesce(:remoteId, remote_id), library_id = coalesce(:libraryId, library_id), title = coalesce(:title, title), last_item = coalesce(:lastItem, last_item), total_items = coalesce(:totalItems, total_items), status = coalesce(:status, status), score = coalesce(:score, score), remote_url = coalesce(:remoteUrl, remote_url), start_date = coalesce(:startDate, start_date), finish_date = coalesce(:finishDate, finish_date), private = coalesce(:private, private) WHERE _id = :id";

pub fn by_id(c: &Connection, id: i64) -> DbResult<TrackRow> {
    let sql = format!("SELECT {COLS} FROM tracks t JOIN entries e ON e._id = t.entry_id WHERE t._id = :id");
    query_opt(c, &sql, &[(":id", &id)], TrackRow::from_row)?.ok_or(DbError::NotFound)
}

pub fn all(c: &Connection, kind: Kind) -> Result<Vec<TrackRow>> {
    let sql = format!("SELECT {COLS} FROM tracks t JOIN entries e ON e._id = t.entry_id WHERE e.kind = :kind ORDER BY t._id");
    query_all(c, &sql, &[(":kind", &kind.as_str())], TrackRow::from_row)
}

pub fn by_entry(c: &Connection, entry_id: i64) -> Result<Vec<TrackRow>> {
    let sql = format!("SELECT {COLS} FROM tracks t JOIN entries e ON e._id = t.entry_id WHERE t.entry_id = :entryId ORDER BY t._id");
    query_all(c, &sql, &[(":entryId", &entry_id)], TrackRow::from_row)
}

/// Upserts by `(entry_id, tracker_id)`: a second track of one tracker updates the row in place, so its id is kept.
/// `id` and `kind` of `t` are not bound. Returns the row id.
pub fn insert(c: &Connection, t: &TrackRow) -> Result<i64> {
    c.prepare_cached(UPSERT)?.query_row(
        &[
            (":entryId", &t.entry_id as &dyn rusqlite::ToSql),
            (":trackerId", &t.tracker_id),
            (":remoteId", &t.remote_id),
            (":libraryId", &t.library_id),
            (":title", &t.title),
            (":lastItem", &t.last_item),
            (":totalItems", &t.total_items),
            (":status", &t.status),
            (":score", &t.score),
            (":remoteUrl", &t.remote_url),
            (":startDate", &t.start_date),
            (":finishDate", &t.finish_date),
            (":private", &t.private),
        ][..],
        |r| r.get(0),
    )
}

/// Coalesce update of one row. Returns the rows changed.
pub fn update_coalesced(c: &Connection, p: &TrackPatch) -> Result<usize> {
    exec(
        c,
        UPDATE,
        &[
            (":entryId", &p.entry_id),
            (":trackerId", &p.tracker_id),
            (":remoteId", &p.remote_id),
            (":libraryId", &p.library_id),
            (":title", &p.title),
            (":lastItem", &p.last_item),
            (":totalItems", &p.total_items),
            (":status", &p.status),
            (":score", &p.score),
            (":remoteUrl", &p.remote_url),
            (":startDate", &p.start_date),
            (":finishDate", &p.finish_date),
            (":private", &p.private),
            (":id", &p.id),
        ],
    )
}

pub fn delete(c: &Connection, entry_id: i64, tracker_id: i64) -> Result<usize> {
    exec(c, "DELETE FROM tracks WHERE entry_id = :entryId AND tracker_id = :trackerId", &[(":entryId", &entry_id), (":trackerId", &tracker_id)])
}
