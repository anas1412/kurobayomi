//! The anime Download menu and the download side of the selection bar (T-39b-063): which episodes each entry queues
//! or deletes. Pure. Upstream: `AnimeScreenModel.kt:762-830` (`getUnseenEpisodes`, `getUnseenEpisodesSorted`,
//! `runDownloadAction`).
//!
//! The menu reads the processed list (the filters apply), keeps the unseen episodes that are not downloaded, and
//! "Next N" takes the oldest N: the list is in display order, so a descending list is reversed first.

use std::collections::HashSet;

use kuro_data::episode::Episode;

/// The entries of the Download menu: the next 1, 5, 10 or 25 unseen episodes, or every unseen one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadMenu {
    Next(usize),
    Unseen,
}

/// The counts of the "Next N" entries, in menu order.
pub const NEXT_COUNTS: [usize; 4] = [1, 5, 10, 25];

/// The menu is not offered for a local entry: its files are already on disk.
pub fn menu_visible(local: bool, has_download_handler: bool) -> bool {
    !local && has_download_handler
}

/// The episodes a menu entry queues. `processed` is the displayed list, `descending` its direction.
// ponytail: an episode that is queued but not yet on disk is not told apart from a missing one; the model keeps no
// queue state, so it can be queued twice (the queue ignores a duplicate).
pub fn download_targets(processed: &[Episode], descending: bool, downloaded: &HashSet<i64>, menu: DownloadMenu) -> Vec<i64> {
    let mut unseen: Vec<i64> = processed.iter().filter(|e| !e.seen && !downloaded.contains(&e.id)).map(|e| e.id).collect();
    if let DownloadMenu::Next(n) = menu {
        if descending {
            unseen.reverse();
        }
        unseen.truncate(n);
    }
    unseen
}

/// The ids of the selected episodes the bar's Download row queues: those not on disk.
pub fn selection_to_download(selected: &[Episode], downloaded: &HashSet<i64>) -> Vec<i64> {
    selected.iter().filter(|e| !downloaded.contains(&e.id)).map(|e| e.id).collect()
}

/// The ids of the selected episodes the bar's Delete row removes: those on disk.
pub fn selection_to_delete(selected: &[Episode], downloaded: &HashSet<i64>) -> Vec<i64> {
    selected.iter().filter(|e| downloaded.contains(&e.id)).map(|e| e.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: i64, seen: bool) -> Episode {
        Episode { id, seen, ..Episode::create() }
    }

    fn set(ids: &[i64]) -> HashSet<i64> {
        ids.iter().copied().collect()
    }

    #[test]
    fn unseen_takes_every_unseen_episode_that_is_not_downloaded_in_list_order() {
        let list = [ep(4, false), ep(3, true), ep(2, false), ep(1, false)];
        assert_eq!(download_targets(&list, true, &set(&[2]), DownloadMenu::Unseen), vec![4, 1]);
    }

    #[test]
    fn next_n_takes_the_oldest_first_for_either_direction() {
        let descending = [ep(4, false), ep(3, false), ep(2, false), ep(1, false)];
        assert_eq!(download_targets(&descending, true, &set(&[]), DownloadMenu::Next(2)), vec![1, 2]);
        let ascending = [ep(1, false), ep(2, false), ep(3, false), ep(4, false)];
        assert_eq!(download_targets(&ascending, false, &set(&[]), DownloadMenu::Next(2)), vec![1, 2]);
    }

    #[test]
    fn next_n_skips_seen_and_downloaded_episodes_and_stops_at_what_exists() {
        let list = [ep(1, true), ep(2, false), ep(3, false)];
        assert_eq!(download_targets(&list, false, &set(&[2]), DownloadMenu::Next(25)), vec![3]);
        assert!(download_targets(&list, false, &set(&[2, 3]), DownloadMenu::Next(5)).is_empty());
    }

    #[test]
    fn the_menu_is_hidden_for_a_local_entry_or_without_a_download_handler() {
        assert!(menu_visible(false, true));
        assert!(!menu_visible(true, true));
        assert!(!menu_visible(false, false));
    }

    #[test]
    fn the_bar_downloads_what_is_missing_and_deletes_what_is_on_disk() {
        let selected = [ep(1, false), ep(2, false), ep(3, true)];
        let on_disk = set(&[2]);
        assert_eq!(selection_to_download(&selected, &on_disk), vec![1, 3]);
        assert_eq!(selection_to_delete(&selected, &on_disk), vec![2]);
    }
}
