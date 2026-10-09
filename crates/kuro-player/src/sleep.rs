//! Sleep timer dialog and button (area 46 `MoreSheet.kt` and `TimePickerDialog`, T-46b-018). Pure:
//! the spin buttons are `SleepPicker`, and the More-sheet button text is `timer_label`. The timer
//! itself (countdown and the action when it fires) is in the player core.

use crate::time::pretty_time;

/// Hours the spin button accepts (E79: above 23 is allowed).
pub const MAX_HOURS: u32 = 99;
/// Minutes wrap in 0..=59.
pub const MINUTES: u32 = 60;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SleepPicker {
    hours: u32,
    minutes: u32,
}

impl SleepPicker {
    /// Starts from the remaining time in seconds, as the dialog does. Hours above `MAX_HOURS` are
    /// clamped to the spin range.
    pub fn from_remaining(remaining_secs: u64) -> Self {
        Self {
            hours: ((remaining_secs / 3600) as u32).min(MAX_HOURS),
            minutes: ((remaining_secs % 3600) / 60) as u32,
        }
    }

    pub fn hours(&self) -> u32 {
        self.hours
    }

    pub fn minutes(&self) -> u32 {
        self.minutes
    }

    pub fn set_hours(&mut self, hours: u32) {
        self.hours = hours.min(MAX_HOURS);
    }

    /// Minutes wrap at both ends, as a wrapping spin button does. The hour does not change.
    pub fn step_minutes(&mut self, delta: i32) {
        self.minutes = (self.minutes as i32 + delta).rem_euclid(MINUTES as i32) as u32;
    }

    /// The timer length in seconds that OK sends. Zero cancels a running timer (E80).
    pub fn seconds(&self) -> u32 {
        self.hours * 3600 + self.minutes * 60
    }
}

/// The More-sheet button text: `Sleep Timer` when idle, `<remaining> remaining` while running.
pub fn timer_label(remaining_secs: Option<u64>) -> String {
    match remaining_secs {
        Some(secs) if secs > 0 => format!("{} remaining", pretty_time(secs as i64)),
        _ => "Sleep Timer".to_owned(),
    }
}

/// `Cancel current timer` is offered only while a timer runs.
pub fn cancel_visible(remaining_secs: Option<u64>) -> bool {
    remaining_secs.is_some_and(|secs| secs > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_starts_from_the_remaining_time() {
        let p = SleepPicker::from_remaining(3723);
        assert_eq!((p.hours(), p.minutes()), (1, 2));
        assert_eq!(SleepPicker::from_remaining(0).seconds(), 0);
    }

    #[test]
    fn minutes_wrap_and_hours_stay() {
        let mut p = SleepPicker::from_remaining(3599);
        assert_eq!(p.minutes(), 59);
        p.step_minutes(1);
        assert_eq!((p.hours(), p.minutes()), (0, 0));
        p.step_minutes(-1);
        assert_eq!((p.hours(), p.minutes()), (0, 59));
    }

    #[test]
    fn hours_accept_above_23_up_to_99() {
        let mut p = SleepPicker::default();
        p.set_hours(30);
        assert_eq!(p.seconds(), 30 * 3600);
        p.set_hours(150);
        assert_eq!(p.hours(), MAX_HOURS);
    }

    #[test]
    fn ok_sends_hours_and_minutes_in_seconds() {
        let mut p = SleepPicker::default();
        p.set_hours(1);
        p.step_minutes(5);
        assert_eq!(p.seconds(), 3900);
    }

    #[test]
    fn label_and_cancel_follow_the_running_state() {
        assert_eq!(timer_label(None), "Sleep Timer");
        assert_eq!(timer_label(Some(0)), "Sleep Timer");
        assert_eq!(timer_label(Some(3723)), "1:02:03 remaining");
        assert_eq!(timer_label(Some(65)), "01:05 remaining");
        assert!(!cancel_visible(None));
        assert!(cancel_visible(Some(10)));
    }
}
