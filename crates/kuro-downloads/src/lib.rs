//! Manga download queue, scheduler, page download, CBZ packing and shared delete/index (areas 23, 24, 25).
//!
//! Dependency direction: the GUI crate uses [`DownloadManager`]; this crate uses `kuro-core` helpers and the
//! `kuro-source` error type. No GTK and no OS-specific code: free disk space, network state, process hold and
//! notification delivery are injected through [`ManagerParams`] and the [`ProcessHold`] and
//! [`notify::NotificationSink`] traits.
//!
//! Dropped (user loss stated): the resmush.it data-saver service (D-DL-09, T-23a-017). Its Kotlin code is plain HTTP and
//! reads JSON by string search. Bandwidth Hero and wsrv.nl stay, off unless the user chose one.

pub mod anime;
pub mod archive;
pub mod comicinfo;
pub mod data_saver;
pub mod delete;
pub mod downloader;
pub mod engine;
pub mod error;
pub mod events;
pub mod images;
pub mod index;
pub mod layout;
pub mod manager;
pub mod model;
pub mod notify;
pub mod page;
pub mod pending;
pub mod pipeline;
pub mod queue;
pub mod queue_model;
pub mod retry;
pub mod sanitize;
pub mod scheduler;
pub mod settings;
pub mod source;
pub mod store;
pub mod throttle;
pub mod view_model;

#[cfg(test)]
mod testing;

pub use delete::{DeleteChapterDownload, DeleteEpisodeDownload, DeleteError, DeleteSettings};
pub use downloader::{Connectivity, DownloadSettings, ProcessHold};
pub use engine::{AnimeEngine, MangaEngine, QueueEngine};
pub use error::DownloadError;
pub use events::DownloadEvent;
pub use manager::{DeleteRules, DownloadManager, EnqueueOutcome, LocalPage, ManagerParams, restore_queue};
pub use model::DownloadState;
