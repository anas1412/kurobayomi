//! ActionButton (T-49a-022): a text button with the icon above the label, 4 px apart.
//!
//! The icon has no accessible name (the source passes `contentDescription = null`); the label is
//! the accessible name, so a screen reader reads the label only.

use gtk::prelude::*;

/// Gap between the icon and the label.
pub const ICON_LABEL_GAP: i32 = 4;

/// The accessible name is the label, never the icon.
pub fn accessible_name(label: &str) -> &str {
    label
}

/// A flat button: icon over label, centred, text-button style.
pub fn action_button(label: &str, icon_name: &str, on_click: impl Fn() + 'static) -> gtk::Button {
    let column = gtk::Box::new(gtk::Orientation::Vertical, ICON_LABEL_GAP);
    column.set_halign(gtk::Align::Center);
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_accessible_role(gtk::AccessibleRole::Presentation);
    let text = gtk::Label::new(Some(accessible_name(label)));
    text.set_justify(gtk::Justification::Center);
    column.append(&icon);
    column.append(&text);
    let button = gtk::Button::new();
    button.add_css_class("flat");
    button.set_child(Some(&column));
    button.set_tooltip_text(Some(label));
    button.connect_clicked(move |_| on_click());
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_is_four_and_the_label_is_the_name() {
        assert_eq!(ICON_LABEL_GAP, 4);
        assert_eq!(accessible_name("Download"), "Download");
    }
}
