//! Watch-progress and history writes (area 45 `ExternalIntents.kt:441-486`, `PlayerActivity`
//! saves; behaviour 16.10 and part 2 behaviour 7.4; T-45a-063, T-45a-064). Pure: the caller runs
//! the returned writes against the database. Times are milliseconds, as in the Kotlin model.

/// The episode fields that a progress write sends (`EpisodeUpdate`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EpisodeProgress {
    pub seen: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub last_second_seen: i64,
    pub total_seconds: i64,
}

/// The writes of one save. `history_seen_at` is `Some(now)` when a history row is upserted.
/// `episode` is `Some` only when a progress write happens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavePlan {
    pub history_seen_at: Option<i64>,
    pub episode: Option<EpisodeProgress>,
}

/// Whether the watched position counts as finished: `lastSecondSeen >= totalSeconds * threshold`.
/// The Kotlin default threshold is 0.85.
pub fn is_completed(last_second_seen: i64, total_seconds: i64, threshold: f64) -> bool {
    last_second_seen as f64 >= total_seconds as f64 * threshold
}

/// Whether progress is written in a save: `!incognito_source || has_trackers` (T56, T57). A source
/// in incognito keeps progress only for a tracked entry, so the tracker can still read it.
pub fn progress_allowed(incognito_source: bool, has_trackers: bool) -> bool {
    !incognito_source || has_trackers
}

/// Whether history is written: never for a source in incognito (T56, T57).
pub fn history_allowed(incognito_source: bool) -> bool {
    !incognito_source
}

/// Whether tracker sync runs. It reads the app-wide incognito flag, not the per-source one (E-17).
pub fn tracker_sync_allowed(incognito_global: bool, has_trackers: bool, auto_update: bool) -> bool {
    !incognito_global && has_trackers && auto_update
}

/// Plans one save of the current episode (T-45a-063, T-45a-064, T-45b-056). `incognito_source` is
/// the per-source state read for the current anime. History gets `now_ms` unless incognito. Progress
/// is written when `progress_allowed` and `total_seconds > 0`. An unseen episode becomes seen when it
/// is completed, and a seen episode stays seen.
pub fn plan_save(
    incognito_source: bool,
    has_trackers: bool,
    now_ms: i64,
    episode: &EpisodeProgress,
    threshold: f64,
) -> SavePlan {
    let progress = (progress_allowed(incognito_source, has_trackers) && episode.total_seconds > 0)
        .then(|| EpisodeProgress {
            seen: episode.seen
                || is_completed(episode.last_second_seen, episode.total_seconds, threshold),
            ..episode.clone()
        });
    SavePlan {
        history_seen_at: history_allowed(incognito_source).then_some(now_ms),
        episode: progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(seen: bool, last: i64, total: i64) -> EpisodeProgress {
        EpisodeProgress {
            seen,
            bookmark: true,
            fillermark: false,
            last_second_seen: last,
            total_seconds: total,
        }
    }

    #[test]
    fn incognito_without_trackers_writes_neither_history_nor_progress() {
        let plan = plan_save(true, false, 42, &episode(false, 500, 1000), 0.85);
        assert_eq!(plan, SavePlan::default());
    }

    #[test]
    fn incognito_with_a_tracker_writes_progress_but_not_history() {
        let plan = plan_save(true, true, 42, &episode(false, 500, 1000), 0.85);
        assert_eq!(plan.history_seen_at, None);
        assert_eq!(plan.episode.unwrap().last_second_seen, 500);
    }

    #[test]
    fn tracker_sync_follows_the_global_flag_only() {
        assert!(tracker_sync_allowed(false, true, true));
        assert!(!tracker_sync_allowed(true, true, true));
        assert!(!tracker_sync_allowed(false, false, true));
        assert!(!tracker_sync_allowed(false, true, false));
    }

    #[test]
    fn normal_save_upserts_history_at_now_and_writes_fields() {
        let plan = plan_save(false, false, 42, &episode(false, 100, 1000), 0.85);
        assert_eq!(plan.history_seen_at, Some(42));
        let written = plan.episode.unwrap();
        assert!(written.bookmark);
        assert!(!written.fillermark);
        assert_eq!(written.last_second_seen, 100);
        assert_eq!(written.total_seconds, 1000);
    }

    #[test]
    fn zero_total_skips_progress_but_keeps_history() {
        let plan = plan_save(false, false, 7, &episode(false, 0, 0), 0.85);
        assert_eq!(plan.history_seen_at, Some(7));
        assert_eq!(plan.episode, None);
    }

    #[test]
    fn unseen_becomes_seen_at_threshold() {
        let at = plan_save(false, false, 1, &episode(false, 850, 1000), 0.85);
        assert!(at.episode.unwrap().seen);
        let before = plan_save(false, false, 1, &episode(false, 849, 1000), 0.85);
        assert!(!before.episode.unwrap().seen);
    }

    #[test]
    fn seen_stays_seen_even_below_threshold() {
        let plan = plan_save(false, false, 1, &episode(true, 10, 1000), 0.85);
        assert!(plan.episode.unwrap().seen);
    }

    #[test]
    fn completion_uses_the_given_threshold() {
        assert!(is_completed(500, 1000, 0.5));
        assert!(!is_completed(499, 1000, 0.5));
    }
}
