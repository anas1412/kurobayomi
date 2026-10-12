//! `stub_sources` statements: the names of sources whose extension is gone. Id 0 (local) exists in both kinds.

use kuro_platform::kind::Kind;
use rusqlite::{Connection, Result};

use super::{exec, query_all, query_opt, rows::StubSourceRow};

fn from_row(r: &rusqlite::Row<'_>) -> Result<StubSourceRow> {
    Ok(StubSourceRow { id: r.get("id")?, lang: r.get("lang")?, name: r.get("name")? })
}

pub fn find_all(c: &Connection, kind: Kind) -> Result<Vec<StubSourceRow>> {
    query_all(c, "SELECT id, lang, name FROM stub_sources WHERE kind = :kind ORDER BY rowid", &[(":kind", &kind.as_str())], from_row)
}

pub fn find_one(c: &Connection, kind: Kind, id: i64) -> Result<Option<StubSourceRow>> {
    query_opt(c, "SELECT id, lang, name FROM stub_sources WHERE kind = :kind AND id = :id", &[(":kind", &kind.as_str()), (":id", &id)], from_row)
}

/// Inserts the stub, or replaces `lang` and `name` of the row with the same id.
pub fn upsert(c: &Connection, kind: Kind, id: i64, lang: &str, name: &str) -> Result<usize> {
    exec(
        c,
        "INSERT INTO stub_sources(kind, id, lang, name) VALUES (:kind, :id, :lang, :name) ON CONFLICT(kind, id) DO UPDATE SET lang = :lang, name = :name",
        &[(":kind", &kind.as_str()), (":id", &id), (":lang", &lang), (":name", &name)],
    )
}
