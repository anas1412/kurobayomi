//! The Browse tab (areas 40 and 41, lane gui-browse). Logic is pure and tested in the modules below; the GTK widgets are
//! thin. `mount` registers the shell with the screen registry (`screens::body("browse")`) and opens its pages. The Sources
//! pages list nothing until the app has a source registry, and the extension pages show what `ext_host` published, so
//! parts of the module still have no caller outside their tests, which is what the allow below covers.
//!
//! Decision records (Android-only parts dropped or replaced):
//! - T-40b-083: Voyager is replaced by the GTK page stack. The browse, global search and migrate pages use `ui::push`,
//!   `ui::pop` and `ui::pop_until`, and Back follows B 4.6.2 at every level.
//! - T-41a-025: the "install unknown apps" banner is dropped (D2). Linux has no such permission, so there is no banner and
//!   no permission request. The installer preference is still read for export parity and has no effect on the list.
//! - T-41b-055: no haptic on a browse long press. The dialog or added badge is the visible result.
//! - T-41b-056: no assist-content hook. The source home URL stays reachable through "Open in WebView". Loss: Android
//!   assistant only.
//! - T-41b-057: no row movement animation. Results reorder as they arrive. Pages change with the stack's crossfade only.
//! - T-41b-058: navigation uses `ui::push` and `ui::pop` with no Voyager types. Escape and Back match section 4 of
//!   `p2-topic-browse-listing.md`.
//! - T-41b-059: collaborators are passed explicitly, not through a service locator. `anime::browse_screen::AnimeEnv` holds
//!   the anime browse services, and the browse and global search modules read no global. The struct is `AnimeEnv`, not
//!   `AppServices`; the name is the only difference.
#![allow(dead_code)] // wired as the source registry and the extension manager arrive

pub mod anime;
pub mod banner;
pub mod browse_screen;
pub mod content;
pub mod cookies;
pub mod deep_link;
pub mod dialogs;
pub mod directory;
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
pub mod global_search;
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
pub mod manga_migrate_dialog;
pub mod migrate_engine;
pub mod migrate_flags;
pub mod migrate_list;
pub mod migrate_live;
pub mod migrate_merge;
pub mod migrate_nav;
pub mod migrate_screen;
pub mod mount;
pub mod nav;
pub mod names;
pub mod pages;
pub mod pending;
pub mod prefs;
pub mod primitives;
pub mod repo_store;
pub mod route;
pub mod search_runner;
pub mod search_screen;
pub mod shell;
pub mod source_prefs_page;
pub mod source_prefs_screen;
pub mod sources;
pub mod sheet;
pub mod sources_page;
pub mod tasks;
pub mod texts;
pub mod trust;
pub mod view;

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod smoke;
#[cfg(test)]
mod fixtures_tests;
#[cfg(test)]
mod spec_tests;
