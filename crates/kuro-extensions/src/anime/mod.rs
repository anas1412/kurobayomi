//! Anime extensions (area 19): the `.kuroext` package container and verification, the store protocol, the trust rule,
//! the install pipeline, and the state the UI reads. Everything here is headless: no GTK, no OS-specific code.

pub mod abi;
pub mod install;
pub mod loader;
pub mod manager;
pub mod model;
pub mod package;
pub mod store;
pub mod trust;

#[cfg(test)]
pub(crate) mod testkit;

#[cfg(test)]
mod smoke;
