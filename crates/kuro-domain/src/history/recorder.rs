//! History recording policy (`HistoryRecorder` for manga, the playback ticker for anime). Pure timers: the caller
//! passes the clock in epoch ms and writes the returned [`HistoryUpdate`] through [`super::repo::HistoryRepo`].
//! Incognito mode returns no update at all.

use crate::history::model::HistoryUpdate;
use crate::kind::EntryKind;

/// A reading session never counts for more than this (`12 h`), so a suspended laptop adds no days.
pub const MAX_SESSION_MS: i64 = 12 * 60 * 60 * 1000;

/// Anime: at most one timed write per this much playback.
pub const WATCH_WRITE_INTERVAL_MS: i64 = 5_000;

/// The manga read timer. `start` on chapter open or window focus; `flush` on chapter change, blur or close.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadTimer {
    started_at: Option<i64>,
}

impl ReadTimer {
    pub fn start(&mut self, now_ms: i64) {
        self.started_at = Some(now_ms);
    }

    /// Ends the current session. The session is `now - start`, clamped to `0..=12 h`, and 0 when there was no start.
    /// The start is cleared either way. Returns `None` in incognito mode.
    pub fn flush(&mut self, now_ms: i64, chapter_id: i64, incognito: bool) -> Option<HistoryUpdate> {
        let session_ms = self.started_at.take().map_or(0, |s| (now_ms - s).clamp(0, MAX_SESSION_MS));
        if incognito {
            return None;
        }
        Some(HistoryUpdate { kind: EntryKind::Manga, item_id: chapter_id, at: now_ms, session_ms })
    }
}

/// The anime playback ticker: `tick` runs every playback second and writes at most every five seconds of playback.
/// `force` writes at once, for pause, seek end, episode change, stop and window close.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WatchTicker {
    last_write_ms: Option<i64>,
}

impl WatchTicker {
    pub fn tick(&mut self, now_ms: i64, episode_id: i64, incognito: bool) -> Option<HistoryUpdate> {
        match self.last_write_ms {
            Some(last) if now_ms - last < WATCH_WRITE_INTERVAL_MS => None,
            _ => self.force(now_ms, episode_id, incognito),
        }
    }

    pub fn force(&mut self, now_ms: i64, episode_id: i64, incognito: bool) -> Option<HistoryUpdate> {
        if incognito {
            return None;
        }
        self.last_write_ms = Some(now_ms);
        Some(HistoryUpdate { kind: EntryKind::Anime, item_id: episode_id, at: now_ms, session_ms: 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_without_a_start_records_zero_and_clamps_both_ways() {
        let mut t = ReadTimer::default();
        let u = t.flush(1_000, 7, false).unwrap();
        assert_eq!((u.item_id, u.at, u.session_ms), (7, 1_000, 0));
        t.start(10_000);
        assert_eq!(t.flush(5_000, 7, false).unwrap().session_ms, 0, "clock went backwards");
        t.start(0);
        assert_eq!(t.flush(MAX_SESSION_MS * 3, 7, false).unwrap().session_ms, MAX_SESSION_MS);
    }

    #[test]
    fn a_second_flush_adds_nothing_but_still_bumps_the_timestamp() {
        let mut t = ReadTimer::default();
        t.start(0);
        assert_eq!(t.flush(4_000, 1, false).unwrap().session_ms, 4_000);
        let again = t.flush(9_000, 1, false).unwrap();
        assert_eq!((again.session_ms, again.at), (0, 9_000));
    }

    #[test]
    fn incognito_writes_nothing() {
        let mut t = ReadTimer::default();
        t.start(0);
        assert_eq!(t.flush(100, 1, true), None);
        let mut w = WatchTicker::default();
        assert_eq!(w.tick(0, 1, true), None);
        assert_eq!(w.force(0, 1, true), None);
    }

    #[test]
    fn six_hundred_ticks_in_ten_seconds_write_at_most_three_times() {
        let mut w = WatchTicker::default();
        let writes = (0..600).filter_map(|i| w.tick(i * 10_000 / 600, 9, false)).count();
        assert!(writes <= 3, "got {writes}");
        assert!(writes >= 2);
    }

    #[test]
    fn force_writes_at_once_and_resets_the_throttle() {
        let mut w = WatchTicker::default();
        assert!(w.force(0, 9, false).is_some());
        assert_eq!(w.tick(1_000, 9, false), None, "throttled for five seconds");
        assert!(w.force(2_000, 9, false).is_some(), "pause writes unconditionally");
        assert_eq!(w.tick(3_000, 9, false), None, "the forced write restarts the window");
        assert!(w.tick(7_000, 9, false).is_some());
    }
}
