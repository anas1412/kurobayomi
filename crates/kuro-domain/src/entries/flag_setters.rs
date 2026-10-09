//! Flag setters of the three entry screens (`SetMangaChapterFlags`, `SetAnimeEpisodeFlags`, `SetAnimeSeasonFlags`).
//! Each setter writes one group of bits. Plain groups are single masked UPDATEs in `kuro-data`; the sort-or-flip
//! and set-all setters read and write the word inside one writer transaction. Pure state functions are separate
//! so the vectors can be checked without a database.

use kuro_data::DataResult;
use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;

use super::anime::{episode_flags as ep, season_flags as se};
use super::season_display::SeasonDisplayMode;
use crate::flags::set_flag;

/// Sort-or-flip state machine (`p1-behavior.md` 3.1.1). When `flag` is the sort already in use, the direction bit
/// flips; otherwise `flag` becomes the sort, ascending. `sorting_mask` and `dir_mask` are the layout of the word;
/// `asc` is the bit that means ascending.
pub fn sort_or_flip(current: i64, flag: i64, sorting_mask: i64, dir_mask: i64, asc: i64) -> i64 {
    if current & sorting_mask == flag {
        let descending = current & dir_mask == 0;
        let order = if descending { asc } else { 0 };
        set_flag(current, order, dir_mask)
    } else {
        set_flag(set_flag(current, flag, sorting_mask), asc, dir_mask)
    }
}

/// Overlay booleans of a season (bit set = shown).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SeasonOverlay {
    Downloaded,
    Unseen,
    Local,
    Language,
    Continue,
}

impl SeasonOverlay {
    pub const fn mask(self) -> i64 {
        match self {
            Self::Downloaded => se::OVERLAY_DOWNLOADED_MASK,
            Self::Unseen => se::OVERLAY_UNSEEN_MASK,
            Self::Local => se::OVERLAY_LOCAL_MASK,
            Self::Language => se::OVERLAY_LANG_MASK,
            Self::Continue => se::OVERLAY_CONT_MASK,
        }
    }
}

/// Sets exactly the overlay bit: `on` sets it, `off` clears it, no other bit moves.
pub fn overlay_flag(current: i64, overlay: SeasonOverlay, on: bool) -> i64 {
    set_flag(current, if on { overlay.mask() } else { 0 }, overlay.mask())
}

/// Grid size is clamped to `0..=15` (four bits).
pub fn clamp_grid_size(size: i64) -> i64 {
    size.clamp(0, 15)
}

/// Words of the chapter set-all, applied from 0 in the order unread, downloaded, bookmarked, sorting, direction,
/// display.
pub fn chapter_all(unread: i64, downloaded: i64, bookmarked: i64, sorting: i64, direction: i64, display: i64) -> i64 {
    use kuro_db::flags::chapter as c;
    let word = set_flag(0, unread, c::UNREAD_MASK);
    let word = set_flag(word, downloaded, c::DOWNLOADED_MASK);
    let word = set_flag(word, bookmarked, c::BOOKMARKED_MASK);
    let word = set_flag(word, sorting, c::SORTING_MASK);
    let word = set_flag(word, direction, c::SORT_DIR_MASK);
    set_flag(word, display, c::DISPLAY_MASK)
}

/// Words of the episode set-all, applied from 0 in the documented order.
#[allow(clippy::too_many_arguments)]
pub fn episode_all(
    unseen: i64,
    downloaded: i64,
    bookmarked: i64,
    fillermarked: i64,
    sorting: i64,
    direction: i64,
    display: i64,
    previews: i64,
    summaries: i64,
) -> i64 {
    let word = set_flag(0, unseen, ep::UNSEEN_MASK);
    let word = set_flag(word, downloaded, ep::DOWNLOADED_MASK);
    let word = set_flag(word, bookmarked, ep::BOOKMARKED_MASK);
    let word = set_flag(word, fillermarked, ep::FILLERMARKED_MASK);
    let word = set_flag(word, sorting, ep::SORTING_MASK);
    let word = set_flag(word, direction, ep::SORT_DIR_MASK);
    let word = set_flag(word, display, ep::DISPLAY_MASK);
    let word = set_flag(word, previews, ep::PREVIEWS_MASK);
    set_flag(word, summaries, ep::SUMMARIES_MASK)
}

/// The season set-all inputs, in the order of `SetAnimeSeasonFlags.awaitSetAllFlags`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SeasonAll {
    pub download: i64,
    pub unseen: i64,
    pub started: i64,
    pub completed: i64,
    pub bookmarked: i64,
    pub fillermarked: i64,
    pub sorting: i64,
    pub direction: i64,
    pub grid_mode: i64,
    pub grid_size: i64,
    pub overlays: [bool; 5],
    pub display: i64,
}

/// Word of the season set-all, applied from 0.
pub fn season_all(a: SeasonAll) -> i64 {
    let word = set_flag(0, a.download, se::DOWNLOADED_MASK);
    let word = set_flag(word, a.unseen, se::UNSEEN_MASK);
    let word = set_flag(word, a.started, se::STARTED_MASK);
    let word = set_flag(word, a.completed, se::COMPLETED_MASK);
    let word = set_flag(word, a.bookmarked, se::BOOKMARKED_MASK);
    let word = set_flag(word, a.fillermarked, se::FILLERMARKED_MASK);
    let word = set_flag(word, a.sorting, se::SORT_MASK);
    let word = set_flag(word, a.direction, se::SORT_DIR_MASK);
    let word = set_flag(word, a.grid_mode << se::GRID_DISPLAY_MODE_BIT_OFFSET, se::GRID_DISPLAY_MODE_MASK);
    let word = set_flag(word, clamp_grid_size(a.grid_size) << se::GRID_DISPLAY_SIZE_BIT_OFFSET, se::GRID_DISPLAY_SIZE_MASK);
    let overlays = [SeasonOverlay::Downloaded, SeasonOverlay::Unseen, SeasonOverlay::Local, SeasonOverlay::Language, SeasonOverlay::Continue];
    let word = overlays.iter().zip(a.overlays).fold(word, |w, (o, on)| overlay_flag(w, *o, on));
    set_flag(word, a.display, se::DISPLAY_MODE_MASK)
}

// Chapter (manga) setters.

/// Sets the downloaded filter group.
pub fn set_chapter_downloaded(repo: &MangaRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_chapter_flags_masked(id, flag, kuro_db::flags::chapter::DOWNLOADED_MASK)
}

/// Sets the unread filter group.
pub fn set_chapter_unread(repo: &MangaRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_chapter_flags_masked(id, flag, kuro_db::flags::chapter::UNREAD_MASK)
}

/// Sets the bookmarked filter group.
pub fn set_chapter_bookmarked(repo: &MangaRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_chapter_flags_masked(id, flag, kuro_db::flags::chapter::BOOKMARKED_MASK)
}

/// Sets the display group (name or number).
pub fn set_chapter_display(repo: &MangaRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_chapter_flags_masked(id, flag, kuro_db::flags::chapter::DISPLAY_MASK)
}

/// Sort-or-flip of the chapter list.
pub fn set_chapter_sort_or_flip(repo: &MangaRepo, id: i64, flag: i64) -> DataResult<bool> {
    use kuro_db::flags::chapter as c;
    repo.update_chapter_flags(id, |cur| sort_or_flip(cur, flag, c::SORTING_MASK, c::SORT_DIR_MASK, c::SORT_ASC))
}

/// Overwrites the whole chapter word with the six groups.
pub fn set_chapter_all(repo: &MangaRepo, id: i64, groups: [i64; 6]) -> DataResult<bool> {
    let [unread, downloaded, bookmarked, sorting, direction, display] = groups;
    let word = chapter_all(unread, downloaded, bookmarked, sorting, direction, display);
    repo.update_chapter_flags(id, |_| word)
}

// Episode (anime) setters.

/// Sets the unseen filter group.
pub fn set_episode_unseen(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_episode_flags_masked(id, flag, ep::UNSEEN_MASK)
}

/// Sets the downloaded filter group.
pub fn set_episode_downloaded(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_episode_flags_masked(id, flag, ep::DOWNLOADED_MASK)
}

/// Sets the bookmarked filter group.
pub fn set_episode_bookmarked(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_episode_flags_masked(id, flag, ep::BOOKMARKED_MASK)
}

/// Sets the fillermarked filter group.
pub fn set_episode_fillermarked(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_episode_flags_masked(id, flag, ep::FILLERMARKED_MASK)
}

/// Sets the display group (name or number).
pub fn set_episode_display(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_episode_flags_masked(id, flag, ep::DISPLAY_MASK)
}

/// Sets the previews group.
pub fn set_episode_previews(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_episode_flags_masked(id, flag, ep::PREVIEWS_MASK)
}

/// Sets the summaries group.
pub fn set_episode_summaries(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_episode_flags_masked(id, flag, ep::SUMMARIES_MASK)
}

/// Sort-or-flip of the episode list.
pub fn set_episode_sort_or_flip(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.update_episode_flags(id, |cur| sort_or_flip(cur, flag, ep::SORTING_MASK, ep::SORT_DIR_MASK, ep::SORT_ASC))
}

/// Overwrites the whole episode word with the nine groups, in the documented order.
pub fn set_episode_all(repo: &AnimeRepo, id: i64, groups: [i64; 9]) -> DataResult<bool> {
    let [unseen, downloaded, bookmarked, fillermarked, sorting, direction, display, previews, summaries] = groups;
    let word = episode_all(unseen, downloaded, bookmarked, fillermarked, sorting, direction, display, previews, summaries);
    repo.update_episode_flags(id, |_| word)
}

// Season (anime parent) setters.

/// Sets the season downloaded filter.
pub fn set_season_downloaded(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_season_flags_masked(id, flag, se::DOWNLOADED_MASK)
}

/// Sets the season unseen filter.
pub fn set_season_unseen(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_season_flags_masked(id, flag, se::UNSEEN_MASK)
}

/// Sets the season started filter.
pub fn set_season_started(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_season_flags_masked(id, flag, se::STARTED_MASK)
}

/// Sets the season completed filter.
pub fn set_season_completed(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_season_flags_masked(id, flag, se::COMPLETED_MASK)
}

/// Sets the season bookmarked filter.
pub fn set_season_bookmarked(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_season_flags_masked(id, flag, se::BOOKMARKED_MASK)
}

/// Sets the season fillermarked filter.
pub fn set_season_fillermarked(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_season_flags_masked(id, flag, se::FILLERMARKED_MASK)
}

/// Sort-or-flip of the season list (mask `0xE000`, direction `0x1`).
pub fn set_season_sort_or_flip(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.update_season_flags(id, |cur| sort_or_flip(cur, flag, se::SORT_MASK, se::SORT_DIR_MASK, se::SORT_ASC))
}

/// Grid layout of the season list (bits 16-17).
pub fn set_season_grid_mode(repo: &AnimeRepo, id: i64, mode: SeasonDisplayMode) -> DataResult<bool> {
    repo.set_season_flags_masked(id, mode.to_i64() << se::GRID_DISPLAY_MODE_BIT_OFFSET, se::GRID_DISPLAY_MODE_MASK)
}

/// Grid size (bits 18-21), clamped to 0..=15.
pub fn set_season_grid_size(repo: &AnimeRepo, id: i64, size: i64) -> DataResult<bool> {
    repo.set_season_flags_masked(
        id,
        clamp_grid_size(size) << se::GRID_DISPLAY_SIZE_BIT_OFFSET,
        se::GRID_DISPLAY_SIZE_MASK,
    )
}

/// Sets exactly one overlay bit.
pub fn set_season_overlay(repo: &AnimeRepo, id: i64, overlay: SeasonOverlay, on: bool) -> DataResult<bool> {
    repo.set_season_flags_masked(id, if on { overlay.mask() } else { 0 }, overlay.mask())
}

/// Season display mode (`SEASON_DISPLAY_MODE`, one bit).
pub fn set_season_display(repo: &AnimeRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_season_flags_masked(id, flag, se::DISPLAY_MODE_MASK)
}

/// Overwrites the whole season word with the 16 inputs.
pub fn set_season_all(repo: &AnimeRepo, id: i64, all: SeasonAll) -> DataResult<bool> {
    let word = season_all(all);
    repo.update_season_flags(id, |_| word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_db::flags::chapter as c;

    #[test]
    fn sort_or_flip_vector_from_zero() {
        let mut w = 0;
        w = sort_or_flip(w, c::SORTING_NUMBER, c::SORTING_MASK, c::SORT_DIR_MASK, c::SORT_ASC);
        assert_eq!(w, 0x100 | 0x1, "new column forces ascending");
        w = sort_or_flip(w, c::SORTING_NUMBER, c::SORTING_MASK, c::SORT_DIR_MASK, c::SORT_ASC);
        assert_eq!(w, 0x100, "same column flips the direction");
    }

    #[test]
    fn a_new_column_forces_ascending_even_when_the_old_one_was_descending() {
        let w = sort_or_flip(0x100, c::SORTING_UPLOAD_DATE, c::SORTING_MASK, c::SORT_DIR_MASK, c::SORT_ASC);
        assert_eq!(w, 0x200 | 0x1);
    }

    #[test]
    fn episode_sort_or_flip_uses_its_own_masks() {
        let w = sort_or_flip(0, ep::SORTING_NUMBER, ep::SORTING_MASK, ep::SORT_DIR_MASK, ep::SORT_ASC);
        assert_eq!(w, 0x201);
        let w = sort_or_flip(w, ep::SORTING_NUMBER, ep::SORTING_MASK, ep::SORT_DIR_MASK, ep::SORT_ASC);
        assert_eq!(w, 0x200);
    }

    #[test]
    fn episode_vectors_reproduce_through_the_setters() {
        // 0x201 -> 0x200 (flip) -> 0x202 (unseen = SHOW_UNSEEN) -> 0x242 (not bookmarked) -> 0x342 (not fillermarked)
        let mut w = sort_or_flip(0, ep::SORTING_NUMBER, ep::SORTING_MASK, ep::SORT_DIR_MASK, ep::SORT_ASC);
        assert_eq!(w, 0x201);
        w = sort_or_flip(w, ep::SORTING_NUMBER, ep::SORTING_MASK, ep::SORT_DIR_MASK, ep::SORT_ASC);
        assert_eq!(w, 0x200);
        w = set_flag(w, ep::SHOW_UNSEEN, ep::UNSEEN_MASK);
        assert_eq!(w, 0x202);
        w = set_flag(w, ep::SHOW_NOT_BOOKMARKED, ep::BOOKMARKED_MASK);
        assert_eq!(w, 0x242);
        w = set_flag(w, ep::SHOW_NOT_FILLERMARKED, ep::FILLERMARKED_MASK);
        assert_eq!(w, 0x342);
        // 0x101B42: number display, not summaries, not previews, number sort, not filler, not bookmarked, unseen.
        let end = episode_all(ep::SHOW_UNSEEN, 0, ep::SHOW_NOT_BOOKMARKED, ep::SHOW_NOT_FILLERMARKED, ep::SORTING_NUMBER, 0, ep::DISPLAY_NUMBER, ep::SHOW_NOT_PREVIEWS, ep::SHOW_NOT_SUMMARIES);
        assert_eq!(end, 0x101B42);
    }

    #[test]
    fn season_grid_and_overlay_vector() {
        let all = SeasonAll {
            download: 0,
            unseen: 0,
            started: 0,
            completed: 0,
            bookmarked: 0,
            fillermarked: 0,
            sorting: 0,
            direction: 0,
            grid_mode: SeasonDisplayMode::List.to_i64(),
            grid_size: 5,
            overlays: [false, true, true, false, true],
            display: 0,
        };
        assert_eq!(season_all(all), 0x5970000);
    }

    #[test]
    fn overlay_setters_touch_exactly_their_bit() {
        assert_eq!(overlay_flag(0, SeasonOverlay::Downloaded, true), 0x400000);
        assert_eq!(overlay_flag(0x400000 | 0x7, SeasonOverlay::Downloaded, false), 0x7);
        assert_eq!(overlay_flag(0, SeasonOverlay::Unseen, true), 0x800000);
        assert_eq!(overlay_flag(0, SeasonOverlay::Local, true), 0x1000000);
        assert_eq!(overlay_flag(0, SeasonOverlay::Language, true), 0x2000000);
        assert_eq!(overlay_flag(0, SeasonOverlay::Continue, true), 0x4000000);
    }

    #[test]
    fn grid_size_is_clamped_to_four_bits() {
        assert_eq!(clamp_grid_size(-3), 0);
        assert_eq!(clamp_grid_size(99), 15);
        assert_eq!(clamp_grid_size(7), 7);
    }

    #[test]
    fn chapter_set_all_applies_in_the_documented_order() {
        let word = chapter_all(c::SHOW_UNREAD, c::SHOW_DOWNLOADED, c::SHOW_BOOKMARKED, c::SORTING_NUMBER, c::SORT_ASC, c::DISPLAY_NUMBER);
        assert_eq!(word, 0x2 | 0x8 | 0x20 | 0x100 | 0x1 | 0x100000);
    }

    fn manga_repo_with_one_entry() -> (tempfile::TempDir, MangaRepo, i64) {
        let (dir, db) = crate::testdb::manga_db();
        let repo = MangaRepo::new(db);
        let id = repo
            .insert(&kuro_data::manga::Manga { favorite: true, source: 1, url: "u".into(), title: "T".into(), ..kuro_testkit::domain::manga() })
            .unwrap();
        (dir, repo, id)
    }

    fn anime_repo_with_one_entry() -> (tempfile::TempDir, AnimeRepo, i64) {
        let (dir, db) = crate::testdb::anime_db();
        let repo = AnimeRepo::new(db);
        let id = repo
            .insert(&kuro_data::anime::Anime { favorite: true, source: 1, url: "u".into(), title: "T".into(), ..kuro_testkit::domain::anime() })
            .unwrap();
        (dir, repo, id)
    }

    #[test]
    fn each_chapter_setter_writes_only_its_group() {
        let (_d, repo, id) = manga_repo_with_one_entry();
        assert!(set_chapter_unread(&repo, id, c::SHOW_UNREAD).unwrap());
        assert!(set_chapter_downloaded(&repo, id, c::SHOW_DOWNLOADED).unwrap());
        assert!(set_chapter_bookmarked(&repo, id, c::SHOW_NOT_BOOKMARKED).unwrap());
        assert!(set_chapter_display(&repo, id, c::DISPLAY_NUMBER).unwrap());
        assert_eq!(repo.by_id(id).unwrap().chapter_flags, 0x2 | 0x8 | 0x40 | 0x100000);
        assert!(set_chapter_unread(&repo, id, c::SHOW_READ).unwrap());
        assert_eq!(repo.by_id(id).unwrap().chapter_flags, 0x4 | 0x8 | 0x40 | 0x100000, "only the unread group changed");
    }

    #[test]
    fn chapter_sort_or_flip_and_set_all_persist() {
        let (_d, repo, id) = manga_repo_with_one_entry();
        set_chapter_sort_or_flip(&repo, id, c::SORTING_NUMBER).unwrap();
        assert_eq!(repo.by_id(id).unwrap().chapter_flags, 0x101);
        set_chapter_sort_or_flip(&repo, id, c::SORTING_NUMBER).unwrap();
        assert_eq!(repo.by_id(id).unwrap().chapter_flags, 0x100);
        set_chapter_all(&repo, id, [c::SHOW_UNREAD, 0, 0, c::SORTING_ALPHABET, c::SORT_ASC, 0]).unwrap();
        assert_eq!(repo.by_id(id).unwrap().chapter_flags, 0x2 | 0x300 | 0x1);
    }

    #[test]
    fn a_missing_row_gives_false_not_an_error() {
        let (_d, repo, _id) = manga_repo_with_one_entry();
        assert!(!set_chapter_unread(&repo, 9_999, c::SHOW_UNREAD).unwrap());
        assert!(!set_chapter_sort_or_flip(&repo, 9_999, c::SORTING_NUMBER).unwrap());
    }

    #[test]
    fn concurrent_setters_on_different_groups_both_persist() {
        let (_d, repo, id) = manga_repo_with_one_entry();
        let repo = std::sync::Arc::new(repo);
        std::thread::scope(|scope| {
            let a = repo.clone();
            let b = repo.clone();
            scope.spawn(move || {
                for _ in 0..50 {
                    set_chapter_unread(&a, id, c::SHOW_UNREAD).unwrap();
                }
            });
            scope.spawn(move || {
                for _ in 0..50 {
                    set_chapter_bookmarked(&b, id, c::SHOW_BOOKMARKED).unwrap();
                }
            });
        });
        let word = repo.by_id(id).unwrap().chapter_flags;
        assert_eq!(word & (c::UNREAD_MASK | c::BOOKMARKED_MASK), c::SHOW_UNREAD | c::SHOW_BOOKMARKED);
    }

    #[test]
    fn episode_and_season_setters_round_trip_through_the_database() {
        let (_d, repo, id) = anime_repo_with_one_entry();
        set_episode_sort_or_flip(&repo, id, ep::SORTING_NUMBER).unwrap();
        set_episode_sort_or_flip(&repo, id, ep::SORTING_NUMBER).unwrap();
        set_episode_unseen(&repo, id, ep::SHOW_UNSEEN).unwrap();
        assert_eq!(repo.by_id(id).unwrap().episode_flags, 0x202);
        set_season_grid_mode(&repo, id, SeasonDisplayMode::List).unwrap();
        set_season_grid_size(&repo, id, 5).unwrap();
        set_season_overlay(&repo, id, SeasonOverlay::Unseen, true).unwrap();
        set_season_overlay(&repo, id, SeasonOverlay::Local, true).unwrap();
        set_season_overlay(&repo, id, SeasonOverlay::Continue, true).unwrap();
        assert_eq!(repo.by_id(id).unwrap().season_flags, 0x5970000);
        set_season_overlay(&repo, id, SeasonOverlay::Local, false).unwrap();
        assert_eq!(repo.by_id(id).unwrap().season_flags, 0x5970000 & !0x1000000);
        set_episode_all(&repo, id, [0, 0, 0, 0, ep::SORTING_NUMBER, ep::SORT_ASC, 0, 0, 0]).unwrap();
        assert_eq!(repo.by_id(id).unwrap().episode_flags, 0x201);
    }

    #[test]
    fn concurrent_episode_setters_on_different_groups_both_persist() {
        let (_d, repo, id) = anime_repo_with_one_entry();
        let repo = std::sync::Arc::new(repo);
        std::thread::scope(|scope| {
            let a = repo.clone();
            let b = repo.clone();
            scope.spawn(move || {
                for _ in 0..50 {
                    set_episode_unseen(&a, id, ep::SHOW_UNSEEN).unwrap();
                }
            });
            scope.spawn(move || {
                for _ in 0..50 {
                    set_episode_bookmarked(&b, id, ep::SHOW_BOOKMARKED).unwrap();
                }
            });
        });
        let word = repo.by_id(id).unwrap().episode_flags;
        assert_eq!(word & (ep::UNSEEN_MASK | ep::BOOKMARKED_MASK), ep::SHOW_UNSEEN | ep::SHOW_BOOKMARKED);
    }
}
