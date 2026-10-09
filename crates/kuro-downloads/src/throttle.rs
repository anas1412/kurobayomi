//! Rate gates for UI updates (T-23a-034) and the shared download speed limit (T-23a-015).

use std::collections::HashMap;
use std::sync::Mutex;
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

/// One byte budget shared by every concurrent page download (T-23a-015). Each chunk books its bytes after the last
/// booked byte and waits until its own share has passed, so the total rate stays at the limit however many pages run.
/// `None` is unlimited.
#[derive(Debug)]
pub struct SpeedLimiter {
    state: Mutex<LimiterState>,
}

#[derive(Debug)]
struct LimiterState {
    bytes_per_sec: Option<u64>,
    /// When the bytes booked so far are paid for.
    free_at: Instant,
}

impl SpeedLimiter {
    pub fn new(bytes_per_sec: Option<u64>) -> Self {
        Self { state: Mutex::new(LimiterState { bytes_per_sec, free_at: Instant::now() }) }
    }

    /// Changes the rate. Called at each chapter start from the stored `download_speed_limit`.
    pub fn set_rate(&self, bytes_per_sec: Option<u64>) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).bytes_per_sec = bytes_per_sec;
    }

    /// Books `bytes` at `now` and returns the instant at which they are paid for. `None` when unlimited.
    pub fn book(&self, bytes: u64, now: Instant) -> Option<Instant> {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let rate = st.bytes_per_sec.filter(|r| *r > 0)?;
        let start = st.free_at.max(now);
        st.free_at = start + Duration::from_secs_f64(bytes as f64 / rate as f64);
        Some(st.free_at)
    }

    /// Waits until `bytes` read from the network are paid for.
    pub async fn take(&self, bytes: u64) {
        if let Some(deadline) = self.book(bytes, Instant::now()) {
            tokio::time::sleep_until(deadline.into()).await;
        }
    }
}

#[cfg(test)]
mod speed_tests {
    use super::*;

    // TST-26
    #[test]
    fn unlimited_books_nothing() {
        let limiter = SpeedLimiter::new(None);
        assert!(limiter.book(1 << 20, Instant::now()).is_none());
        limiter.set_rate(Some(0));
        assert!(limiter.book(1 << 20, Instant::now()).is_none(), "zero means unlimited");
    }

    // TST-26
    #[test]
    fn one_mebibyte_at_512_kib_per_second_takes_two_seconds() {
        let limiter = SpeedLimiter::new(Some(512 * 1024));
        let t0 = Instant::now();
        let first = limiter.book(1 << 20, t0).unwrap();
        assert_eq!(first.duration_since(t0).as_millis(), 2000);
    }

    #[test]
    fn concurrent_pages_share_one_budget() {
        let limiter = SpeedLimiter::new(Some(512 * 1024));
        let t0 = Instant::now();
        let a = limiter.book(512 * 1024, t0).unwrap();
        let b = limiter.book(512 * 1024, t0).unwrap();
        assert_eq!(a.duration_since(t0).as_millis(), 1000);
        assert_eq!(b.duration_since(t0).as_millis(), 2000, "the second page waits behind the first");
    }

    #[tokio::test(start_paused = true)]
    async fn take_waits_for_the_booked_share() {
        let limiter = SpeedLimiter::new(Some(1024));
        let start = tokio::time::Instant::now();
        limiter.take(1024).await;
        assert_eq!(start.elapsed().as_secs(), 1);
    }
}
