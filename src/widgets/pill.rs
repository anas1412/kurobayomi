//! Pill value chip (T-49a-034): one line, 6 px / 1 px padding, a 4 px start margin, a pill radius.
//! The second overload of the source sets the font size and is `pill_sized`.

use gtk::prelude::*;

pub const PILL_PAD_H: i32 = 6;
pub const PILL_PAD_V: i32 = 1;
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

/// A pill with `body medium` and a given font size in pixels (the source's second overload).
pub fn pill_sized(text: &str, font_px: i32) -> gtk::Label {
    let label = pill(text);
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrSize::new_size_absolute(font_px * gtk::pango::SCALE));
    label.set_attributes(Some(&attrs));
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_and_margin_match_the_source() {
        assert_eq!((PILL_PAD_H, PILL_PAD_V), (6, 1));
        assert_eq!(PILL_MARGIN_START, 4);
    }
}
