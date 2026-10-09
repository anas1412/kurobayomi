//! Audio delay panel model (area 46 `AudioDelayPanel.kt`, T-46a-030). mpv stores the delay in
//! seconds; the panel works in whole milliseconds, truncated toward zero. Every change is written to
//! mpv at once. Apply persists the value, Reset zeroes it without persisting, and closing without
//! Apply keeps the session value in mpv.

use crate::delay_card::{DelayCard, DelayKind};

/// mpv `audio-delay` seconds to panel milliseconds (`(seconds * 1000).toInt()`).
pub fn ms_from_mpv(seconds: f64) -> i64 {
    (seconds * 1000.0) as i64
}

/// Panel milliseconds to the mpv `audio-delay` value (`delay / 1000.0`).
pub fn mpv_from_ms(ms: i64) -> f64 {
    ms as f64 / 1000.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DelayPanel {
    live_ms: i64,
    persisted_ms: i64,
}

impl DelayPanel {
    /// Opens with the session value from mpv and the saved `pref_audio_delay`.
    pub fn open(mpv_seconds: f64, persisted_ms: i64) -> Self {
        Self {
            live_ms: ms_from_mpv(mpv_seconds),
            persisted_ms,
        }
    }

    pub fn live_ms(&self) -> i64 {
        self.live_ms
    }

    /// A change from the field or the repeat buttons. Returns the value to write to mpv.
    pub fn change(&mut self, ms: i64) -> f64 {
        self.live_ms = ms;
        mpv_from_ms(ms)
    }

    /// Reset: zero in the session, not persisted. Returns the value to write to mpv.
    pub fn reset(&mut self) -> f64 {
        self.change(0)
    }

    /// Apply: returns the value to store as `pref_audio_delay`.
    pub fn apply(&mut self) -> i64 {
        self.persisted_ms = self.live_ms;
        self.persisted_ms
    }

    pub fn persisted_ms(&self) -> i64 {
        self.persisted_ms
    }

    /// The tap-to-sync card of this panel (`DelayKind::Audio`, the shared `DelayCard`), starting
    /// from the live value.
    pub fn tap_card(&self) -> DelayCard {
        DelayCard::new(DelayKind::Audio, self.live_ms)
    }

    /// A finished tap run: the value goes through the same change path as the field.
    pub fn finish_tap(&mut self, ms: i64) -> f64 {
        self.change(ms)
    }
}

/// Coalesces `audio-delay` writes during fast changes: only the latest value is kept until the mpv
/// thread takes it (E-46-081, test 44).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioDelayCoalescer {
    pending: Option<f64>,
}

impl AudioDelayCoalescer {
    /// A new value replaces any value not yet taken.
    pub fn push(&mut self, seconds: f64) {
        self.pending = Some(seconds);
    }

    /// The value to write now, if any. Taking empties the slot.
    pub fn take(&mut self) -> Option<f64> {
        self.pending.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpv_seconds_convert_with_truncation() {
        assert_eq!(ms_from_mpv(1.2345), 1234);
        assert_eq!(ms_from_mpv(-0.0505), -50);
        assert_eq!(mpv_from_ms(-250), -0.25);
    }

    #[test]
    fn apply_persists_but_reset_does_not() {
        let mut panel = DelayPanel::open(0.2, 100);
        assert_eq!(panel.live_ms(), 200);
        assert_eq!(panel.change(350), 0.35);
        assert_eq!(panel.reset(), 0.0);
        assert_eq!(panel.persisted_ms(), 100);
        panel.change(-40);
        assert_eq!(panel.apply(), -40);
        assert_eq!(panel.persisted_ms(), -40);
    }

    #[test]
    fn a_tap_run_on_the_audio_card_lands_in_the_panel_once() {
        use crate::delay_card::Button;
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let mut panel = DelayPanel::open(0.2, 0);
        let mut card = panel.tap_card();
        card.press(Button::A, start);
        let value = card.press(Button::B, start + Duration::from_millis(250));
        assert_eq!(value, Some(450));
        assert_eq!(panel.finish_tap(value.unwrap()), 0.45);
        assert_eq!(panel.live_ms(), 450);
        assert_eq!(panel.persisted_ms(), 0);
    }

    #[test]
    fn audio_delay_coalescing_keeps_only_the_last_value() {
        let mut c = AudioDelayCoalescer::default();
        assert_eq!(c.take(), None);
        c.push(0.1);
        c.push(0.2);
        c.push(-0.35);
        assert_eq!(c.take(), Some(-0.35));
        assert_eq!(c.take(), None);
    }

    #[test]
    fn closing_without_apply_leaves_the_pref_unchanged() {
        let mut panel = DelayPanel::open(0.0, 75);
        panel.change(500);
        assert_eq!(panel.persisted_ms(), 75);
    }
}
