//! Pill value chip (T-49a-034): one line, 6 px / 1 px padding, a 4 px start margin, a pill radius.

use gtk::prelude::*;

pub const PILL_MARGIN_START: i32 = 4;

/// A one-line pill with the default body size.
pub fn pill(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class(crate::theme::style::class::PILL);
    label.set_single_line_mode(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_margin_start(PILL_MARGIN_START);
    label
}
