//! Gesture math (area 46 `GestureHandler.kt` helpers, T-46a-039). Pure. Steps truncate toward zero,
//! as Kotlin's `Float.toInt()` does. The brightness and volume sensitivities are in `vertical.rs`.

use crate::controls::Zone;

/// Horizontal seek sensitivity: seconds of target move per pixel of drag (`calculateNewHorizontal`).
pub const SEEK_SENS: f32 = 0.15;

/// Integer value after a vertical drag. Upward movement (smaller `new_y`) increases the value.
pub fn vertical_step_int(original: i32, starting_y: f32, new_y: f32, sensitivity: f32) -> i32 {
    original + ((starting_y - new_y) * sensitivity) as i32
}

/// Float value after a vertical drag, without truncation.
pub fn vertical_step_f32(original: f32, starting_y: f32, new_y: f32, sensitivity: f32) -> f32 {
    original + (starting_y - new_y) * sensitivity
}

/// Integer value after a horizontal drag. Rightward movement increases the value.
pub fn horizontal_step(original: i32, starting_x: f32, new_x: f32, sensitivity: f32) -> i32 {
    original + ((new_x - starting_x) * sensitivity) as i32
}

/// The double-tap zone of an x position: right above 0.6 of the width, left below 0.4, centre
/// otherwise (E-46-012). Exactly 0.4 and 0.6 are centre.
pub fn zone_at(x: f32, width: f32) -> Zone {
    if x > width * 0.6 {
        Zone::Right
    } else if x < width * 0.4 {
        Zone::Left
    } else {
        Zone::Center
    }
}

/// The horizontal seek drag runs only when `pref_gesture_horizontal_seek` is on and the controls are
/// not locked (GH:181-186, T-46a-018).
pub fn horizontal_drag_enabled(pref_on: bool, locked: bool) -> bool {
    pref_on && !locked
}

/// Horizontal drag seek (GH:181-217, T-46a-018). The drag points at an absolute second: the start
/// plus the horizontal move times `SEEK_SENS`, truncated, clamped to `0..=duration`. Pure: the
/// caller shows `delta` and writes the seek returned by `release`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HorizontalDrag {
    start_pos: i32,
    start_x: f32,
    target: i32,
}

impl HorizontalDrag {
    /// A drag starts at playhead `position` seconds, truncated toward zero (E-46-020).
    pub fn begin(position: f32, x: f32) -> Self {
        let start = position as i32;
        Self {
            start_pos: start,
            start_x: x,
            target: start,
        }
    }

    /// One move to `x`. `position` is the playhead now and `drag_amount` the move since the last
    /// event. Returns the new target, or `None` when a guard stops it: no drag left at 0, no drag
    /// right at the end (E-46-018). A `None` leaves the target as it was.
    pub fn update(
        &mut self,
        position: f32,
        duration: f32,
        x: f32,
        drag_amount: f32,
    ) -> Option<i32> {
        if (position <= 0.0 && drag_amount < 0.0) || (position >= duration && drag_amount > 0.0) {
            return None;
        }
        let max = duration.max(0.0) as i32;
        self.target = horizontal_step(self.start_pos, self.start_x, x, SEEK_SENS).clamp(0, max);
        Some(self.target)
    }

    /// The target second.
    pub fn target(&self) -> i32 {
        self.target
    }

    /// Seconds moved from the start, for the gesture text (`gesture_seek_text(start, delta)`).
    pub fn delta(&self) -> i32 {
        self.target - self.start_pos
    }

    /// The start second, for the gesture text.
    pub fn start(&self) -> i32 {
        self.start_pos
    }

    /// Release: an absolute seek to the target with `flag` (exact or keyframes, from the smooth
    /// seek preference). A release where the target did not move returns `None` (E-46-037).
    pub fn release(&self, flag: &'static str) -> Option<(i32, &'static str)> {
        (self.target != self.start_pos).then_some((self.target, flag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_horizontal_drag_needs_the_pref_and_no_lock() {
        assert!(horizontal_drag_enabled(true, false));
        assert!(!horizontal_drag_enabled(true, true));
        assert!(!horizontal_drag_enabled(false, false));
    }

    #[test]
    fn a_drag_moves_the_target_by_fifteen_percent_of_the_move() {
        let mut d = HorizontalDrag::begin(100.0, 200.0);
        assert_eq!(d.update(100.0, 500.0, 300.0, 100.0), Some(115));
        assert_eq!(d.delta(), 15);
        assert_eq!(d.update(115.0, 500.0, 190.0, -10.0), Some(99));
        assert_eq!(d.delta(), -1);
    }

    #[test]
    fn the_target_clamps_to_zero_and_the_duration() {
        let mut d = HorizontalDrag::begin(10.0, 0.0);
        assert_eq!(d.update(10.0, 50.0, -1000.0, -1000.0), Some(0));
        assert_eq!(d.update(10.0, 50.0, 5000.0, 5000.0), Some(50));
    }

    #[test]
    fn no_drag_left_at_zero_and_no_drag_right_at_the_end() {
        let mut d = HorizontalDrag::begin(0.0, 100.0);
        assert_eq!(d.update(0.0, 50.0, 50.0, -50.0), None);
        assert_eq!(d.target(), 0);
        let mut e = HorizontalDrag::begin(50.0, 100.0);
        assert_eq!(e.update(50.0, 50.0, 200.0, 100.0), None);
    }

    #[test]
    fn the_start_position_truncates_toward_zero() {
        let d = HorizontalDrag::begin(0.9, 0.0);
        assert_eq!(d.start(), 0);
    }

    #[test]
    fn release_seeks_only_when_the_target_moved() {
        let mut d = HorizontalDrag::begin(20.0, 0.0);
        assert_eq!(d.release("absolute"), None);
        d.update(20.0, 100.0, 100.0, 100.0);
        assert_eq!(
            d.release("absolute+keyframes"),
            Some((35, "absolute+keyframes"))
        );
    }

    #[test]
    fn upward_drag_increases_the_vertical_value() {
        assert_eq!(vertical_step_int(10, 100.0, 90.0, 0.5), 15);
        assert_eq!(vertical_step_f32(0.5, 100.0, 90.0, 0.01), 0.6);
    }

    #[test]
    fn steps_truncate_toward_zero() {
        // 2.9 truncates to 2, -2.9 truncates to -2.
        assert_eq!(vertical_step_int(0, 10.0, 7.1, 1.0), 2);
        assert_eq!(vertical_step_int(0, 10.0, 12.9, 1.0), -2);
        assert_eq!(horizontal_step(0, 0.0, -2.9, 1.0), -2);
    }

    #[test]
    fn horizontal_drag_uses_the_seek_sensitivity() {
        // 100 px right at 0.15 seconds per px is 15 seconds.
        assert_eq!(horizontal_step(85, 200.0, 300.0, SEEK_SENS), 100);
        assert_eq!(horizontal_step(85, 300.0, 200.0, SEEK_SENS), 70);
    }

    #[test]
    fn zones_split_at_forty_and_sixty_percent() {
        assert_eq!(zone_at(399.0, 1000.0), Zone::Left);
        assert_eq!(zone_at(400.0, 1000.0), Zone::Center);
        assert_eq!(zone_at(600.0, 1000.0), Zone::Center);
        assert_eq!(zone_at(601.0, 1000.0), Zone::Right);
    }
}
