//! Reusable GTK widgets (T-49a-022 to T-49a-055). Each file holds pure helpers with unit tests and
//! thin GTK builders on top. CSS classes come from `theme::style` and `theme::css`.
#![allow(dead_code)]

pub mod action_button;
pub mod adaptive_sheet;
pub mod button;
pub mod focus;
pub mod keyboard;
pub mod layout;
pub mod numeric;
pub mod page_header;
pub mod pill;
pub mod refresh;
pub mod repeat_button;
pub mod detachable;
pub mod int_slider;
pub mod rail;
pub mod row_activate;
pub mod scaffold;
pub mod settings_items;
pub mod tabs;
