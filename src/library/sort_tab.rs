//! The sort page of the library settings sheet (T-37a-038). Pure: which sort rows a kind offers and what a tap
//! does. A tap is `kuro_domain::library::order::click` (same type flips the direction, another type keeps it,
//! Random goes ascending with a new seed); the page only lists the rows.

use kuro_domain::kind::EntryKind;
use kuro_domain::library::order::click;
use kuro_domain::library::sort::{Direction, LibrarySort, SortType};
use kuro_i18n::MsgKey;

/// Rows of the sort page for one kind, in the kind's declaration order. `TrackerMean` is offered only when a
/// tracker is logged in; `AiringTime` exists for anime only (through `SortType::types`).
pub fn rows(kind: EntryKind, has_tracker: bool) -> Vec<SortType> {
    SortType::types(kind).iter().copied().filter(|ty| has_tracker || *ty != SortType::TrackerMean).collect()
}

/// What tapping `tapped` does to `current`: the sort to write and whether `set_sort_mode` must draw a new Random
/// seed. The sheet passes the sort to `CategoryService::set_sort_mode`, which draws the seed only for Random.
pub fn tap(current: LibrarySort, tapped: SortType) -> (LibrarySort, bool) {
    click(current, tapped)
}

/// The row title from the string table (the labels differ between manga and anime).
pub fn label_key(kind: EntryKind, ty: SortType) -> MsgKey {
    use EntryKind::{Anime, Manga};
    match (kind, ty) {
        (_, SortType::Alphabetical) => MsgKey::action_sort_alpha,
        (Manga, SortType::LastActivity) => MsgKey::action_sort_last_read,
        (Anime, SortType::LastActivity) => MsgKey::action_sort_last_seen,
        (Manga, SortType::LastUpdate) => MsgKey::action_sort_last_manga_update,
        (Anime, SortType::LastUpdate) => MsgKey::action_sort_last_anime_update,
        (Manga, SortType::UnreadCount) => MsgKey::action_sort_unread_count,
        (Anime, SortType::UnreadCount) => MsgKey::action_sort_unseen_count,
        (Manga, SortType::TotalItems) => MsgKey::action_sort_total,
        (Anime, SortType::TotalItems) => MsgKey::action_sort_total_episodes,
        (Manga, SortType::LatestItem) => MsgKey::action_sort_latest_chapter,
        (Anime, SortType::LatestItem) => MsgKey::action_sort_latest_episode,
        (Manga, SortType::ItemFetchDate) => MsgKey::action_sort_chapter_fetch_date,
        (Anime, SortType::ItemFetchDate) => MsgKey::action_sort_episode_fetch_date,
        (_, SortType::DateAdded) => MsgKey::action_sort_date_added,
        (_, SortType::TrackerMean) => MsgKey::action_sort_tracker_score,
        (_, SortType::AiringTime) => MsgKey::action_sort_airing_time,
        (_, SortType::Random) => MsgKey::action_sort_random,
    }
}

/// The arrow beside the active row: up for ascending, down for descending. Random has no direction.
pub fn direction_icon(sort: LibrarySort) -> Option<&'static str> {
    match (sort.ty, sort.dir) {
        (SortType::Random, _) => None,
        (_, Direction::Ascending) => Some("pan-up-symbolic"),
        (_, Direction::Descending) => Some("pan-down-symbolic"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_mean_is_listed_only_with_a_tracker() {
        assert!(!rows(EntryKind::Manga, false).contains(&SortType::TrackerMean));
        assert!(rows(EntryKind::Manga, true).contains(&SortType::TrackerMean));
        assert!(!rows(EntryKind::Anime, false).contains(&SortType::TrackerMean));
    }

    #[test]
    fn airing_time_is_anime_only() {
        assert!(rows(EntryKind::Anime, false).contains(&SortType::AiringTime));
        assert!(!rows(EntryKind::Manga, true).contains(&SortType::AiringTime));
    }

    #[test]
    fn random_is_always_the_last_row() {
        for kind in [EntryKind::Manga, EntryKind::Anime] {
            assert_eq!(rows(kind, true).last(), Some(&SortType::Random));
        }
    }

    #[test]
    fn same_row_flips_and_other_row_keeps_the_direction() {
        let asc = LibrarySort { ty: SortType::Alphabetical, dir: Direction::Ascending };
        let (flipped, reseed) = click(asc, SortType::Alphabetical);
        assert_eq!((flipped.ty, flipped.dir, reseed), (SortType::Alphabetical, Direction::Descending, false));
        let (moved, reseed) = click(flipped, SortType::DateAdded);
        assert_eq!((moved.ty, moved.dir, reseed), (SortType::DateAdded, Direction::Descending, false));
    }

    #[test]
    fn random_row_reshuffles_with_a_new_seed_and_goes_ascending() {
        let desc = LibrarySort { ty: SortType::DateAdded, dir: Direction::Descending };
        let (random, reseed) = click(desc, SortType::Random);
        assert_eq!((random.ty, random.dir, reseed), (SortType::Random, Direction::Ascending, true));
    }

    #[test]
    fn every_row_has_a_title_and_the_sheet_uses_the_same_click_rule() {
        for kind in [EntryKind::Manga, EntryKind::Anime] {
            for ty in rows(kind, true) {
                assert!(!kuro_i18n::tr(label_key(kind, ty)).is_empty(), "{kind:?} {ty:?}");
            }
        }
        assert_eq!(kuro_i18n::tr(label_key(EntryKind::Anime, SortType::LastActivity)), "Last seen");
        let asc = LibrarySort { ty: SortType::DateAdded, dir: Direction::Ascending };
        assert_eq!(tap(asc, SortType::DateAdded), click(asc, SortType::DateAdded));
        assert_eq!(direction_icon(asc), Some("pan-up-symbolic"));
        assert_eq!(direction_icon(LibrarySort { ty: SortType::Random, dir: Direction::Ascending }), None);
    }
}
