//! Integer slider over `gtk::Scale` (T-49b-020): step 1, no decimals, the integer value shown.

use gtk::prelude::*;

/// The value clamped into `first..=last`. An empty range (last below first) clamps to `first`.
pub fn clamp_value(value: i64, first: i64, last: i64) -> i64 {
    value.clamp(first, last.max(first))
}

/// Steps between the ends. Zero for a single value or an empty range, which is shown disabled.
pub fn steps(first: i64, last: i64) -> i64 {
    (last - first).max(0)
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

}

#[cfg(test)]
mod tests {
    use super::*;

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

}
