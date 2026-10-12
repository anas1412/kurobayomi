//! The anime column codecs and flag layouts (`FetchType`, `Millis`, `Memo`, `flags`) of the `entries` and `items`
//! tables (step 4 of the unify moves them).

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

pub use crate::codec::{FetchType, GenreList, Memo, UpdateStrategy};


/// Epoch milliseconds (`AS Date` in SQLDelight): `animehistory.last_seen`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Millis(pub i64);

impl FromSql for Millis {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

impl ToSql for Millis {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

/// Bit layouts of `animes.episode_flags`, `animes.season_flags`, `animes.viewer` and
/// `categories.flags` of the anime database. Values are copied from Aniyomi `Anime.kt`,
/// `SeasonDisplayMode.kt` and `AnimeLibrarySortMode.kt`. The generic setter is
/// `crate::flags::set_flag`.
pub mod flags {
    pub const EPISODE_SORT_DESC: i64 = crate::flags::item::SORT_DESC;
    pub const EPISODE_SORT_ASC: i64 = crate::flags::item::SORT_ASC;
    pub const EPISODE_SORT_DIR_MASK: i64 = crate::flags::item::SORT_DIR_MASK;
    pub const EPISODE_SHOW_UNSEEN: i64 = crate::flags::item::SHOW_UNDONE;
    pub const EPISODE_SHOW_SEEN: i64 = crate::flags::item::SHOW_DONE;
    pub const EPISODE_UNSEEN_MASK: i64 = crate::flags::item::UNDONE_MASK;
    pub const EPISODE_SHOW_DOWNLOADED: i64 = crate::flags::item::SHOW_DOWNLOADED;
    pub const EPISODE_SHOW_NOT_DOWNLOADED: i64 = crate::flags::item::SHOW_NOT_DOWNLOADED;
    pub const EPISODE_DOWNLOADED_MASK: i64 = crate::flags::item::DOWNLOADED_MASK;
    pub const EPISODE_SHOW_BOOKMARKED: i64 = crate::flags::item::SHOW_BOOKMARKED;
    pub const EPISODE_SHOW_NOT_BOOKMARKED: i64 = crate::flags::item::SHOW_NOT_BOOKMARKED;
    pub const EPISODE_BOOKMARKED_MASK: i64 = crate::flags::item::BOOKMARKED_MASK;
    pub const EPISODE_SHOW_FILLERMARKED: i64 = crate::flags::item::SHOW_FILLERMARKED;
    pub const EPISODE_SHOW_NOT_FILLERMARKED: i64 = crate::flags::item::SHOW_NOT_FILLERMARKED;
    pub const EPISODE_FILLERMARKED_MASK: i64 = crate::flags::item::FILLERMARKED_MASK;
    pub const EPISODE_SORTING_SOURCE: i64 = crate::flags::item::SORTING_SOURCE;
    pub const EPISODE_SORTING_NUMBER: i64 = crate::flags::item::SORTING_NUMBER;
    pub const EPISODE_SORTING_UPLOAD_DATE: i64 = crate::flags::item::SORTING_UPLOAD_DATE;
    pub const EPISODE_SORTING_ALPHABET: i64 = crate::flags::item::SORTING_ALPHABET;
    pub const EPISODE_SORTING_MASK: i64 = crate::flags::item::SORTING_MASK;
    pub const EPISODE_SHOW_PREVIEWS: i64 = crate::flags::item::SHOW_PREVIEWS;
    pub const EPISODE_SHOW_NOT_PREVIEWS: i64 = crate::flags::item::SHOW_NOT_PREVIEWS;
    pub const EPISODE_PREVIEWS_MASK: i64 = crate::flags::item::PREVIEWS_MASK;
    pub const EPISODE_SHOW_SUMMARIES: i64 = crate::flags::item::SHOW_SUMMARIES;
    pub const EPISODE_SHOW_NOT_SUMMARIES: i64 = crate::flags::item::SHOW_NOT_SUMMARIES;
    pub const EPISODE_SUMMARIES_MASK: i64 = crate::flags::item::SUMMARIES_MASK;
    pub const EPISODE_DISPLAY_NAME: i64 = crate::flags::item::DISPLAY_NAME;
    pub const EPISODE_DISPLAY_NUMBER: i64 = crate::flags::item::DISPLAY_NUMBER;
    pub const EPISODE_DISPLAY_MASK: i64 = crate::flags::item::DISPLAY_MASK;
    pub const SEASON_SORT_DESC: i64 = crate::flags::season::SORT_DESC;
    pub const SEASON_SORT_ASC: i64 = crate::flags::season::SORT_ASC;
    pub const SEASON_SORT_DIR_MASK: i64 = crate::flags::season::SORT_DIR_MASK;
    pub const SEASON_SHOW_DOWNLOADED: i64 = crate::flags::season::SHOW_DOWNLOADED;
    pub const SEASON_SHOW_NOT_DOWNLOADED: i64 = crate::flags::season::SHOW_NOT_DOWNLOADED;
    pub const SEASON_DOWNLOADED_MASK: i64 = crate::flags::season::DOWNLOADED_MASK;
    pub const SEASON_SHOW_UNSEEN: i64 = crate::flags::season::SHOW_UNSEEN;
    pub const SEASON_SHOW_SEEN: i64 = crate::flags::season::SHOW_SEEN;
    pub const SEASON_UNSEEN_MASK: i64 = crate::flags::season::UNSEEN_MASK;
    pub const SEASON_SHOW_STARTED: i64 = crate::flags::season::SHOW_STARTED;
    pub const SEASON_SHOW_NOT_STARTED: i64 = crate::flags::season::SHOW_NOT_STARTED;
    pub const SEASON_STARTED_MASK: i64 = crate::flags::season::STARTED_MASK;
    pub const SEASON_SHOW_COMPLETED: i64 = crate::flags::season::SHOW_COMPLETED;
    pub const SEASON_SHOW_NOT_COMPLETED: i64 = crate::flags::season::SHOW_NOT_COMPLETED;
    pub const SEASON_COMPLETED_MASK: i64 = crate::flags::season::COMPLETED_MASK;
    pub const SEASON_SHOW_BOOKMARKED: i64 = crate::flags::season::SHOW_BOOKMARKED;
    pub const SEASON_SHOW_NOT_BOOKMARKED: i64 = crate::flags::season::SHOW_NOT_BOOKMARKED;
    pub const SEASON_BOOKMARKED_MASK: i64 = crate::flags::season::BOOKMARKED_MASK;
    pub const SEASON_SHOW_FILLERMARKED: i64 = crate::flags::season::SHOW_FILLERMARKED;
    pub const SEASON_SHOW_NOT_FILLERMARKED: i64 = crate::flags::season::SHOW_NOT_FILLERMARKED;
    pub const SEASON_FILLERMARKED_MASK: i64 = crate::flags::season::FILLERMARKED_MASK;
    pub const SEASON_SORT_SOURCE: i64 = crate::flags::season::SORT_SOURCE;
    pub const SEASON_SORT_SEASON: i64 = crate::flags::season::SORT_SEASON;
    pub const SEASON_SORT_UPLOAD: i64 = crate::flags::season::SORT_UPLOAD;
    pub const SEASON_SORT_ALPHABET: i64 = crate::flags::season::SORT_ALPHABET;
    pub const SEASON_SORT_COUNT: i64 = crate::flags::season::SORT_COUNT;
    pub const SEASON_SORT_LAST_SEEN: i64 = crate::flags::season::SORT_LAST_SEEN;
    pub const SEASON_SORT_FETCHED: i64 = crate::flags::season::SORT_FETCHED;
    pub const SEASON_SORT_MASK: i64 = crate::flags::season::SORT_MASK;
    pub const SEASON_GRID_DISPLAY_MODE_SHIFT: i64 = crate::flags::season::GRID_DISPLAY_MODE_SHIFT;
    pub const SEASON_GRID_DISPLAY_MODE_MASK: i64 = crate::flags::season::GRID_DISPLAY_MODE_MASK;
    pub const SEASON_GRID_DISPLAY_SIZE_SHIFT: i64 = crate::flags::season::GRID_DISPLAY_SIZE_SHIFT;
    pub const SEASON_GRID_DISPLAY_SIZE_MASK: i64 = crate::flags::season::GRID_DISPLAY_SIZE_MASK;
    pub const SEASON_OVERLAY_DOWNLOADED_MASK: i64 = crate::flags::season::OVERLAY_DOWNLOADED_MASK;
    pub const SEASON_OVERLAY_UNSEEN_MASK: i64 = crate::flags::season::OVERLAY_UNSEEN_MASK;
    pub const SEASON_OVERLAY_LOCAL_MASK: i64 = crate::flags::season::OVERLAY_LOCAL_MASK;
    pub const SEASON_OVERLAY_LANG_MASK: i64 = crate::flags::season::OVERLAY_LANG_MASK;
    pub const SEASON_OVERLAY_CONT_MASK: i64 = crate::flags::season::OVERLAY_CONT_MASK;
    pub const SEASON_DISPLAY_MODE_SOURCE: i64 = crate::flags::season::DISPLAY_MODE_SOURCE;
    pub const SEASON_DISPLAY_MODE_NUMBER: i64 = crate::flags::season::DISPLAY_MODE_NUMBER;
    pub const SEASON_DISPLAY_MODE_MASK: i64 = crate::flags::season::DISPLAY_MODE_MASK;
    pub const ANIME_INTRO_MASK: i64 = crate::flags::viewer::anime::INTRO_MASK;
    pub const ANIME_AIRING_EPISODE_MASK: i64 = crate::flags::viewer::anime::AIRING_EPISODE_MASK;
    /// Bits 24 to 55 (Kotlin `0x0FFFFFFFF000000L`).
    pub const ANIME_AIRING_TIME_MASK: i64 = crate::flags::viewer::anime::AIRING_TIME_MASK;
    pub const ANIME_INTRO_DISABLE_MASK: i64 = crate::flags::viewer::anime::INTRO_DISABLE_MASK;

    pub const CATEGORY_SORT_TYPE_MASK: i64 = 0x3C;
    pub const CATEGORY_SORT_DIRECTION_MASK: i64 = 0x40;
    pub const CATEGORY_SORT_ALPHABETICAL: i64 = 0x00;
    pub const CATEGORY_SORT_LAST_SEEN: i64 = 0x04;
    pub const CATEGORY_SORT_LAST_UPDATE: i64 = 0x08;
    pub const CATEGORY_SORT_UNSEEN_COUNT: i64 = 0x0C;
    pub const CATEGORY_SORT_TOTAL_EPISODES: i64 = 0x10;
    pub const CATEGORY_SORT_LATEST_EPISODE: i64 = 0x14;
    pub const CATEGORY_SORT_EPISODE_FETCH_DATE: i64 = 0x18;
    pub const CATEGORY_SORT_DATE_ADDED: i64 = 0x1C;
    pub const CATEGORY_SORT_TRACKER_MEAN: i64 = 0x20;
    pub const CATEGORY_SORT_AIRING_TIME: i64 = 0x30;
    pub const CATEGORY_SORT_RANDOM: i64 = 0x3C;
    pub const CATEGORY_SORT_ASCENDING: i64 = 0x40;
    pub const CATEGORY_SORT_DESCENDING: i64 = 0x00;

    pub use crate::flags::viewer::anime::{next_episode_airing_at, next_episode_to_air, skip_intro_disabled, skip_intro_length};

    /// Sort type of a `categories.flags` value. Unknown type bits fall back to Alphabetical.
    pub fn category_sort_type(flags: i64) -> i64 {
        const TYPES: [i64; 11] = [
            CATEGORY_SORT_ALPHABETICAL,
            CATEGORY_SORT_LAST_SEEN,
            CATEGORY_SORT_LAST_UPDATE,
            CATEGORY_SORT_UNSEEN_COUNT,
            CATEGORY_SORT_TOTAL_EPISODES,
            CATEGORY_SORT_LATEST_EPISODE,
            CATEGORY_SORT_EPISODE_FETCH_DATE,
            CATEGORY_SORT_DATE_ADDED,
            CATEGORY_SORT_TRACKER_MEAN,
            CATEGORY_SORT_AIRING_TIME,
            CATEGORY_SORT_RANDOM,
        ];
        let t = flags & CATEGORY_SORT_TYPE_MASK;
        if TYPES.contains(&t) { t } else { CATEGORY_SORT_ALPHABETICAL }
    }

    /// `animes.season_flags` grid display mode (Kotlin `SeasonDisplayMode`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SeasonDisplayMode {
        CompactGrid,
        ComfortableGrid,
        CoverOnlyGrid,
        List,
    }

    impl SeasonDisplayMode {
        /// Maps 0 to 3. Any other value is returned as `Err` (Kotlin throws there).
        pub fn from_i64(v: i64) -> Result<Self, i64> {
            match v {
                0 => Ok(Self::CompactGrid),
                1 => Ok(Self::ComfortableGrid),
                2 => Ok(Self::CoverOnlyGrid),
                3 => Ok(Self::List),
                other => Err(other),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DbError;
    use rusqlite::Connection;
    use crate::flags::set_flag;
    use crate::schema_anime::flags as f;

    #[test]
    fn column_codecs_round_trip_through_sqlite() {
        let c = Connection::open_in_memory().unwrap();
        // Millis: INTEGER milliseconds, NULL reads as None.
        let v: Option<Millis> = c.query_row("SELECT 1234", [], |r| r.get(0)).unwrap();
        assert_eq!(v, Some(Millis(1234)));
        let v: Option<Millis> = c.query_row("SELECT NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(v, None);
        // FetchType: 0 Seasons, 1 Episodes, anything else Episodes.
        for (stored, want) in [(0, FetchType::Seasons), (1, FetchType::Episodes), (2, FetchType::Episodes), (-1, FetchType::Episodes)] {
            let v: FetchType = c.query_row("SELECT ?1", [stored], |r| r.get(0)).unwrap();
            assert_eq!(v, want, "stored {stored}");
        }
        let n: i64 = c.query_row("SELECT ?1", [FetchType::Seasons], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
        // Booleans: any non-zero integer is true, writes are 0 or 1.
        let b: bool = c.query_row("SELECT 2", [], |r| r.get(0)).unwrap();
        assert!(b);
        let n: i64 = c.query_row("SELECT ?1", [true], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        // UpdateStrategy: the shared codec, 0 and 1 known, 99 reads as AlwaysUpdate.
        let s: UpdateStrategy = c.query_row("SELECT 99", [], |r| r.get(0)).unwrap();
        assert_eq!(s, UpdateStrategy::AlwaysUpdate);
    }

    #[test]
    fn memo_reads_text_and_blob_and_falls_back_to_empty_object() {
        let c = Connection::open_in_memory().unwrap();
        let read = |sql: &str| -> Memo { c.query_row(sql, [], |r| r.get(0)).unwrap() };
        assert_eq!(read("SELECT '{}'"), Memo::default());
        assert_eq!(read("SELECT x'7b7d'"), Memo::default());
        let mut want = serde_json::Map::new();
        want.insert("a".into(), serde_json::json!([1, 2]));
        assert_eq!(read(r#"SELECT CAST('{"a":[1,2]}' AS BLOB)"#), Memo(want));
        assert_eq!(read("SELECT 'not json'"), Memo::default());
        assert_eq!(read("SELECT '[1,2]'"), Memo::default());
    }

    #[test]
    fn strict_memo_decode_names_the_table_and_row_of_a_bad_cell() {
        let ok = Memo::decode(ValueRef::Blob(b"{\"a\":[1,2]}"), "animes", 7).unwrap();
        assert_eq!(ok.0["a"], serde_json::json!([1, 2]));
        assert_eq!(Memo::decode(ValueRef::Text(b"{}"), "animes", 7).unwrap(), Memo::default());
        for bad in [ValueRef::Blob(b"[1]"), ValueRef::Text(b"not json"), ValueRef::Blob(&[0xFF, 0xFE]), ValueRef::Null] {
            let err = Memo::decode(bad, "episodes", 9).unwrap_err();
            assert!(matches!(err, DbError::BadMemo { table: "episodes", id: 9 }), "{err:?}");
        }
    }

    #[test]
    fn memo_writes_compact_utf8_json_as_blob() {
        let c = Connection::open_in_memory().unwrap();
        let mut map = serde_json::Map::new();
        map.insert("k".into(), serde_json::json!(1));
        let stored: Vec<u8> = c
            .query_row("SELECT ?1", [Memo(map)], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, br#"{"k":1}"#);
        let stored: Vec<u8> = c.query_row("SELECT ?1", [Memo::default()], |r| r.get(0)).unwrap();
        assert_eq!(stored, b"{}");
    }

    #[test]
    fn genre_list_round_trip_uses_the_shared_codec() {
        let c = Connection::open_in_memory().unwrap();
        let v: GenreList = c.query_row("SELECT 'Action, Sci-Fi'", [], |r| r.get(0)).unwrap();
        assert_eq!(v, GenreList(vec!["Action".into(), "Sci-Fi".into()]));
    }

    #[test]
    fn episode_flag_constants_match_the_kotlin_values() {
        let want = [
            (f::EPISODE_SORT_ASC, 0x1), (f::EPISODE_SORT_DIR_MASK, 0x1), (f::EPISODE_UNSEEN_MASK, 0x6),
            (f::EPISODE_SHOW_SEEN, 0x4), (f::EPISODE_DOWNLOADED_MASK, 0x18), (f::EPISODE_SHOW_NOT_DOWNLOADED, 0x10),
            (f::EPISODE_BOOKMARKED_MASK, 0x60), (f::EPISODE_SHOW_NOT_BOOKMARKED, 0x40),
            (f::EPISODE_FILLERMARKED_MASK, 0x180), (f::EPISODE_SHOW_NOT_FILLERMARKED, 0x100),
            (f::EPISODE_SORTING_ALPHABET, 0x600), (f::EPISODE_SORTING_UPLOAD_DATE, 0x400),
            (f::EPISODE_SORTING_NUMBER, 0x200), (f::EPISODE_SORTING_MASK, 0x600),
            (f::EPISODE_SHOW_NOT_PREVIEWS, 0x800), (f::EPISODE_SHOW_NOT_SUMMARIES, 0x1000),
            (f::EPISODE_DISPLAY_NUMBER, 0x100000), (f::EPISODE_DISPLAY_MASK, 0x100000),
        ];
        for (got, expected) in want {
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn season_flag_constants_match_the_kotlin_values() {
        let want = [
            (f::SEASON_SORT_ASC, 0x1), (f::SEASON_DOWNLOADED_MASK, 0x6), (f::SEASON_SHOW_NOT_DOWNLOADED, 0x4),
            (f::SEASON_UNSEEN_MASK, 0x18), (f::SEASON_SHOW_SEEN, 0x10), (f::SEASON_STARTED_MASK, 0x60),
            (f::SEASON_SHOW_NOT_STARTED, 0x40), (f::SEASON_COMPLETED_MASK, 0x180),
            (f::SEASON_SHOW_NOT_COMPLETED, 0x100), (f::SEASON_BOOKMARKED_MASK, 0x600),
            (f::SEASON_SHOW_NOT_BOOKMARKED, 0x400), (f::SEASON_FILLERMARKED_MASK, 0x1800),
            (f::SEASON_SHOW_NOT_FILLERMARKED, 0x1000), (f::SEASON_SORT_MASK, 0xE000),
            (f::SEASON_SORT_LAST_SEEN, 0xA000), (f::SEASON_SORT_FETCHED, 0xC000),
            (f::SEASON_SORT_COUNT, 0x8000), (f::SEASON_GRID_DISPLAY_MODE_MASK, 0x30000),
            (f::SEASON_GRID_DISPLAY_SIZE_MASK, 0x3C0000), (f::SEASON_OVERLAY_DOWNLOADED_MASK, 0x400000),
            (f::SEASON_OVERLAY_CONT_MASK, 0x4000000), (f::SEASON_DISPLAY_MODE_NUMBER, 0x8000000),
            (f::SEASON_DISPLAY_MODE_MASK, 0x8000000), (f::SEASON_GRID_DISPLAY_MODE_SHIFT, 16),
            (f::SEASON_GRID_DISPLAY_SIZE_SHIFT, 18),
        ];
        for (got, expected) in want {
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn library_sort_flags_match_the_kotlin_values() {
        assert_eq!(f::CATEGORY_SORT_DATE_ADDED | f::CATEGORY_SORT_ASCENDING, 0b0101_1100);
        assert_eq!(f::CATEGORY_SORT_UNSEEN_COUNT | f::CATEGORY_SORT_DESCENDING, 0b0000_1100);
        assert_eq!(f::CATEGORY_SORT_ALPHABETICAL | f::CATEGORY_SORT_ASCENDING, 0b0100_0000);
        // Replacing the sort on UnseenCount + Descending with DateAdded + Ascending.
        let mask = f::CATEGORY_SORT_TYPE_MASK | f::CATEGORY_SORT_DIRECTION_MASK;
        let composed = set_flag(
            f::CATEGORY_SORT_UNSEEN_COUNT | f::CATEGORY_SORT_DESCENDING,
            f::CATEGORY_SORT_DATE_ADDED | f::CATEGORY_SORT_ASCENDING,
            mask,
        );
        assert_eq!(composed, 0b0101_1100);
        assert_ne!(composed, 0b0000_1100);
        let last_seen_then_date_added = set_flag(
            f::CATEGORY_SORT_LAST_SEEN | f::CATEGORY_SORT_ASCENDING,
            f::CATEGORY_SORT_DATE_ADDED | f::CATEGORY_SORT_ASCENDING,
            mask,
        );
        assert_eq!(last_seen_then_date_added, 0b0101_1100);
    }

    #[test]
    fn category_sort_constants_and_fallback() {
        assert_eq!(f::CATEGORY_SORT_TYPE_MASK, 0x3C);
        assert_eq!(f::CATEGORY_SORT_DIRECTION_MASK, 0x40);
        assert_eq!(f::CATEGORY_SORT_AIRING_TIME, 0x30);
        assert_eq!(f::CATEGORY_SORT_RANDOM, 0x3C);
        assert_eq!(f::category_sort_type(0x44), f::CATEGORY_SORT_LAST_SEEN);
        assert_eq!(f::category_sort_type(0x40), f::CATEGORY_SORT_ALPHABETICAL);
        // 0x24 masks to 0x24, which is not one of the eleven types.
        assert_eq!(f::category_sort_type(0x24), f::CATEGORY_SORT_ALPHABETICAL);
    }

    #[test]
    fn set_flag_reproduces_the_kotlin_setter() {
        assert_eq!(set_flag(0, 0x2, 0x6), 2);
        assert_eq!(set_flag(0x7, 0x4, 0x6), 0x5);
    }

    #[test]
    fn set_all_episode_flags_matches_the_kotlin_results() {
        // SetAnimeEpisodeFlags.awaitSetAllFlags builds from 0 with one setFlag per mask. Each row is
        // (unseen, downloaded, bookmarked, fillermarked, sorting, direction, display, previews,
        // summaries) and the expected value was computed with the Kotlin formula.
        let vectors: [([i64; 9], i64); 10] = [
            ([0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0], 0x0),
            ([0x2, 0x8, 0x20, 0x80, 0x200, 0x1, 0x100000, 0x800, 0x1000], 0x101aab),
            ([0x4, 0x10, 0x40, 0x100, 0x400, 0x0, 0x0, 0x0, 0x1000], 0x1554),
            ([0x2, 0x18, 0x60, 0x180, 0x600, 0x1, 0x0, 0x800, 0x0], 0xffb),
            ([0x0, 0x8, 0x0, 0x100, 0x200, 0x0, 0x100000, 0x0, 0x0], 0x100308),
            ([0x4, 0x0, 0x20, 0x0, 0x0, 0x1, 0x0, 0x800, 0x1000], 0x1825),
            ([0x2, 0x10, 0x0, 0x80, 0x400, 0x0, 0x100000, 0x0, 0x1000], 0x101492),
            ([0x0, 0x0, 0x40, 0x180, 0x600, 0x1, 0x0, 0x0, 0x0], 0x7c1),
            ([0x4, 0x18, 0x0, 0x0, 0x200, 0x1, 0x100000, 0x800, 0x0], 0x100a1d),
            ([0x2, 0x0, 0x60, 0x100, 0x0, 0x0, 0x100000, 0x0, 0x1000], 0x101162),
        ];
        let masks = [
            f::EPISODE_UNSEEN_MASK,
            f::EPISODE_DOWNLOADED_MASK,
            f::EPISODE_BOOKMARKED_MASK,
            f::EPISODE_FILLERMARKED_MASK,
            f::EPISODE_SORTING_MASK,
            f::EPISODE_SORT_DIR_MASK,
            f::EPISODE_DISPLAY_MASK,
            f::EPISODE_PREVIEWS_MASK,
            f::EPISODE_SUMMARIES_MASK,
        ];
        for (inputs, expected) in vectors {
            let flags = inputs.into_iter().zip(masks).fold(0, |acc, (flag, mask)| set_flag(acc, flag, mask));
            assert_eq!(flags, expected, "inputs {inputs:x?}");
        }
    }

    #[test]
    fn viewer_accessors_match_the_kotlin_getters() {
        let viewer = (1_700_000_000_i64 << 24) | (12 << 8) | 85;
        assert_eq!(f::skip_intro_length(viewer), 85);
        assert_eq!(f::next_episode_to_air(viewer), 12);
        assert_eq!(f::next_episode_airing_at(viewer), 1_700_000_000);
        assert!(!f::skip_intro_disabled(viewer));
        let disabled = viewer | (1 << 56);
        assert!(f::skip_intro_disabled(disabled));
        assert_eq!(f::skip_intro_length(disabled), 85);
        assert_eq!(f::next_episode_to_air(disabled), 12);
        assert_eq!(f::next_episode_airing_at(disabled), 1_700_000_000);
        assert_eq!((f::skip_intro_length(0), f::next_episode_to_air(0), f::next_episode_airing_at(0)), (0, 0, 0));
        assert!(!f::skip_intro_disabled(0));
    }

    #[test]
    fn season_display_mode_maps_zero_to_three_and_rejects_the_rest() {
        use f::SeasonDisplayMode as Mode;
        assert_eq!(
            (0..4).map(Mode::from_i64).collect::<Result<Vec<_>, _>>().unwrap(),
            [Mode::CompactGrid, Mode::ComfortableGrid, Mode::CoverOnlyGrid, Mode::List]
        );
        assert_eq!(Mode::from_i64(4), Err(4));
        assert_eq!(Mode::from_i64(-1), Err(-1));
    }
}
