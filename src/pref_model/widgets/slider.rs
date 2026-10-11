//! Slider row (Kotlin `BaseSliderItem` on the preference path): label left, value pill right, full-width
//! scale below. The caller owns the value; a write goes through the veto, and the pill follows only accepted values.

use std::cell::Cell;
use std::ops::RangeInclusive;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, Align, Box as GBox, Label, ListBoxRow, Orientation, Scale};

use super::super::bind::{self, Veto, WriteGate};
use super::super::model::ItemCommon;
use super::{PREF_HORIZONTAL_PADDING, SLIDER_ROW_PADDING};

/// The value clamped into `range` (E-38). Applied before display and before the write.
pub fn clamp_to_range(value: i32, range: &RangeInclusive<i32>) -> i32 {
    value.clamp(*range.start(), *range.end())
}

/// Pill text: the subtitle when it is not empty, otherwise the number (E-40).
pub fn pill_text(subtitle: Option<&str>, value: i32) -> String {
    match subtitle {
        Some(text) if !text.is_empty() => text.to_owned(),
        _ => value.to_string(),
    }
}

/// The distance between two stops of a slider with `steps` stops between its ends (Material `steps`): `(last - first)
/// / (steps + 1)`. The default `steps` of the model, `last - first - 1`, gives 1.
pub fn step_size(range: &RangeInclusive<i32>, steps: u32) -> f64 {
    let span = f64::from(*range.end()) - f64::from(*range.start());
    (span / (f64::from(steps) + 1.0)).max(1.0)
}

/// The stop nearest to `value`, counted from the start of the range, kept inside it.
pub fn snap_to_step(value: f64, range: &RangeInclusive<i32>, steps: u32) -> i32 {
    let (first, step) = (f64::from(*range.start()), step_size(range, steps));
    clamp_to_range((first + ((value - first) / step).round() * step).round() as i32, range)
}

/// A drag that lands on the current value is not a change and does not write (E-39).
pub fn is_change(current: i32, next: i32) -> bool {
    current != next
}

pub fn row(common: &ItemCommon, value: i32, range: RangeInclusive<i32>, steps: u32, on_change: Veto<i32>) -> ListBoxRow {
    let start = clamp_to_range(value, &range);
    let current = Rc::new(Cell::new(start));
    let suppress = Rc::new(Cell::new(false));
    let gate = Rc::new(WriteGate::default());

    let outer = GBox::new(Orientation::Vertical, 2);
    outer.set_margin_start(SLIDER_ROW_PADDING);
    outer.set_margin_end(SLIDER_ROW_PADDING);
    outer.set_margin_top(SLIDER_ROW_PADDING);
    outer.set_margin_bottom(SLIDER_ROW_PADDING);
    let header = GBox::new(Orientation::Horizontal, 8);
    let label = Label::new(Some(&common.title));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    label.set_halign(Align::Start);
    label.add_css_class("pref-title");
    header.append(&label);
    let pill = Label::new(Some(&pill_text(common.subtitle.as_deref(), start)));
    pill.add_css_class("pref-slider-pill");
    pill.add_css_class("dim-label");
    header.append(&pill);
    outer.append(&header);

    let step = step_size(&range, steps);
    let scale = Scale::with_range(Orientation::Horizontal, f64::from(*range.start()), f64::from(*range.end()), step);
    scale.set_draw_value(false);
    scale.set_round_digits(0);
    scale.set_hexpand(true);
    scale.set_value(f64::from(start));
    scale.set_sensitive(common.enabled);
    scale.set_margin_start(0);
    scale.set_margin_end(PREF_HORIZONTAL_PADDING - SLIDER_ROW_PADDING);
    outer.append(&scale);

    let subtitle = common.subtitle.clone();
    scale.connect_value_changed({
        let (current, suppress, gate, pill) = (current.clone(), suppress.clone(), gate.clone(), pill.clone());
        move |scale| {
            if suppress.get() {
                return;
            }
            let next = snap_to_step(scale.value(), &range, steps);
            if !is_change(current.get(), next) {
                return;
            }
            let ticket = gate.begin();
            let (current, gate, pill, subtitle, scale, suppress, on_change) = (
                current.clone(),
                gate.clone(),
                pill.clone(),
                subtitle.clone(),
                scale.clone(),
                suppress.clone(),
                on_change.clone(),
            );
            glib::spawn_future_local(async move {
                let decision = bind::decide(on_change(next).await, gate.is_latest(ticket));
                match decision {
                    Ok(true) => {
                        current.set(next);
                        pill.set_text(&pill_text(subtitle.as_deref(), next));
                    }
                    Ok(false) | Err(_) => {
                        suppress.set(true);
                        scale.set_value(f64::from(current.get()));
                        suppress.set(false);
                    }
                }
            });
        }
    });

    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&outer));
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_outside_the_range_are_clamped() {
        assert_eq!(clamp_to_range(12, &(0..=10)), 10);
        assert_eq!(clamp_to_range(-3, &(0..=10)), 0);
    }

    #[test]
    fn pill_falls_back_to_the_number_when_the_subtitle_is_empty() {
        assert_eq!(pill_text(Some(""), 7), "7");
        assert_eq!(pill_text(None, 7), "7");
        assert_eq!(pill_text(Some("ms"), 7), "ms");
    }

    // T-47ba-052: the model's `steps` set the stops of the scale; the default is every whole number.
    #[test]
    fn steps_set_the_stops_and_a_drag_snaps_to_one() {
        assert_eq!(step_size(&(0..=10), 9), 1.0, "the model default");
        assert_eq!(step_size(&(0..=100), 9), 10.0);
        assert_eq!(snap_to_step(34.0, &(0..=100), 9), 30);
        assert_eq!(snap_to_step(36.0, &(0..=100), 9), 40);
        assert_eq!(snap_to_step(7.4, &(0..=10), 9), 7);
        assert_eq!(snap_to_step(500.0, &(0..=100), 9), 100, "inside the range");
        assert_eq!(step_size(&(5..=5), 0), 1.0, "a one-value range does not divide by zero");
    }

    #[test]
    fn equal_values_are_not_a_change() {
        assert!(!is_change(4, 4));
        assert!(is_change(4, 5));
    }
}
