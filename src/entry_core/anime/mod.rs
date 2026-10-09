//! Anime entry details (plan areas 39a and 39b). Pure rules and state: `airing`, `entry_state`, `episode_row`, `format`,
//! `gesture`, `header`, `info`, `layout`, `model` (the load sequence), `runner` (the store runner), `settings` and
//! `store`. The thin widgets are `info_view`, `row_view`, `settings_view` and `route`; they read those values.
//!
//! Reused from the domain layer, not copied here: episode and season flag masks and codecs
//! (`kuro_domain::entries::anime`), the flag setters (`kuro_domain::entries::flag_setters`), the episode sorter
//! (`kuro_domain::items::sorter`), the filter engine (`kuro_domain::app::items`), the missing-episode gaps
//! (`kuro_domain::items::missing`), the anime preference keys (`kuro_prefs`), and the shared manga text rules
//! (`entry_core::text`) for title, author, artist and description.
#![allow(dead_code)] // wired by the anime entry page (T-39b-063, T-39b-064) as it lands

pub mod airing;
pub mod entry_state;
pub mod episode_row;
pub mod format;
pub mod gesture;
pub mod header;
pub mod info;
pub mod info_view;
pub mod layout;
pub mod actions;
pub mod bottom;
pub mod library;
pub mod refresh;
pub mod model;
pub mod route;
pub mod row_view;
pub mod runner;
pub mod settings;
pub mod settings_view;
pub mod store;
