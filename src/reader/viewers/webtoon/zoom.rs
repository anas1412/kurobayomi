//! The webtoon zoom model (T-44a-038, T-44a-039, T-44a-042, B§10.7, E-31 to E-34). Pure: the whole strip is scaled by
//! `scale` and moved by a pan `(x, y)`. The view turns these numbers into a transform. Constants are upstream's
//! (`MIN_RATE` 0.5, `MAX_SCALE_RATE` 3, 200 ms animations).
//!
//! Pinch and drag change the state at once. A double tap and a turn-off of zoom-out animate it: [`Anim`] samples the
//! animation, clamps every frame (E-34) and reports the end state.

/// The largest webtoon zoom (`MAX_SCALE_RATE`).
pub const MAX_RATE: f64 = 3.0;
/// The smallest zoom with zoom-out allowed (`MIN_RATE`).
pub const MIN_RATE: f64 = 0.5;
/// The smallest zoom with `webtoon_disable_zoom_out` on.
pub const MIN_RATE_NO_ZOOM_OUT: f64 = 1.0;
/// The double-tap zoom level (`zoom(1, 2, ...)`).
pub const DOUBLE_TAP_RATE: f64 = 2.0;
/// Duration of a double tap, a zoom-out animation and the minimum change (`ANIMATOR_DURATION_TIME`).
pub const ANIMATION_MS: u32 = 200;

/// The zoom and pan of the webtoon strip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WebtoonZoom {
    scale: f64,
    pan: (f64, f64),
    zoom_out_disabled: bool,
}

/// An animation toward `to` that the view samples each frame. `from` is where it started.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anim {
    from: (f64, (f64, f64)),
    to: (f64, (f64, f64)),
    duration_ms: u32,
}

impl Anim {
    /// The scale and pan `elapsed_ms` into the animation, with the decelerate curve. The pan is clamped by `clamp`
    /// each frame (E-34), so the strip never shows past its edges mid-animation.
    pub fn sample(&self, elapsed_ms: u32, clamp: impl Fn(f64, (f64, f64)) -> (f64, f64)) -> (f64, (f64, f64)) {
        let t = if self.duration_ms == 0 { 1.0 } else { (f64::from(elapsed_ms) / f64::from(self.duration_ms)).clamp(0.0, 1.0) };
        let f = 1.0 - (1.0 - t) * (1.0 - t);
        let scale = self.from.0 + (self.to.0 - self.from.0) * f;
        let pan = (self.from.1.0 + (self.to.1.0 - self.from.1.0) * f, self.from.1.1 + (self.to.1.1 - self.from.1.1) * f);
        (scale, clamp(scale, pan))
    }

    /// The end state of the animation.
    pub fn end(&self) -> (f64, (f64, f64)) {
        self.to
    }

    pub fn duration_ms(&self) -> u32 {
        self.duration_ms
    }
}

impl WebtoonZoom {
    pub fn new(zoom_out_disabled: bool) -> Self {
        Self { scale: 1.0, pan: (0.0, 0.0), zoom_out_disabled }
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn pan(&self) -> (f64, f64) {
        self.pan
    }

    /// The smallest allowed scale: 1 with zoom-out disabled, else 0.5.
    pub fn min_rate(&self) -> f64 {
        if self.zoom_out_disabled { MIN_RATE_NO_ZOOM_OUT } else { MIN_RATE }
    }

    /// Keeps a scale in `[min_rate, MAX_RATE]`. A NaN input falls back to 1 so no later step can panic.
    pub fn clamp_scale(&self, scale: f64) -> f64 {
        if scale.is_nan() {
            return 1.0;
        }
        scale.max(self.min_rate()).min(MAX_RATE)
    }

    /// The height of an item of the strip at the current scale: `original / scale` below 1, else `original` (E-31).
    pub fn item_height(&self, original: i32) -> i32 {
        if self.scale < 1.0 { (f64::from(original) / self.scale) as i32 } else { original }
    }

    /// Pinch (`onScale`, B§10.7): multiplies the scale by `factor`, clamps it and keeps the pan inside the strip.
    /// `half` is half the view size, `original_h` the first measured height.
    pub fn pinch(&mut self, factor: f64, half: (f64, f64), original_h: f64) -> f64 {
        self.scale = self.clamp_scale(self.scale * factor);
        self.pan = self.clamp_pan(self.pan, half, original_h);
        self.scale
    }

    /// Pinch end (`onScaleEnd`): a scale under the minimum animates back to it. `None` when nothing changed.
    pub fn settle(&mut self) -> Option<Anim> {
        if self.scale >= self.min_rate() {
            return None;
        }
        Some(self.animate_to((self.min_rate(), self.pan)))
    }

    /// Turns zoom-out off or on. Turning it on while the strip is below 1 animates back to 1 over 200 ms (E-32).
    pub fn set_zoom_out_disabled(&mut self, disabled: bool) -> Option<Anim> {
        self.zoom_out_disabled = disabled;
        if self.scale >= self.min_rate() {
            return None;
        }
        Some(self.animate_to((self.min_rate(), self.pan)))
    }

    /// A drag (B§10.7): x always pans, y only when the strip is at its first or last item (`at_edge`). Above 1 the pan
    /// is clamped to the strip; at or below 1 the strip is centred and x is forced to 0.
    pub fn drag(&mut self, dx: f64, dy: f64, at_edge: bool, half: (f64, f64), original_h: f64) {
        if self.scale <= 1.0 {
            return;
        }
        let dy = if at_edge { dy } else { 0.0 };
        self.pan = self.clamp_pan((self.pan.0 + dx, self.pan.1 + dy), half, original_h);
    }

    /// A double tap (`onDoubleTapConfirmed`, E-33). Ignored while `animating` or when double-tap zoom is off. At scale
    /// 1 the strip zooms to 2 centred on the touch point `(x, y)` in view pixels; otherwise it goes back to 1.
    pub fn double_tap(&mut self, enabled: bool, animating: bool, touch: (f64, f64), half: (f64, f64)) -> Option<Anim> {
        if !enabled || animating {
            return None;
        }
        if self.scale != 1.0 {
            return Some(self.animate_to((1.0, (0.0, 0.0))));
        }
        let pan = ((half.0 - touch.0) * (DOUBLE_TAP_RATE - 1.0), (half.1 - touch.1) * (DOUBLE_TAP_RATE - 1.0));
        Some(Anim { from: (self.scale, self.pan), to: (DOUBLE_TAP_RATE, pan), duration_ms: ANIMATION_MS })
    }

    /// The pan clamp for the current scale (B§10.7): `x` is `[-half_w * (s - 1), half_w * (s - 1)]` and `y` likewise
    /// with the half height of the item, while below 1 the strip is centred.
    fn clamp_pan(&self, pan: (f64, f64), half: (f64, f64), original_h: f64) -> (f64, f64) {
        if self.scale == 1.0 {
            return (0.0, 0.0);
        }
        clamp_pan(self.scale, pan, half, original_h)
    }

    /// An animation from the current state to `to`. The caller applies the end state with [`WebtoonZoom::apply`].
    fn animate_to(&self, to: (f64, (f64, f64))) -> Anim {
        Anim { from: (self.scale, self.pan), to, duration_ms: ANIMATION_MS }
    }

    /// Stores an end state (after an animation has run to its end).
    pub fn apply(&mut self, end: (f64, (f64, f64)), half: (f64, f64), original_h: f64) {
        self.scale = self.clamp_scale(end.0);
        self.pan = self.clamp_pan(end.1, half, original_h);
    }

    /// Moves to the state `elapsed_ms` into `anim`. The pan is clamped on every frame (E-34). `true` when the animation
    /// has run its course.
    pub fn step(&mut self, anim: &Anim, elapsed_ms: u32, half: (f64, f64), original_h: f64) -> bool {
        let (scale, pan) = anim.sample(elapsed_ms, |scale, pan| clamp_pan(scale, pan, half, original_h));
        self.scale = self.clamp_scale(scale);
        self.pan = if self.scale == 1.0 { (0.0, 0.0) } else { pan };
        elapsed_ms >= anim.duration_ms()
    }
}

/// The clamp of a pan at `scale`. Below 1 the strip is centred vertically (`originalHeight / 2 - halfHeight`) and x is
/// 0. Above 1 each axis is limited to half the overflow.
fn clamp_pan(scale: f64, pan: (f64, f64), half: (f64, f64), original_h: f64) -> (f64, f64) {
    if scale < 1.0 {
        let item_half = original_h / (2.0 * scale);
        return (0.0, original_h / 2.0 - item_half);
    }
    let (lx, ly) = (half.0 * (scale - 1.0), half.1 * (scale - 1.0));
    (pan.0.max(-lx).min(lx), pan.1.max(-ly).min(ly))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: (f64, f64) = (400.0, 300.0);
    const ORIGINAL: f64 = 600.0;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn no_clamp(_: f64, pan: (f64, f64)) -> (f64, f64) {
        pan
    }

    #[test]
    fn the_minimum_is_one_half_and_one_with_zoom_out_disabled() {
        assert!(close(WebtoonZoom::new(false).min_rate(), 0.5));
        assert!(close(WebtoonZoom::new(true).min_rate(), 1.0));
    }

    #[test]
    fn e31_scale_below_one_makes_the_items_taller() {
        let mut z = WebtoonZoom::new(false);
        z.pinch(0.5, HALF, ORIGINAL);
        assert!(close(z.scale(), 0.5));
        assert_eq!(z.item_height(1000), 2000, "the strip is twice the viewport at 0.5");
        z.pinch(2.0, HALF, ORIGINAL);
        assert_eq!(z.item_height(1000), 1000, "at 1 and above the height is the original");
        z.pinch(2.0, HALF, ORIGINAL);
        assert_eq!(z.item_height(1000), 1000);
    }

    #[test]
    fn the_scale_clamps_to_the_range_and_a_nan_becomes_one() {
        let mut z = WebtoonZoom::new(false);
        assert!(close(z.pinch(0.1, HALF, ORIGINAL), 0.5));
        assert!(close(z.pinch(100.0, HALF, ORIGINAL), 3.0));
        assert!(close(z.clamp_scale(f64::NAN), 1.0));
        let limited = WebtoonZoom::new(true);
        assert!(close(limited.clamp_scale(0.5), 1.0));
    }

    #[test]
    fn e32_turning_zoom_out_off_below_one_animates_back_to_one() {
        let mut z = WebtoonZoom::new(false);
        z.pinch(0.5, HALF, ORIGINAL);
        let anim = z.set_zoom_out_disabled(true).expect("animates");
        assert_eq!(anim.duration_ms(), 200);
        assert!(close(anim.end().0, 1.0));
        assert!(WebtoonZoom::new(false).set_zoom_out_disabled(true).is_none(), "already at one");
    }

    #[test]
    fn a_pinch_end_below_the_minimum_animates_back_to_it() {
        let mut z = WebtoonZoom::new(false);
        z.pinch(0.5, HALF, ORIGINAL);
        assert!(z.settle().is_none(), "0.5 is the minimum with zoom-out allowed");
        z.zoom_out_disabled = true;
        let anim = z.settle().expect("animates to the new minimum");
        assert_eq!(anim.end().0, 1.0);
        assert!(WebtoonZoom::new(true).settle().is_none());
    }

    #[test]
    fn below_one_the_strip_is_centred_vertically_and_x_is_zero() {
        let mut z = WebtoonZoom::new(false);
        z.pinch(0.5, HALF, ORIGINAL);
        // Height 1200 at 0.5: the centre is 300 - 600 = -300 whatever the requested pan.
        assert_eq!(z.clamp_pan((50.0, 999.0), HALF, ORIGINAL), (0.0, -300.0));
    }

    #[test]
    fn e33_double_tap_zooms_about_the_touch_and_is_ignored_while_animating() {
        let mut z = WebtoonZoom::new(false);
        let anim = z.double_tap(true, false, (100.0, 200.0), HALF).expect("zoom in");
        let (scale, pan) = anim.end();
        assert!(close(scale, 2.0));
        assert!(close(pan.0, 300.0) && close(pan.1, 100.0), "centred on (100, 200): {pan:?}");
        assert!(z.double_tap(true, true, (100.0, 200.0), HALF).is_none(), "animating");
        assert!(z.double_tap(false, false, (100.0, 200.0), HALF).is_none(), "setting off");
    }

    #[test]
    fn a_second_double_tap_goes_back_to_one() {
        let mut z = WebtoonZoom::new(false);
        let zoom_in = z.double_tap(true, false, (100.0, 200.0), HALF).expect("in");
        z.apply(zoom_in.end(), HALF, ORIGINAL);
        let back = z.double_tap(true, false, (100.0, 200.0), HALF).expect("out");
        assert_eq!(back.end(), (1.0, (0.0, 0.0)));
    }

    #[test]
    fn the_animation_decelerates_and_is_clamped_every_frame() {
        let anim = Anim { from: (1.0, (0.0, 0.0)), to: (2.0, (900.0, 0.0)), duration_ms: 200 };
        let clamp = |_: f64, p: (f64, f64)| (p.0.max(-400.0).min(400.0), p.1);
        assert_eq!(anim.sample(0, clamp).0, 1.0);
        assert!(close(anim.sample(200, clamp).0, 2.0));
        let mid = anim.sample(100, no_clamp).0;
        assert!(close(mid, 1.75), "1 - (1 - 0.5)^2 is three quarters of the way: {mid}");
        assert!(close(anim.sample(200, clamp).1.0, 400.0), "the end pan is clamped");
    }

    #[test]
    fn the_drag_pans_x_always_and_y_only_at_the_strip_edges() {
        let mut z = WebtoonZoom::new(false);
        z.pinch(2.0, HALF, ORIGINAL);
        z.drag(-100.0, 50.0, false, HALF, ORIGINAL);
        assert!(close(z.pan().0, -100.0));
        assert!(close(z.pan().1, 0.0), "y is not panned mid strip");
        z.drag(0.0, 50.0, true, HALF, ORIGINAL);
        assert!(close(z.pan().1, 50.0), "y pans at the edge");
        z.drag(10000.0, 0.0, true, HALF, ORIGINAL);
        assert!(close(z.pan().0, 400.0), "the pan stays inside half a view of overflow");
    }

    #[test]
    fn a_pinch_sequence_matches_the_expected_steps() {
        // Golden sequence: zoom in twice, out past the minimum, then a return.
        let mut z = WebtoonZoom::new(false);
        let steps = [1.2, 1.2, 0.5, 0.5, 1.5];
        let got: Vec<f64> = steps.iter().map(|f| z.pinch(*f, HALF, ORIGINAL)).collect();
        let want = [1.2, 1.44, 0.72, 0.5, 0.75];
        for (g, w) in got.iter().zip(want) {
            assert!(close(*g, w), "{got:?}");
        }
        assert!(z.settle().is_none(), "the pinch stays at the minimum");
    }

    /// T-44a-080: the pager pinch sequence (a page fitted at 0.3, range 0.3 to 1.5) and the webtoon pinch sequence,
    /// compared with the golden file. The expected numbers are also written out here, worked by hand.
    #[test]
    fn the_golden_pinch_sequences_match() {
        use crate::reader::page_zoom::{ZoomState, pinch_scale};
        let page = ZoomState::fitted(0.3, (500.0, 1000.0));
        // each gesture starts at the scale the page has, and reports the factor since it began
        let gestures: [(f64, &[f64]); 2] = [(0.3, &[1.2, 2.0, 3.0, 8.0, 0.5]), (1.5, &[0.5, 0.1])];
        let pager: Vec<serde_json::Value> = gestures
            .iter()
            .map(|(start, factors)| {
                let scales: Vec<f64> = factors.iter().map(|f| pinch_scale(*start, *f, &page)).collect();
                serde_json::json!({ "start": start, "factors": factors, "scales": scales })
            })
            .collect();
        let first: Vec<f64> = pager[0]["scales"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        for (got, want) in first.iter().zip([0.36, 0.6, 0.9, 1.5, 0.3]) {
            assert!(close(*got, want), "{first:?}");
        }
        let second: Vec<f64> = pager[1]["scales"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert!(close(second[0], 0.75) && close(second[1], 0.3), "{second:?}");

        let mut z = WebtoonZoom::new(false);
        let steps = [1.2, 1.2, 0.5, 0.5, 1.5, 4.0, 4.0];
        let scales: Vec<f64> = steps.iter().map(|f| z.pinch(*f, HALF, ORIGINAL)).collect();
        for (got, want) in scales.iter().zip([1.2, 1.44, 0.72, 0.5, 0.75, 3.0, 3.0]) {
            assert!(close(*got, want), "{scales:?}");
        }
        let golden = serde_json::json!({
            "about": "pager: page fitted at 0.3, range 0.3 to 1.5, one entry per gesture; webtoon: factors per event, range 0.5 to 3",
            "pager": { "min": 0.3, "max": 1.5, "gestures": pager },
            "webtoon": { "min": 0.5, "max": 3.0, "factors": steps, "scales": scales },
        });
        crate::reader::golden::check("zoom_pinch.json", &golden);
    }
}
