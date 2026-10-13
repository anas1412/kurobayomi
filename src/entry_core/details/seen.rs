//! Mark seen and unseen (T-39b-013) and mark previous episodes as seen (T-39b-014). Upstream:
//! `domain/.../items/episode/interactor/SetSeenStatus.kt` and `AnimeScreenModel.kt:855-866`. The plan is the shared one
//! of both pages (`readstate::plan_done`); `EntryStore::set_seen` writes it.

use std::collections::HashSet;

use kuro_data::item::Item;

use crate::entry_core::readstate::plan_done;

/// The items chronologically before `pointer`, in chronological order (6.9.8). `view` is the displayed list. In a
/// descending view the earlier items come after the pointer; the list is reversed to chronological order. A pointer
/// that is not in the view marks nothing (E-PREV-1).
pub fn previous_ids(view: &[i64], pointer: i64, descending: bool) -> Vec<i64> {
    let Some(at) = view.iter().position(|id| *id == pointer) else {
        return Vec::new();
    };
    if descending {
        let mut earlier = view[at + 1..].to_vec();
        earlier.reverse();
        earlier
    } else {
        view[..at].to_vec()
    }
}

/// What a mark seen or unseen sends: the ids to write, and the ids whose downloads go after it (T-39b-013, T-MRK-3). Only
/// episodes that are on disk can lose a download.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeenEffects {
    pub write_ids: Vec<i64>,
    pub delete_ids: Vec<i64>,
}

/// The effects of marking `episodes` seen or unseen. `remove_after_seen` is `pref_remove_after_marked_as_read`: a seen mark
/// deletes the downloads of the episodes it changed. Nothing changes (no write, no delete) when there is nothing to change.
pub fn seen_effects(seen: bool, episodes: &[Item], remove_after_seen: bool, downloaded: &HashSet<i64>) -> SeenEffects {
    let Some(plan) = plan_done(seen, episodes, remove_after_seen) else { return SeenEffects::default() };
    SeenEffects {
        write_ids: plan.changed,
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
    use kuro_platform::kind::Kind;

    use super::*;

    fn ep(id: i64, seen: bool, progress: i64) -> Item {
        Item { id, done: seen, progress, ..Item::create(Kind::Anime) }
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
    fn marking_unseen_takes_seen_or_positioned_episodes() {
        let list = [ep(1, true, 0), ep(2, false, 500), ep(3, false, 0)];
        let off = seen_effects(false, &list, true, &HashSet::new());
        assert_eq!(off.write_ids, vec![1, 2]);
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
