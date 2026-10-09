//! Labeled checkbox row (T-49a-027): full-width, at least 48 px high, the whole row toggles.
//!
//! A disabled row is insensitive, so it neither toggles nor reacts (fixes the source quirk noted in
//! `p1-behavior.md` 9, where a disabled row still changed).

use gtk::prelude::*;

pub const ROW_MIN_HEIGHT: i32 = 48;
pub const ROW_GAP: i32 = 8;

/// The value a click produces: the opposite of `checked`, or nothing when the row is disabled.
pub fn toggled_value(enabled: bool, checked: bool) -> Option<bool> {
    enabled.then_some(!checked)
}

/// A checkbox whose label is the whole clickable row (role checkbox).
pub fn labeled_checkbox(
    label: &str,
    checked: bool,
    enabled: bool,
    on_toggle: impl Fn(bool) + 'static,
) -> gtk::CheckButton {
    let check = gtk::CheckButton::with_label(label);
    check.set_active(checked);
    check.set_sensitive(enabled);
    check.set_size_request(-1, ROW_MIN_HEIGHT);
    check.set_hexpand(true);
    // The toggle already happened: the state before it is the opposite of `is_active`.
    check.connect_toggled(move |c| {
        if let Some(value) = toggled_value(enabled, !c.is_active()) {
            on_toggle(value);
        }
    });
    check
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_row_toggles_and_disabled_row_does_not() {
        assert_eq!(toggled_value(true, false), Some(true));
        assert_eq!(toggled_value(true, true), Some(false));
        assert_eq!(toggled_value(false, false), None);
    }

    #[test]
    fn row_is_at_least_48_high_with_an_8_gap() {
        assert_eq!(ROW_MIN_HEIGHT, 48);
        assert_eq!(ROW_GAP, 8);
    }
}
