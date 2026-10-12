//! The Updates feed: items of library entries fetched after the entry was added. One statement pair for both kinds,
//! ordered in SQL by `datefetch DESC, itemId DESC`, the limit in SQL.

use kuro_platform::kind::Kind;
use rusqlite::{Connection, Result};

use super::{query_all, rows::UpdateRow};

const COLS: &str = "kind, entryId, entryTitle, itemId, itemName, scanlator, done, bookmark, fillermark, progress, totalSeconds, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch";

/// Updates of the kind with an upload date after `after`, newest fetch first, at most `limit` rows.
pub fn recent(c: &Connection, kind: Kind, after: i64, limit: i64) -> Result<Vec<UpdateRow>> {
    let sql = format!("SELECT {COLS} FROM updatesView WHERE kind = :kind AND dateUpload > :after ORDER BY datefetch DESC, itemId DESC LIMIT :limit");
    query_all(c, &sql, &[(":kind", &kind.as_str()), (":after", &after), (":limit", &limit)], UpdateRow::from_row)
}

/// As [`recent`], only the items whose `done` equals `done`.
pub fn by_done(c: &Connection, kind: Kind, done: bool, after: i64, limit: i64) -> Result<Vec<UpdateRow>> {
    let sql = format!("SELECT {COLS} FROM updatesView WHERE kind = :kind AND done = :done AND dateUpload > :after ORDER BY datefetch DESC, itemId DESC LIMIT :limit");
    query_all(c, &sql, &[(":kind", &kind.as_str()), (":done", &done), (":after", &after), (":limit", &limit)], UpdateRow::from_row)
}
