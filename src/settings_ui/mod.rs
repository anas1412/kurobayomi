//! Settings screens built as Rust preference trees (tasks T-47a-003..088 and T-48a-002..025, desktop subset).
//!
//! Each builder returns `pref_model::model` items bound to typed `kuro-prefs` handles. The rules (what is shown,
//! what is enabled, which values a list offers, which key a Custom gesture sends) are plain functions with tests.
//! Rendering goes through `pref_model::page::body`. Hide or grey (tasks T-47a-002 and T-48a-012): the reader hides a
//! row its rules disable by leaving it out of the tree (Android `StatusWrapper`); the player keeps its disabled rows
//! and greys them, because the shared renderer greys every row whose `enabled` is false. The 300 ms reveal and
//! collapse is not drawn: the page is rebuilt (`view.rs`) when a rule input changes.
//! `route.rs` is what `screens::body` calls, so the screens here are the ones the app shows.
//!
//! Android items with no desktop row (the stored value is ignored, no migration is written):
//! - T-47a-085 the extension installer choice: no installer row; `extension_installer` is ignored. Installs follow area 20.
//! - T-47b-035 the MIUI banner, T-47b-042 the ART profile row, T-47b-051 the OneUI and MIUI rows: no such device on Linux.
//! - T-47b-054 the persistable URI permission: paths are plain paths or `gio::File` URIs.
//! - T-47b-056 the list item animations: lists update at once.
//! - T-47ba-017 the More storage route: the More list opens the settings screens only.
//! - T-47ba-075 haptic slider ticks, `ActivityCompat.recreate` (the theme swaps live), Compose previews, the animated More
//!   icon, and the Android storage-manager gate (the platform data directory replaces it). Dynamic colour: see T-48a-058.
//! - T-48a-007 the tablet UI mode: its four values are not offered; the shell switches panes on a 720 px width.
//! - T-48a-019 the orientation row: not offered; the Display group says "Screen rotation is not used on desktop."
//! - T-48a-020 the system status bar row: not offered; the header bar stays in fullscreen unless the user hides it (area 36).
//! - T-48a-053 the external player package list: built from the installed `video/*` handlers (`player::external`).
//! - T-48a-054 the PiP and fullscreen SDK gates: the mini player group follows the display probe; Fullscreen is always offered.
//! - T-48a-058 Dynamic (Monet): shown as "System colours"; it follows the desktop accent or the GTK theme.

pub mod about;
pub mod advanced;
pub mod appearance;
pub mod auth;
pub mod browse;
pub mod cache;
pub mod category;
pub mod category_sets;
pub mod changelog;
pub mod chooser;
pub mod clipboard;
pub mod cookie_import;
pub mod data_saver;
pub mod decode;
pub mod downloads;
pub mod jobs;
pub mod lang;
pub mod library;
pub mod library_csv;
pub mod library_export;
pub mod library_update;
pub mod licenses;
pub mod menu;
pub mod merge;
pub mod notifications;
pub mod player;
pub mod reader;
pub mod route;
pub mod rows;
pub mod search;
pub mod security;
pub mod storage;
pub mod tracker_host;
pub mod tracking;
pub mod usage;
pub mod view;
pub mod worker_info;

pub use route::{body, handles};
