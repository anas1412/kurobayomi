//! The unified download system, built beside the old modules (docs/design/unified-downloads.md). Phase D moves these
//! files up one level and deletes the old ones. Phase A: shared types and the pure parts.

pub mod delete;
pub mod events;
pub mod gate;
pub mod index;
pub mod layout;
pub mod locks;
pub mod naming;
pub mod queue_model;
pub mod runner;
pub mod state;
pub mod unit;
pub mod watch;
