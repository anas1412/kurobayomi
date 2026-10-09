//! Page states as a `gtk::Stack` (T-42a-008, T-42a-022): a spinner while loading, the empty message, the error row
//! with a retry, and the list. The child names are the ones [`PageState::child_name`] gives. Widgets only; the rules
//! are in [`crate::updates::list`] and [`crate::updates::updates_page`].

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O, Spinner, Stack, StackTransitionType};

use crate::updates::list::PageState;

pub struct PageStates {
    stack: Stack,
    empty: Label,
    error: Label,
}

impl PageStates {
    /// The stack with its four children. `list` is the page's list widget; `on_retry` runs the load again.
    pub fn new(list: &impl IsA<gtk::Widget>, on_retry: impl Fn() + 'static) -> Self {
        let stack = Stack::builder().transition_type(StackTransitionType::Crossfade).vexpand(true).build();
        stack.add_named(&Spinner::builder().spinning(true).halign(Align::Center).valign(Align::Center).build(), Some("loading"));

        let empty = Label::builder().css_classes(["dim-label"]).halign(Align::Center).valign(Align::Center).build();
        stack.add_named(&empty, Some("empty"));

        let error_box = GBox::new(O::Vertical, 8);
        error_box.set_halign(Align::Center);
        error_box.set_valign(Align::Center);
        let error = Label::builder().wrap(true).build();
        let retry = Button::with_label("Retry");
        retry.connect_clicked(move |_| on_retry());
        error_box.append(&error);
        error_box.append(&retry);
        stack.add_named(&error_box, Some("error"));

        stack.add_named(list, Some("list"));
        Self { stack, empty, error }
    }

    pub fn widget(&self) -> &Stack {
        &self.stack
    }

    /// Shows the child of `state`, and sets the message the empty or error child carries.
    pub fn show(&self, state: &PageState) {
        match state {
            PageState::Empty(message) => self.empty.set_label(message),
            PageState::Error(message) => self.error.set_label(message),
            PageState::Loading | PageState::List => {}
        }
        self.stack.set_visible_child_name(state.child_name());
    }
}
