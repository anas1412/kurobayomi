//! Numeric chooser (T-49a-042): minus and plus repeat buttons around a number entry, with an
//! optional suffix. Buttons step by `step` and clamp to `min`. Typed text keeps digits and a leading
//! minus, and is clamped to `min` only on commit. Deviation from the source (E-49-20, E-49-21): an
//! intermediate "-" is allowed, and a value that is not a number reverts to the last valid one.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

use super::repeat_button::repeat_button;

pub const FIELD_MIN_WIDTH: i32 = 140;

/// `value + delta`, clamped to `min` when there is one (E-49-19).
pub fn step_value(value: i32, delta: i32, min: Option<i32>) -> i32 {
    let next = value.saturating_add(delta);
    match min {
        Some(min) => next.max(min),
        None => next,
    }
}

/// The text kept while typing: a leading minus and the digits, everything else dropped.
pub fn sanitize_typed(text: &str) -> String {
    let negative = text.starts_with('-');
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    if negative { format!("-{digits}") } else { digits }
}

/// The number a text stands for, or `None` for an intermediate or empty text such as "-".
pub fn parse_typed(text: &str) -> Option<i32> {
    let sanitized = sanitize_typed(text);
    let (negative, digits) = match sanitized.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, sanitized.as_str()),
    };
    if digits.is_empty() {
        return None;
    }
    let magnitude: i64 = digits.parse().ok()?;
    i32::try_from(if negative { -magnitude } else { magnitude }).ok()
}

/// The value to keep when the text is committed: the typed number clamped to `min`, or the
/// previous value when the text is not a number (E-49-20, E-49-21).
pub fn commit_typed(text: &str, previous: i32, min: Option<i32>) -> i32 {
    match parse_typed(text) {
        Some(value) => match min {
            Some(min) => value.max(min),
            None => value,
        },
        None => previous,
    }
}

/// The chooser. `on_change` fires only when the value differs from the last one (E-49-25).
pub fn numeric_chooser(
    value: i32,
    step: i32,
    min: Option<i32>,
    suffix: Option<&str>,
    placeholder: Option<&str>,
    on_change: impl Fn(i32) + 'static,
) -> gtk::Box {
    let current = Rc::new(Cell::new(value));
    let on_change: Rc<dyn Fn(i32)> = Rc::new(on_change);
    let entry = gtk::Entry::new();
    entry.set_input_purpose(gtk::InputPurpose::Digits);
    entry.set_size_request(FIELD_MIN_WIDTH, -1);
    entry.set_text(&value.to_string());
    if let Some(placeholder) = placeholder {
        entry.set_placeholder_text(Some(placeholder));
    }

    let set: Rc<dyn Fn(i32)> = {
        let entry = entry.clone();
        let current = current.clone();
        let on_change = on_change.clone();
        Rc::new(move |next: i32| {
            entry.set_text(&next.to_string());
            if next != current.get() {
                current.set(next);
                on_change(next);
            }
        })
    };

    let minus = repeat_button("list-remove-symbolic", {
        let set = set.clone();
        let current = current.clone();
        move || set(step_value(current.get(), -step, min))
    });
    let plus = repeat_button("list-add-symbolic", {
        let set = set.clone();
        let current = current.clone();
        move || set(step_value(current.get(), step, min))
    });

    {
        let set = set.clone();
        let current = current.clone();
        entry.connect_activate(move |e| set(commit_typed(&e.text(), current.get(), min)));
    }
    {
        let set = set.clone();
        let current = current.clone();
        let focus = gtk::EventControllerFocus::new();
        let entry_weak = entry.downgrade();
        focus.connect_leave(move |_| {
            if let Some(e) = entry_weak.upgrade() {
                set(commit_typed(&e.text(), current.get(), min));
            }
        });
        entry.add_controller(focus);
    }
    entry.connect_changed(|e| {
        let text = e.text();
        let clean = sanitize_typed(&text);
        if clean != text.as_str() {
            e.set_text(&clean);
        }
    });

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.append(&minus);
    row.append(&entry);
    if let Some(suffix) = suffix {
        row.append(&gtk::Label::new(Some(suffix)));
    }
    row.append(&plus);
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_clamps_to_the_minimum() {
        // E-49-19
        assert_eq!(step_value(0, -1, Some(0)), 0);
        assert_eq!(step_value(5, -1, Some(0)), 4);
        assert_eq!(step_value(5, 2, None), 7);
    }

    // TEST-49-16
    #[test]
    fn typed_text_keeps_digits_and_a_leading_minus() {
        assert_eq!(parse_typed("abc1-2"), Some(12));
        assert_eq!(sanitize_typed("-x5"), "-5");
        assert_eq!(parse_typed("-"), None);
        assert_eq!(parse_typed(""), None);
        assert_eq!(parse_typed("-42"), Some(-42));
    }

    #[test]
    fn commit_clamps_and_reverts_a_non_number() {
        // E-49-20, E-49-21
        assert_eq!(commit_typed("-3", 7, Some(0)), 0);
        assert_eq!(commit_typed("", 7, Some(0)), 7);
        assert_eq!(commit_typed("15", 7, Some(0)), 15);
    }

    #[test]
    fn overflowing_text_is_not_a_number() {
        assert_eq!(parse_typed("99999999999999"), None);
    }
}
