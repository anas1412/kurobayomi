//! Icon and stack animations of the Android resources (T-49ba-022, T-49ba-024, T-49ba-028):
//! the stack crossfade, the icon rotation classes, and the double-tap seek triangle schedule.
//!
//! Rotations are CSS transitions defined in `style.css`, so no per-frame code runs. The durations
//! are in `motion`, and every duration goes through `motion::duration_ms` when it is set from Rust.

use std::time::Duration;

use gtk::prelude::*;

use super::motion;

/// The base class of every rotating icon; carries the shared transition.
pub const ROTATION_BASE: &str = "kb-icon-rot";

/// Every target class a rotation can carry, so a switch removes the old one first.
const ROTATION_TARGETS: [&str; 4] = ["kb-rot-0", "kb-rot-180", "kb-rot-0-slow", "kb-rot-360"];

/// The two rotations of the tab-enter icons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    /// `anim_browse_enter`: 0 to 180 degrees over `BROWSE_ROTATE_MS` when selected.
    Half,
    /// `anim_history_enter`: 360 to 0 degrees over `HISTORY_ROTATE_MS` when selected.
    FullBackSlow,
}

impl Rotation {
    /// The target class for the icon's state. The transition runs from the current angle to this one.
    pub const fn class(self, selected: bool) -> &'static str {
        match (self, selected) {
            (Rotation::Half, true) => "kb-rot-180",
            (Rotation::Half, false) => "kb-rot-0",
            (Rotation::FullBackSlow, true) => "kb-rot-0-slow",
            (Rotation::FullBackSlow, false) => "kb-rot-360",
        }
    }

    /// The transition length of the selected state, in milliseconds.
    pub const fn duration_ms(self) -> u32 {
        match self {
            Rotation::Half => motion::BROWSE_ROTATE_MS,
            Rotation::FullBackSlow => motion::HISTORY_ROTATE_MS,
        }
    }
}

/// Sets the rotation state of an icon. A reversal (deselect, or a fast toggle) starts from the
/// angle the icon has now, so no icon is left mid-state.
pub fn rotate_icon(image: &gtk::Image, rotation: Rotation, selected: bool) {
    for class in ROTATION_TARGETS {
        image.remove_css_class(class);
    }
    image.add_css_class(ROTATION_BASE);
    image.add_css_class(rotation.class(selected));
}

/// Crossfades `stack` to `child_name` over `ms`, or instantly when animations are off (T-49ba-046).
pub fn crossfade_to(stack: &gtk::Stack, child_name: &str, ms: u32) {
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(motion::duration_ms(ms, motion::enabled()));
    stack.set_visible_child_name(child_name);
}

/// Cubic-bezier easing, the same curve as CSS and Compose `FastOutSlowIn` (x1 0.4, y1 0, x2 0.2,
/// y2 1). Used for the widget-side tweens that CSS cannot run (double-tap triangles).
pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    let bx = |t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * x1 + 3.0 * (1.0 - t) * t * t * x2 + t * t * t;
    let by = |t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * y1 + 3.0 * (1.0 - t) * t * t * y2 + t * t * t;
    let dx = |t: f64| 3.0 * (1.0 - t) * (1.0 - t) * x1 + 6.0 * (1.0 - t) * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2);
    let mut t = x.clamp(0.0, 1.0);
    for _ in 0..8 {
        let err = bx(t) - x;
        let d = dx(t);
        if d.abs() < 1e-9 {
            break;
        }
        t = (t - err / d).clamp(0.0, 1.0);
    }
    by(t)
}

/// Length of one double-tap seek step (`DoubleTapSeekTriangles.kt`).
pub const SEEK_STEP_MS: u64 = 150;

/// Row `s` is the target alpha of the three triangles at the end of step `s`. Steps 0 to 2 raise
/// triangles 0 to 2, steps 3 to 5 lower them again, so one cycle is 900 ms.
pub const SEEK_ROWS: [[f64; 3]; 6] = [
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [1.0, 1.0, 1.0],
    [0.0, 1.0, 1.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, 0.0],
];

/// Opacity of triangle `idx` (0 to 2) at time `t` since the seek overlay appeared. Each step eases
/// from the previous row to its own row. The cycle repeats, so it restarts from zero by itself.
pub fn seek_alpha(idx: usize, t: Duration) -> f64 {
    let ms = t.as_millis() as u64;
    let step = ((ms / SEEK_STEP_MS) % 6) as usize;
    let frac = (ms % SEEK_STEP_MS) as f64 / SEEK_STEP_MS as f64;
    let prev = SEEK_ROWS[(step + 5) % 6][idx];
    let next = SEEK_ROWS[step][idx];
    prev + (next - prev) * cubic_bezier(0.4, 0.0, 0.2, 1.0, frac)
}

/// Rotation of the seek triangles in degrees: backward seeks point the other way (T-49ba-028).
pub const fn seek_rotation_deg(backward: bool) -> f64 {
    if backward { 180.0 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browse_rotates_half_a_turn_and_history_back_from_a_full_turn() {
        assert_eq!(Rotation::Half.class(true), "kb-rot-180");
        assert_eq!(Rotation::Half.class(false), "kb-rot-0");
        assert_eq!(Rotation::FullBackSlow.class(true), "kb-rot-0-slow");
        assert_eq!(Rotation::FullBackSlow.class(false), "kb-rot-360");
        assert_eq!(Rotation::Half.duration_ms(), 300);
        assert_eq!(Rotation::FullBackSlow.duration_ms(), 500);
    }

    #[test]
    fn every_target_class_is_listed_for_removal() {
        for r in [Rotation::Half, Rotation::FullBackSlow] {
            for selected in [true, false] {
                assert!(ROTATION_TARGETS.contains(&r.class(selected)));
            }
        }
    }

    #[test]
    fn cubic_bezier_hits_both_ends_and_eases_in_between() {
        assert_eq!(cubic_bezier(0.4, 0.0, 0.2, 1.0, 0.0), 0.0);
        assert!((cubic_bezier(0.4, 0.0, 0.2, 1.0, 1.0) - 1.0).abs() < 1e-9);
        let mid = cubic_bezier(0.4, 0.0, 0.2, 1.0, 0.5);
        assert!(mid > 0.5 && mid < 1.0, "fast-out-slow-in is ahead of linear at 0.5: {mid}");
    }

    // T10: the seek schedule and easing.
    #[test]
    fn seek_triangles_start_empty_and_fill_one_step_at_a_time() {
        for idx in 0..3 {
            assert_eq!(seek_alpha(idx, Duration::ZERO), 0.0);
        }
        assert!((seek_alpha(0, Duration::from_millis(150)) - 1.0).abs() < 1e-9);
        assert_eq!(seek_alpha(1, Duration::from_millis(150)), 0.0);
        assert!((seek_alpha(1, Duration::from_millis(300)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn seek_cycle_is_900_ms_and_restarts() {
        for idx in 0..3 {
            for ms in [0_u64, 40, 170, 400, 777] {
                let a = seek_alpha(idx, Duration::from_millis(ms));
                let b = seek_alpha(idx, Duration::from_millis(ms + 900));
                assert!((a - b).abs() < 1e-9, "idx {idx} at {ms} ms");
                assert!((0.0..=1.0).contains(&a));
            }
        }
        assert_eq!(seek_alpha(2, Duration::from_millis(900)), 0.0);
    }

    #[test]
    fn backward_seek_rotates_the_triangles_half_a_turn() {
        assert_eq!(seek_rotation_deg(true), 180.0);
        assert_eq!(seek_rotation_deg(false), 0.0);
    }
}
