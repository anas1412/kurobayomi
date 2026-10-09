//! Entry behaviour: flag accessors, defaults, covers, updates, the fetch interval and the read-side facades.

pub mod anime;
pub mod combine;
#[cfg(test)]
mod coverage;
pub mod cover;
pub mod defaults;
pub mod fetch_interval;
pub mod flag_setters;
pub mod manga;
pub mod network_to_local;
pub mod queries;
pub mod season_display;
pub mod source_convert;
pub mod seasons;
