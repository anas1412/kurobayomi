//! Source refresh decisions of the anime page (T-39b-021, T-39b-061). Upstream: `AnimeScreenModel.kt:350-363` (the
//! error messages), `UpdateAnimeFromRemote` and `SyncEpisodesWithSource` (auto-download). Behaviour 2.4 and 6.2, fix
//! 23.24 (the 10-minute guard), E-REF-3, E-REF-5 and E-CONC-1. Pure: the runner asks these and starts the fetch.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, PoisonError};


/// A zero-episode entry is fetched again on open only after this long (fix 23.24).
pub const EMPTY_OPEN_REFRESH_GAP_MS: i64 = 10 * 60 * 1000;

/// When an open-time refresh last started for an entry with no episodes, in Unix ms. Process-wide, so every page of
/// the same entry shares the guard.
static EMPTY_OPEN_FETCHES: LazyLock<Mutex<HashMap<i64, i64>>> = LazyLock::new(Mutex::default);

/// The time of the last open-time refresh of `anime_id` that found no episodes, if any.
pub fn last_empty_open_fetch(anime_id: i64) -> Option<i64> {
    EMPTY_OPEN_FETCHES.lock().unwrap_or_else(PoisonError::into_inner).get(&anime_id).copied()
}

/// Remembers that an open-time refresh of `anime_id` started at `now_ms` for an entry with no episodes.
pub fn record_empty_open_fetch(anime_id: i64, now_ms: i64) {
    EMPTY_OPEN_FETCHES.lock().unwrap_or_else(PoisonError::into_inner).insert(anime_id, now_ms);
}

/// What started a refresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// The refresh button or F5 (T-39a-038).
    Manual,
    /// The first open of the page.
    Open,
}

/// Only a manual refresh auto-downloads the new episodes (E-REF-5, `manualFetch`).
pub fn auto_download_allowed(trigger: Trigger) -> bool {
    trigger == Trigger::Manual
}

/// The notification a failed refresh shows (behaviour 19): the error text. A cancellation shows nothing, and neither do
/// the two empty-list errors: the list's empty state says them, with Retry.
pub fn refresh_snack(error: &crate::entry_core::errors::DetailError) -> Option<crate::entry_core::message::UserMessage> {
    use crate::entry_core::errors::DetailError;
    use crate::entry_core::message::UserMessage;
    match error {
        DetailError::Cancelled | DetailError::NoEpisodes | DetailError::NoSeasons => None,
        other => other.refresh_message().map(UserMessage::Plain),
    }
}

/// The new episodes to queue after a refresh (T-REF-4, behaviour 5.1): only a manual refresh auto-downloads (E-REF-5).
/// `enabled` is the `download_new_episode` preference, `eligible` the category rule for this entry, and `unseen_only`
/// the "skip duplicate seen episodes" preference. `existing` is the stored list the numbers are compared against.
pub fn auto_download_targets(
    trigger: Trigger,
    enabled: bool,
    eligible: bool,
    unseen_only: bool,
    new_episodes: &[kuro_data::item::Item],
    existing: &[kuro_data::item::Item],
) -> Vec<kuro_data::item::Item> {
    if !auto_download_allowed(trigger) {
        return Vec::new();
    }
    kuro_domain::download::filter::filter_new(enabled, eligible, unseen_only, new_episodes, existing)
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
    fn the_open_refresh_never_auto_downloads_and_a_manual_one_follows_the_filter() {
        use kuro_data::item::Item;
        let ep = |number: f64, seen: bool| Item { number, done: seen, ..Item::create(kuro_platform::kind::Kind::Anime) };
        let new = [ep(3.0, false), ep(2.0, false)];
        let existing = [ep(2.0, true)];
        assert!(auto_download_targets(Trigger::Open, true, true, false, &new, &existing).is_empty(), "E-REF-5");
        assert_eq!(auto_download_targets(Trigger::Manual, true, true, false, &new, &existing).len(), 2);
        let only_unseen = auto_download_targets(Trigger::Manual, true, true, true, &new, &existing);
        assert_eq!(only_unseen.iter().map(|e| e.number).collect::<Vec<_>>(), [3.0], "a seen duplicate is skipped");
        assert!(auto_download_targets(Trigger::Manual, false, true, false, &new, &existing).is_empty(), "the preference is off");
        assert!(auto_download_targets(Trigger::Manual, true, false, false, &new, &existing).is_empty(), "an excluded category");
    }

    #[test]
    fn a_failed_refresh_shows_its_text_and_a_cancelled_one_nothing() {
        use crate::entry_core::errors::DetailError;
        use crate::entry_core::message::text;
        assert_eq!(refresh_snack(&DetailError::NoEpisodes), None, "the empty list says it");
        assert_eq!(refresh_snack(&DetailError::NoSeasons), None, "the empty list says it");
        assert_eq!(refresh_snack(&DetailError::Cancelled), None);
        assert_eq!(text(&refresh_snack(&DetailError::Http(500)).expect("a snack")), DetailError::Http(500).user_message());
    }

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
    fn the_empty_open_fetch_time_is_remembered_per_entry() {
        let id = 910_001;
        assert_eq!(last_empty_open_fetch(id), None);
        record_empty_open_fetch(id, 5_000);
        assert_eq!(last_empty_open_fetch(id), Some(5_000));
        assert_eq!(last_empty_open_fetch(id + 1), None, "another entry has its own guard");
        assert!(!open_refresh_due(true, last_empty_open_fetch(id), 5_000 + 1));
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
