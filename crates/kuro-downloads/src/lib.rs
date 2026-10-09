//! Manga download queue, scheduler, page download, CBZ packing and shared delete/index (areas 23, 24, 25).
//!
//! Dependency direction: the GUI crate uses [`DownloadManager`]; this crate uses `kuro-core` helpers and the
//! `kuro-source` error type. No GTK and no OS-specific code: free disk space, network state, process hold and
//! notification delivery are injected through [`ManagerParams`] and the [`ProcessHold`] and
//! [`notify::NotificationSink`] traits.

pub mod archive;
pub mod comicinfo;
pub mod downloader;
pub mod error;
pub mod events;
pub mod images;
pub mod index;
pub mod layout;
pub mod manager;
pub mod model;
pub mod notify;
pub mod pending;
pub mod pipeline;
pub mod queue;
pub mod sanitize;
pub mod scheduler;
pub mod source;
pub mod store;
pub mod throttle;

#[cfg(test)]
mod testing;

pub use downloader::{Connectivity, DownloadSettings, ProcessHold};
pub use error::DownloadError;
pub use events::DownloadEvent;
pub use manager::{DeleteRules, DownloadManager, EnqueueOutcome, LocalPage, ManagerParams};
pub use model::DownloadState;
