//! Window size classes (T-49a-021, `WindowSize.kt`): Compact, Medium, Expanded, by width in dp.
//!
//! The thresholds are strict greater-than (E-49-13): 600 dp is Compact, 840 dp is Medium. The
//! width is the content width in device pixels over the scale factor. Only class changes are
//! reported, and a zero width (startup, before the first layout) is ignored (F-49-05).

use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use super::tokens::{EXPANDED_WIDTH_WINDOW_SIZE, MEDIUM_WIDTH_WINDOW_SIZE};

/// The width class of a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowClass {
    Compact,
    Medium,
    Expanded,
}

/// The class of a width in dp. Strict greater-than at 600 and 840.
pub fn classify(width_dp: f32) -> WindowClass {
    if is_expanded_width(width_dp) {
        WindowClass::Expanded
    } else if is_medium_width(width_dp) {
        WindowClass::Medium
    } else {
        WindowClass::Compact
    }
}

/// `w > 600` dp (`WindowSize.kt`, strict as in Compose). Used by the sidebar label rule (T-49b-017)
/// and the two-pane layouts (T-49b-042).
pub fn is_medium_width(width_dp: f32) -> bool {
    width_dp > MEDIUM_WIDTH_WINDOW_SIZE
}

/// `w > 840` dp (strict as in Compose). The expanded layouts start here.
pub fn is_expanded_width(width_dp: f32) -> bool {
    width_dp > EXPANDED_WIDTH_WINDOW_SIZE
}

/// Width in dp from device pixels and the scale. `None` for a zero or negative width.
pub fn width_dp(width_px: i32, scale: i32) -> Option<f32> {
    (width_px > 0 && scale > 0).then(|| width_px as f32 / scale as f32)
}

/// Remembers the last class so only changes are reported.
#[derive(Default)]
pub struct ClassTracker(Option<WindowClass>);

impl ClassTracker {
    /// `Some(class)` when the class changed (or the first valid width arrived), else `None`.
    pub fn update(&mut self, width_px: i32, scale: i32) -> Option<WindowClass> {
        let class = classify(width_dp(width_px, scale)?);
        if self.0 == Some(class) {
            return None;
        }
        self.0 = Some(class);
        Some(class)
    }
}

/// Calls `on_change` whenever the window's class changes. Checked once per frame, which also
/// catches resizes that leave `default-width` unchanged.
pub fn attach<F: Fn(WindowClass) + 'static>(window: &impl IsA<gtk::Window>, on_change: F) {
    let tracker = Rc::new(RefCell::new(ClassTracker::default()));
    window.as_ref().add_tick_callback(move |widget, _clock| {
        if let Some(class) = tracker.borrow_mut().update(widget.width(), widget.scale_factor()) {
            on_change(class);
        }
        glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_are_strict_greater_than() {
        // TEST-49-10 and E-49-13
        assert_eq!(classify(0.0), WindowClass::Compact);
        assert_eq!(classify(599.0), WindowClass::Compact);
        assert_eq!(classify(600.0), WindowClass::Compact);
        assert_eq!(classify(601.0), WindowClass::Medium);
        assert_eq!(classify(840.0), WindowClass::Medium);
        assert_eq!(classify(841.0), WindowClass::Expanded);
        assert_eq!(classify(2000.0), WindowClass::Expanded);
    }

    // T-49b-042: the breakpoint helpers are strict, and they agree with `classify`.
    #[test]
    fn breakpoint_helpers_are_strict() {
        assert!(!is_medium_width(600.0));
        assert!(is_medium_width(600.5));
        assert!(!is_expanded_width(840.0));
        assert!(is_expanded_width(840.5));
        for w in [0.0, 600.0, 600.5, 840.0, 840.5, 2000.0] {
            let expected = if is_expanded_width(w) {
                WindowClass::Expanded
            } else if is_medium_width(w) {
                WindowClass::Medium
            } else {
                WindowClass::Compact
            };
            assert_eq!(classify(w), expected, "{w}");
        }
    }

    #[test]
    fn width_is_divided_by_the_scale_and_zero_is_ignored() {
        assert_eq!(width_dp(1200, 2), Some(600.0));
        assert_eq!(width_dp(0, 1), None);
        assert_eq!(width_dp(800, 0), None);
    }

    #[test]
    fn only_class_changes_are_reported_and_zero_width_is_ignored() {
        // F-49-05
        let mut t = ClassTracker::default();
        assert_eq!(t.update(0, 1), None);
        assert_eq!(t.update(500, 1), Some(WindowClass::Compact));
        assert_eq!(t.update(550, 1), None);
        assert_eq!(t.update(900, 1), Some(WindowClass::Expanded));
        assert_eq!(t.update(1000, 1), None);
        assert_eq!(t.update(0, 1), None);
        assert_eq!(t.update(700, 1), Some(WindowClass::Medium));
    }
}
