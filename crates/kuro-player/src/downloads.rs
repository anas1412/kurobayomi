//! Download-ahead and delete-after-read (area 45 `PlayerViewModel.onSecondReached`,
//! `downloadNextEpisodes` and `deleteEpisodeIfNeeded`; behaviour 7.6 and 7.7; E-13; T-45b-029,
//! T-45a-065). Pure: the caller hands the decisions to the download manager.

use std::collections::HashSet;

use kuro_domain::download::policy::remove_after_read_index;
use kuro_domain::entries::anime::episode_flags;

/// Download-ahead starts once more than this fraction of the episode has played.
pub const DOWNLOAD_AHEAD_FRACTION: f64 = 0.35;

/// Whether playback is past the download-ahead point: `seconds / totalSeconds > 0.35`. An unknown
/// duration (zero) never passes.
pub fn past_download_ahead(seconds: i64, total_seconds: i64) -> bool {
    total_seconds > 0 && seconds as f64 / total_seconds as f64 > DOWNLOAD_AHEAD_FRACTION
}

/// Download-ahead state for the session. Each episode starts at most once (E-13): the window asks
/// every second, and the first request that qualifies records the episode.
#[derive(Debug, Default)]
pub struct DownloadAhead {
    started: HashSet<i64>,
}

impl DownloadAhead {
    /// The episodes to queue now, or none. `following` lists the episodes after the next one in
    /// play order; at most `amount` of them are taken. Nothing is queued unless the current and the
    /// next episode are both downloaded already, and the episode has not started before.
    pub fn plan(
        &mut self,
        current_id: i64,
        past_point: bool,
        current_downloaded: bool,
        next_downloaded: bool,
        amount: i32,
        following: &[i64],
    ) -> Vec<i64> {
        if amount <= 0
            || !past_point
            || !current_downloaded
            || !next_downloaded
            || self.started.contains(&current_id)
        {
            return Vec::new();
        }
        self.started.insert(current_id);
        following
            .iter()
            .take(usize::try_from(amount).unwrap_or(0))
            .copied()
            .collect()
    }
}

/// The anime sorting is one the delete rule understands. The Kotlin code throws for the alphabet
/// order, so the Rust port refuses it too and deletes nothing (EC-103).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownSorting(pub i64);

/// The index in `sorted` of the seen episode to delete after `current` was completed (behaviour
/// 7.6). The target is `current - slots`; with `slots` of `-1` (or any negative value) nothing is
/// deleted, and a target before the first episode deletes nothing. With `slots = 0` the current
/// episode itself is the target (kept as in Kotlin). `sorted` is the list already sorted by
/// `sorting`. Returns `Err` for an unknown sorting, so the caller skips the deletion.
pub fn episode_to_delete<T>(
    sorted: &[T],
    current: usize,
    slots: i32,
    sorting: i64,
    is_seen: impl Fn(&T) -> bool,
) -> Result<Option<usize>, UnknownSorting> {
    let Some(index) = remove_after_read_index(current, slots) else {
        return Ok(None);
    };
    let known = matches!(
        sorting,
        episode_flags::SORTING_SOURCE
            | episode_flags::SORTING_NUMBER
            | episode_flags::SORTING_UPLOAD_DATE
    );
    if !known {
        tracing::error!(sorting, "unknown episode sorting; deletion skipped");
        return Err(UnknownSorting(sorting));
    }
    Ok(sorted
        .get(index)
        .filter(|episode| is_seen(*episode))
        .map(|_| index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_ahead_is_past_the_point_only_above_thirty_five_percent() {
        assert!(!past_download_ahead(350, 1000));
        assert!(past_download_ahead(351, 1000));
        assert!(!past_download_ahead(500, 0));
    }

    #[test]
    fn download_ahead_queues_the_next_episodes_once_per_episode() {
        let mut ahead = DownloadAhead::default();
        let following = [12, 13, 14];
        assert_eq!(
            ahead.plan(11, true, true, true, 2, &following),
            vec![12, 13]
        );
        assert!(ahead.plan(11, true, true, true, 2, &following).is_empty());
        assert_eq!(ahead.plan(12, true, true, true, 1, &[13, 14]), vec![13]);
    }

    #[test]
    fn download_ahead_waits_for_both_episodes_to_be_downloaded() {
        let mut ahead = DownloadAhead::default();
        let following = [12];
        assert!(ahead.plan(11, true, true, false, 1, &following).is_empty());
        assert!(ahead.plan(11, true, false, true, 1, &following).is_empty());
        assert!(ahead.plan(11, false, true, true, 1, &following).is_empty());
        assert!(ahead.plan(11, true, true, true, 0, &following).is_empty());
        // None of those calls recorded the episode, so a later qualifying call still queues.
        assert_eq!(ahead.plan(11, true, true, true, 1, &following), vec![12]);
    }

    #[test]
    fn remove_after_read_follows_the_slot_rule() {
        let list = ["a", "b", "c"];
        let seen = |_: &&str| true;
        assert_eq!(episode_to_delete(&list, 2, -1, 0, seen), Ok(None));
        assert_eq!(episode_to_delete(&list, 2, 1, 0, seen), Ok(Some(1)));
        assert_eq!(episode_to_delete(&list, 1, 0, 0, seen), Ok(Some(1)));
        assert_eq!(episode_to_delete(&list, 0, 1, 0, seen), Ok(None));
    }

    #[test]
    fn remove_after_read_skips_unseen_episodes() {
        let list = [("a", true), ("b", false), ("c", true)];
        let seen = |e: &(&str, bool)| e.1;
        assert_eq!(episode_to_delete(&list, 2, 1, 0, seen), Ok(None));
        assert_eq!(episode_to_delete(&list, 2, 2, 0, seen), Ok(Some(0)));
    }

    #[test]
    fn alphabet_sorting_is_refused_without_deleting() {
        let list = ["a", "b"];
        assert_eq!(
            episode_to_delete(&list, 1, 0, episode_flags::SORTING_ALPHABET, |_| true),
            Err(UnknownSorting(0x600))
        );
        assert_eq!(
            episode_to_delete(&list, 1, -1, episode_flags::SORTING_ALPHABET, |_| true),
            Ok(None)
        );
    }
}
