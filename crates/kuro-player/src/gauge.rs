//! Gauge math for the vertical sliders (area 46 `SliderItem.kt` and `VerticalSliders.kt`, T-46a-037).
//! Pure: the GTK layer animates the returned fractions and draws the overflow above the main fill.

/// Fraction of `value` within `range`, clamped to 0..=1. An empty or inverted range gives 0
/// (E-46-014), so a zero maximum never divides by zero.
pub fn percentage(value: f32, range: (f32, f32)) -> f32 {
    let span = range.1 - range.0;
    if !(span > 0.0) {
        return 0.0;
    }
    ((value - range.0) / span).clamp(0.0, 1.0)
}

/// The two stacked fills of a gauge: the main fill and the error-coloured overflow above it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gauge {
    pub fill: f32,
    pub overflow: f32,
}

/// Fill fractions for a value and its overflow. The overflow has its own range (for example
/// boost above 100 percent, or the negative brightness range).
pub fn gauge(value: f32, range: (f32, f32), overflow: f32, overflow_range: (f32, f32)) -> Gauge {
    Gauge {
        fill: percentage(value, range),
        overflow: percentage(overflow, overflow_range),
    }
}

/// Width and height of the rotated horizontal slider control (`SliderItem.kt:212-213`), in pixels.
/// The control is rotated by `VERTICAL_SLIDER_ROTATION_DEG`.
pub const VERTICAL_SLIDER_SIZE: (i32, i32) = (180, 50);
pub const VERTICAL_SLIDER_ROTATION_DEG: u32 = 270;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentage_is_the_fraction_of_the_range() {
        assert_eq!(percentage(5.0, (0.0, 10.0)), 0.5);
        assert_eq!(percentage(0.3, (0.0, 1.0)), 0.3);
    }

    #[test]
    fn percentage_clamps_outside_the_range() {
        assert_eq!(percentage(-4.0, (0.0, 10.0)), 0.0);
        assert_eq!(percentage(14.0, (0.0, 10.0)), 1.0);
    }

    #[test]
    fn empty_range_is_zero_not_a_division() {
        assert_eq!(percentage(3.0, (0.0, 0.0)), 0.0);
        assert_eq!(percentage(3.0, (5.0, 1.0)), 0.0);
    }

    #[test]
    fn overflow_has_its_own_range() {
        let g = gauge(1.0, (0.0, 1.0), -0.4, (0.0, 0.75));
        assert_eq!(g.fill, 1.0);
        assert_eq!(g.overflow, 0.0);
        let boosted = gauge(100.0, (0.0, 100.0), 15.0, (0.0, 30.0));
        assert_eq!(boosted.overflow, 0.5);
    }
}
