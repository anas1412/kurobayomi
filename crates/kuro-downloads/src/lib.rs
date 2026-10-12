//! Manga download queue, scheduler, page download, CBZ packing and shared delete/index (areas 23, 24, 25).
//!
//! Dependency direction: the GUI crate uses [`DownloadManager`]; this crate uses `kuro-core` helpers and the
//! `kuro-source` error type. No GTK and no OS-specific code: free disk space, network state, process hold and
//! notification delivery are injected through [`ManagerParams`] and the [`ProcessHold`] and
//! [`notify::NotificationSink`] traits.
//!
//! Dropped (settings diet, docs/design/settings-diet.md): the data-saver image proxy (T-23a-017, D-DL-09), resmush.it,
//! Bandwidth Hero and wsrv.nl alike. Page requests use the source's image URL as it is.
//!
//! Dropped Android machinery (T-23a-038): WorkManager and its foreground service types, FlexibleAdapter, the Voyager
//! ScreenModel, Compose AndroidView, MediaScannerConnection, and the SAF and UniFile document trees. Nothing visible is
//! lost: the replacements are the tokio actor (T-23a-023) and the GTK page (T-23a-033). This decision is also the PR
//! description of that task.
//!
//! Dropped Android download state (T-23a-039): the SharedPreferences `active_downloads`, `chapters_to_delete` and
//! `dl_index_cache_v3`. The Android queue and pending deletes were short-lived, so the user loses nothing durable. The
//! downloaded folders themselves are reused: a copied Aniyomi download folder is indexed on the first scan (TST-37, see
//! `index`).
//!
//! Dropped anime range model (T-24a-003): `AnimeDownloadPart` had no references, so no range-part type exists here, and
//! multi-part range downloads are not offered for anime.

pub mod anime;
pub mod comicinfo;
pub mod delete;
pub mod download_prefs;
pub mod downloader;
pub mod engine;
pub mod error;
pub mod events;
pub mod images;
pub mod index;
pub mod layout;
pub mod local;
pub mod manager;
pub mod model;
pub mod notify;
pub mod pending;
pub mod pipeline;
pub mod queue;
pub mod queue_model;
pub mod redact;
pub mod retry;
pub mod sanitize;
pub mod scheduler;
pub mod settings;
pub mod source;
pub mod store;
pub mod throttle;
pub mod watch;

#[cfg(test)]
mod testdata_tests;
#[cfg(test)]
mod testing;

pub use downloader::{Connectivity, DownloadSettings, ProcessHold};
pub use engine::{AnimeEngine, MangaEngine, QueueEngine};
pub use error::DownloadError;
pub use events::DownloadEvent;
pub use manager::{DeleteRules, DownloadManager, EnqueueOutcome, LocalPage, ManagerParams};
pub use model::DownloadState;

/// A private database holding only the three download tables, for the stores' tests. Foreign keys are switched off
/// (rusqlite turns them on by default), so rows may name entries and items that are not there.
pub(crate) fn memory_downloads_db() -> rusqlite::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    conn.execute_batch(kuro_db::schema_unified::DOWNLOAD_DDL)?;
    Ok(conn)
}
