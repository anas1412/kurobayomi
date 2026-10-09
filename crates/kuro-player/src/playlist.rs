//! Player playlist (area 45 p2 T-45b-004, T-45b-005, T-45b-006; behaviour 2.3, 2.4, 2.7). Pure: the
//! caller passes the episodes, the anime flags and a downloaded check.

use kuro_data::episode::Episode;
use kuro_domain::entries::anime::episode_flags as flags;
use kuro_domain::items::sorter::episode_cmp;

use crate::downloads::UnknownSorting;
use crate::episodes::neighbour_flags;

/// A sorting value the playlist understands (`EpisodeSorter.kt`, source, number, date and name).
pub fn sort_is_known(sorting: i64) -> bool {
    matches!(
        sorting,
        flags::SORTING_SOURCE
            | flags::SORTING_NUMBER
            | flags::SORTING_UPLOAD_DATE
            | flags::SORTING_ALPHABET
    )
}

/// `initEpisodeList` (behaviour 2.3): the episodes sorted with `sortDescending = false`, so the
/// player order does not follow the anime's saved direction. Source order is inverted by the domain
/// sorter (2.6, a compat quirk). `downloaded_only` keeps only the downloaded episodes. An unknown
/// sorting is an error, as in Kotlin.
pub fn build(
    episodes: &[Episode],
    sorting: i64,
    downloaded_only: bool,
    is_downloaded: impl Fn(&Episode) -> bool,
) -> Result<Vec<Episode>, UnknownSorting> {
    if !sort_is_known(sorting) {
        return Err(UnknownSorting(sorting));
    }
    let mut out = episodes.to_vec();
    out.sort_by(episode_cmp(sorting, false));
    if downloaded_only {
        out.retain(|e| is_downloaded(e));
    }
    Ok(out)
}

/// Whether the filter flags hide an episode (behaviour 2.4): the OR of the eight clauses. Each clause
/// is a filter value compared with the masked flags, and a conjunction with the episode's state.
pub fn hidden(
    episode: &Episode,
    episode_flags: i64,
    is_downloaded: impl Fn(&Episode) -> bool,
) -> bool {
    let unseen = episode_flags & flags::UNSEEN_MASK;
    let downloaded = episode_flags & flags::DOWNLOADED_MASK;
    let bookmarked = episode_flags & flags::BOOKMARKED_MASK;
    let filler = episode_flags & flags::FILLERMARKED_MASK;
    (unseen == flags::SHOW_SEEN && !episode.seen)
        || (unseen == flags::SHOW_UNSEEN && episode.seen)
        || (downloaded == flags::SHOW_DOWNLOADED && !is_downloaded(episode))
        || (downloaded == flags::SHOW_NOT_DOWNLOADED && is_downloaded(episode))
        || (bookmarked == flags::SHOW_BOOKMARKED && !episode.bookmark)
        || (bookmarked == flags::SHOW_NOT_BOOKMARKED && episode.bookmark)
        || (filler == flags::SHOW_FILLERMARKED && !episode.fillermark)
        || (filler == flags::SHOW_NOT_FILLERMARKED && episode.fillermark)
}

/// `filterEpisodeList` (behaviour 2.4): drops the hidden episodes. The selected episode is always
/// kept: when the filter hides it, it is appended last. `None` when the selected id is not in the
/// list at all (Kotlin throws there).
pub fn filter(
    episodes: &[Episode],
    episode_flags: i64,
    selected_id: i64,
    is_downloaded: impl Fn(&Episode) -> bool,
) -> Option<Vec<Episode>> {
    let selected = episodes.iter().find(|e| e.id == selected_id)?;
    let mut out: Vec<Episode> = episodes
        .iter()
        .filter(|e| !hidden(e, episode_flags, &is_downloaded))
        .cloned()
        .collect();
    if !out.iter().any(|e| e.id == selected_id) {
        out.push(selected.clone());
    }
    Some(out)
}

/// `getCurrentEpisodeIndex` (behaviour 2.7): the index of the current episode, or `-1`.
pub fn current_index(playlist: &[Episode], current_id: i64) -> i64 {
    playlist
        .iter()
        .position(|e| e.id == current_id)
        .map_or(-1, |i| i as i64)
}

/// `hasPreviousEpisode` and `hasNextEpisode` for the current episode (behaviour 2.2).
pub fn neighbours(playlist: &[Episode], current_id: i64) -> (bool, bool) {
    neighbour_flags(current_index(playlist, current_id), playlist.len())
}

/// `getAdjacentEpisodeId(previous)` (behaviour 2.7): the id of the neighbour, or `-1` at the ends and
/// when there is none. With the current episode not in the list (index `-1`) and `previous = false`,
/// the first episode is returned, as in Kotlin.
pub fn adjacent_id(playlist: &[Episode], current_id: i64, previous: bool) -> i64 {
    let index = current_index(playlist, current_id);
    let last = playlist.len() as i64 - 1;
    if previous && index == 0 {
        return -1;
    }
    if !previous && last == index {
        return -1;
    }
    let target = if previous { index - 1 } else { index + 1 };
    usize::try_from(target)
        .ok()
        .and_then(|i| playlist.get(i))
        .map_or(-1, |e| e.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(id: i64, number: f64) -> Episode {
        Episode {
            id,
            episode_number: number,
            name: format!("Ep {id}"),
            ..Episode::create()
        }
    }

    fn none(_: &Episode) -> bool {
        false
    }

    #[test]
    fn build_sorts_by_number_ascending_whatever_the_anime_direction() {
        let eps = vec![episode(1, 3.0), episode(2, 1.0), episode(3, 2.0)];
        let out = build(&eps, flags::SORTING_NUMBER, false, none).unwrap();
        assert_eq!(out.iter().map(|e| e.id).collect::<Vec<_>>(), vec![2, 3, 1]);
    }

    #[test]
    fn build_inverts_source_order_with_sort_descending_false() {
        let mut a = episode(1, 1.0);
        a.source_order = 0;
        let mut b = episode(2, 2.0);
        b.source_order = 1;
        let out = build(&[a, b], flags::SORTING_SOURCE, false, none).unwrap();
        assert_eq!(out.iter().map(|e| e.id).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[test]
    fn build_keeps_only_downloaded_when_asked() {
        let eps = vec![episode(1, 1.0), episode(2, 2.0)];
        let out = build(&eps, flags::SORTING_NUMBER, true, |e| e.id == 2).unwrap();
        assert_eq!(out.iter().map(|e| e.id).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn build_refuses_an_unknown_sorting() {
        assert_eq!(build(&[], 0x800, false, none), Err(UnknownSorting(0x800)));
    }

    #[test]
    fn filter_keeps_the_selected_episode_even_when_hidden() {
        let mut watched = episode(1, 1.0);
        watched.seen = true;
        let eps = vec![watched, episode(2, 2.0)];
        // SHOW_UNSEEN hides the seen episode 1, which is also the selected one.
        let out = filter(&eps, flags::SHOW_UNSEEN, 1, none).unwrap();
        assert_eq!(out.iter().map(|e| e.id).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[test]
    fn filter_with_show_all_keeps_everything() {
        let eps = vec![episode(1, 1.0), episode(2, 2.0)];
        let out = filter(&eps, 0, 2, none).unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn filter_clauses_match_the_masks() {
        let mut bookmarked = episode(1, 1.0);
        bookmarked.bookmark = true;
        assert!(hidden(&bookmarked, flags::SHOW_NOT_BOOKMARKED, none));
        assert!(!hidden(&bookmarked, flags::SHOW_BOOKMARKED, none));
        let mut filler = episode(2, 2.0);
        filler.fillermark = true;
        assert!(hidden(&filler, flags::SHOW_NOT_FILLERMARKED, none));
        assert!(hidden(&episode(3, 3.0), flags::SHOW_FILLERMARKED, none));
        assert!(hidden(&episode(4, 4.0), flags::SHOW_DOWNLOADED, none));
        assert!(!hidden(&episode(4, 4.0), flags::SHOW_NOT_DOWNLOADED, none));
    }

    #[test]
    fn filter_for_a_missing_selected_episode_is_none() {
        assert_eq!(filter(&[episode(1, 1.0)], 0, 9, none), None);
    }

    #[test]
    fn adjacent_ids_stop_at_the_ends() {
        let eps = vec![episode(10, 1.0), episode(11, 2.0), episode(12, 3.0)];
        assert_eq!(adjacent_id(&eps, 10, true), -1);
        assert_eq!(adjacent_id(&eps, 10, false), 11);
        assert_eq!(adjacent_id(&eps, 12, false), -1);
        assert_eq!(adjacent_id(&eps, 12, true), 11);
        assert_eq!(neighbours(&eps, 10), (false, true));
    }

    #[test]
    fn adjacent_for_a_missing_current_episode() {
        let eps = vec![episode(10, 1.0), episode(11, 2.0)];
        assert_eq!(adjacent_id(&eps, 99, true), -1);
        assert_eq!(adjacent_id(&eps, 99, false), 10);
        assert_eq!(adjacent_id(&[], 99, false), -1);
    }
}
