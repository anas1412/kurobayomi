//! GUI logic of area 42 (History, Updates, the tab shell and the statistics and storage helpers).
//!
//! Pure modules come first and carry their own tests: `cargo test -p kurobayomi updates`. The widgets (`shell`) stay
//! thin. The domain queries and writes live in `kuro-domain`; these modules decide what the screens do with them.
#![allow(dead_code)] // wired by the screens lane; the helpers are tested here before the screens call them

pub mod bottom_bar;
pub mod bulk;
pub mod coalesce;
pub mod copy;
pub mod dates;
pub mod download_actions;
pub mod download_state;
pub mod format;
pub mod feed;
pub mod history;
pub mod history_data;
pub mod history_dialog;
pub mod history_screen;
pub mod history_row;
pub mod list;
pub mod mount;
pub mod nav;
pub mod prefs;
pub mod screen_toast;
pub mod selection;
pub mod shell;
pub mod signals;
pub mod state_page;
pub mod stats;
pub mod stats_screen;
pub mod store;
pub mod storage;
pub mod storage_row;
pub mod storage_screen;
pub mod updates_dialog;
pub mod updates_page;
pub mod updates_screen;
