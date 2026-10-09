//! Kurobayomi tracking core: tracker trait, OAuth/PKCE, remote tracker clients and retry queue.
//!
//! Layout follows `plans/27-tracking-core/p1-rust-design.md` section 1. No GTK, `gio` or `keyring` here.

pub mod base;
pub mod bind;
pub mod connectivity;
pub mod ctx;
pub mod detach;
pub mod dialog;
pub mod entry_kind;
pub mod error;
pub mod events;
pub mod home;
pub mod item;
pub mod kitsu;
pub mod login_rows;
pub mod mal;
pub mod manager;
pub mod messages;
pub mod model;
pub mod nav;
pub mod oauth;
pub mod picker;
pub mod policy;
pub mod prefs;
pub mod progress;
pub mod queue;
pub mod record;
pub mod remote;
pub mod remove;
pub mod repo;
pub mod search_screen;
pub mod sheet;
pub mod sync;
pub mod toast;
pub mod tracker;
pub mod write;

#[cfg(test)]
mod fakes;
