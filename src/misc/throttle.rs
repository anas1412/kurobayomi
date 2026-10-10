//! Progress throttle of a notice (T-35a-008): the Android rule of the update download. A percent is emitted when it is
//! strictly greater than the last emitted percent and strictly more than 200 ms passed since the last emit.

use std::time::{Duration, Instant};

pub struct ProgressThrottle {
    last_pct: u8,
    last_emit: Option<Instant>,
    min_gap: Duration,
}

impl Default for ProgressThrottle {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressThrottle {
    pub fn new() -> Self {
        Self { last_pct: 0, last_emit: None, min_gap: Duration::from_millis(200) }
    }

    /// `total` of `None` or 0 is an unknown length: nothing is ever emitted. The percent truncates toward zero.
    pub fn offer(&mut self, now: Instant, bytes: u64, total: Option<u64>) -> Option<u8> {
        let total = total.filter(|t| *t > 0)?;
        let pct = ((100.0 * bytes as f64) / total as f64).clamp(0.0, 100.0) as u8;
        if pct <= self.last_pct {
            return None;
        }
        if self.last_emit.is_some_and(|prev| now.duration_since(prev) <= self.min_gap) {
            return None;
        }
        self.last_pct = pct;
        self.last_emit = Some(now);
        Some(pct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_5_3_sequence_emits_only_strict_increases_after_200_ms() {
        // p1-edge-cases-and-tests.md 5.3: 10, 20 at 100 ms, 20 at 200 ms, 20 at 201 ms, 20 at 300 ms, 21 at 450 ms
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut throttle = ProgressThrottle::new();
        assert_eq!(throttle.offer(at(0), 10, Some(100)), Some(10));
        assert_eq!(throttle.offer(at(100), 20, Some(100)), None, "inside the gap");
        assert_eq!(throttle.offer(at(200), 20, Some(100)), None, "exactly 200 ms is not more than 200 ms");
        assert_eq!(throttle.offer(at(201), 20, Some(100)), Some(20));
        assert_eq!(throttle.offer(at(300), 20, Some(100)), None, "the percent did not grow");
        assert_eq!(throttle.offer(at(450), 21, Some(100)), Some(21));
    }

    #[test]
    fn an_unknown_total_never_emits() {
        let mut throttle = ProgressThrottle::new();
        let now = Instant::now();
        assert_eq!(throttle.offer(now, 50, None), None);
        assert_eq!(throttle.offer(now, 50, Some(0)), None);
        assert_eq!(throttle.offer(now, 0, Some(100)), None, "0 percent is not greater than the start");
    }
}
