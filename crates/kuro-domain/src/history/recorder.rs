//! History recording policy: the session timer of reading and the playback ticker of watching, one rule for both kinds.
//! Pure timers: the caller passes the clock in epoch ms and writes the returned [`HistoryUpdate`] through
//! [`super::repo::HistoryRepo`]. Incognito mode returns no update at all.

use kuro_platform::kind::Kind;
use crate::history::model::HistoryUpdate;

/// A session never counts for more than this (`12 h`), so a suspended laptop adds no days.
pub const MAX_SESSION_MS: i64 = 12 * 60 * 60 * 1000;

/// Anime: at most one timed write per this much playback.
pub const WATCH_WRITE_INTERVAL_MS: i64 = 5_000;

/// The session timer of either kind. `start` on item open or window focus; `flush` on item change, blur or close.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionTimer {
    started_at: Option<i64>,
}

impl SessionTimer {
    pub fn start(&mut self, now_ms: i64) {
        self.started_at = Some(now_ms);
    }

    /// Ends the current session. The session is `now - start`, clamped to `0..=12 h`, and 0 when there was no start.
    /// The start is cleared in every case, incognito included. Returns `None` in incognito mode.
    pub fn flush(&mut self, now_ms: i64, kind: Kind, item_id: i64, incognito: bool) -> Option<HistoryUpdate> {
        let session_ms = self.started_at.take().map_or(0, |s| (now_ms - s).clamp(0, MAX_SESSION_MS));
        if incognito {
            return None;
        }
        Some(HistoryUpdate { kind, item_id, at: now_ms, session_ms })
    }
}

/// The manga name of [`SessionTimer`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadTimer(SessionTimer);

impl ReadTimer {
    pub fn start(&mut self, now_ms: i64) {
        self.0.start(now_ms);
    }

    /// [`SessionTimer::flush`] for a chapter.
    pub fn flush(&mut self, now_ms: i64, chapter_id: i64, incognito: bool) -> Option<HistoryUpdate> {
        self.0.flush(now_ms, Kind::Manga, chapter_id, incognito)
    }
}

/// The anime playback ticker: `tick` runs every playback second and writes at most every five seconds of playback.
/// `force` writes at once, for pause, seek end, episode change, stop and window close. Each write carries the playback
/// time since the previous write (0 for the first), clamped like a session, so the history stores the watched time.
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
        let session_ms = self.last_write_ms.map_or(0, |last| (now_ms - last).clamp(0, MAX_SESSION_MS));
        self.last_write_ms = Some(now_ms);
        Some(HistoryUpdate { kind: Kind::Anime, item_id: episode_id, at: now_ms, session_ms })
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

    #[test]
    fn the_session_timer_clamps_and_clears_in_incognito_for_both_kinds() {
        for kind in [Kind::Manga, Kind::Anime] {
            let mut t = SessionTimer::default();
            t.start(0);
            assert_eq!(t.flush(MAX_SESSION_MS * 3, kind, 4, false).unwrap().session_ms, MAX_SESSION_MS, "{kind}");
            t.start(10_000);
            assert_eq!(t.flush(5_000, kind, 4, false).unwrap().session_ms, 0, "clock went backwards");
            t.start(0);
            assert_eq!(t.flush(1_000, kind, 4, true), None, "nothing is written in incognito");
            let after = t.flush(9_000, kind, 4, false).unwrap();
            assert_eq!((after.kind, after.session_ms), (kind, 0), "the start was cleared by the incognito flush");
        }
    }

    #[test]
    fn the_watch_ticker_reports_the_playback_delta() {
        let mut w = WatchTicker::default();
        assert_eq!(w.tick(1_000, 9, false).unwrap().session_ms, 0, "the first write has no previous one");
        assert_eq!(w.tick(3_000, 9, false), None, "throttled");
        assert_eq!(w.tick(6_500, 9, false).unwrap().session_ms, 5_500);
        assert_eq!(w.force(7_000, 9, false).unwrap().session_ms, 500, "pause writes the time since the last write");
        assert_eq!(w.force(7_000 + MAX_SESSION_MS * 2, 9, false).unwrap().session_ms, MAX_SESSION_MS, "clamped");
    }
}

