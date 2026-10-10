//! Quick scale: double tap, hold and drag vertically (T-44a-040, T-44a-080, TZ §4.2, B§10.4). Pure: the scale factor
//! of each one-finger move, from the vertical distance to where the drag started. The caller applies the factor to its
//! scale and clamps it; `lastDist` is updated on every move, as the source does (the `golden/quick_scale.json` sequence
//! pins it).

/// The distance added to every move so a drag that has not moved is not a zero distance (20 dp, in px).
pub const THRESHOLD_DP: f64 = 20.0;
/// A change of the span below this is ignored until the drag has moved once (`0.03`).
pub const SPAN_LIMIT: f64 = 0.03;
/// The share of the span that becomes the scale change (`0.5`).
pub const SPAN_FACTOR: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuickScale {
    threshold: f64,
    y_start: f64,
    y_last: f64,
    /// `None` is the source's `-1`: no move seen yet.
    last_dist: Option<f64>,
    moved: bool,
}

impl QuickScale {
    /// A quick scale that starts at `y_start`. `threshold` is `THRESHOLD_DP` in pixels.
    pub fn new(threshold: f64, y_start: f64) -> Self {
        Self { threshold, y_start, y_last: y_start, last_dist: None, moved: false }
    }

    /// One move to `y_now`. Returns the multiplier for the scale: 1 while the span stays under the limit and nothing
    /// has moved yet. `crossed` is called with the multiplier and says whether applying it made an axis cross from
    /// smaller than the view to at least the view; the next step then starts from a zero distance (the multiplier of
    /// that next move is 1).
    pub fn on_move(&mut self, y_now: f64, crossed: impl FnOnce(f64) -> bool) -> f64 {
        let mut dist = (self.y_start - y_now).abs() * 2.0 + self.threshold;
        let last = *self.last_dist.get_or_insert(dist);
        // the source compares exactly this way: a larger y counts as "upwards"
        let upwards = y_now > self.y_last;
        self.y_last = y_now;
        let span = (1.0 - dist / last).abs() * SPAN_FACTOR;
        let mut multiplier = 1.0;
        if span > SPAN_LIMIT || self.moved {
            self.moved = true;
            multiplier = if last > 0.0 {
                if upwards { 1.0 + span } else { 1.0 - span }
            } else {
                1.0
            };
            if crossed(multiplier) {
                dist = 0.0;
            }
        }
        self.last_dist = Some(dist);
        multiplier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn never(_: f64) -> bool {
        false
    }

    #[test]
    fn a_drag_that_has_not_moved_changes_nothing() {
        let mut q = QuickScale::new(20.0, 100.0);
        assert!(close(q.on_move(100.0, never), 1.0));
        assert!(close(q.on_move(100.5, never), 1.0), "dist 21 against 20 is a span of 0.025, under the limit");
    }

    #[test]
    fn a_hand_computed_drag_gives_the_observed_multipliers() {
        let mut q = QuickScale::new(20.0, 100.0);
        // dist 20, 40, 60, 50, 50 with lastDist 20, 20, 40, 60, 50
        let got: Vec<f64> = [100.0, 110.0, 120.0, 115.0, 115.0].iter().map(|y| q.on_move(*y, never)).collect();
        let want = [1.0, 1.5, 1.25, 1.0 - (1.0 - 50.0 / 60.0) * 0.5, 1.0];
        for (g, w) in got.iter().zip(want) {
            assert!(close(*g, w), "{got:?}");
        }
    }

    #[test]
    fn after_a_crossing_the_next_step_has_no_effect_and_the_one_after_resumes() {
        let mut q = QuickScale::new(20.0, 100.0);
        q.on_move(100.0, never);
        let first = q.on_move(110.0, |m| {
            assert!(close(m, 1.5));
            true
        });
        assert!(close(first, 1.5));
        // lastDist was reset to 0, so this move keeps the scale and stores dist (60)
        assert!(close(q.on_move(120.0, never), 1.0));
        // now lastDist is 60 and dist is 80: span 0.1667
        assert!(close(q.on_move(130.0, never), 1.0 + (1.0 - 80.0 / 60.0_f64).abs() * 0.5));
    }

    /// T-44a-080: the quick-scale sequence (TZ §4.2) with a crossing in the middle, compared with the golden file. The
    /// first five multipliers are worked out by hand in `a_hand_computed_drag_gives_the_observed_multipliers`.
    #[test]
    fn the_golden_quick_scale_sequence_matches() {
        let ys = [100.0, 100.0, 110.0, 120.0, 115.0, 115.0, 140.0, 150.0, 130.0, 130.0];
        // the scale is clamped to [1, 4] and the axis crosses once, on the sixth move
        let (mut scale, mut q) = (1.0_f64, QuickScale::new(THRESHOLD_DP, ys[0]));
        let mut steps = Vec::new();
        for (i, y) in ys.iter().enumerate() {
            let multiplier = q.on_move(*y, |m| {
                scale = (scale * m).clamp(1.0, 4.0);
                i == 5
            });
            steps.push(serde_json::json!({ "y": y, "multiplier": multiplier, "scale": scale }));
        }
        assert!(close(steps[1]["multiplier"].as_f64().unwrap(), 1.0), "the second move has not moved yet");
        assert!(close(steps[2]["multiplier"].as_f64().unwrap(), 1.5));
        let golden = serde_json::json!({
            "about": "threshold 20, y_start 100; the scale is clamped to 1..4; the sixth move crosses, so the seventh keeps the scale",
            "threshold": THRESHOLD_DP,
            "steps": steps,
        });
        crate::reader::golden::check("quick_scale.json", &golden);
    }
}
