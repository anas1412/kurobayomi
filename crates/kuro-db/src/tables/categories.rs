//! `categories` statements. Ids are per kind: the primary key is `(kind, _id)`.

use kuro_platform::kind::Kind;
use rusqlite::{Connection, Result, Transaction};

use super::{exec, query_all, query_opt, rows::*};

const COLS: &str = "C.kind, C._id AS id, C.name, C.sort AS \"order\", C.flags, C.hidden";

pub fn get(c: &Connection, kind: Kind, id: i64) -> Result<Option<CategoryRow>> {
    let sql = format!("SELECT {COLS} FROM categories C WHERE C.kind = :kind AND C._id = :id LIMIT 1");
    query_opt(c, &sql, &[(":kind", &kind.as_str()), (":id", &id)], CategoryRow::from_row)
}

/// Every category of the kind in display order (`sort`, then id), the system one included.
pub fn all(c: &Connection, kind: Kind) -> Result<Vec<CategoryRow>> {
    let sql = format!("SELECT {COLS} FROM categories C WHERE C.kind = :kind ORDER BY C.sort, C._id");
    query_all(c, &sql, &[(":kind", &kind.as_str())], CategoryRow::from_row)
}

pub fn visible(c: &Connection, kind: Kind) -> Result<Vec<CategoryRow>> {
    let sql = format!("SELECT {COLS} FROM categories C WHERE C.kind = :kind AND C.hidden = 0 ORDER BY C.sort, C._id");
    query_all(c, &sql, &[(":kind", &kind.as_str())], CategoryRow::from_row)
}

pub fn by_entry(c: &Connection, kind: Kind, entry_id: i64) -> Result<Vec<CategoryRow>> {
    let sql = format!(
        "SELECT {COLS} FROM categories C JOIN entry_categories EC ON C._id = EC.category_id AND C.kind = EC.kind WHERE EC.entry_id = :entryId AND C.kind = :kind ORDER BY C.sort, C._id"
    );
    query_all(c, &sql, &[(":kind", &kind.as_str()), (":entryId", &entry_id)], CategoryRow::from_row)
}

pub fn visible_by_entry(c: &Connection, kind: Kind, entry_id: i64) -> Result<Vec<CategoryRow>> {
    let sql = format!(
        "SELECT {COLS} FROM categories C JOIN entry_categories EC ON C._id = EC.category_id AND C.kind = EC.kind WHERE EC.entry_id = :entryId AND C.kind = :kind AND C.hidden = 0 ORDER BY C.sort, C._id"
    );
    query_all(c, &sql, &[(":kind", &kind.as_str()), (":entryId", &entry_id)], CategoryRow::from_row)
}

/// Creates a category of the kind with id `max(_id) + 1` of that kind and returns the id.
pub fn insert(c: &Connection, kind: Kind, name: &str, order: i64, flags: i64) -> Result<i64> {
    c.prepare_cached(
        "INSERT INTO categories(kind, _id, name, sort, flags) SELECT :kind, coalesce(max(_id), 0) + 1, :name, :order, :flags FROM categories WHERE kind = :kind RETURNING _id",
    )?
    .query_row(&[(":kind", &kind.as_str() as &dyn rusqlite::ToSql), (":name", &name), (":order", &order), (":flags", &flags)][..], |r| r.get(0))
}

/// Coalesce update. Returns the rows changed.
pub fn update(c: &Connection, kind: Kind, p: &CategoryPatch) -> Result<usize> {
    exec(
        c,
        "UPDATE categories SET name = coalesce(:name, name), sort = coalesce(:order, sort), flags = coalesce(:flags, flags), hidden = coalesce(:hidden, hidden) WHERE _id = :id AND kind = :kind",
        &[(":name", &p.name), (":order", &p.order), (":flags", &p.flags), (":hidden", &p.hidden), (":id", &p.id), (":kind", &kind.as_str())],
    )
}

/// Deletes a category; its links cascade. Id 0 fails with `System category can't be deleted`.
pub fn delete(c: &Connection, kind: Kind, id: i64) -> Result<usize> {
    exec(c, "DELETE FROM categories WHERE _id = :id AND kind = :kind", &[(":id", &id), (":kind", &kind.as_str())])
}

/// Sets `flags` on every category of the kind, the system one included. `None` changes nothing.
pub fn update_all_flags(c: &Connection, kind: Kind, flags: Option<i64>) -> Result<usize> {
    exec(c, "UPDATE categories SET flags = coalesce(:flags, flags) WHERE kind = :kind", &[(":flags", &flags), (":kind", &kind.as_str())])
}

/// Renumbers every category of the kind, the system row included, to its index in display order from 0 (what a delete
/// needs). One UPDATE per row: a correlated subquery would read rows the same statement already renumbered.
pub fn renumber_all(tx: &Transaction<'_>, kind: Kind) -> Result<()> {
    let ids: Vec<i64> = query_all(tx, "SELECT _id FROM categories WHERE kind = :kind ORDER BY sort, _id", &[(":kind", &kind.as_str())], |r| r.get(0))?;
    for (order, id) in (0_i64..).zip(ids.iter()) {
        exec(tx, "UPDATE categories SET sort = :order WHERE _id = :id AND kind = :kind", &[(":order", &order), (":id", id), (":kind", &kind.as_str())])?;
    }
    Ok(())
}
