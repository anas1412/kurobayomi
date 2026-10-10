//! Settings screens built as Rust preference trees (tasks T-47a-003..088 and T-48a-002..025, desktop subset).
//!
//! Each builder returns `pref_model::model` items bound to typed `kuro-prefs` handles. The rules (what is shown,
//! what is enabled, which values a list offers, which key a Custom gesture sends) are plain functions with tests.
//! Rendering goes through `pref_model::page::body`. Hide or grey (tasks T-47a-002 and T-48a-012): the reader hides a
//! row its rules disable by leaving it out of the tree (Android `StatusWrapper`); the player keeps its disabled rows
//! and greys them, because the shared renderer greys every row whose `enabled` is false. The 300 ms reveal and
//! collapse is not drawn: the page is rebuilt (`view.rs`) when a rule input changes.
//! `route.rs` is what `screens::body` calls, so the screens here are the ones the app shows.

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
