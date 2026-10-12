//! Settings preference model and widgets (area 47b): the page tree, veto-gated writes, the search-jump
//! highlight, the More tab and the theme pickers. Widgets are thin; logic is pure and tested.
//!
//! `settings::body` builds these screens through [`pages::body`]; Escape, Alt+Left and the mouse back button reach
//! the settings shell through [`shell::go_back`].
#![allow(dead_code)]

pub mod backup_pages;
pub mod bind;
pub mod debug_info;
pub mod enabled;
pub mod highlight;
pub mod language;
pub mod model;
pub mod net;
pub mod more;
pub mod page;
pub mod pages;
pub mod queue_state;
pub mod render;
pub mod router;
pub mod shell;
pub mod spec_convert;
pub mod style;
pub mod stores;
pub mod stores_backend;
pub mod stores_page;
pub mod storage_rows;
pub mod strings;
pub mod widgets;
