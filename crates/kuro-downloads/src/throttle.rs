//! Rate gates for UI updates (T-23a-034). The shared bytes-per-second limiter (T-23a-015) is not here yet.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Progress for one chapter is shown at most once per this interval (TST-20, RC-05).
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// Lets at most one update per key through per interval. The caller keeps the newest value and redraws it on the
/// next status change, so a dropped progress update never leaves a stale final value.
#[derive(Debug)]
pub struct ProgressThrottle {
    interval: Duration,
    last: HashMap<i64, Instant>,
}

impl ProgressThrottle {
    pub fn new(interval: Duration) -> Self {
        Self { interval, last: HashMap::new() }
    }

    /// `true` when an update for `key` may be shown at `now`. Records `now` when it returns `true`.
    pub fn allow(&mut self, key: i64, now: Instant) -> bool {
        match self.last.get(&key) {
            Some(last) if now.saturating_duration_since(*last) < self.interval => false,
            _ => {
                self.last.insert(key, now);
                true
            }
        }
    }

    /// Ends the gate for `key`: its progress job is cancelled on DOWNLOADED and ERROR (behaviour 14.6).
    pub fn forget(&mut self, key: i64) {
        self.last.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_most_one_update_per_fifty_milliseconds_per_chapter() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut gate = ProgressThrottle::new(PROGRESS_INTERVAL);
        // TST-20: events at 0, 10, 49, 50 and 60 ms show at 0 and 50 ms only.
        let shown: Vec<bool> = [0, 10, 49, 50, 60].into_iter().map(|ms| gate.allow(1, at(ms))).collect();
        assert_eq!(shown, vec![true, false, false, true, false]);
    }

    #[test]
    fn chapters_are_gated_separately() {
        let start = Instant::now();
        let mut gate = ProgressThrottle::new(PROGRESS_INTERVAL);
        assert!(gate.allow(1, start));
        assert!(gate.allow(2, start));
        assert!(!gate.allow(1, start));
    }

    #[test]
    fn forget_ends_the_gate_for_a_finished_chapter() {
        let start = Instant::now();
        let mut gate = ProgressThrottle::new(PROGRESS_INTERVAL);
        assert!(gate.allow(9, start));
        gate.forget(9);
        assert!(gate.allow(9, start), "a fresh gate after forget");
    }
}
