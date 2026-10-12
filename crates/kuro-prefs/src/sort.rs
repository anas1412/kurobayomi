//! Library sort modes: the stored `<TYPE>,<DIRECTION>` text (Kotlin `MangaLibrarySortMode.kt`,
//! `AnimeLibrarySortMode.kt`). The integer flag form is `kuro_db::flags::library_sort`.

/// Sort direction, stored as `ASCENDING` or any other word for descending (Kotlin only accepts the
/// exact word `ASCENDING` as ascending).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// A sort type with its stored token.
pub trait SortKind: Copy + PartialEq + std::fmt::Debug + 'static {
    /// The type used when the stored text names no known type.
    const DEFAULT: Self;
    /// Every type with its stored token, in Kotlin order.
    const ROWS: &'static [(Self, &'static str)];
}

/// A library sort: a type and a direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibrarySort<K: SortKind> {
    pub kind: K,
    pub direction: SortDirection,
}

pub type MangaLibrarySort = LibrarySort<MangaSortKind>;
pub type AnimeLibrarySort = LibrarySort<AnimeSortKind>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MangaSortKind {
    Alphabetical,
    LastRead,
    LastUpdate,
    UnreadCount,
    TotalChapters,
    LatestChapter,
    ChapterFetchDate,
    DateAdded,
    TrackerMean,
    Random,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimeSortKind {
    Alphabetical,
    LastSeen,
    LastAnimeUpdate,
    UnseenCount,
    TotalEpisodes,
    LatestEpisode,
    EpisodeFetchDate,
    DateAdded,
    TrackerMean,
    AiringTime,
    Random,
}

impl SortKind for MangaSortKind {
    const DEFAULT: Self = Self::Alphabetical;
    const ROWS: &'static [(Self, &'static str)] = &[
        (Self::Alphabetical, "ALPHABETICAL"),
        (Self::LastRead, "LAST_READ"),
        (Self::LastUpdate, "LAST_MANGA_UPDATE"),
        (Self::UnreadCount, "UNREAD_COUNT"),
        (Self::TotalChapters, "TOTAL_CHAPTERS"),
        (Self::LatestChapter, "LATEST_CHAPTER"),
        (Self::ChapterFetchDate, "CHAPTER_FETCH_DATE"),
        (Self::DateAdded, "DATE_ADDED"),
        (Self::TrackerMean, "TRACKER_MEAN"),
        (Self::Random, "RANDOM"),
    ];
}

impl SortKind for AnimeSortKind {
    const DEFAULT: Self = Self::Alphabetical;
    const ROWS: &'static [(Self, &'static str)] = &[
        (Self::Alphabetical, "ALPHABETICAL"),
        (Self::LastSeen, "LAST_SEEN"),
        (Self::LastAnimeUpdate, "LAST_ANIME_UPDATE"),
        (Self::UnseenCount, "UNSEEN_COUNT"),
        (Self::TotalEpisodes, "TOTAL_EPISODES"),
        (Self::LatestEpisode, "LATEST_EPISODE"),
        (Self::EpisodeFetchDate, "EPISODE_FETCH_DATE"),
        (Self::DateAdded, "DATE_ADDED"),
        (Self::TrackerMean, "TRACKER_MEAN"),
        (Self::AiringTime, "AIRING_TIME"),
        (Self::Random, "RANDOM"),
    ];
}

impl<K: SortKind> LibrarySort<K> {
    /// The stored text, such as `LAST_READ,DESCENDING`.
    pub fn serialize(self) -> String {
        let token = K::ROWS
            .iter()
            .find(|(k, _)| *k == self.kind)
            .map_or("ALPHABETICAL", |(_, t)| *t);
        let direction = match self.direction {
            SortDirection::Ascending => "ASCENDING",
            SortDirection::Descending => "DESCENDING",
        };
        format!("{token},{direction}")
    }

    /// Reads the stored text. Empty text and text without a comma give `None` (the caller uses the
    /// default). An unknown type gives [`SortKind::DEFAULT`]; only the exact word `ASCENDING` is ascending.
    pub fn deserialize(stored: &str) -> Option<Self> {
        let (token, direction) = stored.split_once(',')?;
        let kind = K::ROWS
            .iter()
            .find(|(_, t)| *t == token)
            .map_or(K::DEFAULT, |(k, _)| *k);
        let direction = if direction == "ASCENDING" {
            SortDirection::Ascending
        } else {
            SortDirection::Descending
        };
        Some(Self { kind, direction })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{ANIME_SORT_TOKENS, MANGA_SORT_TOKENS};

    #[test]
    fn every_type_round_trips_with_both_directions() {
        for (kind, token) in MangaSortKind::ROWS {
            for direction in [SortDirection::Ascending, SortDirection::Descending] {
                let sort = MangaLibrarySort { kind: *kind, direction };
                let text = sort.serialize();
                assert_eq!(text, format!("{token},{}", if direction == SortDirection::Ascending { "ASCENDING" } else { "DESCENDING" }));
                assert_eq!(MangaLibrarySort::deserialize(&text), Some(sort));
            }
        }
        assert_eq!(MangaSortKind::ROWS.len(), 10);
        assert_eq!(AnimeSortKind::ROWS.len(), 11);
    }

    #[test]
    fn the_token_lists_match_the_library_constants() {
        let manga: Vec<&str> = MangaSortKind::ROWS.iter().map(|r| r.1).collect();
        assert_eq!(manga, MANGA_SORT_TOKENS);
        let anime: Vec<&str> = AnimeSortKind::ROWS.iter().map(|r| r.1).collect();
        assert_eq!(anime, ANIME_SORT_TOKENS);
    }

    #[test]
    fn empty_and_comma_less_text_gives_no_sort() {
        assert_eq!(MangaLibrarySort::deserialize(""), None);
        assert_eq!(MangaLibrarySort::deserialize("ALPHABETICAL"), None);
    }

    #[test]
    fn unknown_type_falls_back_to_alphabetical_and_keeps_the_direction() {
        assert_eq!(
            MangaLibrarySort::deserialize("FOO,DESCENDING"),
            Some(MangaLibrarySort { kind: MangaSortKind::Alphabetical, direction: SortDirection::Descending })
        );
    }

    #[test]
    fn only_the_exact_word_ascending_is_ascending() {
        let sort = MangaLibrarySort::deserialize("LAST_READ,asc").unwrap();
        assert_eq!(sort.direction, SortDirection::Descending);
        assert_eq!(sort.kind, MangaSortKind::LastRead);
    }
}
