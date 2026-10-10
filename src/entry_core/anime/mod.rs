//! Anime entry details (plan areas 39a and 39b). Pure rules and state: `airing`, `entry_state`, `episode_row`, `format`,
//! `header`, `info`, `layout`, `model` (the load sequence), `remote` (the source refresh), `runner` (the store
//! runner), `settings` and `store`, plus `dispatch` (what the bar and the swipes send), `downloads` (the Download menu rules)
//! and `download_backend` (the downloads folder and queue). The thin widgets are `info_view`, `row_view`,
//! `season_grid`, `settings_view`, `season_settings_view`, `images_view`, `duplicate_view` and `route`; they read those
//! values (`swipe_look` is the pure side of the swipe strip and the row menu, `images_io` of the image files).
//!
//! Reused from the domain layer, not copied here: episode and season flag masks and codecs
//! (`kuro_domain::entries::anime`), the flag setters (`kuro_domain::entries::flag_setters`), the episode sorter
//! (`kuro_domain::items::sorter`), the filter engine (`kuro_domain::app::items`), the missing-episode gaps
//! (`kuro_domain::items::missing`), the anime preference keys (`kuro_prefs`), and the shared manga text rules
//! (`entry_core::text`) for title, author, artist and description.

pub mod airing;
pub mod entry_state;
pub mod episode_item;
pub mod episode_row;
pub mod format;
pub mod selection;
pub mod header;
pub mod info;
pub mod info_view;
pub mod layout;
#[cfg(test)]
mod acceptance;
pub mod actions;
pub mod bottom;
pub mod dispatch;
pub mod download_backend;
pub mod downloads;
pub mod library;
pub mod refresh;
pub mod remote;
pub mod model;
pub mod route;
pub mod row_view;
pub mod runner;
pub mod related_pack;
pub mod related_view;
pub mod details_error;
pub mod duplicate;
pub mod duplicate_view;
pub mod summary;
pub mod swipe_look;
pub mod images;
pub mod images_io;
pub mod images_view;
pub mod season_settings;
pub mod season_settings_view;
pub mod set_default;
pub mod season_grid;
pub mod season_tile;
pub mod seen;
pub mod settings;
pub mod settings_view;
pub mod store;
pub mod viewer;
