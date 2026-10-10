//! Manga entry details (plan area 38). The model, the chapter engine, the store runner and the page controller have no
//! GTK in them: `backend`, `banner`, `chapter`, `detail`, `dialog`, `errors`, `flags`, `header`, `list_model`, `live`,
//! `numfmt`, `page`, `readstate`, `runner`, `scanlator_sheet` (its rules), `selection`, `store`, `text` and `view`, plus
//! `downloaded` (the downloaded-only chapter helper) and `handoff` (what a page parks for the screen it opens). The thin
//! widgets are `banner_view`, `category_dialog_view`, `cover_view`, `description_view`, `header_view`, `list_view` and
//! `route`; they read those types and draw them.

pub mod action_row;
pub mod empty_state;
pub mod action_row_view;
pub mod fetch_interval;
pub mod fetch_interval_dialog;
pub mod backend;
pub mod bulk_bar;
pub mod chapter_actions;
pub mod cover_view;
pub mod banner;
pub mod banner_view;
pub mod chapter;
pub mod description_view;
pub mod detail;
pub mod dialog;
pub mod downloaded;
pub mod errors;
pub mod favorite;
pub mod category_dialog_view;
pub mod flags;
pub mod header;
pub mod handoff;
pub mod header_view;
pub mod list_model;
pub mod list_view;
pub mod live;
pub mod message;
pub mod numfmt;
pub mod page;
pub mod readstate;
pub mod route;
pub mod runner;
pub mod scanlator_sheet;
pub mod selection;
pub mod store;
pub mod swipe;
pub mod text;
pub mod toolbar;
pub mod view;

/// Anime entry details (plan areas 39a and 39b): pure rules for the anime screen.
pub mod anime;

/// Tracking on the entry screens (plan areas 38b and 39b): the pure rules of the track sheet, the pickers, the detached
/// writes, the search screen, the auto-track prompt and tracker removal.
pub mod tracking;
