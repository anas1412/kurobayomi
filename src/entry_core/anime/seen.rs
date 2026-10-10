//! Mark seen and unseen (T-39b-013) and mark previous episodes as seen (T-39b-014). Upstream:
//! `domain/.../items/episode/interactor/SetSeenStatus.kt` and `AnimeScreenModel.kt:855-866`. The store applies the plan
//! in one update (`AnimeStore::set_seen` does the same write for the marks alone).

use std::collections::HashSet;

use kuro_data::episode::Episode;

use crate::entry_core::chapter_actions::previous_ids;

/// One episode write. `last_second_seen` is `Some(0)` when marking unseen (the position resets) and `None` when marking
/// seen (the position stays, E-MRK-3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeenWrite {
    pub id: i64,
    pub seen: bool,
    pub last_second_seen: Option<i64>,
}

/// The seen plan. `delete_downloads` lists the changed episodes whose downloads go after a mark seen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeenPlan {
    pub writes: Vec<SeenWrite>,
    pub delete_downloads: Vec<i64>,
}

/// Plans a mark seen (`seen = true`) or unseen over `episodes`. Seen writes only episodes that are not seen. Unseen
/// writes episodes that are seen or have a position past zero. Returns `None` when nothing changes (`NoEpisodes`).
/// With the remove-after-seen preference, a seen mark deletes the downloads of the episodes it changed.
pub fn plan_seen(seen: bool, episodes: &[Episode], remove_after_seen: bool) -> Option<SeenPlan> {
    let changed: Vec<&Episode> = episodes
        .iter()
        .filter(|e| if seen { !e.seen } else { e.seen || e.last_second_seen > 0 })
        .collect();
    if changed.is_empty() {
        return None;
    }
    let writes = changed
        .iter()
        .map(|e| SeenWrite { id: e.id, seen, last_second_seen: (!seen).then_some(0) })
        .collect();
    let delete_downloads = if seen && remove_after_seen { changed.iter().map(|e| e.id).collect() } else { Vec::new() };
    Some(SeenPlan { writes, delete_downloads })
}

/// What a mark seen or unseen sends: the ids to write, and the ids whose downloads go after it (T-39b-013, T-MRK-3). Only
/// episodes that are on disk can lose a download.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeenEffects {
    pub write_ids: Vec<i64>,
    pub delete_ids: Vec<i64>,
}

/// The effects of marking `episodes` seen or unseen. `remove_after_seen` is `pref_remove_after_marked_as_read`.
/// Nothing changes (no write, no delete) when `plan_seen` finds nothing to change.
pub fn seen_effects(seen: bool, episodes: &[Episode], remove_after_seen: bool, downloaded: &HashSet<i64>) -> SeenEffects {
    let Some(plan) = plan_seen(seen, episodes, remove_after_seen) else { return SeenEffects::default() };
    SeenEffects {
        write_ids: plan.writes.iter().map(|w| w.id).collect(),
        delete_ids: plan.delete_downloads.into_iter().filter(|id| downloaded.contains(id)).collect(),
    }
}

/// `markPreviousEpisodeSeen`: the episodes chronologically before `pointer`, as the displayed `view` orders them. In a
/// descending view they come after the pointer and are reversed to chronological order. A pointer outside the view
/// marks nothing (T-39b-014).
pub fn previous_to_mark(view: &[i64], pointer: i64, descending: bool) -> Vec<i64> {
    previous_ids(view, pointer, descending)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: i64, seen: bool, last_second_seen: i64) -> Episode {
        let mut e = Episode::create();
        e.id = id;
        e.seen = seen;
        e.last_second_seen = last_second_seen;
        e
    }

    #[test]
    fn marking_seen_writes_only_unseen_episodes_and_keeps_the_position() {
        let list = [ep(1, true, 0), ep(2, false, 500), ep(3, false, 0)];
        let plan = plan_seen(true, &list, false).unwrap();
        assert_eq!(plan.writes.iter().map(|w| w.id).collect::<Vec<_>>(), vec![2, 3]);
        assert!(plan.writes.iter().all(|w| w.seen && w.last_second_seen.is_none()));
        assert!(plan.delete_downloads.is_empty());
    }

    #[test]
    fn marking_unseen_writes_seen_or_positioned_episodes_and_resets_the_position() {
        let list = [ep(1, true, 0), ep(2, false, 500), ep(3, false, 0)];
        let plan = plan_seen(false, &list, true).unwrap();
        assert_eq!(plan.writes.iter().map(|w| w.id).collect::<Vec<_>>(), vec![1, 2]);
        assert!(plan.writes.iter().all(|w| !w.seen && w.last_second_seen == Some(0)));
        assert!(plan.delete_downloads.is_empty(), "unseen never removes downloads");
    }

    #[test]
    fn nothing_to_change_is_no_plan() {
        assert!(plan_seen(true, &[ep(1, true, 0)], true).is_none());
        assert!(plan_seen(false, &[ep(1, false, 0)], true).is_none());
        assert!(plan_seen(true, &[], true).is_none());
    }

    #[test]
    fn the_effects_delete_only_changed_episodes_that_are_on_disk_and_only_with_the_preference() {
        let list = [ep(1, true, 0), ep(2, false, 0), ep(3, false, 0)];
        let on_disk: HashSet<i64> = [1, 3].into();
        let on = seen_effects(true, &list, true, &on_disk);
        assert_eq!((on.write_ids, on.delete_ids), (vec![2, 3], vec![3]), "episode 1 did not change, episode 2 has no file");
        let off = seen_effects(true, &list, false, &on_disk);
        assert_eq!((off.write_ids, off.delete_ids), (vec![2, 3], Vec::<i64>::new()));
        assert_eq!(seen_effects(false, &list, true, &on_disk).delete_ids, Vec::<i64>::new(), "unseen never deletes");
        assert_eq!(seen_effects(true, &[ep(1, true, 0)], true, &on_disk), SeenEffects::default(), "nothing changed");
    }

    #[test]
    fn remove_after_seen_deletes_the_downloads_of_the_changed_episodes() {
        let list = [ep(1, true, 0), ep(2, false, 0), ep(3, false, 0)];
        let on = plan_seen(true, &list, true).unwrap();
        assert_eq!(on.delete_downloads, vec![2, 3]);
        let off = plan_seen(true, &list, false).unwrap();
        assert!(off.delete_downloads.is_empty());
    }

    #[test]
    fn previous_episodes_in_an_ascending_view_are_those_before_the_pointer() {
        assert_eq!(previous_to_mark(&[1, 2, 3, 4], 3, false), vec![1, 2]);
    }

    #[test]
    fn previous_episodes_in_a_descending_view_are_after_the_pointer_in_chronological_order() {
        // the view is 4, 3, 2, 1 (newest first); the episodes before 3 chronologically are 1 and 2, oldest first
        assert_eq!(previous_to_mark(&[4, 3, 2, 1], 3, true), vec![1, 2]);
    }

    #[test]
    fn a_pointer_outside_the_view_marks_nothing() {
        assert!(previous_to_mark(&[1, 2, 3], 9, false).is_empty());
        assert!(previous_to_mark(&[1, 2, 3], 9, true).is_empty());
    }
}
