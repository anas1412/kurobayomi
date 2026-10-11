//! The anime Browse tab (area 41, part 2). The anime sources list and its tab state, the browse page's rules, the anime
//! extension list model and the anime page loader, with the thin GTK views over them: the Sources tab (`sources_view`)
//! and the browse page (`browse_view`). The kind-agnostic pieces (grouping, listing state, content plan, toasts,
//! favourites, nav rules, languages) and the shared widgets are the parent module's and are reused unchanged. The
//! screen lane registers the two views; until it does they have no caller outside their tests.
#![allow(dead_code)] // wired by the screen registration, which this lane does not own

pub mod browse;
pub mod browse_screen;
pub mod browse_view;
pub mod directory;
pub mod extensions;
pub mod favorite_store;
pub mod loader;
pub mod migrate_dialog;
pub mod migrate_engine;
pub mod migrate_live;
pub mod migrate_rules;
pub mod prefs;
pub mod sources;
pub mod sources_view;

#[cfg(test)]
pub(crate) mod fixtures;
