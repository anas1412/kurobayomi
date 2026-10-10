//! Completion side effects and duplicate marking (area 45 p2 T-45b-027; behaviour 7.2, edge E-14,
//! E-17, E-50). Pure: the caller writes the returned updates and the tracker sync.

use std::collections::HashSet;

use kuro_data::episode::Episode;
use kuro_domain::items::model::EpisodeExt;

use crate::progress::tracker_sync_allowed;

/// The string-set value of `mark_duplicate_seen_episode_seen` that enables duplicate marking.
pub const MARK_DUPLICATE_EXISTING: &str = "existing_episode";

/// Completion runs once per episode (E-14). The episode is marked seen in memory before the save.
#[derive(Debug, Default)]
pub struct Completion {
    done: HashSet<i64>,
}

impl Completion {
    /// Marks the episode seen and returns `true` the first time it completes. A second completion of
    /// the same episode changes nothing and returns `false`.
    pub fn complete(&mut self, episode: &mut Episode) -> bool {
        if !self.done.insert(episode.id) {
            return false;
        }
        episode.seen = true;
        true
    }
}

/// The ids to mark seen when `completed` finishes (behaviour 7.2 steps 4 to 7). Only the filtered
/// playlist counts, so duplicates the filter hides are not touched. An episode qualifies when it is
/// unseen, has a recognised number, and has the same number as the completed one. Nothing is marked
/// unless `prefs` contains `existing_episode`.
pub fn duplicate_ids(playlist: &[Episode], completed: &Episode, prefs: &[String]) -> Vec<i64> {
    if !prefs.iter().any(|p| p == MARK_DUPLICATE_EXISTING) {
        return Vec::new();
    }
    playlist
        .iter()
        .filter(|e| {
            !e.seen
                && e.id != completed.id
                && e.is_recognized_number()
                && e.episode_number == completed.episode_number
        })
        .map(|e| e.id)
        .collect()
}

/// The tracker sync rule of completion (E-17): the app-wide incognito flag decides, not the source's.
pub fn should_sync_tracker(incognito_global: bool, has_trackers: bool, auto_update: bool) -> bool {
    tracker_sync_allowed(incognito_global, has_trackers, auto_update)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(id: i64, number: f64, seen: bool) -> Episode {
        Episode {
            id,
            episode_number: number,
            seen,
            ..Episode::create()
        }
    }

    fn marker() -> Vec<String> {
        vec![MARK_DUPLICATE_EXISTING.to_owned()]
    }

    #[test]
    fn t54_completion_marks_seen_once_per_episode() {
        let mut completion = Completion::default();
        let mut ep = episode(1, 1.0, false);
        assert!(completion.complete(&mut ep));
        assert!(ep.seen);
        ep.seen = false;
        assert!(!completion.complete(&mut ep));
        assert!(!ep.seen);
    }

    #[test]
    fn duplicates_with_the_same_number_are_marked_when_enabled() {
        let completed = episode(1, 5.0, true);
        let playlist = vec![
            completed.clone(),
            episode(2, 5.0, false),
            episode(3, 5.0, true),
            episode(4, 6.0, false),
        ];
        assert_eq!(duplicate_ids(&playlist, &completed, &marker()), vec![2]);
    }

    #[test]
    fn no_duplicates_marked_without_the_preference() {
        let completed = episode(1, 5.0, true);
        let playlist = vec![completed.clone(), episode(2, 5.0, false)];
        assert!(duplicate_ids(&playlist, &completed, &[]).is_empty());
    }

    #[test]
    fn unrecognised_numbers_are_never_duplicates() {
        let completed = episode(1, -1.0, true);
        let playlist = vec![completed.clone(), episode(2, -1.0, false)];
        assert!(duplicate_ids(&playlist, &completed, &marker()).is_empty());
    }

    #[test]
    fn e17_tracker_sync_follows_the_global_incognito_flag_only() {
        assert!(should_sync_tracker(false, true, true));
        assert!(!should_sync_tracker(true, true, true));
        assert!(!should_sync_tracker(false, false, true));
        assert!(!should_sync_tracker(false, true, false));
    }
}
