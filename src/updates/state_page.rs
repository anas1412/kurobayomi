//! Page states as a `gtk::Stack` (T-42a-008, T-42a-022): a spinner while loading, the empty state (icon, message, one
//! action), the error state with a retry, and the list. The child names are the ones [`PageState::child_name`] gives.
//! Widgets only; the rules are in [`crate::updates::list`] and [`crate::updates::updates_page`].

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Spinner, Stack, StackTransitionType};

use crate::updates::list::PageState;

pub struct PageStates {
    stack: Stack,
    empty: GBox,
    empty_icon: gtk::Image,
    empty_label: Label,
    error: Label,
}

impl PageStates {
    /// The stack with its four children. `list` is the page's list widget; `on_retry` runs the load again.
    pub fn new(list: &impl IsA<gtk::Widget>, on_retry: impl Fn() + 'static) -> Self {
        let stack = Stack::builder().transition_type(StackTransitionType::Crossfade).vexpand(true).build();
        stack.add_named(&Spinner::builder().spinning(true).halign(Align::Center).valign(Align::Center).build(), Some("loading"));

        // the empty look of the Library page (docs/design/other-tabs.md); the screen sets its icon and action
        let (empty, empty_label) = crate::browse::look::empty_state("view-list-symbolic", "", None);
        let empty_icon = empty.first_child().and_downcast::<gtk::Image>().expect("the empty state starts with its icon");
        stack.add_named(&empty, Some("empty"));

        let retry = Button::with_label(&kuro_i18n::tr(kuro_i18n::MsgKey::action_retry));
        retry.connect_clicked(move |_| on_retry());
        let (error_box, error) = crate::browse::look::empty_state("dialog-warning-symbolic", "", Some(&retry));
        stack.add_named(&error_box, Some("error"));

        stack.add_named(list, Some("list"));
        Self { stack, empty, empty_icon, empty_label, error }
    }

    /// The icon of the empty page and its one action (none by default).
    pub fn set_empty_look(&self, icon: &str, action: Option<&Button>) {
        self.empty_icon.set_icon_name(Some(icon));
        if let Some(action) = action {
            action.set_halign(Align::Center);
            self.empty.append(action);
        }
    }

    pub fn widget(&self) -> &Stack {
        &self.stack
    }

    /// Shows the child of `state`, and sets the message the empty or error child carries.
    pub fn show(&self, state: &PageState) {
        match state {
            PageState::Empty(message) => self.empty_label.set_label(message),
            PageState::Error(message) => self.error.set_label(message),
            PageState::Loading | PageState::List => {}
        }
        self.stack.set_visible_child_name(state.child_name());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_page_has_the_library_look_and_one_action() {
        crate::testing::with_display(|| {
            let pages = PageStates::new(&gtk::Label::new(Some("rows")), || {});
            let action = Button::with_label("Update library");
            pages.set_empty_look("software-update-available-symbolic", Some(&action));
            pages.show(&PageState::Empty("No recent updates".into()));
            assert_eq!(pages.stack.visible_child_name().as_deref(), Some("empty"));
            assert_eq!(pages.empty_label.label(), "No recent updates");
            assert!(pages.empty_label.has_css_class("kb-empty-title"));
            assert_eq!(pages.empty_icon.icon_name().as_deref(), Some("software-update-available-symbolic"));
            assert_eq!(action.parent().as_ref(), Some(pages.empty.upcast_ref::<gtk::Widget>()));
            pages.show(&PageState::Error("boom".into()));
            assert_eq!(pages.error.label(), "boom");
        });
    }
}
