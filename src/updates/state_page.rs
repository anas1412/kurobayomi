//! Page states as a `gtk::Stack` (T-42a-008, T-42a-022): a spinner while loading, the empty state (icon, message, one
//! action), the error state with a retry, and the list. The child names are the ones [`PageState::child_name`] gives.
//! Widgets only; the rules are in [`crate::updates::list`] and [`crate::updates::updates_page`].

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Stack, StackTransitionType};

use crate::browse::look;
use crate::ui::{LoadSize, loading};
use crate::updates::list::PageState;

pub struct PageStates {
    stack: Stack,
    empty: GBox,
    empty_icon: gtk::Image,
    empty_label: Label,
    /// Runs the load again; the error child is rebuilt with each message, so it keeps this.
    on_retry: Rc<dyn Fn()>,
}

impl PageStates {
    /// The stack with its four children. `list` is the page's list widget; `on_retry` runs the load again.
    pub fn new(list: &impl IsA<gtk::Widget>, on_retry: impl Fn() + 'static) -> Self {
        let stack = Stack::builder().transition_type(StackTransitionType::Crossfade).vexpand(true).build();
        stack.add_named(&loading(LoadSize::Page), Some("loading"));

        // the empty look of the Library page (docs/design/other-tabs.md); the screen sets its icon and action
        let (empty, empty_label) = look::empty_state("view-list-symbolic", "", None);
        let empty_icon = empty.first_child().and_downcast::<gtk::Image>().expect("the empty state starts with its icon");
        stack.add_named(&empty, Some("empty"));

        // the error child is added by `show`, with its message
        stack.add_named(list, Some("list"));
        Self { stack, empty, empty_icon, empty_label, on_retry: Rc::new(on_retry) }
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
            PageState::Error(message) => {
                if let Some(old) = self.stack.child_by_name("error") {
                    self.stack.remove(&old);
                }
                self.stack.add_named(&look::error_state(message, Some(Rc::clone(&self.on_retry))), Some("error"));
            }
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
            assert_eq!(pages.stack.visible_child_name().as_deref(), Some("error"));
            let error = pages.stack.child_by_name("error").and_then(|page| page.first_child()).and_then(|icon| icon.next_sibling()).and_downcast::<Label>().expect("the error message");
            assert_eq!(error.label(), "boom");
        });
    }
}
