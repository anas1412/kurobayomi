//! The one dialog helper every preference dialog uses: transient, modal, fixed width, Escape = Cancel,
//! and a confirm button that the caller enables or disables.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, Align, Box as GBox, Button, Label, Orientation};

use super::super::strings;
use super::DIALOG_WIDTH;

/// A dialog built by [`show`]. The caller owns the body and decides when to close it.
#[derive(Clone)]
pub struct Dialog {
    pub window: gtk::Window,
    pub confirm: Option<Button>,
    error: Label,
    done: Rc<Cell<bool>>,
}

impl Dialog {
    /// Enables or disables the confirm button. Call on every change that affects the rule.
    pub fn set_confirm_enabled(&self, enabled: bool) {
        if let Some(confirm) = &self.confirm {
            confirm.set_sensitive(enabled);
        }
    }

    /// Presses the confirm button when it is enabled (Ctrl+Enter in a multi-line field).
    pub fn submit(&self) {
        if let Some(confirm) = self.confirm.as_ref().filter(|c| c.is_sensitive()) {
            confirm.emit_clicked();
        }
    }

    /// Shows `message` under the body (a failed or vetoed-with-error write), or hides it for `None`.
    pub fn set_error(&self, message: Option<&str>) {
        self.error.set_text(message.unwrap_or(""));
        self.error.set_visible(message.is_some());
    }

    /// Closes the dialog after an accepted write. The cancel callback does not run.
    pub fn finish(&self) {
        self.done.set(true);
        self.window.destroy();
    }
}

/// Return on the main block or the keypad.
pub fn is_enter(key: gtk::gdk::Key) -> bool {
    matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter | gtk::gdk::Key::ISO_Enter)
}

/// Ctrl+Enter, the submit chord of a multi-line field where a bare Enter adds a line.
pub fn is_ctrl_enter(key: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool {
    is_enter(key) && state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
}

/// Builds and shows a dialog. `confirm` is the confirm label, or `None` for a Cancel-only dialog.
/// `on_confirm` gets the dialog so it can run a veto and call [`Dialog::finish`] on acceptance.
/// `on_cancel` runs once, on Escape, Cancel or the window close button, never after `finish`.
pub fn show(
    parent: &impl IsA<gtk::Window>,
    title: &str,
    body: &impl IsA<gtk::Widget>,
    confirm: Option<&str>,
    on_confirm: impl Fn(Dialog) + 'static,
    on_cancel: impl Fn() + 'static,
) -> Dialog {
    let window = gtk::Window::builder()
        .transient_for(parent)
        .modal(true)
        .resizable(false)
        .default_width(DIALOG_WIDTH)
        .title(title)
        .build();
    let done = Rc::new(Cell::new(false));
    let on_cancel: Rc<dyn Fn()> = Rc::new(on_cancel);
    let cancel: Rc<dyn Fn()> = {
        let (done, window, on_cancel) = (done.clone(), window.clone(), on_cancel.clone());
        Rc::new(move || {
            if !done.get() {
                done.set(true);
                on_cancel();
                window.destroy();
            }
        })
    };

    let column = GBox::new(Orientation::Vertical, 16);
    column.set_margin_top(24);
    column.set_margin_bottom(16);
    column.set_margin_start(24);
    column.set_margin_end(24);
    // the window title shows the title once; the body does not repeat it
    column.append(body);
    let error = Label::new(None);
    error.add_css_class("pref-error");
    error.set_wrap(true);
    error.set_xalign(0.0);
    error.set_visible(false);
    column.append(&error);

    let buttons = GBox::new(Orientation::Horizontal, 8);
    buttons.set_halign(Align::End);
    let cancel_button = Button::with_label(&strings::cancel());
    buttons.append(&cancel_button);
    let mut dialog = Dialog { window: window.clone(), confirm: None, error, done: done.clone() };
    if let Some(label) = confirm {
        let confirm_button = Button::with_label(label);
        confirm_button.add_css_class("suggested-action");
        buttons.append(&confirm_button);
        dialog.confirm = Some(confirm_button.clone());
        let handle = dialog.clone();
        confirm_button.connect_clicked(move |_| on_confirm(handle.clone()));
    }
    column.append(&buttons);
    window.set_child(Some(&column));

    let cancel_click = cancel.clone();
    cancel_button.connect_clicked(move |_| cancel_click());
    let (close_done, close_cancel) = (done.clone(), on_cancel.clone());
    window.connect_close_request(move |_| {
        if !close_done.get() {
            close_done.set(true);
            close_cancel();
        }
        glib::Propagation::Proceed
    });

    let keys = gtk::EventControllerKey::new();
    let cancel_key = cancel.clone();
    let confirm_key = dialog.confirm.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            cancel_key();
            return glib::Propagation::Stop;
        }
        // Enter submits an enabled dialog (T-19a-067); a field or button that handles Enter itself never gets here
        if let Some(confirm) = confirm_key.as_ref().filter(|_| is_enter(key)) {
            if confirm.is_sensitive() {
                confirm.emit_clicked();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);

    window.present();
    dialog
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How many labels at or below `root` read exactly `text`.
    fn labels_reading(root: &gtk::Widget, text: &str) -> usize {
        let mut n = usize::from(root.downcast_ref::<Label>().is_some_and(|l| l.text() == text));
        let mut child = root.first_child();
        while let Some(c) = child {
            n += labels_reading(&c, text);
            child = c.next_sibling();
        }
        n
    }

    // the dialog title is the window title only; the body does not say it a second time
    #[test]
    fn the_title_is_shown_once_in_the_window_title_and_not_repeated_in_the_body() {
        crate::testing::with_display(|| {
            let parent = gtk::Window::new();
            let body = Label::new(Some("Some text"));
            let dialog = show(&parent, "Pick one", &body, Some("OK"), |_| {}, || {});
            assert_eq!(dialog.window.title().as_deref(), Some("Pick one"));
            let content = dialog.window.child().expect("the dialog content");
            assert_eq!(labels_reading(&content, "Pick one"), 0);
            assert_eq!(labels_reading(&content, "Some text"), 1);
            dialog.window.destroy();
            parent.destroy();
        });
    }

    #[test]
    fn t_19a_067_every_enter_key_submits_and_other_keys_do_not() {
        use gtk::gdk::Key;
        assert!(is_enter(Key::Return) && is_enter(Key::KP_Enter) && is_enter(Key::ISO_Enter));
        assert!(!is_enter(Key::Escape) && !is_enter(Key::space) && !is_enter(Key::n));
    }

    #[test]
    fn ctrl_enter_is_the_multi_line_submit_chord_and_bare_enter_is_not() {
        use gtk::gdk::{Key, ModifierType};
        assert!(is_ctrl_enter(Key::Return, ModifierType::CONTROL_MASK));
        assert!(is_ctrl_enter(Key::KP_Enter, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK));
        assert!(!is_ctrl_enter(Key::Return, ModifierType::empty()));
        assert!(!is_ctrl_enter(Key::a, ModifierType::CONTROL_MASK));
    }
}
