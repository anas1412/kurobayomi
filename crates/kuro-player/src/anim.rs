//! Animation timings, motion preference and a tick-based tween (area 46 `PlayerControls.kt`,
//! T-46a-041). Pure: the GTK frame clock passes `now_ms` and applies the value. Nothing here
//! depends on CSS transitions.

/// Enter animation of the controls: 100 ms, linear out, slow in (`playerControlsEnterAnimationSpec`).
pub const ENTER_MS: u64 = 100;
/// Exit animation of the controls: 300 ms, fast out, slow in (`playerControlsExitAnimationSpec`).
pub const EXIT_MS: u64 = 300;
/// Side panel switch: slide and fade (T-46a-028).
pub const PANEL_MS: u64 = 300;
/// Sheet open and the scrim fade (`PlayerSheet.kt:74`).
pub const SHEET_MS: u64 = 350;

/// Cubic bezier control points of Material's `LinearOutSlowInEasing` (the enter curve).
pub const LINEAR_OUT_SLOW_IN: [f32; 4] = [0.0, 0.0, 0.2, 1.0];
/// Cubic bezier control points of Material's `FastOutSlowInEasing` (the exit curve).
pub const FAST_OUT_SLOW_IN: [f32; 4] = [0.4, 0.0, 0.2, 1.0];

/// How a show or hide is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Slide,
    Fade,
}

/// Slides become fades when `pref_reduce_motion` is on, or when GTK has animations disabled
/// (`gtk-enable-animations`, D-5).
pub fn motion(reduce_motion: bool, gtk_animations: bool) -> Motion {
    if reduce_motion || !gtk_animations {
        Motion::Fade
    } else {
        Motion::Slide
    }
}

fn bezier_axis(p1: f32, p2: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
}

/// Eased progress for a linear `progress` in 0..=1 along the cubic bezier `[x1, y1, x2, y2]`.
/// The curve's x is inverted by bisection.
pub fn ease(curve: [f32; 4], progress: f32) -> f32 {
    let x = progress.clamp(0.0, 1.0);
    if x == 0.0 || x == 1.0 {
        return x;
    }
    let [x1, y1, x2, y2] = curve;
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..32 {
        let mid = (lo + hi) / 2.0;
        if bezier_axis(x1, x2, mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bezier_axis(y1, y2, (lo + hi) / 2.0)
}

/// A value moving from `from` to `to` between `start_ms` and `start_ms + duration_ms`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub curve: [f32; 4],
}

impl Tween {
    /// The value at `now_ms`. Before the start it is `from`; after the end it is `to`.
    pub fn value_at(&self, now_ms: u64) -> f32 {
        let elapsed = now_ms.saturating_sub(self.start_ms);
        if self.duration_ms == 0 || elapsed >= self.duration_ms {
            return self.to;
        }
        let progress = elapsed as f32 / self.duration_ms as f32;
        self.from + (self.to - self.from) * ease(self.curve, progress)
    }

    pub fn finished(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.start_ms) >= self.duration_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timings_match_the_kotlin_specs() {
        assert_eq!(ENTER_MS, 100);
        assert_eq!(EXIT_MS, 300);
        assert_eq!(SHEET_MS, 350);
    }

    #[test]
    fn reduce_motion_or_disabled_gtk_animations_make_fades() {
        assert_eq!(motion(false, true), Motion::Slide);
        assert_eq!(motion(true, true), Motion::Fade);
        assert_eq!(motion(false, false), Motion::Fade);
    }

    #[test]
    fn ease_hits_both_ends_and_is_monotonic() {
        for curve in [LINEAR_OUT_SLOW_IN, FAST_OUT_SLOW_IN] {
            assert_eq!(ease(curve, 0.0), 0.0);
            assert_eq!(ease(curve, 1.0), 1.0);
            let mut last = 0.0;
            for step in 1..=20 {
                let value = ease(curve, step as f32 / 20.0);
                assert!(value >= last);
                last = value;
            }
        }
    }

    #[test]
    fn enter_curve_is_ahead_of_linear() {
        // An ease-out reaches more than half way in half the time.
        assert!(ease(LINEAR_OUT_SLOW_IN, 0.5) > 0.5);
    }

    #[test]
    fn tween_reads_from_before_and_to_after_its_span() {
        let t = Tween {
            from: 0.0,
            to: 1.0,
            start_ms: 1000,
            duration_ms: ENTER_MS,
            curve: LINEAR_OUT_SLOW_IN,
        };
        assert_eq!(t.value_at(0), 0.0);
        assert_eq!(t.value_at(1000), 0.0);
        assert!(t.value_at(1050) > 0.0 && t.value_at(1050) < 1.0);
        assert_eq!(t.value_at(1100), 1.0);
        assert!(!t.finished(1099));
        assert!(t.finished(1100));
    }
}
