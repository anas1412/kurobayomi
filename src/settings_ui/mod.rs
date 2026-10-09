//! Settings screens built as Rust preference trees (tasks T-47a-003..088 and T-48a-002..025, desktop subset).
//!
//! Each builder returns `pref_model::model` items bound to typed `kuro-prefs` handles. The rules (what is shown,
//! what is enabled, which values a list offers, which key a Custom gesture sends) are plain functions with tests.
//! Rendering goes through `pref_model::page::body`. Disabled rows of the reader are left out of the tree (Android
//! `StatusWrapper` hides them); the player keeps disabled rows and greys them (task T-48a-012).
#![allow(dead_code)]

pub mod appearance;
pub mod category;
pub mod library;
pub mod menu;
pub mod player;
pub mod reader;
pub mod rows;
pub mod search;
pub mod storage;
