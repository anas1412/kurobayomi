//! Reusable GTK widgets (T-49a-022 to T-49a-055). Each file holds pure helpers with unit tests and
//! thin GTK builders on top. CSS classes come from `theme::style` and `theme::css`.
#![allow(dead_code)]

pub mod action_button;
pub mod adaptive_sheet;
pub mod button;
pub mod extended_fab;
pub mod focus;
pub mod keyboard;
pub mod labeled_checkbox;
pub mod layout;
pub mod link_icon;
pub mod list_with_action;
pub mod numeric;
pub mod pill;
pub mod refresh;
pub mod repeat_button;
pub mod collapsible;
pub mod icon_toggle;
pub mod info_screen;
pub mod insets;
pub mod int_slider;
pub mod observe;
pub mod rail;
pub mod scaffold;
pub mod scroll_fab;
pub mod settings_items;
pub mod surface;
pub mod tabs;
pub mod wheel_picker;
