//! Double-tap seek feedback: the accumulator, the indicator text and the clear timer (area 46
//! `GestureHandler.kt` `onDoubleTap` and `LaunchedEffect(seekAmount)`, T-46a-015, T-46a-016).
//! Pure: the window calls `seek_step` for each seek, `tick` every frame and writes the seekbar
//! hide when `tick` reports a clear. Times are milliseconds.

use crate::oval::{feedback_text, oval_alpha};

/// The feedback clears this long after the last change (`delay(800)`, GH:96-103).
pub const CLEAR_MS: u64 = 800;

/// What the oval shows right now (GH:315-342).
#[derive(Clone, Debug, PartialEq)]
pub struct Indicator {
    /// `true` draws the oval on the right (amount above zero), `false` on the left.
    pub forward: bool,
    pub text: String,
    /// Fill alpha: 0.2 while the amount is not zero.
    pub alpha: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DoubleTap {
    /// Seconds accumulated by the taps in the current run. Signed: back taps are negative.
    amount: i64,
    /// Direction of the last seek step (`isSeekingForwards`).
    forward: bool,
    /// Text set by something else (for example the skip-intro message). Replaces the amount text.
    text: Option<String>,
    /// Set while a seek step is active (`isDoubleTapSeeking`).
    seeking: bool,
    /// When the feedback clears. Each change restarts it (E-46-045).
    clear_at: Option<u64>,
}

impl DoubleTap {
    /// One relative seek step of `offset` seconds (`leftSeek` and `rightSeek`). A change of
    /// direction starts the run from zero first (E-46-046). A same-direction step adds to it, and
    /// repeats in the same zone keep adding (E-46-047). Steps at the start (back) or the end
    /// (forward) do not accumulate. The caller still sends the seek.
    pub fn seek_step(&mut self, offset: i64, pos: i64, duration: i64, now_ms: u64) {
        let forward = offset > 0;
        let before = self.amount;
        if forward != self.forward {
            self.amount = 0;
        }
        self.forward = forward;
        let may_accumulate = if forward { pos < duration } else { pos > 0 };
        if may_accumulate {
            self.amount += offset;
        }
        self.seeking = true;
        if self.amount != before || self.clear_at.is_none() {
            self.clear_at = Some(now_ms + CLEAR_MS);
        }
    }

    /// Sets the text that replaces the amount text. It does not start the clear timer.
    pub fn set_text(&mut self, text: Option<String>) {
        self.text = text;
    }

    /// Clears the feedback when its timer is due. Returns `true` when it cleared now, so the
    /// caller can start the seekbar hide (`SeekbarVisibility::double_tap_cleared`).
    pub fn tick(&mut self, now_ms: u64) -> bool {
        match self.clear_at {
            Some(at) if now_ms >= at => {
                self.amount = 0;
                self.text = None;
                self.seeking = false;
                self.clear_at = None;
                true
            }
            _ => false,
        }
    }

    /// Whether a seek step is active (`isDoubleTapSeeking`).
    pub fn is_seeking(&self) -> bool {
        self.seeking
    }

    pub fn amount(&self) -> i64 {
        self.amount
    }

    /// The oval to draw, or `None` when there is nothing to show: a zero amount with no text.
    pub fn indicator(&self) -> Option<Indicator> {
        if self.amount == 0 && self.text.is_none() {
            return None;
        }
        Some(Indicator {
            forward: self.amount > 0,
            text: feedback_text(self.amount, self.text.as_deref()),
            alpha: oval_alpha(self.amount),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_direction_taps_add_and_show_the_signed_plural() {
        let mut d = DoubleTap::default();
        d.seek_step(-10, 60, 100, 0);
        d.seek_step(-10, 50, 100, 100);
        let i = d.indicator().expect("visible");
        assert!(!i.forward);
        assert_eq!(i.text, "-20 seconds");
        assert_eq!(i.alpha, 0.2);
    }

    #[test]
    fn a_single_back_step_reads_singular_and_forward_has_no_sign() {
        let mut d = DoubleTap::default();
        d.seek_step(-1, 60, 100, 0);
        assert_eq!(d.indicator().unwrap().text, "-1 second");
        let mut f = DoubleTap::default();
        f.seek_step(10, 0, 100, 0);
        let i = f.indicator().unwrap();
        assert!(i.forward);
        assert_eq!(i.text, "10 seconds");
    }

    #[test]
    fn reversing_direction_starts_from_zero_first() {
        let mut d = DoubleTap::default();
        d.seek_step(-10, 60, 100, 0);
        d.seek_step(-10, 50, 100, 10);
        d.seek_step(10, 30, 100, 20);
        assert_eq!(d.amount(), 10);
    }

    #[test]
    fn repeats_in_the_same_zone_keep_accumulating_while_seeking() {
        let mut d = DoubleTap::default();
        d.seek_step(10, 0, 100, 0);
        assert!(d.is_seeking());
        d.seek_step(10, 10, 100, 300);
        assert_eq!(d.amount(), 20);
    }

    #[test]
    fn each_change_restarts_the_800_ms_clear() {
        let mut d = DoubleTap::default();
        d.seek_step(10, 0, 100, 0);
        d.seek_step(10, 10, 100, 700);
        assert!(!d.tick(1499));
        assert!(d.tick(1500));
        assert!(d.indicator().is_none());
        assert!(!d.is_seeking());
    }

    #[test]
    fn steps_at_the_edges_do_not_accumulate() {
        let mut d = DoubleTap::default();
        d.seek_step(-10, 0, 100, 0);
        assert_eq!(d.amount(), 0);
        d.seek_step(10, 100, 100, 0);
        assert_eq!(d.amount(), 0);
    }

    #[test]
    fn zero_amount_with_text_draws_on_the_left() {
        let mut d = DoubleTap::default();
        d.set_text(Some("Skipped".to_owned()));
        let i = d.indicator().expect("text shows");
        assert!(!i.forward);
        assert_eq!(i.text, "Skipped");
        assert_eq!(i.alpha, 0.0);
    }

    #[test]
    fn nothing_is_drawn_without_amount_or_text() {
        assert!(DoubleTap::default().indicator().is_none());
    }
}
