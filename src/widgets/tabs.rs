//! Tab label with an optional count pill (T-49b-022). One line with an ellipsis; the pill shows when
//! the count is `Some`, including zero.

use gtk::prelude::*;

use crate::theme::style::class;
use crate::widgets::pill;

/// Pill text size in points (the source's `10 pt`).
pub const PILL_FONT_PT: i32 = 10;

/// Alpha of the onBackground pill background: 12% in dark, 8% in light.
pub fn pill_alpha(dark: bool) -> f32 {
    if dark { 0.12 } else { 0.08 }
}

/// A tab label, with its count pill when `count` is `Some`.
pub fn tab_label(text: &str, count: Option<i64>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let label = gtk::Label::new(Some(text));
    label.set_single_line_mode(true);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&label);
    if let Some(count) = count {
        let pill = pill::pill(&count.to_string());
        pill.add_css_class(class::TAB_PILL);
        let attrs = gtk::pango::AttrList::new();
        attrs.insert(gtk::pango::AttrSize::new(PILL_FONT_PT * gtk::pango::SCALE));
        pill.set_attributes(Some(&attrs));
        row.append(&pill);
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pill_alpha_per_mode() {
        assert_eq!(pill_alpha(true), 0.12);
        assert_eq!(pill_alpha(false), 0.08);
    }

    #[test]
    fn pill_font_is_ten_points() {
        assert_eq!(PILL_FONT_PT, 10);
    }
}
