//! Integer slider over `gtk::Scale` (T-49b-020): step 1, no decimals, the integer value shown.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

/// Rounds half up, as the source slider does: `-2.5` gives `-2`, `2.5` gives `3`.
pub fn round_half_up(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

/// The value clamped into `first..=last`. An empty range (last below first) clamps to `first`.
pub fn clamp_value(value: i64, first: i64, last: i64) -> i64 {
    value.clamp(first, last.max(first))
}

/// Steps between the ends. Zero for a single value or an empty range, which is shown disabled.
pub fn steps(first: i64, last: i64) -> i64 {
    (last - first).max(0)
}

/// The finished event fires on release only when the value differs from the value at press.
pub fn finished_changed(at_press: i64, at_release: i64) -> bool {
    at_press != at_release
}

/// An integer slider. `scale` is the widget to pack.
pub struct IntSlider {
    pub scale: gtk::Scale,
}

impl IntSlider {
    /// A slider over `first..=last` showing `value` (clamped into the range).
    pub fn new(first: i64, last: i64, value: i64) -> Self {
        let scale = gtk::Scale::with_range(
            gtk::Orientation::Horizontal,
            first as f64,
            last.max(first) as f64,
            1.0,
        );
        scale.set_round_digits(0);
        scale.set_draw_value(true);
        scale.set_value(clamp_value(value, first, last) as f64);
        scale.set_sensitive(steps(first, last) > 0);
        IntSlider { scale }
    }

    /// The current integer value.
    pub fn value(&self) -> i64 {
        round_half_up(self.scale.value())
    }

    /// Calls `on_finished` with the new value on release, only when the value changed since the last
    /// finish (or since construction). The reference is kept here, not read at press: GtkRange may
    /// move the value on press, and a click-to-jump must still count as a change.
    pub fn on_finished<F: Fn(i64) + 'static>(&self, on_finished: F) {
        let settled = Rc::new(Cell::new(self.value()));
        let gesture = gtk::GestureClick::new();
        // capture phase: the range may claim the sequence, and the release must still reach us
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        let scale = self.scale.clone();
        gesture.connect_released(move |_, _, _, _| {
            let now = round_half_up(scale.value());
            if finished_changed(settled.get(), now) {
                settled.set(now);
                on_finished(now);
            }
        });
        self.scale.add_controller(gesture);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_values_round_up() {
        assert_eq!(round_half_up(-2.5), -2);
        assert_eq!(round_half_up(2.5), 3);
        assert_eq!(round_half_up(0.49), 0);
    }

    #[test]
    fn values_clamp_into_the_range() {
        assert_eq!(clamp_value(15, 0, 10), 10);
        assert_eq!(clamp_value(-3, 1, 10), 1);
        assert_eq!(clamp_value(5, 3, 3), 3);
    }

    #[test]
    fn single_value_and_empty_ranges_have_no_steps() {
        assert_eq!(steps(4, 4), 0);
        assert_eq!(steps(5, 5), 0);
        assert_eq!(steps(9, 2), 0);
        assert_eq!(steps(0, 10), 10);
    }

    #[test]
    fn the_five_range_shapes_of_the_app() {
        assert_eq!(steps(0, 10), 10);
        assert_eq!(steps(1, 15), 14);
        assert_eq!(steps(1, 10), 9);
        assert_eq!(steps(0, 200), 200);
        assert_eq!(steps(0, 100), 100);
    }

    #[test]
    fn finished_fires_only_on_a_change() {
        assert!(finished_changed(3, 4));
        assert!(!finished_changed(3, 3));
    }
}
