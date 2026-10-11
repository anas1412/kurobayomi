//! Keyboard behaviour shared by the widgets (T-49a-055, MANUAL-49-03).
//!
//! Buttons activate on Enter and Space by themselves, so a control is built from a `gtk::Button`
//! (or `repeat_button`, which routes `activate`) and needs no key helper. These helpers cover the
//! containers: Escape to close a sheet or a dialog, and a Tab trap. Focus rings come from the user's
//! GTK theme and are not overridden here.

use gtk::glib;
use gtk::prelude::*;

/// True for the Escape key name.
pub fn is_escape_name(name: &str) -> bool {
    name == "Escape"
}

/// The focus direction a Tab-family key name asks for: Tab forward, Shift+Tab or ISO_Left_Tab back.
/// `None` for any other key.
pub fn tab_direction(name: &str, shift: bool) -> Option<gtk::DirectionType> {
    match name {
        "ISO_Left_Tab" => Some(gtk::DirectionType::TabBackward),
        "Tab" if shift => Some(gtk::DirectionType::TabBackward),
        "Tab" => Some(gtk::DirectionType::TabForward),
        _ => None,
    }
}

/// Keeps Tab and Shift+Tab inside `scope`: when focus reaches the last (or first) control it wraps
/// to the other end instead of leaving (a focus trap for sheets and dialogs, T-49a-055).
pub fn trap_tab_focus(scope: &impl IsA<gtk::Widget>) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = scope.as_ref().downgrade();
    keys.connect_key_pressed(move |_, keyval, _, state| {
        let shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
        let Some(dir) = keyval.name().and_then(|n| tab_direction(&n, shift)) else {
            return glib::Propagation::Proceed;
        };
        let Some(scope) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if !scope.child_focus(dir) {
            if let Some(root) = scope.root() {
                root.set_focus(None::<&gtk::Widget>);
            }
            scope.child_focus(dir);
        }
        glib::Propagation::Stop
    });
    scope.as_ref().add_controller(keys);
}

/// Calls `on_close` when Escape is pressed inside `widget` (a sheet, a dialog or a window).
pub fn close_on_escape<F: Fn() + 'static>(widget: &impl IsA<gtk::Widget>, on_close: F) {
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, keyval, _, _| {
        if keyval.name().is_some_and(|n| is_escape_name(&n)) {
            on_close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    widget.as_ref().add_controller(keys);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_family_keys_pick_a_direction() {
        assert_eq!(tab_direction("Tab", false), Some(gtk::DirectionType::TabForward));
        assert_eq!(tab_direction("Tab", true), Some(gtk::DirectionType::TabBackward));
        assert_eq!(tab_direction("ISO_Left_Tab", true), Some(gtk::DirectionType::TabBackward));
        assert_eq!(tab_direction("Return", false), None);
    }

    #[test]
    fn escape_is_its_own_key() {
        assert!(is_escape_name("Escape"));
        assert!(!is_escape_name("Return"));
    }
}
