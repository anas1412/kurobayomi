//! The library screen (area 37). The logic is pure and tested here; it sits on kuro-domain's library model, category
//! service and preferences. GTK widgets are in `view`; `mount` puts the view in the window.
//!
//! Tests: the package is a binary, so there is no `--lib` target. Run them without a display with
//! `cargo test -p kurobayomi --bin kurobayomi library::`. The widget checks in `view::sheet` need a display and skip
//! themselves without one (they run on the shared GTK thread of `crate::testing`). `t2` holds the T2-* tests of
//! `p1-edge-cases-and-tests.md` section T-2.
//!
//! Fixtures: the T-3 data sets live with the code that reads them, in `crates/kuro-domain` (its `testdb` and the
//! row builders in `fixtures`), not in `tests/fixtures/library`, because every pure rule they feed is tested there
//! and in `t2` without a JSON round trip.
#![allow(dead_code)] // some pure helpers are tested but not called yet

pub mod actions;
pub mod cells;
pub mod display_tab;
pub mod download_facts;
pub mod filter_tab;
#[cfg(test)]
mod fixtures;
pub mod keys;
pub mod mount;
pub mod name_dialog;
pub mod random;
pub mod category_dialog;
pub mod category_screen;
pub mod delete_dialog;
pub mod layout;
pub mod load;
pub mod query;
pub mod sort_tab;
pub mod toast;
pub mod trackers;
#[cfg(test)]
mod t2;
pub mod select;
pub mod view;
pub mod worker;
