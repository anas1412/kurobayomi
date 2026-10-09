//! Source refresh decisions of the anime page (T-39b-021, T-39b-061). Upstream: `AnimeScreenModel.kt:350-363` (the
//! error messages), `UpdateAnimeFromRemote` and `SyncEpisodesWithSource` (auto-download). Behaviour 2.4 and 6.2, fix
//! 23.24 (the 10-minute guard), E-REF-3, E-REF-5 and E-CONC-1. Pure: the runner asks these and starts the fetch.

use std::collections::HashSet;

/// A zero-episode entry is fetched again on open only after this long (fix 23.24).
pub const EMPTY_OPEN_REFRESH_GAP_MS: i64 = 10 * 60 * 1000;

/// What started a refresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// The refresh button or the pull-to-refresh gesture.
    Manual,
    /// The first open of the page.
    Open,
}

/// Only a manual refresh auto-downloads the new episodes (E-REF-5, `manualFetch`).
pub fn auto_download_allowed(trigger: Trigger) -> bool {
    trigger == Trigger::Manual
}

/// An open-time refresh of an entry with no episodes waits for the 10-minute guard. A manual refresh always runs,
/// and a list that already has episodes is only fetched when the open asks for it (E-REF-3).
pub fn open_refresh_due(episodes_empty: bool, last_empty_fetch_ms: Option<i64>, now_ms: i64) -> bool {
    match last_empty_fetch_ms {
        Some(last) if episodes_empty => now_ms - last >= EMPTY_OPEN_REFRESH_GAP_MS,
        _ => true,
    }
}

/// Whether a refresh started now is the one that runs, or joins the refresh already in flight (E-CONC-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Begin {
    Started,
    Joined,
}

/// Single-flight refreshes per anime id: a second request while one runs joins it and does not fetch again.
#[derive(Default)]
pub struct RefreshGate {
    running: HashSet<i64>,
}

impl RefreshGate {
    pub fn begin(&mut self, anime_id: i64) -> Begin {
        if self.running.insert(anime_id) { Begin::Started } else { Begin::Joined }
    }

    /// The refresh for `anime_id` has ended, with or without an error.
    pub fn end(&mut self, anime_id: i64) {
        self.running.remove(&anime_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_manual_refresh_auto_downloads() {
        assert!(auto_download_allowed(Trigger::Manual));
        assert!(!auto_download_allowed(Trigger::Open));
    }

    #[test]
    fn zero_episode_open_refresh_waits_ten_minutes() {
        let t0 = 1_000_000;
        assert!(open_refresh_due(true, None, t0), "never fetched: fetch");
        assert!(!open_refresh_due(true, Some(t0), t0 + EMPTY_OPEN_REFRESH_GAP_MS - 1), "second open within 10 min");
        assert!(open_refresh_due(true, Some(t0), t0 + EMPTY_OPEN_REFRESH_GAP_MS), "10 min later: fetch again");
    }

    #[test]
    fn a_non_empty_list_is_never_throttled() {
        assert!(open_refresh_due(false, Some(5), 6));
    }

    #[test]
    fn second_refresh_of_the_same_entry_joins_the_first() {
        let mut gate = RefreshGate::default();
        assert_eq!(gate.begin(7), Begin::Started);
        assert_eq!(gate.begin(7), Begin::Joined);
        assert_eq!(gate.begin(8), Begin::Started, "another entry is independent");
        gate.end(7);
        assert_eq!(gate.begin(7), Begin::Started, "after the end a new refresh starts");
    }
}
