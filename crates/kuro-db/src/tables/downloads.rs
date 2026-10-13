//! `download_queue`, `download_pending_delete` and `download_pending_corrupt` statements for both kinds
//! (docs/design/unified-downloads.md section 2.7). Rows hold the stored layout. The caller owns the transaction
//! (`Db::write` gives one), so a multi-statement function here rolls back as a whole on an error.

use kuro_platform::kind::Kind;
use rusqlite::{params, Connection, Result, Row};

use super::{exec, kind_col, query_all};

/// One `download_queue` row. `state` is the `status` column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueRow {
    pub unit_id: i64,
    pub entry_id: i64,
    pub source: i64,
    pub position: i64,
    pub state: u8,
    pub enqueued_at: i64,
}

impl QueueRow {
    fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            unit_id: r.get("unit_id")?,
            entry_id: r.get("entry_id")?,
            source: r.get("source")?,
            position: r.get("position")?,
            state: r.get::<_, i64>("status")? as u8,
            enqueued_at: r.get("enqueued_at")?,
        })
    }
}

/// One `download_pending_delete` row. `seq` is the insertion order inside an entry; `pending_add` assigns it
/// (the value passed in is ignored), `pending_take` and `pending_put_back` carry it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRow {
    pub unit_id: i64,
    pub kind: Kind,
    pub entry_id: i64,
    pub entry_url: String,
    pub entry_title: String,
    pub source: i64,
    pub unit_url: String,
    pub unit_name: String,
    pub scanlator: Option<String>,
    pub done: bool,
    pub bookmark: bool,
    pub seq: i64,
    pub enqueued_at: i64,
}

impl PendingRow {
    fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            unit_id: r.get("unit_id")?,
            kind: kind_col(r, "kind")?,
            entry_id: r.get("entry_id")?,
            entry_url: r.get("entry_url")?,
            entry_title: r.get("entry_title")?,
            source: r.get("source")?,
            unit_url: r.get("unit_url")?,
            unit_name: r.get("unit_name")?,
            scanlator: r.get("scanlator")?,
            done: r.get("done")?,
            bookmark: r.get("bookmark")?,
            seq: r.get("seq")?,
            enqueued_at: r.get("enqueued_at")?,
        })
    }
}

const PENDING_COLS: &str = "unit_id, kind, entry_id, entry_url, entry_title, source, unit_url, unit_name, scanlator, done, bookmark, seq, enqueued_at";

/// The rows of `kind`, ordered by `position`.
pub fn queue_load(c: &Connection, kind: Kind) -> Result<Vec<QueueRow>> {
    query_all(
        c,
        "SELECT unit_id, entry_id, source, position, status, enqueued_at FROM download_queue WHERE kind = :kind ORDER BY position",
        &[(":kind", &kind.as_str())],
        QueueRow::from_row,
    )
}

/// Replaces the rows of `kind` with `rows`, renumbering positions `0..n` in slice order. The other kind is
/// untouched. Each row is inserted through a join on `items` and `entries`, so `entry_id` and `source` come
/// from the item's entry and a unit whose item is gone (or belongs to the other kind) is skipped instead of failing
/// the whole save. Returns how many rows were stored.
pub fn queue_replace(c: &Connection, kind: Kind, rows: &[QueueRow]) -> Result<usize> {
    exec(c, "DELETE FROM download_queue WHERE kind = :kind", &[(":kind", &kind.as_str())])?;
    let mut stored = 0;
    for row in rows {
        let position = stored as i64;
        let status = row.state as i64;
        stored += exec(
            c,
            "INSERT INTO download_queue(unit_id, kind, entry_id, source, position, status, enqueued_at) \
             SELECT i._id, e.kind, i.entry_id, e.source, :position, :status, :enqueuedAt \
             FROM items i JOIN entries e ON e._id = i.entry_id WHERE i._id = :unitId AND e.kind = :kind",
            &[
                (":position", &position),
                (":status", &status),
                (":enqueuedAt", &row.enqueued_at),
                (":unitId", &row.unit_id),
                (":kind", &kind.as_str()),
            ],
        )?;
    }
    Ok(stored)
}

/// Adds rows to the pending list. A unit already pending (same `unit_id`) is ignored, also inside the batch. `seq`
/// continues after the entry's highest one. Returns how many rows were new.
pub fn pending_add(c: &Connection, rows: &[PendingRow]) -> Result<usize> {
    let mut added = 0;
    for row in rows {
        let seq: i64 = c.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM download_pending_delete WHERE entry_id = ?1",
            params![row.entry_id],
            |r| r.get(0),
        )?;
        added += insert_pending(c, row, seq)?;
    }
    Ok(added)
}

/// Puts rows taken by `pending_take` back unchanged, `seq` included. A unit that is pending again is kept as it is.
/// Returns how many rows were restored.
pub fn pending_put_back(c: &Connection, rows: &[PendingRow]) -> Result<usize> {
    let mut back = 0;
    for row in rows {
        back += insert_pending(c, row, row.seq)?;
    }
    Ok(back)
}

fn insert_pending(c: &Connection, row: &PendingRow, seq: i64) -> Result<usize> {
    exec(
        c,
        "INSERT OR IGNORE INTO download_pending_delete(unit_id, kind, entry_id, entry_url, entry_title, source, unit_url, unit_name, scanlator, done, bookmark, seq, enqueued_at) \
         VALUES (:unitId, :kind, :entryId, :entryUrl, :entryTitle, :source, :unitUrl, :unitName, :scanlator, :done, :bookmark, :seq, :enqueuedAt)",
        &[
            (":unitId", &row.unit_id),
            (":kind", &row.kind.as_str()),
            (":entryId", &row.entry_id),
            (":entryUrl", &row.entry_url),
            (":entryTitle", &row.entry_title),
            (":source", &row.source),
            (":unitUrl", &row.unit_url),
            (":unitName", &row.unit_name),
            (":scanlator", &row.scanlator),
            (":done", &row.done),
            (":bookmark", &row.bookmark),
            (":seq", &seq),
            (":enqueuedAt", &row.enqueued_at),
        ],
    )
}

/// Takes the rows whose `(kind, source)` `known` accepts and deletes exactly those; the rest stay. Ordered by kind,
/// entry, `seq`. A row that cannot be decoded is recorded with `pending_corrupt` (stamped `now`) and removed, so the
/// list never keeps it and never loses it silently.
pub fn pending_take(c: &Connection, now: i64, known: &dyn Fn(Kind, i64) -> bool) -> Result<Vec<PendingRow>> {
    let sql = format!("SELECT rowid AS rid, {PENDING_COLS} FROM download_pending_delete ORDER BY kind, entry_id, seq");
    let rows: Vec<(i64, Result<PendingRow>)> = {
        let mut stmt = c.prepare(&sql)?;
        let mapped = stmt.query_map([], |r| Ok((r.get::<_, i64>("rid")?, PendingRow::from_row(r))))?;
        mapped.collect::<Result<_>>()?
    };
    let mut taken = Vec::new();
    for (rowid, decoded) in rows {
        match decoded {
            Err(err) => {
                pending_corrupt(c, rowid, &err.to_string(), now)?;
                exec(c, "DELETE FROM download_pending_delete WHERE rowid = :rid", &[(":rid", &rowid)])?;
            }
            Ok(row) if known(row.kind, row.source) => {
                exec(c, "DELETE FROM download_pending_delete WHERE rowid = :rid", &[(":rid", &rowid)])?;
                taken.push(row);
            }
            Ok(_) => {}
        }
    }
    Ok(taken)
}

/// Records one pending row that could not be read, by its row id in `download_pending_delete`.
pub fn pending_corrupt(c: &Connection, source_rowid: i64, reason: &str, now: i64) -> Result<()> {
    exec(
        c,
        "INSERT INTO download_pending_corrupt(source_rowid, reason, recorded_at) VALUES (:rowid, :reason, :now)",
        &[(":rowid", &source_rowid), (":reason", &reason), (":now", &now)],
    )?;
    Ok(())
}

/// Pending rows stored for `kind`.
pub fn pending_len(c: &Connection, kind: Kind) -> Result<usize> {
    let n: i64 = c.query_row(
        "SELECT COUNT(*) FROM download_pending_delete WHERE kind = ?1",
        params![kind.as_str()],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}
