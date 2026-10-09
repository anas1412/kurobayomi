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

/// Plans one save of the current episode (T-45a-063, T-45a-064). Incognito writes nothing. Otherwise
/// the history gets `now_ms`. Progress is written only when `total_seconds > 0`. An unseen episode
/// becomes seen when it is completed, and a seen episode stays seen.
pub fn plan_save(
    incognito: bool,
    now_ms: i64,
    episode: &EpisodeProgress,
    threshold: f64,
) -> SavePlan {
    if incognito {
        return SavePlan::default();
    }
    let progress = (episode.total_seconds > 0).then(|| EpisodeProgress {
        seen: episode.seen
            || is_completed(episode.last_second_seen, episode.total_seconds, threshold),
        ..episode.clone()
    });
    SavePlan {
        history_seen_at: Some(now_ms),
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
    fn incognito_writes_neither_history_nor_progress() {
        let plan = plan_save(true, 42, &episode(false, 500, 1000), 0.85);
        assert_eq!(plan, SavePlan::default());
    }

    #[test]
    fn normal_save_upserts_history_at_now_and_writes_fields() {
        let plan = plan_save(false, 42, &episode(false, 100, 1000), 0.85);
        assert_eq!(plan.history_seen_at, Some(42));
        let written = plan.episode.unwrap();
        assert!(written.bookmark);
        assert!(!written.fillermark);
        assert_eq!(written.last_second_seen, 100);
        assert_eq!(written.total_seconds, 1000);
    }

    #[test]
    fn zero_total_skips_progress_but_keeps_history() {
        let plan = plan_save(false, 7, &episode(false, 0, 0), 0.85);
        assert_eq!(plan.history_seen_at, Some(7));
        assert_eq!(plan.episode, None);
    }

    #[test]
    fn unseen_becomes_seen_at_threshold() {
        let at = plan_save(false, 1, &episode(false, 850, 1000), 0.85);
        assert!(at.episode.unwrap().seen);
        let before = plan_save(false, 1, &episode(false, 849, 1000), 0.85);
        assert!(!before.episode.unwrap().seen);
    }

    #[test]
    fn seen_stays_seen_even_below_threshold() {
        let plan = plan_save(false, 1, &episode(true, 10, 1000), 0.85);
        assert!(plan.episode.unwrap().seen);
    }

    #[test]
    fn completion_uses_the_given_threshold() {
        assert!(is_completed(500, 1000, 0.5));
        assert!(!is_completed(499, 1000, 0.5));
    }
}
