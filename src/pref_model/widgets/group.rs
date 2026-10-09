//! Group header (Kotlin `PreferenceGroupHeader`). A blank title gives no header; the page keeps the spacer (T-E44).

use gtk::prelude::*;
use gtk::{Align, Label};

use super::{GROUP_HEADER_BOTTOM, GROUP_HEADER_TOP, PREF_HORIZONTAL_PADDING};

pub fn header(title: &str) -> Option<Label> {
    if title.trim().is_empty() {
        return None;
    }
    let label = Label::new(Some(title));
    label.add_css_class("dim-label");
    label.set_halign(Align::Start);
    label.set_xalign(0.0);
    label.set_margin_top(GROUP_HEADER_TOP);
    label.set_margin_bottom(GROUP_HEADER_BOTTOM);
    label.set_margin_start(PREF_HORIZONTAL_PADDING);
    label.set_margin_end(PREF_HORIZONTAL_PADDING);
    Some(label)
}
