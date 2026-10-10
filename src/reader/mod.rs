//! The reader (area 43). Pure models and loaders are GTK-free; the widget builders are `transition` and `chrome`.
//!
//! `model` holds the chapter, page and transition types. `flags` holds mode, orientation and crop state. `image`
//! and `cache` hold the page rules and the disk cache. `loader` holds the page loaders and the chapter state
//! machine. `chapters` builds the reader list. `session` runs the reader state. `launch` opens the reader for the app. `screen` is the reader page: open, failure card, chapter buttons, crop. `writer` is the database writer.
//!
//! Gap list G-01 to G-35 (`plans/43-reader-core/p1-ui-parity.md` section 12), the ones that were open at the review (T-43a-078):
//! - G-27 navigation overlay labels with fade: done in `nav_overlay` (zones, colours, labels, 1000 ms fades, shown by the
//!   on-start and new-user preferences and by a changed tap-zone layout). The tap zones themselves are area 44 (`viewers::nav`).
//! - G-34 the "Long strip with gaps" label: `strings::VERTICAL_PLUS_VIEWER`.
//! - G-35 Open in WebView: assigned to area 36 (T-36a-053, T-36a-054, `webkit6` window in `src/web/window.rs`). The reader keeps the
//!   entry in the overflow menu and opens the chapter in the desktop browser until that window exists.
//! - Real page data for an online chapter waits for the source registry (area 17); `launch` documents the dependency and
//!   reports "Source not found" until the registry hands the loader a source.
//!
//! Decisions recorded here so they are not mistaken for gaps:
//! - The reader settings sheet (`settings_window`) is built from the preference keys directly, not from the spec items of
//!   `src/spec.rs`: the spec `Item` has no preference key and `ui::item_row` binds no `ReaderPrefs`. Each row reads its default,
//!   writes its key and is enabled or disabled by `settings_ui::reader::rules::row_enabled`; the acceptance text of T-43b-031 is read that way.
//! - The data saver keeps Bandwidth Hero and wsrv.nl and drops resmush.it (`loader::saver`, D-DL-09); the HTTP page loader applies it.
#![allow(dead_code, unused_imports)] // the viewers (area 44) and the source registry (area 17) attach later

pub mod blend_box;
pub mod bottom_bar;
pub mod cache;
pub mod crop_borders;
pub mod decode_guard;
pub mod decoded_lru;
pub mod delete_disk;
pub mod delete_queue;
pub mod download_paths;
pub mod error_card;
pub mod chapters;
pub mod chrome;
#[cfg(test)]
mod chrome_tests;
pub mod flags;
#[cfg(test)]
pub mod golden;
pub mod history_rules;
pub mod image;
pub mod indicator;
pub mod keep_awake;
pub mod launch;
pub mod loader;
pub mod long_press;
pub mod mode_sheet;
pub mod model;
pub mod nav;
pub mod nav_overlay;
pub mod navigator;
pub mod overlay;
pub mod page_actions;
pub mod page_decode;
pub mod page_save;
pub mod page_transform;
pub mod page_view;
pub mod page_zoom;
pub mod pref_rules;
pub mod progress;
pub mod restore;
pub mod rotation_sheet;
pub mod saved_notice;
pub mod quick_scale;
pub mod release_notes;
pub mod rules;
pub mod screen;
pub mod session;
pub mod settings_model;
pub mod settings_window;
pub mod sheet;
pub mod state;
pub mod strings;
pub mod sync;
pub mod theme;
pub mod tiles;
pub mod transition;
pub mod viewer;
pub mod viewers;
pub mod writer;

pub use screen::{ReaderChrome, ReaderDeps, ReaderExtras, ReaderScreen, open, open_with};
