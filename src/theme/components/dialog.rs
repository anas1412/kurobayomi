//! Dialog content layout (T-49a-049): width clamped to 280..560 px, 24 px paddings, an icon, a
//! title, a text block that scrolls at 60% of the window height, and end-aligned buttons.

use gtk::prelude::*;

use super::classes;
use crate::theme::tokens::{DIALOG_MAX_WIDTH, DIALOG_MIN_WIDTH, DIALOG_PAD};

/// The fraction of the window height that the text block may use before it scrolls (E-49-27).
pub const TEXT_MAX_FRACTION: f32 = 0.6;

/// The dialog width for a window width: clamped to 280..560 px.
pub fn dialog_width(window_width: f32) -> f32 {
    window_width.clamp(DIALOG_MIN_WIDTH, DIALOG_MAX_WIDTH)
}

/// The maximum text height for a window height, in px.
pub fn text_max_height(window_height: f32) -> i32 {
    (window_height * TEXT_MAX_FRACTION).round() as i32
}

/// The dialog body: optional icon, title, text, and a row of buttons. The caller sets the width
/// with `dialog_width`.
pub fn dialog_content(
    icon_name: Option<&str>,
    title: &str,
    text: &str,
    max_text_height: i32,
    buttons: &[gtk::Widget],
) -> gtk::Box {
    let pad = DIALOG_PAD as i32;
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.add_css_class(classes::DIALOG);
    body.set_margin_top(pad);
    body.set_margin_start(pad);
    body.set_margin_end(pad);
    body.set_margin_bottom(pad);

    if let Some(name) = icon_name {
        let icon = gtk::Image::from_icon_name(name);
        icon.set_halign(gtk::Align::Center);
        icon.set_margin_bottom(16);
        body.append(&icon);
    }

    let heading = gtk::Label::new(Some(title));
    heading.add_css_class(classes::DIALOG_TITLE);
    heading.set_halign(if icon_name.is_some() {
        gtk::Align::Center
    } else {
        gtk::Align::Start
    });
    heading.set_margin_bottom(16);
    heading.set_wrap(true);
    body.append(&heading);

    let message = gtk::Label::new(Some(text));
    message.add_css_class(classes::DIALOG_TEXT);
    message.set_wrap(true);
    message.set_xalign(0.0);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_propagate_natural_height(true);
    scroller.set_max_content_height(max_text_height);
    scroller.set_child(Some(&message));
    scroller.set_margin_bottom(pad);
    body.append(&scroller);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::End);
    for button in buttons {
        row.append(button);
    }
    body.append(&row);
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_is_clamped_to_280_through_560() {
        assert_eq!(dialog_width(100.0), 280.0);
        assert_eq!(dialog_width(400.0), 400.0);
        assert_eq!(dialog_width(2000.0), 560.0);
    }

    #[test]
    fn text_scrolls_at_sixty_percent_of_height() {
        assert_eq!(text_max_height(500.0), 300);
        assert_eq!(text_max_height(0.0), 0);
    }
}
