//! Downloads for both kinds (docs/design/unified-downloads.md). One actor (`actor`, handle in `queue`) runs a lane per
//! kind and starts units through a `UnitRunner` (`manga::runner::ChapterPipeline` for pages, `anime::runner::EpisodeEngine`
//! for video). `store`, `pending` and `restore` keep the queue and the deferred deletes in the app database, `layout`,
//! `naming` and `index` own the folders, and `service::Downloads` is the one handle every caller uses.
//!
//! No GTK and no OS-specific code: free disk space, network state and notification delivery are injected.
//!
//! Dropped (settings diet, docs/design/settings-diet.md): the data-saver image proxy, resmush.it, Bandwidth Hero and
//! wsrv.nl. Page requests use the source's image URL as it is. Dropped Android machinery: WorkManager and its
//! foreground service, the SAF and UniFile document trees, the SharedPreferences download state.

pub mod actor;
pub mod anime;
pub mod delete;
pub mod error;
pub mod events;
pub mod gate;
pub mod index;
pub mod layout;
pub mod local;
pub mod locks;
pub mod manga;
pub mod model;
pub mod naming;
pub mod notice;
pub mod notify;
pub mod pending;
pub mod queue;
pub mod queue_model;
pub mod redact;
pub mod restore;
pub mod retry;
pub mod runner;
pub mod sanitize;
pub mod scheduler;
pub mod service;
pub mod source;
pub mod state;
pub mod store;
pub mod throttle;
pub mod unit;
pub mod watch;

#[cfg(test)]
mod testing;

pub use error::DownloadError;
pub use model::DownloadState;

/// A private database holding only the three download tables, for the stores' tests. Foreign keys are switched off
/// (rusqlite turns them on by default), so rows may name entries and items that are not there.
pub(crate) fn memory_downloads_db() -> rusqlite::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    conn.execute_batch(kuro_db::schema_unified::DOWNLOAD_DDL)?;
    Ok(conn)
}
