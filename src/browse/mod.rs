//! The Browse tab (areas 40 and 41, lane gui-browse). Logic is pure and tested in the modules below; the GTK widgets are
//! thin. `mount` registers the shell with the screen registry (`screens::body("browse")`) and opens its pages. The Sources
//! pages list nothing until the app has a source registry, and the extension pages show what `ext_host` published, so
//! parts of the module still have no caller outside their tests, which is what the allow below covers.
#![allow(dead_code)] // wired as the source registry and the extension manager arrive

pub mod anime;
pub mod banner;
pub mod content;
pub mod dialogs;
pub mod entries;
pub mod ext_badges;
pub mod ext_problems;
pub mod extensions;
pub mod ext_details;
pub mod ext_details_page;
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
pub mod keys;
pub mod lang_pages;
pub mod listing;
pub mod listing_view;
pub mod loader;
pub mod migrate_nav;
pub mod mount;
pub mod nav;
pub mod names;
pub mod pages;
pub mod prefs;
pub mod primitives;
pub mod repo_store;
pub mod route;
pub mod shell;
pub mod source_prefs_page;
pub mod sources;
pub mod sheet;
pub mod sources_page;
pub mod tasks;
pub mod trust;
pub mod view;

#[cfg(test)]
mod smoke;
#[cfg(test)]
mod spec_tests;
