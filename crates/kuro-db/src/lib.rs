//! SQLite runtime for Kurobayomi: connections, schemas, SQL text and migrations.
//!
//! Module map (plan area 03, section 1): `error` (`DbError`), `open` (connection pragmas, the open
//! gate), `schema` (baseline and `SchemaBundle`), `migrate` (steps after the baseline), `codec`
//! (column codecs), `flags` (bit layouts), `models` (row and patch structs), `notify` (change sets
//! and hooks), `runtime` (the `Db` handle), `subscribe` (live queries), and the statement modules
//! (`statements_manga` is exported as `sql`). One file holds both kinds (`schema_unified`); the two statement
//! modules pin the kind in their SQL.

pub mod error;
pub mod open;
pub mod schema;
pub mod migrate;
pub mod codec;
pub mod flags;
pub mod models;
pub mod notify;
pub mod runtime;
pub mod subscribe;
pub mod anime;
pub mod schema_unified;
pub mod schema_anime;
pub mod statements_manga;
pub mod statements_anime;
pub mod episode_sanitizer;
pub mod maintenance;
#[cfg(test)]
mod testdb;
#[cfg(test)]
mod tests_anime;

/// Compatibility names for callers that used the earlier module layout.
pub use codec as codecs;
pub use runtime as handle;

/// The manga SQL modules under the plan's name: `kuro_db::sql::mangas`, `sql::chapters` and so on.
pub mod sql {
    pub use crate::statements_manga::{
        categories, chapters, excluded_scanlators, history, manga_sync, mangas, mangas_categories,
        sources, views,
    };
}

pub use error::{DbError, DbResult};
pub use notify::TABLES as DB_TABLES;
pub use runtime::{Db, Savepoint, TxExt};

/// The write transaction handed to `Db::write` closures, re-exported so dependants can name it without
/// depending on `rusqlite` themselves.
pub use rusqlite::Transaction;
