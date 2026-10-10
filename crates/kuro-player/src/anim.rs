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
/// Play and pause icon cross-fade when the playback state changes (T-46a-053, D-7).
pub const ICON_FADE_MS: u64 = 150;

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

/// The play/pause icon cross-fade started at `start_ms`: a 0 to 1 tween of the incoming icon's
/// alpha over `ICON_FADE_MS`. The outgoing icon uses `1 - value`. No vector morph is drawn.
pub fn icon_crossfade(start_ms: u64) -> Tween {
    Tween {
        from: 0.0,
        to: 1.0,
        start_ms,
        duration_ms: ICON_FADE_MS,
        curve: FAST_OUT_SLOW_IN,
    }
}

/// The play/pause icon with its cross-fade (T-46a-053). `set` starts a fade when the icon
/// changes, and `alphas` gives the alpha of the incoming and the outgoing icon at a time, for the
/// GTK widget to draw each frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconFade<T: Copy + PartialEq> {
    shown: T,
    outgoing: Option<(T, Tween)>,
}

impl<T: Copy + PartialEq> IconFade<T> {
    pub fn new(icon: T) -> Self {
        Self {
            shown: icon,
            outgoing: None,
        }
    }

    /// The icon that is fading in, or fully shown.
    pub fn shown(&self) -> T {
        self.shown
    }

    /// Changes the icon at `now_ms`. The same icon again changes nothing. A change in the middle
    /// of a fade starts a new one from the icon that is showing.
    pub fn set(&mut self, icon: T, now_ms: u64) {
        if icon != self.shown {
            self.outgoing = Some((self.shown, icon_crossfade(now_ms)));
            self.shown = icon;
        }
    }

    /// `(incoming alpha, outgoing)` at `now_ms`. The outgoing icon and its alpha are `None` once
    /// the fade has finished.
    pub fn alphas(&self, now_ms: u64) -> (f32, Option<(T, f32)>) {
        match &self.outgoing {
            Some((icon, fade)) if !fade.finished(now_ms) => {
                let alpha = fade.value_at(now_ms);
                (alpha, Some((*icon, 1.0 - alpha)))
            }
            _ => (1.0, None),
        }
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

    #[test]
    fn icon_crossfade_runs_150_ms_between_the_two_icons() {
        let fade = icon_crossfade(1000);
        assert_eq!(fade.duration_ms, 150);
        assert_eq!(fade.value_at(999), 0.0);
        assert_eq!(fade.value_at(1150), 1.0);
        assert!(fade.finished(1150) && !fade.finished(1149));
        let mid = fade.value_at(1075);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn a_changed_icon_fades_over_150_ms_and_the_same_icon_does_not() {
        let mut icon = IconFade::new('p');
        assert_eq!(icon.alphas(0), (1.0, None));
        icon.set('p', 100);
        assert_eq!(icon.alphas(100), (1.0, None));
        icon.set('q', 1000);
        assert_eq!(icon.shown(), 'q');
        assert_eq!(icon.alphas(1000), (0.0, Some(('p', 1.0))));
        let (incoming, outgoing) = icon.alphas(1075);
        let (old, alpha) = outgoing.unwrap();
        assert_eq!(old, 'p');
        assert!(incoming > 0.0 && incoming < 1.0);
        assert!((incoming + alpha - 1.0).abs() < 1e-6);
        assert_eq!(icon.alphas(1150), (1.0, None));
    }
}
