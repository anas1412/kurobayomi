//! Download progress: an `i32` in `0..=100`, where 0 is indeterminate in the UI (T-24a-002, behaviour 5.7).

use tokio::sync::watch;

/// Clamps a raw percent to `0..=100`.
pub fn clamp_percent(value: i64) -> i32 {
    value.clamp(0, 100) as i32
}

/// Percent for an ffmpeg run: `100 * out / duration` with integer truncation. 0 when the duration or the output time is
/// 0 (or negative), and clamped to 100 when the output is ahead of the duration.
pub fn ffmpeg_percent(out_seconds: i64, duration_seconds: i64) -> i32 {
    if duration_seconds <= 0 || out_seconds <= 0 {
        return 0;
    }
    clamp_percent(out_seconds.saturating_mul(100) / duration_seconds)
}

/// The published progress value. Subscribers see a change only when the integer changes.
pub struct ProgressCell {
    tx: watch::Sender<i32>,
}

impl Default for ProgressCell {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressCell {
    pub fn new() -> Self {
        Self { tx: watch::channel(0).0 }
    }

    pub fn get(&self) -> i32 {
        *self.tx.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<i32> {
        self.tx.subscribe()
    }

    /// Stores `value`, clamped. Returns `true` only when the stored integer changed.
    pub fn set(&self, value: i64) -> bool {
        let next = clamp_percent(value);
        self.tx.send_if_modified(|current| {
            if *current == next {
                false
            } else {
                *current = next;
                true
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_clamped_to_zero_and_one_hundred() {
        assert_eq!(clamp_percent(-5), 0);
        assert_eq!(clamp_percent(42), 42);
        assert_eq!(clamp_percent(250), 100);
    }

    #[test]
    fn ffmpeg_percent_truncates_and_handles_zero_inputs() {
        assert_eq!(ffmpeg_percent(0, 1440), 0, "output time 0");
        assert_eq!(ffmpeg_percent(600, 0), 0, "duration 0");
        assert_eq!(ffmpeg_percent(600, -1), 0, "unknown duration");
        assert_eq!(ffmpeg_percent(1, 3), 33, "100 * 1 / 3 truncates to 33");
        assert_eq!(ffmpeg_percent(720, 1440), 50);
        assert_eq!(ffmpeg_percent(2000, 1440), 100, "out past the duration is clamped");
    }

    #[test]
    fn published_value_changes_only_when_the_integer_changes() {
        let cell = ProgressCell::new();
        let mut rx = cell.subscribe();
        assert!(cell.set(10));
        assert!(!cell.set(10), "same value is not published again");
        assert!(cell.set(-3), "a clamped change to 0 is a change");
        assert_eq!(cell.get(), 0);
        assert!(cell.set(250));
        assert_eq!(cell.get(), 100);
        assert!(rx.has_changed().unwrap());
        assert_eq!(*rx.borrow_and_update(), 100);
    }
}
