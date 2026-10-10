//! Anime extensions (area 19): the `.kuroext` package container and verification, the store protocol, the trust rule,
//! the install pipeline, and the state the UI reads. Everything here is headless: no GTK, no OS-specific code.

pub mod abi;
pub mod icons;
pub mod install;
pub mod loader;
pub mod manager;
pub mod model;
pub mod languages;
pub mod logs;
pub mod messages;
pub mod runtime;
pub mod service;
pub mod browse;
pub mod dialog;
pub mod backup;
pub mod package;
pub mod sources;
pub mod store;
pub mod stores_page;
pub mod trust;
pub mod update;

#[cfg(test)]
pub(crate) mod testkit;

#[cfg(test)]
mod smoke;
