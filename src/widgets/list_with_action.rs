//! List with a bottom action button (T-49a-028): the list fills the space, a separator, then a
//! full-width action button with 16 px side and 8 px vertical margins.

use gtk::prelude::*;

pub const ACTION_MARGIN_H: i32 = 16;
pub const ACTION_MARGIN_V: i32 = 8;

/// The list above a full-width suggested action. `enabled` is the button's sensitivity.
pub fn list_with_action(
    contents: &impl IsA<gtk::Widget>,
    label: &str,
    enabled: bool,
    on_action: impl Fn() + 'static,
) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(contents));
    column.append(&scroller);
    column.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    let button = gtk::Button::with_label(label);
    button.add_css_class("suggested-action");
    button.set_hexpand(true);
    button.set_margin_start(ACTION_MARGIN_H);
    button.set_margin_end(ACTION_MARGIN_H);
    button.set_margin_top(ACTION_MARGIN_V);
    button.set_margin_bottom(ACTION_MARGIN_V);
    button.set_sensitive(enabled);
    button.connect_clicked(move |_| on_action());
    column.append(&button);
    column
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_margins_are_16_and_8() {
        assert_eq!((ACTION_MARGIN_H, ACTION_MARGIN_V), (16, 8));
    }
}
