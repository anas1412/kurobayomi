//! Library sort modes: the stored `<TYPE>,<DIRECTION>` text and the integer flag form used by
//! `categories.flags` (`p1-behavior.md` section 3.3, Kotlin `MangaLibrarySortMode.kt`,
//! `AnimeLibrarySortMode.kt` and `Flag.kt`).

use crate::values::{Decoded, PrefType, PrefValue};

/// Mask of the sort type bits in a category flag integer.
pub const SORT_TYPE_MASK: i64 = 0x3C;
/// The direction bit: set for ascending.
pub const SORT_ASCENDING_FLAG: i64 = 0x40;

/// Sort direction, stored as `ASCENDING` or any other word for descending (Kotlin only accepts the
/// exact word `ASCENDING` as ascending).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    /// The direction bit of the flag integer.
    pub const fn flag(self) -> i64 {
        match self {
            Self::Ascending => SORT_ASCENDING_FLAG,
            Self::Descending => 0,
        }
    }
}

/// A sort type with its stored token and flag bits.
pub trait SortKind: Copy + PartialEq + std::fmt::Debug + 'static {
    /// The type used when the stored text names no known type, and for a flag with unassigned bits.
    const DEFAULT: Self;
    /// Every type with its stored token and flag bits, in Kotlin order.
    const ROWS: &'static [(Self, &'static str, i64)];
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
    const ROWS: &'static [(Self, &'static str, i64)] = &[
        (Self::Alphabetical, "ALPHABETICAL", 0x00),
        (Self::LastRead, "LAST_READ", 0x04),
        (Self::LastUpdate, "LAST_MANGA_UPDATE", 0x08),
        (Self::UnreadCount, "UNREAD_COUNT", 0x0C),
        (Self::TotalChapters, "TOTAL_CHAPTERS", 0x10),
        (Self::LatestChapter, "LATEST_CHAPTER", 0x14),
        (Self::ChapterFetchDate, "CHAPTER_FETCH_DATE", 0x18),
        (Self::DateAdded, "DATE_ADDED", 0x1C),
        (Self::TrackerMean, "TRACKER_MEAN", 0x20),
        (Self::Random, "RANDOM", 0x3C),
    ];
}

impl SortKind for AnimeSortKind {
    const DEFAULT: Self = Self::Alphabetical;
    const ROWS: &'static [(Self, &'static str, i64)] = &[
        (Self::Alphabetical, "ALPHABETICAL", 0x00),
        (Self::LastSeen, "LAST_SEEN", 0x04),
        (Self::LastAnimeUpdate, "LAST_ANIME_UPDATE", 0x08),
        (Self::UnseenCount, "UNSEEN_COUNT", 0x0C),
        (Self::TotalEpisodes, "TOTAL_EPISODES", 0x10),
        (Self::LatestEpisode, "LATEST_EPISODE", 0x14),
        (Self::EpisodeFetchDate, "EPISODE_FETCH_DATE", 0x18),
        (Self::DateAdded, "DATE_ADDED", 0x1C),
        (Self::TrackerMean, "TRACKER_MEAN", 0x20),
        (Self::AiringTime, "AIRING_TIME", 0x30),
        (Self::Random, "RANDOM", 0x3C),
    ];
}

impl<K: SortKind> LibrarySort<K> {
    /// `ALPHABETICAL, ASCENDING`, the default of both libraries.
    pub fn default_sort() -> Self {
        Self {
            kind: K::DEFAULT,
            direction: SortDirection::Ascending,
        }
    }

    /// The stored text, such as `LAST_READ,DESCENDING`.
    pub fn serialize(self) -> String {
        let token = K::ROWS
            .iter()
            .find(|(k, _, _)| *k == self.kind)
            .map_or("ALPHABETICAL", |(_, t, _)| *t);
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
            .find(|(_, t, _)| *t == token)
            .map_or(K::DEFAULT, |(k, _, _)| *k);
        let direction = if direction == "ASCENDING" {
            SortDirection::Ascending
        } else {
            SortDirection::Descending
        };
        Some(Self { kind, direction })
    }

    /// The integer flag form: type bits plus the direction bit.
    pub fn to_flag(self) -> i64 {
        let kind = K::ROWS
            .iter()
            .find(|(k, _, _)| *k == self.kind)
            .map_or(0, |(_, _, f)| *f);
        kind | self.direction.flag()
    }

    /// Reads the integer flag form. Unassigned type bits give [`SortKind::DEFAULT`].
    pub fn from_flag(flag: i64) -> Self {
        let kind = K::ROWS
            .iter()
            .find(|(_, _, f)| flag & SORT_TYPE_MASK == *f)
            .map_or(K::DEFAULT, |(k, _, _)| *k);
        let direction = if flag & SORT_ASCENDING_FLAG != 0 {
            SortDirection::Ascending
        } else {
            SortDirection::Descending
        };
        Self { kind, direction }
    }
}

impl<K: SortKind> PrefType for LibrarySort<K> {
    fn encode(&self) -> PrefValue {
        PrefValue::Str(self.serialize())
    }

    fn decode(v: &PrefValue) -> Decoded<Self> {
        match v {
            PrefValue::Str(s) => Self::deserialize(s).map_or(Decoded::Invalid, Decoded::Ok),
            _ => Decoded::WrongType,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{ANIME_SORT_TOKENS, MANGA_SORT_TOKENS};

    #[test]
    fn every_type_round_trips_with_both_directions() {
        for (kind, token, _) in MangaSortKind::ROWS {
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
        assert_eq!(MangaLibrarySort::decode(&PrefValue::Str("ALPHABETICAL".into())), Decoded::Invalid);
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

    #[test]
    fn the_flag_form_matches_the_plan_table() {
        let cases: [(MangaSortKind, i64); 10] = [
            (MangaSortKind::Alphabetical, 0x00),
            (MangaSortKind::LastRead, 0x04),
            (MangaSortKind::LastUpdate, 0x08),
            (MangaSortKind::UnreadCount, 0x0C),
            (MangaSortKind::TotalChapters, 0x10),
            (MangaSortKind::LatestChapter, 0x14),
            (MangaSortKind::ChapterFetchDate, 0x18),
            (MangaSortKind::DateAdded, 0x1C),
            (MangaSortKind::TrackerMean, 0x20),
            (MangaSortKind::Random, 0x3C),
        ];
        for (kind, bits) in cases {
            let asc = MangaLibrarySort { kind, direction: SortDirection::Ascending };
            let desc = MangaLibrarySort { kind, direction: SortDirection::Descending };
            assert_eq!(asc.to_flag(), bits | 0x40);
            assert_eq!(desc.to_flag(), bits);
            assert_eq!(MangaLibrarySort::from_flag(asc.to_flag()), asc);
            assert_eq!(MangaLibrarySort::from_flag(desc.to_flag()), desc);
        }
        let airing = AnimeLibrarySort { kind: AnimeSortKind::AiringTime, direction: SortDirection::Descending };
        assert_eq!(airing.to_flag(), 0x30);
        assert_eq!(AnimeLibrarySort::from_flag(0x30).kind, AnimeSortKind::AiringTime);
        assert_eq!(AnimeLibrarySort::from_flag(0x20).kind, AnimeSortKind::TrackerMean);
    }

    #[test]
    fn unassigned_flag_bits_fall_back_to_alphabetical() {
        for bits in [0x24, 0x28, 0x2C, 0x34, 0x38] {
            assert_eq!(MangaLibrarySort::from_flag(bits).kind, MangaSortKind::Alphabetical, "{bits:#x}");
            assert_eq!(AnimeLibrarySort::from_flag(bits).kind, AnimeSortKind::Alphabetical, "{bits:#x}");
        }
        assert_eq!(MangaLibrarySort::from_flag(0x3C).kind, MangaSortKind::Random);
    }
}
