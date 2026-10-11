//! Settings rows (T-49a-036 to T-49a-039, T-49a-045). Each row is a `gtk::ListBoxRow` inside a
//! `.boxed-list`, with 24 px side and 10 px vertical padding and a 24 px gap.
//!
//! Pure rules first: the slider step count, change detection, the select index, and the sort icon.
//! The builders below them only wire widgets to those rules.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

use super::classes;
use crate::theme::tokens::{SETTINGS_PAD_H, SETTINGS_PAD_V};

/// Gap between the label and the trailing widget, px.
const ROW_GAP: i32 = 24;

/// Slider steps: the explicit count, or `(last - first) - 1`, never negative (E-49-32).
pub fn slider_steps(first: i32, last: i32, steps: Option<i32>) -> i32 {
    steps.unwrap_or(last - first - 1).max(0)
}

/// The step count of a player Int slider: `max - min` (T-46a-035). It is not the settings default
/// `(max - min) - 1` above, so the player sliders get their own builder.
pub fn int_slider_steps(min: i32, max: i32) -> i32 {
    (max - min).max(0)
}

/// Whether a slider change should call `on_change` (E-49-25: not when the value is unchanged).
pub fn slider_changed(previous: i32, next: i32) -> bool {
    previous != next
}

/// The default slider pill text: the value itself.
pub fn default_value_text(value: i32) -> String {
    value.to_string()
}

/// The new state to show when `current` differs from what the check box `shown` (None: no change).
pub fn external_change(shown: bool, current: bool) -> Option<bool> {
    (shown != current).then_some(current)
}

/// The select index when it is in range; an out-of-range index shows an empty value (E-49-22).
pub fn selected_option(len: usize, index: usize) -> Option<usize> {
    (index < len).then_some(index)
}

/// The sort icon: down for `true`, up for `false`, and none for `None` (a blank 24 px spacer).
pub fn sort_icon(desc: Option<bool>) -> Option<&'static str> {
    match desc {
        Some(true) => Some("view-sort-descending-symbolic"),
        Some(false) => Some("view-sort-ascending-symbolic"),
        None => None,
    }
}

/// The shared row shell: label on the left, `trailing` on the right, activatable by Enter and Space.
pub fn row(label: &str, trailing: Option<&gtk::Widget>, activatable: bool) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class(classes::SETTINGS_ROW);
    row.set_activatable(activatable);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, ROW_GAP);
    line.set_margin_start(SETTINGS_PAD_H as i32);
    line.set_margin_end(SETTINGS_PAD_H as i32);
    line.set_margin_top(SETTINGS_PAD_V as i32);
    line.set_margin_bottom(SETTINGS_PAD_V as i32);
    line.set_valign(gtk::Align::Center);
    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    text.set_wrap(true);
    line.append(&text);
    if let Some(widget) = trailing {
        line.append(widget);
    }
    row.set_child(Some(&line));
    row
}

/// A header row: a label in the heading style, not activatable (24/10 px).
pub fn heading_row(text: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    let label = gtk::Label::new(Some(text));
    label.add_css_class(classes::SETTINGS_HEADING);
    label.set_xalign(0.0);
    label.set_margin_start(SETTINGS_PAD_H as i32);
    label.set_margin_end(SETTINGS_PAD_H as i32);
    label.set_margin_top(SETTINGS_PAD_V as i32);
    label.set_margin_bottom(SETTINGS_PAD_V as i32);
    row.set_child(Some(&label));
    row
}

/// A row with a primary-coloured icon; activation runs `on_click`.
pub fn icon_row(label: &str, icon_name: &str, on_click: impl Fn() + 'static) -> gtk::ListBoxRow {
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.add_css_class(classes::SETTINGS_ICON);
    let row = row(label, Some(icon.upcast_ref()), true);
    crate::widgets::row_activate::on_activate(&row, move |_| on_click());
    row
}

/// A row with the sort arrow for `desc` (see `sort_icon`).
pub fn sort_row(label: &str, desc: Option<bool>, on_click: impl Fn() + 'static) -> gtk::ListBoxRow {
    let trailing: gtk::Widget = match sort_icon(desc) {
        Some(name) => gtk::Image::from_icon_name(name).upcast(),
        None => {
            let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            spacer.set_size_request(24, 24);
            spacer.upcast()
        }
    };
    let row = row(label, Some(&trailing), true);
    crate::widgets::row_activate::on_activate(&row, move |_| on_click());
    row
}

/// A checkbox row bound to a preference. `get` reads the current value. Activation flips the box
/// and calls `on_toggle` with the new value. `subscribe` is called once with a `refresh` closure:
/// register it on the preference's change signal, and the box follows external changes through
/// `set_active` without calling `on_toggle` again (the `collectAsState` equivalent). `refresh`
/// holds the check box weakly, so a dropped row does not stay alive through the subscription.
pub fn checkbox_row(
    label: &str,
    get: impl Fn() -> bool + 'static,
    on_toggle: impl Fn(bool) + 'static,
    subscribe: impl FnOnce(Box<dyn Fn()>),
) -> gtk::ListBoxRow {
    let check = gtk::CheckButton::new();
    check.set_active(get());
    check.set_sensitive(false);
    let get = Rc::new(get);
    {
        let weak = check.downgrade();
        let get = get.clone();
        subscribe(Box::new(move || {
            if let Some(check) = weak.upgrade() {
                if let Some(next) = external_change(check.is_active(), get()) {
                    check.set_active(next);
                }
            }
        }));
    }
    let row = row(label, Some(check.upcast_ref()), true);
    crate::widgets::row_activate::on_activate(&row, move |_| {
        let next = !check.is_active();
        check.set_active(next);
        on_toggle(next);
    });
    row
}

/// A radio row. The check control is insensitive; activation calls `on_select`.
pub fn radio_row(label: &str, selected: bool, on_select: impl Fn() + 'static) -> gtk::ListBoxRow {
    let check = gtk::CheckButton::new();
    check.set_active(selected);
    check.set_sensitive(false);
    let row = row(label, Some(check.upcast_ref()), true);
    crate::widgets::row_activate::on_activate(&row, move |_| on_select());
    row
}

/// A slider row: the label and a value pill on top, a `gtk::Scale` below. The pill shows
/// `value_text(value)` at construction and after every change (pass `default_value_text` for the
/// plain number). Changes of the value call `on_change` only when the value differs from the last
/// one (E-49-25).
pub fn slider_row(
    label: &str,
    first: i32,
    last: i32,
    value: i32,
    steps: Option<i32>,
    value_text: impl Fn(i32) -> String + 'static,
    on_change: impl Fn(i32) + 'static,
) -> gtk::ListBoxRow {
    let pill = gtk::Label::new(Some(&value_text(value)));
    pill.add_css_class(classes::PILL);
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, first as f64, last as f64, 1.0);
    scale.set_round_digits(0);
    scale.set_draw_value(false);
    scale.set_value(value as f64);
    let count = slider_steps(first, last, steps);
    if steps.is_some() {
        // Tick marks at the step boundaries (count + 1 intervals).
        let span = (last - first) as f64;
        for i in 0..=count {
            let at = first as f64 + span * (i + 1) as f64 / (count + 1) as f64;
            scale.add_mark(at.round(), gtk::PositionType::Bottom, None);
        }
    }

    let prev = Rc::new(Cell::new(value));
    {
        let prev = prev.clone();
        let pill = pill.clone();
        scale.connect_value_changed(move |s| {
            let next = s.value().round() as i32;
            if slider_changed(prev.get(), next) {
                prev.set(next);
                pill.set_label(&value_text(next));
                on_change(next);
            }
        });
    }

    let top = gtk::Box::new(gtk::Orientation::Horizontal, ROW_GAP);
    let name = gtk::Label::new(Some(label));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    top.append(&name);
    top.append(&pill);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
    column.append(&top);
    column.append(&scale);
    column.set_margin_start(SETTINGS_PAD_H as i32);
    column.set_margin_end(SETTINGS_PAD_H as i32);
    column.set_margin_top(SETTINGS_PAD_V as i32);
    column.set_margin_bottom(SETTINGS_PAD_V as i32);
    let row = gtk::ListBoxRow::new();
    row.add_css_class(classes::SETTINGS_ROW);
    row.set_activatable(false);
    row.set_child(Some(&column));
    row
}

/// The player SwitchRow (T-46a-035): the label and a switch, and a click anywhere on the row flips the switch and
/// calls `on_toggle` with the new state. The switch itself is not focusable, so the row is the one focus stop.
pub fn player_switch_row(label: &str, active: bool, on_toggle: impl Fn(bool) + 'static) -> gtk::ListBoxRow {
    let switch = gtk::Switch::new();
    switch.set_active(active);
    switch.set_valign(gtk::Align::Center);
    switch.set_focusable(false);
    switch.set_can_target(false);
    switch.update_property(&[gtk::accessible::Property::Label(label)]);
    let row = row(label, Some(switch.upcast_ref()), true);
    crate::widgets::row_activate::on_activate(&row, move |_| {
        let next = !switch.is_active();
        switch.set_active(next);
        on_toggle(next);
    });
    row
}

/// The player Int SliderRow (T-46a-035): label, value pill and a slider with `max - min` discrete steps that calls
/// `on_change` only when the integer value changes.
pub fn player_int_slider_row(
    label: &str,
    min: i32,
    max: i32,
    value: i32,
    value_text: impl Fn(i32) -> String + 'static,
    on_change: impl Fn(i32) + 'static,
) -> gtk::ListBoxRow {
    slider_row(label, min, max, value, Some(int_slider_steps(min, max)), value_text, on_change)
}

/// A drop-down row. An out-of-range `selected` shows an empty value and logs a warning (E-49-22).
/// `on_select` runs once per change, with the new index.
pub fn select_row(
    label: &str,
    options: &[String],
    selected: usize,
    on_select: impl Fn(usize) + 'static,
) -> gtk::ListBoxRow {
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    let dropdown = gtk::DropDown::from_strings(&refs);
    match selected_option(options.len(), selected) {
        Some(index) => dropdown.set_selected(index as u32),
        None => {
            tracing::warn!(selected, len = options.len(), "select index out of range");
            dropdown.set_selected(gtk::INVALID_LIST_POSITION);
        }
    }
    let prev = Rc::new(Cell::new(dropdown.selected()));
    dropdown.connect_selected_notify(move |d| {
        let next = d.selected();
        if next != gtk::INVALID_LIST_POSITION && next != prev.get() {
            prev.set(next);
            on_select(next as usize);
        }
    });
    row(label, Some(dropdown.upcast_ref()), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_default_to_the_range_minus_one_and_never_go_negative() {
        assert_eq!(slider_steps(0, 10, None), 9);
        assert_eq!(slider_steps(0, 0, None), 0);
        assert_eq!(slider_steps(5, 5, None), 0);
        assert_eq!(slider_steps(0, 3, Some(4)), 4);
    }

    #[test]
    fn t_46a_035_the_player_int_slider_has_max_minus_min_steps() {
        assert_eq!(int_slider_steps(0, 10), 10);
        assert_eq!(int_slider_steps(-100, 100), 200);
        assert_eq!(int_slider_steps(5, 5), 0);
        assert_ne!(int_slider_steps(0, 10), slider_steps(0, 10, None));
    }

    #[test]
    fn t_46a_035_a_row_click_flips_the_player_switch_and_reports_it() {
        crate::testing::with_display(|| {
            let seen = Rc::new(Cell::new(None));
            let row = {
                let seen = Rc::clone(&seen);
                player_switch_row("Auto-play", false, move |on| seen.set(Some(on)))
            };
            let list = gtk::ListBox::new();
            list.append(&row);
            crate::widgets::row_activate::click_row(&row);
            assert_eq!(seen.get(), Some(true));
            row.emit_by_name::<()>("activate", &[]);
            assert_eq!(seen.get(), Some(false));
        });
    }

    #[test]
    fn unchanged_slider_value_does_not_report() {
        assert!(!slider_changed(3, 3));
        assert!(slider_changed(3, 4));
    }

    #[test]
    fn default_pill_text_is_the_number() {
        assert_eq!(default_value_text(-3), "-3");
        assert_eq!(default_value_text(10), "10");
    }

    #[test]
    fn external_change_reports_only_a_real_difference() {
        assert_eq!(external_change(false, true), Some(true));
        assert_eq!(external_change(true, false), Some(false));
        assert_eq!(external_change(true, true), None);
        assert_eq!(external_change(false, false), None);
    }

    #[test]
    fn select_index_must_be_in_range() {
        assert_eq!(selected_option(3, 2), Some(2));
        assert_eq!(selected_option(3, 3), None);
        assert_eq!(selected_option(0, 0), None);
    }

    #[test]
    fn sort_icon_follows_the_three_states() {
        assert_eq!(sort_icon(Some(true)), Some("view-sort-descending-symbolic"));
        assert_eq!(sort_icon(Some(false)), Some("view-sort-ascending-symbolic"));
        assert_eq!(sort_icon(None), None);
    }
}
