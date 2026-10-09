//! Manga entry details (plan area 38). The model, the chapter engine, the store runner and the page controller have no
//! GTK in them: `backend`, `banner`, `chapter`, `detail`, `dialog`, `errors`, `flags`, `header`, `list_model`, `live`,
//! `numfmt`, `page`, `readstate`, `runner`, `selection`, `store`, `text` and `view`. The thin widgets are
//! `banner_view`, `description_view`, `header_view`, `list_view` and `route`; they read those types and draw them.

pub mod backend;
pub mod banner;
pub mod banner_view;
pub mod chapter;
pub mod description_view;
pub mod detail;
pub mod dialog;
pub mod errors;
pub mod flags;
pub mod header;
pub mod header_view;
pub mod list_model;
pub mod list_view;
pub mod live;
pub mod numfmt;
pub mod page;
pub mod readstate;
pub mod route;
pub mod runner;
pub mod selection;
pub mod store;
pub mod text;
pub mod view;

/// Anime entry details (plan areas 39a and 39b): pure rules for the anime screen.
pub mod anime;
