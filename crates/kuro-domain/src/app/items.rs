//! Chapter and episode list filters and sorting (`ChapterFilter.kt`, `EpisodeFilter.kt`; T-10a-012). The filters are
//! conjunctive: a row stays when every filter that is on lets it through, then the rows are sorted. Local entries count
//! every row as downloaded. An unknown sorting value is an error here, where the item sorter of area 07 falls back.

use kuro_data::anime::Anime;
use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;
use kuro_data::manga::Manga;
use kuro_prefs::tristate::TriState;

use crate::entries::anime::{AnimeExt, episode_flags};
use crate::entries::manga::{MangaExt, chapter_flags};
use crate::error::DomainError;
use crate::flags::tri;
use crate::items::sorter::{SortItem, SortKey, compare_items};

/// What the list filters read from one chapter or episode.
pub trait FilterRow: SortItem + Clone {
    /// `read` for a chapter, `seen` for an episode.
    fn is_read(&self) -> bool;
    fn is_bookmarked(&self) -> bool;
    /// Episodes only; a chapter is never fillermarked.
    fn is_fillermarked(&self) -> bool;
}

impl FilterRow for Chapter {
    fn is_read(&self) -> bool {
        self.read
    }
    fn is_bookmarked(&self) -> bool {
        self.bookmark
    }
    fn is_fillermarked(&self) -> bool {
        false
    }
}

impl FilterRow for Episode {
    fn is_read(&self) -> bool {
        self.seen
    }
    fn is_bookmarked(&self) -> bool {
        self.bookmark
    }
    fn is_fillermarked(&self) -> bool {
        self.fillermark
    }
}

/// The downloaded filter (`Manga.downloadedFilter`, `Anime.downloadedFilter`): with the downloaded-only preference on it
/// is always `EnabledIs`; otherwise the masked flag word decides. `raw` is the already-masked downloaded group.
pub fn downloaded_filter(raw: i64, show_downloaded: i64, show_not_downloaded: i64, downloaded_only: bool) -> TriState {
    if downloaded_only {
        TriState::ENABLED_IS
    } else {
        tri(raw, show_downloaded, show_not_downloaded)
    }
}

/// The filters of one list and its sort, resolved from the entry's flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListFilters {
    /// `EnabledIs` keeps unread (or unseen) rows.
    pub unread: TriState,
    pub bookmarked: TriState,
    pub downloaded: TriState,
    /// Episodes only; `Disabled` for a chapter list.
    pub fillermarked: TriState,
    pub key: SortKey,
    pub descending: bool,
}

/// The chapter list filters of a manga (`ChapterFilter.kt`). Errors on an unknown sorting value.
pub fn chapter_filters(manga: &Manga, downloaded_only: bool) -> Result<ListFilters, DomainError> {
    Ok(ListFilters {
        unread: manga.unread_filter(),
        bookmarked: manga.bookmarked_filter(),
        downloaded: downloaded_filter(
            manga.downloaded_filter_raw(),
            chapter_flags::SHOW_DOWNLOADED,
            chapter_flags::SHOW_NOT_DOWNLOADED,
            downloaded_only,
        ),
        fillermarked: TriState::DISABLED,
        key: chapter_sort_key(manga.sorting())?,
        descending: manga.sort_descending(),
    })
}

/// The episode list filters of an anime (`EpisodeFilter.kt`). Errors on an unknown sorting value.
pub fn episode_filters(anime: &Anime, downloaded_only: bool) -> Result<ListFilters, DomainError> {
    Ok(ListFilters {
        unread: anime.unseen_filter(),
        bookmarked: anime.bookmarked_filter(),
        downloaded: downloaded_filter(
            anime.downloaded_filter_raw(),
            episode_flags::SHOW_DOWNLOADED,
            episode_flags::SHOW_NOT_DOWNLOADED,
            downloaded_only,
        ),
        fillermarked: anime.fillermarked_filter(),
        key: episode_sort_key(anime.sorting())?,
        descending: anime.sort_descending(),
    })
}

/// `episodes_filtered` (behavior 7): any of the four episode filters is on.
pub fn episodes_filtered(filters: &ListFilters) -> bool {
    [filters.unread, filters.bookmarked, filters.downloaded, filters.fillermarked]
        .iter()
        .any(|f| *f != TriState::DISABLED)
}

/// `chapters_filtered` (behavior 7): unread, bookmarked or downloaded is on.
pub fn chapters_filtered(filters: &ListFilters) -> bool {
    [filters.unread, filters.bookmarked, filters.downloaded].iter().any(|f| *f != TriState::DISABLED)
}

/// `applyFilters`: keeps the rows that pass every filter, then sorts them stably. `is_downloaded` says whether a row is
/// on disk; a local entry (`is_local`) counts every row as downloaded.
pub fn apply_filters<T: FilterRow>(
    rows: Vec<T>,
    filters: &ListFilters,
    is_local: bool,
    is_downloaded: impl Fn(&T) -> bool,
) -> Vec<T> {
    let mut kept: Vec<T> = rows
        .into_iter()
        .filter(|row| filters.unread.apply_filter(|| !row.is_read()))
        .filter(|row| filters.bookmarked.apply_filter(|| row.is_bookmarked()))
        .filter(|row| filters.downloaded.apply_filter(|| is_local || is_downloaded(row)))
        .filter(|row| filters.fillermarked.apply_filter(|| row.is_fillermarked()))
        .collect();
    kept.sort_by(|a, b| compare_items(filters.key, filters.descending, a, b));
    kept
}

/// The sort key of a chapter sorting value, strictly: an unknown value is an error (T-10a-012).
pub fn chapter_sort_key(sorting: i64) -> Result<SortKey, DomainError> {
    match sorting {
        chapter_flags::SORTING_SOURCE => Ok(SortKey::Source),
        chapter_flags::SORTING_NUMBER => Ok(SortKey::Number),
        chapter_flags::SORTING_UPLOAD_DATE => Ok(SortKey::UploadDate),
        chapter_flags::SORTING_ALPHABET => Ok(SortKey::Alphabet),
        _ => Err(DomainError::Invalid("unknown chapter sorting method")),
    }
}

/// The sort key of an episode sorting value, strictly.
pub fn episode_sort_key(sorting: i64) -> Result<SortKey, DomainError> {
    match sorting {
        episode_flags::SORTING_SOURCE => Ok(SortKey::Source),
        episode_flags::SORTING_NUMBER => Ok(SortKey::Number),
        episode_flags::SORTING_UPLOAD_DATE => Ok(SortKey::UploadDate),
        episode_flags::SORTING_ALPHABET => Ok(SortKey::Alphabet),
        _ => Err(DomainError::Invalid("unknown episode sorting method")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(name: &str, read: bool, bookmark: bool, source_order: i64, number: f64) -> Chapter {
        Chapter { name: name.into(), read, bookmark, source_order, chapter_number: number, ..Chapter::create() }
    }

    fn episode(name: &str, seen: bool, bookmark: bool, fillermark: bool, number: f64) -> Episode {
        Episode { name: name.into(), seen, bookmark, fillermark, episode_number: number, ..Episode::create() }
    }

    fn manga_with(chapter_flags: i64) -> Manga {
        Manga { chapter_flags, ..Manga::create() }
    }

    fn filters(unread: TriState, bookmarked: TriState, downloaded: TriState, key: SortKey, descending: bool) -> ListFilters {
        ListFilters { unread, bookmarked, downloaded, fillermarked: TriState::DISABLED, key, descending }
    }

    #[test]
    fn downloaded_only_forces_the_filter_on_whatever_the_flags_say() {
        assert_eq!(downloaded_filter(0x10, 0x8, 0x10, true), TriState::ENABLED_IS);
        assert_eq!(downloaded_filter(0x10, 0x8, 0x10, false), TriState::ENABLED_NOT);
        assert_eq!(downloaded_filter(0x18, 0x8, 0x10, false), TriState::DISABLED, "both bits is invalid");
        assert_eq!(chapter_filters(&manga_with(0x8), true).expect("ok").downloaded, TriState::ENABLED_IS);
        assert_eq!(chapter_filters(&manga_with(0), false).expect("ok").downloaded, TriState::DISABLED);
    }

    #[test]
    fn the_or_lists_decide_whether_a_list_counts_as_filtered() {
        let none = filters(TriState::DISABLED, TriState::DISABLED, TriState::DISABLED, SortKey::Source, false);
        assert!(!chapters_filtered(&none));
        assert!(chapters_filtered(&filters(TriState::ENABLED_IS, TriState::DISABLED, TriState::DISABLED, SortKey::Source, false)));
        assert!(chapters_filtered(&filters(TriState::DISABLED, TriState::DISABLED, TriState::ENABLED_NOT, SortKey::Source, false)));
        let fillers = ListFilters { fillermarked: TriState::ENABLED_IS, ..none };
        assert!(episodes_filtered(&fillers), "episodes also OR the fillermark filter");
        assert!(!chapters_filtered(&fillers), "chapters do not look at fillermarks");
    }

    #[test]
    fn all_twenty_seven_tri_state_combinations_keep_the_right_rows() {
        let rows: Vec<Chapter> = (0..8)
            .map(|bits: u8| chapter(&format!("c{bits}"), bits & 1 != 0, bits & 2 != 0, i64::from(bits), f64::from(bits)))
            .collect();
        let states = [TriState::DISABLED, TriState::ENABLED_IS, TriState::ENABLED_NOT];
        for unread in states {
            for bookmarked in states {
                for downloaded in states {
                    let f = filters(unread, bookmarked, downloaded, SortKey::Source, false);
                    let downloaded_rows = |c: &Chapter| c.source_order % 4 == 0;
                    let kept = apply_filters(rows.clone(), &f, false, downloaded_rows);
                    let expected: Vec<&str> = rows
                        .iter()
                        .filter(|c| unread.apply_filter(|| !c.read))
                        .filter(|c| bookmarked.apply_filter(|| c.bookmark))
                        .filter(|c| downloaded.apply_filter(|| downloaded_rows(c)))
                        .map(|c| c.name.as_str())
                        .collect();
                    let got: Vec<&str> = kept.iter().map(|c| c.name.as_str()).collect();
                    let mut got_sorted = got.clone();
                    got_sorted.sort_unstable();
                    let mut expected_sorted = expected.clone();
                    expected_sorted.sort_unstable();
                    assert_eq!(got_sorted, expected_sorted, "{unread:?} {bookmarked:?} {downloaded:?}");
                }
            }
        }
    }

    #[test]
    fn a_local_entry_counts_every_row_as_downloaded() {
        let rows = vec![chapter("a", false, false, 0, 1.0), chapter("b", false, false, 1, 2.0)];
        let f = filters(TriState::DISABLED, TriState::DISABLED, TriState::ENABLED_NOT, SortKey::Source, false);
        assert_eq!(apply_filters(rows.clone(), &f, true, |_| false).len(), 0, "local rows are downloaded, so NOT keeps none");
        assert_eq!(apply_filters(rows, &f, false, |_| false).len(), 2, "not local: NOT keeps the rows not on disk");
    }

    #[test]
    fn episodes_filter_by_fillermark_and_seen_state() {
        let rows = vec![episode("a", true, false, true, 1.0), episode("b", false, false, false, 2.0)];
        let f = ListFilters { fillermarked: TriState::ENABLED_NOT, ..filters(TriState::ENABLED_IS, TriState::DISABLED, TriState::DISABLED, SortKey::Number, false) };
        let kept = apply_filters(rows, &f, false, |_| true);
        assert_eq!(kept.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["b"], "unseen and not a filler");
    }

    #[test]
    fn sorting_follows_the_key_and_direction() {
        let rows = vec![chapter("b", false, false, 1, 2.0), chapter("a", false, false, 0, 3.0), chapter("c", false, false, 2, 1.0)];
        let off = filters(TriState::DISABLED, TriState::DISABLED, TriState::DISABLED, SortKey::Source, false);
        let by = |f: &ListFilters| apply_filters(rows.clone(), f, false, |_| true).iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(by(&off), ["c", "b", "a"], "source ascending lists the oldest source order last");
        assert_eq!(by(&ListFilters { descending: true, ..off }), ["a", "b", "c"]);
        assert_eq!(by(&ListFilters { key: SortKey::Number, ..off }), ["c", "b", "a"]);
        assert_eq!(by(&ListFilters { key: SortKey::Number, descending: true, ..off }), ["a", "b", "c"]);
    }

    #[test]
    fn an_unknown_sorting_value_is_an_error_not_a_panic() {
        assert!(matches!(chapter_sort_key(0x500), Err(DomainError::Invalid(_))));
        assert!(matches!(episode_sort_key(0x500), Err(DomainError::Invalid(_))));
        assert!(matches!(chapter_filters(&manga_with(0x300), false), Ok(f) if f.key == SortKey::Alphabet));
        assert!(matches!(chapter_filters(&manga_with(0x700), false), Ok(f) if f.key == SortKey::Alphabet), "masked to the sort bits");
    }
}
