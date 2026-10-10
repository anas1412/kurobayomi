//! Empty screen (Kotlin `EmptyScreen`, T-36a-048): an icon, a message and up to two action buttons. The list areas use it.
//!
//! The Kotlin preview helpers are not ported. The kaomoji face of `src/theme/empty_state.rs` (T-49b-025) is a separate
//! widget with three buttons per row; this one follows T-36a-048.

use gtk::prelude::*;

/// The most actions one empty screen shows.
pub const MAX_ACTIONS: usize = 2;

/// Width of the message text, in characters.
const MESSAGE_WIDTH_CHARS: i32 = 40;

/// The actions that are shown: the first `MAX_ACTIONS`, in the order the caller lists them.
pub fn shown<T>(actions: Vec<T>) -> Vec<T> {
    actions.into_iter().take(MAX_ACTIONS).collect()
}

/// One action button: a label, a freedesktop icon name and what it does.
pub struct Action {
    pub label: String,
    pub icon_name: &'static str,
    pub on_click: Box<dyn Fn()>,
}

/// The empty screen: icon, message, then the buttons in one row, centred in the parent.
pub fn widget(icon_name: &str, message: &str, actions: Vec<Action>) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_halign(gtk::Align::Center);
    column.set_valign(gtk::Align::Center);
    column.set_margin_start(24);
    column.set_margin_end(24);

    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(64);
    icon.add_css_class("dim-label");
    column.append(&icon);

    let text = gtk::Label::new(Some(message));
    text.set_wrap(true);
    text.set_max_width_chars(MESSAGE_WIDTH_CHARS);
    text.set_justify(gtk::Justification::Center);
    text.add_css_class("dim-label");
    column.append(&text);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_halign(gtk::Align::Center);
    row.set_margin_top(12);
    for action in shown(actions) {
        row.append(&action_button(action));
    }
    if row.first_child().is_some() {
        column.append(&row);
    }
    column
}

fn action_button(action: Action) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&gtk::Image::from_icon_name(action.icon_name));
    content.append(&gtk::Label::new(Some(&action.label)));
    let button = gtk::Button::new();
    button.set_child(Some(&content));
    let on_click = action.on_click;
    button.connect_clicked(move |_| on_click());
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_most_two_actions_are_shown_in_order() {
        assert_eq!(MAX_ACTIONS, 2);
        assert_eq!(shown(vec!["retry", "help", "more"]), vec!["retry", "help"]);
        assert_eq!(shown(vec!["retry"]), vec!["retry"]);
        assert!(shown(Vec::<&str>::new()).is_empty());
    }
}
