//! Manga entry details (plan area 38). The model, the chapter engine, the store runner and the page controller have no
//! GTK in them: `backend`, `banner`, `chapter`, `detail`, `dialog`, `errors`, `flags`, `header`, `list_model`, `live`,
//! `numfmt`, `page`, `readstate`, `runner`, `scanlator_sheet` (its rules), `selection`, `store`, `text` and `view`, plus
//! `downloaded` (the downloaded lookup of the chapters) and `handoff` (what a page parks for the screen it opens). The thin
//! widgets are `banner_view`, `cover_view`, `description_view`, `header_view`, `list_view` and
//! `route`; they read those types and draw them.
//!
//! Decision records (Android-only parts dropped or replaced; the user loses no function unless a line says so):
//! - T-38a-048: the assist content URL is dropped. The Share overflow item (`toolbar::Overflow::Share`) gives the same address.
//! - T-38a-049: Voyager, the Compose runtime, Injekt, Coil, Android Intents and ContentResolver are replaced by page
//!   structs, `ui::push` and `ui::pop`, constructor injection and the Rust equivalents of the rust design table (section 4).
//! - T-38a-050: Refresh is R or Ctrl+R (`shortcuts::header_key`), the overflow Refresh item (`toolbar::Overflow::Refresh`)
//!   and the header spinner.
//! - T-38a-060: the Android reset-job bug (E-12) is not ported. Each long press starts a new generation, so an earlier
//!   1-second timeout hides nothing.
//! - T-38b-042: dropped: Voyager `StateScreenModel` and `rememberScreenModel` (the reducer, T-38b-039, replaces them),
//!   Compose `State` and `@Immutable`, Injekt field injection (constructor injection), `Serializable` on `Manga` and
//!   `MangaTrack` (plain structs), and `Lifecycle` flows (widget map and unmap). No visible change; state survives a
//!   restart only in the DB and preferences, as upstream.
//! - T-39a-052: the ignorePadding layout trick is dropped. Rows take their own GTK margins instead.
//! - T-39a-053: the preview composables are dropped; `AnimeEpisodeListItemPreview` is not ported.
//! - T-39a-059: Compose item keys are dropped. The GTK list model's items (`glib::BoxedAnyObject` rows) replace them.
//! - T-39a-064: the fast scroller is dropped. GTK overlay scrollbars are used.
//! - T-39b-066: `AssistContentScreen` and `onProvideAssistUrl` are dropped. Loss: the Android assistant cannot read the
//!   entry URL. Share, which copies the address (T-39b-051), replaces it.
//! - T-39b-068: the Voyager `Screen` and `Navigator` are dropped. Sheets are pages on
//!   the GTK stack: Escape and Back pop them (`escape_controller`), and the close button works too.
//! - T-39b-069: `rememberScreenModel` retention is dropped. The entry model lives in the page object, so a window resize
//!   keeps it.

pub mod action_row;
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
pub mod flags;
pub mod header;
pub mod handoff;
pub mod header_view;
pub mod info_block;
pub mod info_pane;
pub mod list_model;
pub mod list_options;
pub mod list_view;
pub mod live;
pub mod message;
pub mod markdown;
pub mod numfmt;
pub mod page;
/// The GTK pieces both details pages build the same way (the state stack, the item header row, the Start button, Back,
/// menu rows, the scroll fade).
pub mod page_shell;
pub mod readstate;
pub mod route;
pub mod runner;
/// The runner handle and the entry-and-items live subscription both store runners use.
pub mod runner_kit;
pub mod scanlator_sheet;
pub mod selection;
pub mod store;
pub mod text;
pub mod toolbar;
pub mod view;

/// Shared constant enums and separator labels (T-38a-045, T-38a-043), the base list row (T-38a-044, T-39a-061), cover
/// frames (T-38a-039), the header backdrop (T-38a-014) and the header shortcuts (T-38a-046).
pub mod backdrop;
pub mod backdrop_view;
pub mod shortcut_help_view;
pub mod constants;
pub mod list_row;
pub mod shortcuts;

/// Anime entry details (plan areas 39a and 39b): pure rules for the anime screen.
pub mod anime;

/// Tracking on the entry screens (plan areas 38b and 39b): the pure rules of the track sheet, the pickers, the detached
/// writes, the search screen, the auto-track prompt and tracker removal.
pub mod tracking;
