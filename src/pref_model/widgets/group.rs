//! Group header over a boxed list: the stock `heading` style, flush with the list's edge, 12 px above the list (the
//! same look as `crate::ui::heading`). A blank title gives no header; the page keeps the spacer (T-E44).

use gtk::prelude::*;
use gtk::{Align, Label};

use super::GROUP_HEADER_BOTTOM;

pub fn header(title: &str) -> Option<Label> {
    if title.trim().is_empty() {
        return None;
    }
    let label = Label::new(Some(title));
    label.add_css_class("heading");
    label.set_halign(Align::Start);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_margin_bottom(GROUP_HEADER_BOTTOM);
    Some(label)
}
