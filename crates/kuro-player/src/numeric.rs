//! Outlined numeric chooser rules (area 46 `OutlinedNumericChooser.kt`, T-46a-032). Pure: the widget
//! feeds the entry text in and writes back what these functions return. The value is never clamped
//! here; `error` only reports that it is outside the range, and the caller decides.

use crate::errors::PlayerMessage;

/// Integer chooser. `minus` and `plus` are the repeat buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntChooser {
    min: i32,
    max: i32,
    step: i32,
}

impl IntChooser {
    /// `None` when `min` is larger than `max`, which is rejected at construction.
    pub fn new(min: i32, max: i32, step: i32) -> Option<Self> {
        (min <= max).then_some(Self { min, max, step })
    }

    /// Value minus one step. Saturates at the integer limit (E-46-024).
    pub fn minus(&self, value: i32) -> i32 {
        value.saturating_sub(self.step)
    }

    pub fn plus(&self, value: i32) -> i32 {
        value.saturating_add(self.step)
    }

    /// The value for entry text. Blank is 0. A lone `-` is 0 and the caller keeps the `-` on
    /// screen while focused (E-46-019). Invalid text is `None` and is ignored (E-46-096).
    pub fn parse(&self, text: &str) -> Option<i32> {
        if text.trim().is_empty() || text.trim_start() == "-" {
            return Some(0);
        }
        text.parse::<i32>().ok()
    }

    /// The message shown under the field when `value` is outside the range.
    pub fn error(&self, value: i32) -> Option<PlayerMessage> {
        range_error(value < self.min, value > self.max)
    }
}

/// Float chooser.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatChooser {
    min: f32,
    max: f32,
    step: f32,
}

impl FloatChooser {
    pub fn new(min: f32, max: f32, step: f32) -> Option<Self> {
        (min <= max).then_some(Self { min, max, step })
    }

    pub fn minus(&self, value: f32) -> f32 {
        value - self.step
    }

    pub fn plus(&self, value: f32) -> f32 {
        value + self.step
    }

    /// The value for entry text. Blank is 0 and a lone `-` is `-0`. A leading `.` is rejected, so
    /// `.5` cannot be typed (`None`). Other invalid text is `None`.
    pub fn parse(&self, text: &str) -> Option<f32> {
        if text.trim().is_empty() {
            return Some(0.0);
        }
        if text.trim_start() == "-" {
            return Some(-0.0);
        }
        if text.starts_with('.') {
            return None;
        }
        text.parse::<f32>().ok()
    }

    /// Entry text for a value: no trailing zeros, no trailing point (`1.50` is `1.5`, `2.0` is `2`).
    pub fn display(&self, value: f32) -> String {
        format!("{value}")
    }

    pub fn error(&self, value: f32) -> Option<PlayerMessage> {
        range_error(value < self.min, value > self.max)
    }
}

fn range_error(too_small: bool, too_big: bool) -> Option<PlayerMessage> {
    if too_big {
        Some(PlayerMessage::ValueTooBig)
    } else if too_small {
        Some(PlayerMessage::ValueTooSmall)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_greater_than_max_is_rejected() {
        assert!(IntChooser::new(5, 4, 1).is_none());
        assert!(FloatChooser::new(1.0, 0.0, 0.1).is_none());
        assert!(IntChooser::new(4, 4, 1).is_some());
    }

    #[test]
    fn steps_change_the_value_and_subtraction_saturates() {
        let c = IntChooser::new(i32::MIN, i32::MAX, 10).unwrap();
        assert_eq!(c.plus(5), 15);
        assert_eq!(c.minus(i32::MIN + 3), i32::MIN);
        assert_eq!(c.plus(i32::MAX), i32::MAX);
    }

    #[test]
    fn int_text_rules() {
        let c = IntChooser::new(0, 100, 1).unwrap();
        assert_eq!(c.parse("-"), Some(0));
        assert_eq!(c.parse(""), Some(0));
        assert_eq!(c.parse("42"), Some(42));
        assert_eq!(c.parse("abc"), None);
        assert_eq!(c.parse("1.5"), None);
    }

    #[test]
    fn float_text_rules() {
        let c = FloatChooser::new(-5.0, 5.0, 0.5).unwrap();
        assert_eq!(c.parse(".5"), None);
        assert!(c.parse("-").unwrap().is_sign_negative());
        assert_eq!(c.parse("1.25"), Some(1.25));
        assert_eq!(c.parse(""), Some(0.0));
        assert_eq!(c.display(1.5), "1.5");
        assert_eq!(c.display(2.0), "2");
    }

    #[test]
    fn out_of_range_reports_a_message_without_clamping() {
        let c = IntChooser::new(0, 10, 1).unwrap();
        assert_eq!(c.error(11), Some(PlayerMessage::ValueTooBig));
        assert_eq!(c.error(-1), Some(PlayerMessage::ValueTooSmall));
        assert_eq!(c.error(10), None);
        let f = FloatChooser::new(0.0, 1.0, 0.1).unwrap();
        assert_eq!(f.error(1.5), Some(PlayerMessage::ValueTooBig));
        assert_eq!(f.error(-0.5), Some(PlayerMessage::ValueTooSmall));
    }
}
