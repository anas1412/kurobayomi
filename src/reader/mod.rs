//! The reader (area 43). Pure models and loaders are GTK-free; the widget builders are `transition` and `chrome`.
//!
//! `model` holds the chapter, page and transition types. `flags` holds mode, orientation and crop state. `image`
//! and `cache` hold the page rules and the disk cache. `loader` holds the page loaders and the chapter state
//! machine. `chapters` builds the reader list. `session` runs the reader state. `launch` opens the reader for the app. `screen` is the reader page: open, failure card, chapter buttons, crop. `writer` is the database writer.
#![allow(dead_code, unused_imports)] // the viewers (area 44) and the source registry (area 17) attach later

pub mod blend_box;
pub mod bottom_bar;
pub mod cache;
pub mod chapters;
pub mod chrome;
pub mod flags;
pub mod image;
pub mod indicator;
pub mod keep_awake;
pub mod launch;
pub mod loader;
pub mod long_press;
pub mod mode_sheet;
pub mod model;
pub mod nav;
pub mod navigator;
pub mod overlay;
pub mod page_decode;
pub mod page_view;
pub mod page_zoom;
pub mod pref_rules;
pub mod progress;
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
pub mod transition;
pub mod viewer;
pub mod viewers;
pub mod writer;

pub use screen::{ReaderChrome, ReaderDeps, ReaderScreen, open};
