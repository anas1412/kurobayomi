//! SQLite runtime for Kurobayomi: connections, schemas, SQL text and migrations.
//!
//! Module map (plan area 03, section 1): `error` (`DbError`), `open` (connection pragmas, the open
//! gate), `schema` (baseline and `SchemaBundle`), `migrate` (steps after the baseline), `codec`
//! (column codecs), `flags` (bit layouts), `notify` (change sets and hooks), `runtime` (the `Db` handle),
//! `subscribe` (live queries), and `tables` (the statements: one function per statement, `kind` is a parameter,
//! rows hold the stored layout). One file holds both kinds (`schema_unified`).

pub mod error;
pub mod open;
pub mod schema;
pub mod migrate;
pub mod codec;
pub mod flags;
pub mod notify;
pub mod runtime;
pub mod subscribe;
pub mod schema_unified;
pub mod tables;
pub mod episode_sanitizer;
pub mod maintenance;
#[cfg(test)]
mod tests_anime;

/// Compatibility names for callers that used the earlier module layout.
pub use codec as codecs;
pub use runtime as handle;

pub use error::{DbError, DbResult};
pub use notify::TABLES as DB_TABLES;
pub use runtime::{Db, Savepoint, TxExt};

/// The write transaction handed to `Db::write` closures, re-exported so dependants can name it without
/// depending on `rusqlite` themselves.
pub use rusqlite::Transaction;
