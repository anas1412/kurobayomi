//! Gesture math (area 46 `GestureHandler.kt` helpers, T-46a-039). Pure. Steps truncate toward zero,
//! as Kotlin's `Float.toInt()` does.

use crate::controls::Zone;

/// Horizontal seek sensitivity: seconds of target move per pixel of drag (`calculateNewHorizontal`).
pub const SEEK_SENS: f32 = 0.15;

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
    fn zones_split_at_forty_and_sixty_percent() {
        assert_eq!(zone_at(399.0, 1000.0), Zone::Left);
        assert_eq!(zone_at(400.0, 1000.0), Zone::Center);
        assert_eq!(zone_at(600.0, 1000.0), Zone::Center);
        assert_eq!(zone_at(601.0, 1000.0), Zone::Right);
    }
}
