//! Icon toggle button (T-49a-053): 48 px high, an icon and up to two title lines with an ellipsis,
//! filled while checked.

use gtk::prelude::*;

use crate::theme::style::class;

/// Button height in px.
pub const HEIGHT: i32 = 48;
/// Title lines before the ellipsis.
pub const TITLE_LINES: i32 = 2;

/// The filled style applies while the toggle is checked.
pub fn is_filled(checked: bool) -> bool {
    checked
}

/// An icon toggle with `title` beside `icon`. Filled when `checked`.
pub fn icon_toggle(icon: &str, title: &str, checked: bool) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_height_request(HEIGHT);
    button.set_active(checked);
    button.add_css_class(class::ICON_TOGGLE);
    apply_fill(&button, is_filled(checked));
    button.connect_toggled(|b| apply_fill(b, is_filled(b.is_active())));

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&gtk::Image::from_icon_name(icon));
    let label = gtk::Label::new(Some(title));
    label.set_wrap(true);
    label.set_lines(TITLE_LINES);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_xalign(0.0);
    row.append(&label);
    button.set_child(Some(&row));
    button
}

fn apply_fill(button: &gtk::ToggleButton, filled: bool) {
    if filled {
        button.add_css_class(class::BUTTON_FILLED);
    } else {
        button.remove_css_class(class::BUTTON_FILLED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filled_follows_checked() {
        assert!(is_filled(true));
        assert!(!is_filled(false));
    }

    #[test]
    fn height_and_title_lines() {
        assert_eq!((HEIGHT, TITLE_LINES), (48, 2));
    }
}
