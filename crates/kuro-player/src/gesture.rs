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

#[cfg(test)]
mod tests {
    use super::*;

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
