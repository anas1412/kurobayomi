//! Library update passes (area 26): the entry model and queue, the fetch window and interval, the store reads, the
//! per-entry flow, the pass runner with its lock, start and stop, the automatic schedule and the notification text.
//!
//! Pure logic only; no GTK. Notifications, downloads and the item sync are ports (`Host`, `ItemSync`) that the GUI
//! and the download lanes implement. Tests run with `cargo test -p kurobayomi library_update`.
#![allow(dead_code)] // the pass is wired by the screens lane; some helpers are tested but not called yet

pub mod constraints;
pub mod counters;
pub mod download;
pub mod entry;
pub mod error;
pub mod fetch_interval;
pub mod notice;
pub mod pass;
pub mod politeness;
pub mod progress;
pub mod queue;
pub mod reading;
pub mod schedule;
pub mod source;
pub mod store;
pub mod sync;
pub mod text;

#[cfg(test)]
pub(crate) mod fake;
