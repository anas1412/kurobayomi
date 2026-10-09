//! The manga Browse tab (area 40, lane gui-browse). Logic is pure and tested in the modules below; the GTK widgets are
//! thin. The pages, dialogs, popovers, filter sheet, favourite flow and the production `RemoteLoader` and `RepoStore` are
//! built and tested here, but nothing is registered with the screen registry yet: the screen lane owns `screens.rs` and
//! `entry.rs`, and the app has no source registry or extension manager to hand them (see `source_bridge`). Until a mount
//! passes real models in, the module has no caller outside its own tests, which is what the allow below covers.
#![allow(dead_code)] // wired by the screen registration, which this lane does not own

pub mod anime;
pub mod banner;
pub mod content;
pub mod dialogs;
pub mod entries;
pub mod ext_badges;
pub mod ext_problems;
pub mod extensions;
pub mod extensions_page;
pub mod favorite;
pub mod favorite_flow;
pub mod filter_sheet;
pub mod filters;
pub mod grid;
pub mod glue;
pub mod header;
pub mod header_bar;
pub mod icons;
pub mod installs;
pub mod listing;
pub mod listing_view;
pub mod loader;
pub mod migrate_nav;
pub mod nav;
pub mod names;
pub mod pages;
pub mod prefs;
pub mod primitives;
pub mod repo_store;
pub mod route;
pub mod shell;
pub mod sources;
pub mod sheet;
pub mod sources_page;
pub mod tasks;
pub mod trust;
pub mod view;

#[cfg(test)]
mod smoke;
