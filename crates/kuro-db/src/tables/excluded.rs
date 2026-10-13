//! `excluded_scanlators` statements: the groups an entry hides from its item list.

use std::collections::BTreeSet;

use rusqlite::{Connection, Result, Transaction};

use super::{exec, query_all};
use crate::codec::strings_json;

pub fn insert(c: &Connection, entry_id: i64, scanlator: &str) -> Result<usize> {
    exec(c, "INSERT INTO excluded_scanlators(entry_id, scanlator) VALUES (:id, :scanlator)", &[(":id", &entry_id), (":scanlator", &scanlator)])
}

/// Deletes the listed names of `entry_id`. Empty `scanlators` runs no statement.
pub fn remove(c: &Connection, entry_id: i64, scanlators: &[String]) -> Result<usize> {
    if scanlators.is_empty() {
        return Ok(0);
    }
    let json = strings_json(scanlators);
    exec(
        c,
        "DELETE FROM excluded_scanlators WHERE entry_id = :id AND scanlator IN (SELECT value FROM json_each(:scanlators))",
        &[(":id", &entry_id), (":scanlators", &json)],
    )
}

pub fn by_entry_id(c: &Connection, entry_id: i64) -> Result<Vec<String>> {
    query_all(c, "SELECT scanlator FROM excluded_scanlators WHERE entry_id = :id", &[(":id", &entry_id)], |r| r.get(0))
}

/// Makes the excluded set of `entry_id` equal to `wanted`: inserts the names not stored yet and removes the stored
/// names not wanted. The caller owns `tx`, so the read and the writes commit together.
pub fn set_excluded(tx: &Transaction<'_>, entry_id: i64, wanted: &BTreeSet<String>) -> Result<()> {
    let current: BTreeSet<String> = by_entry_id(tx, entry_id)?.into_iter().collect();
    let removed: Vec<String> = current.difference(wanted).cloned().collect();
    for name in wanted.difference(&current) {
        insert(tx, entry_id, name)?;
    }
    remove(tx, entry_id, &removed)?;
    Ok(())
}
