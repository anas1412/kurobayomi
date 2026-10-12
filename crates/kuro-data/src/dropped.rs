//! Android plumbing and sync features this crate does not port (T-05a-027, T-05a-029, T-06a-006, T-06a-007).
//!
//! * No paging source. Aniyomi's Paging3 wrapper over `limit`/`offset` queries has no caller outside its
//!   handlers. Lists load in full and the UI virtualises them (`gtk::ListView`). User-visible loss: none, since
//!   library sizes fit in memory.
//! * No coroutine transaction context and no thread-local transaction element. The replacement is `Db::write`,
//!   a single writer that runs a synchronous closure inside one transaction. The debug-only database inspector
//!   switch is dropped because it exists only in Android Studio. `rusqlite` with bundled SQLite is used in every
//!   build.
//! * No sync-state column write path. The `is_syncing` column stays in the schema for compatibility, and every
//!   update statement still writes `is_syncing = 0`. The reset statements get no Rust method. The SyncYomi-style
//!   sync that would set the flag is not ported, so the user loses cross-device sync. Backups remain the transfer
//!   mechanism.
