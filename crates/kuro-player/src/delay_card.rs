//! Delay card and the tap-to-sync measure (area 46 `SubtitleDelayPanel.kt` `DelayCard`, T-46b-008,
//! T-46b-031). The card is shared by the subtitle delay panel and the audio delay panel
//! (`DelayKind::Audio`). Pure: the GTK layer feeds button presses with the current `Instant`,
//! ticks its 20 ms redraw from the same clock, and applies the value that `press` returns once.

use std::time::{Duration, Instant};

/// Which delay the card measures. The sign of a run follows the button that started it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayKind {
    Audio,
    Subtitle,
}

/// The two tap buttons of the card. `A` is the first label, `B` the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    A,
    B,
}

/// Direction of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Positive,
    Negative,
}

impl DelayKind {
    /// Button labels (behaviour 11.4).
    pub fn label(self, button: Button) -> &'static str {
        match (self, button) {
            (Self::Audio, Button::A) => "Sound heard",
            (Self::Audio, Button::B) => "Sound spotted",
            (Self::Subtitle, Button::A) => "Voice heard",
            (Self::Subtitle, Button::B) => "Text seen",
        }
    }

    /// The sign a run gets when it starts from `button` (behaviour 11.5 truth table).
    pub fn start_sign(self, button: Button) -> Sign {
        match (self, button) {
            (Self::Audio, Button::A) | (Self::Subtitle, Button::B) => Sign::Positive,
            (Self::Audio, Button::B) | (Self::Subtitle, Button::A) => Sign::Negative,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Run {
    sign: Sign,
    start: Instant,
    base_ms: i64,
}

impl Run {
    /// `start ± elapsed`, with the elapsed time from `Instant` and saturating at zero.
    fn value_at(&self, now: Instant) -> i64 {
        let elapsed = now.saturating_duration_since(self.start);
        let millis = i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX);
        match self.sign {
            Sign::Positive => self.base_ms.saturating_add(millis),
            Sign::Negative => self.base_ms.saturating_sub(millis),
        }
    }
}

/// Result of a button press on the measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Started,
    /// The run ended. The value is the delay to apply once.
    Stopped(i64),
    /// The button is disabled for the current run.
    Ignored,
}

/// The tap-to-sync state machine. `run` is `None` while idle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DelayMeasure {
    kind: DelayKind,
    run: Option<Run>,
}

impl DelayMeasure {
    pub fn new(kind: DelayKind) -> Self {
        Self { kind, run: None }
    }

    pub fn running(&self) -> bool {
        self.run.is_some()
    }

    /// The sign of the current run, `None` while idle.
    pub fn sign(&self) -> Option<Sign> {
        self.run.map(|r| r.sign)
    }

    /// A button is enabled iff the running sign differs from its own start sign, or nothing runs.
    pub fn enabled(&self, button: Button) -> bool {
        match self.run {
            None => true,
            Some(run) => run.sign != self.kind.start_sign(button),
        }
    }

    /// Press a button. Starting a run records `base_ms` as the value the delta is added to. The
    /// other button stops the run and returns the final value.
    pub fn press(&mut self, button: Button, base_ms: i64, now: Instant) -> Press {
        if !self.enabled(button) {
            return Press::Ignored;
        }
        match self.run.take() {
            None => {
                self.run = Some(Run {
                    sign: self.kind.start_sign(button),
                    start: now,
                    base_ms,
                });
                Press::Started
            }
            Some(run) => Press::Stopped(run.value_at(now)),
        }
    }

    /// The value shown while a run is active (the live value), `None` while idle.
    pub fn live_value(&self, now: Instant) -> Option<i64> {
        self.run.map(|r| r.value_at(now))
    }
}

/// The card's state: the measure, the delay in milliseconds and the rules for the buttons below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DelayCard {
    pub measure: DelayMeasure,
    pub delay_ms: i64,
}

impl DelayCard {
    pub fn new(kind: DelayKind, delay_ms: i64) -> Self {
        Self {
            measure: DelayMeasure::new(kind),
            delay_ms,
        }
    }

    /// The number the field shows: the live value during a run, else the stored delay.
    pub fn shown_ms(&self, now: Instant) -> i64 {
        self.measure.live_value(now).unwrap_or(self.delay_ms)
    }

    /// Press a tap button. The final value of a run becomes the delay and is returned once, so the
    /// caller applies it through its single change path.
    pub fn press(&mut self, button: Button, now: Instant) -> Option<i64> {
        match self.measure.press(button, self.delay_ms, now) {
            Press::Stopped(value) => {
                self.delay_ms = value;
                Some(value)
            }
            Press::Started | Press::Ignored => None,
        }
    }

    /// Closing the panel during a run applies the current value (E8 decision). `None` when idle.
    pub fn close(&mut self, now: Instant) -> Option<i64> {
        let value = self.measure.live_value(now)?;
        self.measure = DelayMeasure::new(self.measure.kind);
        self.delay_ms = value;
        Some(value)
    }

    /// "Set as default" and "Reset" are disabled while a run is active.
    pub fn default_and_reset_enabled(&self) -> bool {
        !self.measure.running()
    }
}

/// The tick interval of the live value (behaviour 11.5, 20 ms).
pub const TICK: Duration = Duration::from_millis(20);

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn labels_and_start_signs_follow_the_table() {
        assert_eq!(DelayKind::Audio.label(Button::A), "Sound heard");
        assert_eq!(DelayKind::Subtitle.label(Button::B), "Text seen");
        assert_eq!(DelayKind::Audio.start_sign(Button::A), Sign::Positive);
        assert_eq!(DelayKind::Audio.start_sign(Button::B), Sign::Negative);
        assert_eq!(DelayKind::Subtitle.start_sign(Button::A), Sign::Negative);
        assert_eq!(DelayKind::Subtitle.start_sign(Button::B), Sign::Positive);
    }

    #[test]
    fn idle_enables_both_and_a_run_disables_its_own_start_button() {
        let start = Instant::now();
        let mut m = DelayMeasure::new(DelayKind::Subtitle);
        assert!(m.enabled(Button::A) && m.enabled(Button::B));
        assert_eq!(m.press(Button::A, 0, start), Press::Started);
        assert_eq!(m.sign(), Some(Sign::Negative));
        assert!(!m.enabled(Button::A));
        assert!(m.enabled(Button::B));
    }

    #[test]
    fn the_other_button_stops_and_the_value_is_start_plus_or_minus_elapsed() {
        let start = Instant::now();
        let mut m = DelayMeasure::new(DelayKind::Audio);
        m.press(Button::A, 100, start);
        assert_eq!(m.press(Button::A, 100, at(start, 350)), Press::Ignored);
        assert_eq!(m.press(Button::B, 100, at(start, 350)), Press::Stopped(450));

        let mut neg = DelayMeasure::new(DelayKind::Audio);
        neg.press(Button::B, 100, start);
        assert_eq!(neg.press(Button::A, 100, at(start, 40)), Press::Stopped(60));
        assert!(!neg.running());
    }

    #[test]
    fn subtitle_voice_heard_moves_the_delay_down_and_text_seen_stops_it() {
        let start = Instant::now();
        let mut card = DelayCard::new(DelayKind::Subtitle, 500);
        assert_eq!(card.press(Button::A, start), None);
        assert_eq!(card.shown_ms(at(start, 1200)), -700);
        assert_eq!(card.press(Button::B, at(start, 1200)), Some(-700));
        assert_eq!(card.delay_ms, -700);

        let mut text = DelayCard::new(DelayKind::Subtitle, 0);
        text.press(Button::B, start);
        assert_eq!(text.press(Button::A, at(start, 300)), Some(300));
    }

    #[test]
    fn elapsed_saturates_when_the_clock_goes_backwards() {
        let start = Instant::now();
        let mut m = DelayMeasure::new(DelayKind::Subtitle);
        m.press(Button::B, 0, at(start, 500));
        assert_eq!(m.press(Button::A, 0, start), Press::Stopped(0));
    }

    #[test]
    fn live_value_shows_the_run_and_idle_shows_the_stored_delay() {
        let start = Instant::now();
        let mut card = DelayCard::new(DelayKind::Audio, 10);
        assert_eq!(card.shown_ms(start), 10);
        card.press(Button::A, start);
        assert_eq!(card.shown_ms(at(start, 60)), 70);
        assert!(!card.default_and_reset_enabled());
    }

    #[test]
    fn closing_during_a_run_applies_the_current_value_once() {
        let start = Instant::now();
        let mut card = DelayCard::new(DelayKind::Audio, 0);
        card.press(Button::B, start);
        assert_eq!(card.close(at(start, 250)), Some(-250));
        assert_eq!(card.delay_ms, -250);
        assert_eq!(card.close(at(start, 300)), None);
        assert!(card.default_and_reset_enabled());
    }

    #[test]
    fn audio_positive_run_ends_on_the_second_button() {
        let start = Instant::now();
        let mut card = DelayCard::new(DelayKind::Audio, 0);
        card.press(Button::A, start);
        assert_eq!(card.press(Button::A, at(start, 120)), None);
        assert_eq!(card.press(Button::B, at(start, 120)), Some(120));
        assert!(card.default_and_reset_enabled());
    }
}
