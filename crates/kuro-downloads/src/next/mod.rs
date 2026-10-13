//! The unified download system, built beside the old modules (docs/design/unified-downloads.md). Phase D moves these
//! files up one level and deletes the old ones. Phase A: shared types and the pure parts.

pub mod actor;
pub mod delete;
pub mod events;
pub mod gate;
pub mod index;
pub mod layout;
pub mod locks;
pub mod naming;
pub mod notify;
pub mod pending;
pub mod queue;
pub mod queue_model;
pub mod restore;
pub mod runner;
pub mod service;
pub mod state;
pub mod store;
pub mod unit;
pub mod watch;
