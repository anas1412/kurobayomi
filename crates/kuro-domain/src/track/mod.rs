//! Tracker bindings (area 09 section 1, `track/`). The model is the one `Track` of `kuro-data`; this module adds the
//! repository port, the interactors with the log-and-default policy, and the per-entry grouping.

pub mod delayed;
pub mod repo;
pub mod service;

pub use kuro_data::track::Track;
