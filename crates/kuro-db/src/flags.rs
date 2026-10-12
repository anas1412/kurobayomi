//! Bit layouts of `entries.item_flags` (`item`), `entries.season_flags` (`season`), `entries.viewer` (`viewer`),
//! `categories.flags` (`library_sort`, a shim over `kuro_platform::sort`) and the old manga `chapter_flags` (`chapter`, a shim).
//! Values are copied from Aniyomi `Manga.kt`, `MangaLibrarySortMode.kt`, `ReadingMode.kt` and
//! `ReaderOrientation.kt`. Every mask is applied before comparing, so stray high bits are ignored.

/// `(value & !mask) | (flag & mask)`: `FlagWithMask.plus` and `SetMangaChapterFlags.setFlag`.
pub const fn set_flag(value: i64, flag: i64, mask: i64) -> i64 {
    (value & !mask) | (flag & mask)
}

/// `mangas.chapter_flags`.
pub mod chapter {
    pub const SORT_DESC: i64 = 0x0;
    pub const SORT_ASC: i64 = 0x1;
    pub const SORT_DIR_MASK: i64 = 0x1;

    pub const SHOW_ALL: i64 = 0x0;
    pub const SHOW_UNREAD: i64 = 0x2;
    pub const SHOW_READ: i64 = 0x4;
    pub const UNREAD_MASK: i64 = 0x6;

    pub const SHOW_DOWNLOADED: i64 = 0x8;
    pub const SHOW_NOT_DOWNLOADED: i64 = 0x10;
    pub const DOWNLOADED_MASK: i64 = 0x18;

    pub const SHOW_BOOKMARKED: i64 = 0x20;
    pub const SHOW_NOT_BOOKMARKED: i64 = 0x40;
    pub const BOOKMARKED_MASK: i64 = 0x60;

    pub const SORTING_SOURCE: i64 = 0x000;
    pub const SORTING_NUMBER: i64 = 0x100;
    pub const SORTING_UPLOAD_DATE: i64 = 0x200;
    pub const SORTING_ALPHABET: i64 = 0x300;
    pub const SORTING_MASK: i64 = 0x300;

    pub const DISPLAY_NAME: i64 = 0x000000;
    pub const DISPLAY_NUMBER: i64 = 0x100000;
    pub const DISPLAY_MASK: i64 = 0x100000;
}

/// Manga `chapter_flags` to the unified `entries.item_flags` (the anime layout): the two sorting bits 0x300 move up
/// one place to 0x600. Manga rows never carry the fillermark, preview or summary bits, so `un(tr(x)) == x`.
pub const fn manga_item_flags_to_unified(x: i64) -> i64 {
    (x & !0x300) | ((x & 0x300) << 1)
}

/// The inverse of [`manga_item_flags_to_unified`]. The statements use the same expression in SQL.
pub const fn manga_item_flags_from_unified(x: i64) -> i64 {
    (x & !0x600) | ((x & 0x600) >> 1)
}

/// `entries.item_flags` for BOTH kinds: the anime superset layout (docs/design/unified-model.md section 0, item 5).
/// A manga row only ever carries the groups of [`MANGA_MASK`]. Values are those of the old anime `episode_flags`;
/// the manga `chapter_flags` layout differs only in the two sorting bits (0x300 there, 0x600 here).
pub mod item {
    pub const SORT_DESC: i64 = 0x0;
    pub const SORT_ASC: i64 = 0x1;
    pub const SORT_DIR_MASK: i64 = 0x1;
    pub const SHOW_UNDONE: i64 = 0x2;
    pub const SHOW_DONE: i64 = 0x4;
    pub const UNDONE_MASK: i64 = 0x6;
    pub const SHOW_DOWNLOADED: i64 = 0x8;
    pub const SHOW_NOT_DOWNLOADED: i64 = 0x10;
    pub const DOWNLOADED_MASK: i64 = 0x18;
    pub const SHOW_BOOKMARKED: i64 = 0x20;
    pub const SHOW_NOT_BOOKMARKED: i64 = 0x40;
    pub const BOOKMARKED_MASK: i64 = 0x60;
    pub const SHOW_FILLERMARKED: i64 = 0x80;
    pub const SHOW_NOT_FILLERMARKED: i64 = 0x100;
    pub const FILLERMARKED_MASK: i64 = 0x180;
    pub const SORTING_SOURCE: i64 = 0x0;
    pub const SORTING_NUMBER: i64 = 0x200;
    pub const SORTING_UPLOAD_DATE: i64 = 0x400;
    pub const SORTING_ALPHABET: i64 = 0x600;
    pub const SORTING_MASK: i64 = 0x600;
    pub const SHOW_PREVIEWS: i64 = 0x0;
    pub const SHOW_NOT_PREVIEWS: i64 = 0x800;
    pub const PREVIEWS_MASK: i64 = 0x800;
    pub const SHOW_SUMMARIES: i64 = 0x0;
    pub const SHOW_NOT_SUMMARIES: i64 = 0x1000;
    pub const SUMMARIES_MASK: i64 = 0x1000;
    pub const DISPLAY_NAME: i64 = 0x0;
    pub const DISPLAY_NUMBER: i64 = 0x100000;
    pub const DISPLAY_MASK: i64 = 0x100000;
    /// The groups a manga row may carry (direction, undone, downloaded, bookmarked, sorting, display).
    pub const MANGA_MASK: i64 =
        SORT_DIR_MASK | UNDONE_MASK | DOWNLOADED_MASK | BOOKMARKED_MASK | SORTING_MASK | DISPLAY_MASK;
}

/// `entries.season_flags` (anime only), the `SEASON_*` constants of the old anime layout with the prefix dropped.
pub mod season {
    pub const SORT_DESC: i64 = 0x0;
    pub const SORT_ASC: i64 = 0x1;
    pub const SORT_DIR_MASK: i64 = 0x1;
    pub const SHOW_DOWNLOADED: i64 = 0x2;
    pub const SHOW_NOT_DOWNLOADED: i64 = 0x4;
    pub const DOWNLOADED_MASK: i64 = 0x6;
    pub const SHOW_UNSEEN: i64 = 0x8;
    pub const SHOW_SEEN: i64 = 0x10;
    pub const UNSEEN_MASK: i64 = 0x18;
    pub const SHOW_STARTED: i64 = 0x20;
    pub const SHOW_NOT_STARTED: i64 = 0x40;
    pub const STARTED_MASK: i64 = 0x60;
    pub const SHOW_COMPLETED: i64 = 0x80;
    pub const SHOW_NOT_COMPLETED: i64 = 0x100;
    pub const COMPLETED_MASK: i64 = 0x180;
    pub const SHOW_BOOKMARKED: i64 = 0x200;
    pub const SHOW_NOT_BOOKMARKED: i64 = 0x400;
    pub const BOOKMARKED_MASK: i64 = 0x600;
    pub const SHOW_FILLERMARKED: i64 = 0x800;
    pub const SHOW_NOT_FILLERMARKED: i64 = 0x1000;
    pub const FILLERMARKED_MASK: i64 = 0x1800;
    pub const SORT_SOURCE: i64 = 0x0;
    pub const SORT_SEASON: i64 = 0x2000;
    pub const SORT_UPLOAD: i64 = 0x4000;
    pub const SORT_ALPHABET: i64 = 0x6000;
    pub const SORT_COUNT: i64 = 0x8000;
    pub const SORT_LAST_SEEN: i64 = 0xA000;
    pub const SORT_FETCHED: i64 = 0xC000;
    pub const SORT_MASK: i64 = 0xE000;
    pub const GRID_DISPLAY_MODE_SHIFT: i64 = 16;
    pub const GRID_DISPLAY_MODE_MASK: i64 = 0x30000;
    pub const GRID_DISPLAY_SIZE_SHIFT: i64 = 18;
    pub const GRID_DISPLAY_SIZE_MASK: i64 = 0x3C0000;
    pub const OVERLAY_DOWNLOADED_MASK: i64 = 0x400000;
    pub const OVERLAY_UNSEEN_MASK: i64 = 0x800000;
    pub const OVERLAY_LOCAL_MASK: i64 = 0x1000000;
    pub const OVERLAY_LANG_MASK: i64 = 0x2000000;
    pub const OVERLAY_CONT_MASK: i64 = 0x4000000;
    pub const DISPLAY_MODE_SOURCE: i64 = 0x0;
    pub const DISPLAY_MODE_NUMBER: i64 = 0x8000000;
    pub const DISPLAY_MODE_MASK: i64 = 0x8000000;
}

/// `categories.flags`: the library sort of one category. The default is Alphabetical, Ascending.
pub mod library_sort {
    pub const TYPE_MASK: i64 = 0b0011_1100;
    pub const DIRECTION_MASK: i64 = 0b0100_0000;

    pub const ALPHABETICAL: i64 = 0b0000_0000;
    pub const LAST_READ: i64 = 0b0000_0100;
    pub const LAST_UPDATE: i64 = 0b0000_1000;
    pub const UNREAD_COUNT: i64 = 0b0000_1100;
    pub const TOTAL_CHAPTERS: i64 = 0b0001_0000;
    pub const LATEST_CHAPTER: i64 = 0b0001_0100;
    pub const CHAPTER_FETCH_DATE: i64 = 0b0001_1000;
    pub const DATE_ADDED: i64 = 0b0001_1100;
    pub const TRACKER_MEAN: i64 = 0b0010_0000;
    pub const RANDOM: i64 = 0b0011_1100;

    pub const ASCENDING: i64 = 0b0100_0000;
    pub const DESCENDING: i64 = 0b0000_0000;

    pub const DEFAULT: i64 = ALPHABETICAL | ASCENDING;

    const TYPES: [i64; 10] = [
        ALPHABETICAL,
        LAST_READ,
        LAST_UPDATE,
        UNREAD_COUNT,
        TOTAL_CHAPTERS,
        LATEST_CHAPTER,
        CHAPTER_FETCH_DATE,
        DATE_ADDED,
        TRACKER_MEAN,
        RANDOM,
    ];

    /// Sort type of a stored value. Unknown type bits fall back to Alphabetical.
    pub fn sort_type(value: i64) -> i64 {
        let t = value & TYPE_MASK;
        if TYPES.contains(&t) { t } else { ALPHABETICAL }
    }

    pub fn is_ascending(value: i64) -> bool {
        value & DIRECTION_MASK == ASCENDING
    }

    /// Stored value for a sort type (one of the constants above) and direction.
    pub fn encode(sort_type: i64, ascending: bool) -> i64 {
        (sort_type & TYPE_MASK) | if ascending { ASCENDING } else { DESCENDING }
    }

    /// `flags` with its sort type and direction replaced and every other bit kept. Aniyomi adds the two
    /// values (`flags + type + direction`), which corrupts the bits it does not mask (T-05a-025).
    pub fn with_sort(flags: i64, sort_type: i64, ascending: bool) -> i64 {
        super::set_flag(flags, encode(sort_type, ascending), TYPE_MASK | DIRECTION_MASK)
    }

    /// The `MangaLibrarySort.Serializer` names, one per sort type.
    pub const NAMES: [(&str, i64); 10] = [
        ("ALPHABETICAL", ALPHABETICAL),
        ("LAST_READ", LAST_READ),
        ("LAST_MANGA_UPDATE", LAST_UPDATE),
        ("UNREAD_COUNT", UNREAD_COUNT),
        ("TOTAL_CHAPTERS", TOTAL_CHAPTERS),
        ("LATEST_CHAPTER", LATEST_CHAPTER),
        ("CHAPTER_FETCH_DATE", CHAPTER_FETCH_DATE),
        ("DATE_ADDED", DATE_ADDED),
        ("TRACKER_MEAN", TRACKER_MEAN),
        ("RANDOM", RANDOM),
    ];

    /// Flags of a stored sort such as `RANDOM,ASCENDING`. A string without a direction (including an
    /// empty one) gives the default, and an unknown type name gives Alphabetical.
    pub fn from_serialized(s: &str) -> i64 {
        kuro_platform::sort::flags_from_text(kuro_platform::kind::Kind::Manga, s)
    }

    /// The stored string of a sort's flags, the inverse of [`from_serialized`].
    pub fn to_serialized(value: i64) -> String {
        kuro_platform::sort::text_from_flags(kuro_platform::kind::Kind::Manga, value)
    }
}

/// `mangas.viewer`: reading mode in the low bits, reader orientation above them.
pub mod viewer {
    pub const READING_MODE_MASK: i64 = 0x7;
    pub const MODE_DEFAULT: i64 = 0;
    pub const MODE_LEFT_TO_RIGHT: i64 = 1;
    pub const MODE_RIGHT_TO_LEFT: i64 = 2;
    pub const MODE_VERTICAL: i64 = 3;
    pub const MODE_WEBTOON: i64 = 4;
    pub const MODE_CONTINUOUS_VERTICAL: i64 = 5;

    pub const ORIENTATION_MASK: i64 = 0x38;
    pub const ORIENTATION_DEFAULT: i64 = 0x00;
    pub const ORIENTATION_FREE: i64 = 0x08;
    pub const ORIENTATION_PORTRAIT: i64 = 0x10;
    pub const ORIENTATION_LANDSCAPE: i64 = 0x18;
    pub const ORIENTATION_LOCKED_PORTRAIT: i64 = 0x20;
    pub const ORIENTATION_LOCKED_LANDSCAPE: i64 = 0x28;
    pub const ORIENTATION_REVERSE_PORTRAIT: i64 = 0x30;

    /// Reading mode of a viewer value. Masks first; values outside 0..=5 fall back to DEFAULT.
    pub fn reading_mode(value: i64) -> i64 {
        let m = value & READING_MODE_MASK;
        if m <= MODE_CONTINUOUS_VERTICAL { m } else { MODE_DEFAULT }
    }

    /// Orientation of a viewer value. Masks first; values outside the table fall back to DEFAULT.
    pub fn orientation(value: i64) -> i64 {
        let o = value & ORIENTATION_MASK;
        const ORIENTATIONS: [i64; 7] = [
            ORIENTATION_DEFAULT,
            ORIENTATION_FREE,
            ORIENTATION_PORTRAIT,
            ORIENTATION_LANDSCAPE,
            ORIENTATION_LOCKED_PORTRAIT,
            ORIENTATION_LOCKED_LANDSCAPE,
            ORIENTATION_REVERSE_PORTRAIT,
        ];
        if ORIENTATIONS.contains(&o) { o } else { ORIENTATION_DEFAULT }
    }

    /// The manga reader groups of `entries.viewer`.
    pub mod manga {
        pub use super::{
            ORIENTATION_DEFAULT, ORIENTATION_FREE, ORIENTATION_LANDSCAPE, ORIENTATION_LOCKED_LANDSCAPE,
            ORIENTATION_LOCKED_PORTRAIT, ORIENTATION_MASK, ORIENTATION_PORTRAIT, ORIENTATION_REVERSE_PORTRAIT,
            MODE_CONTINUOUS_VERTICAL, MODE_DEFAULT, MODE_LEFT_TO_RIGHT, MODE_RIGHT_TO_LEFT, MODE_VERTICAL, MODE_WEBTOON,
            READING_MODE_MASK, orientation, reading_mode,
        };
    }

    /// The anime player groups of `entries.viewer`: skip-intro length, the next airing episode and its time.
    pub mod anime {
        pub const INTRO_MASK: i64 = 0xFF;
        pub const AIRING_EPISODE_MASK: i64 = 0xFFFF00;
        /// Bits 24 to 55 (Kotlin `0x0FFFFFFFF000000L`).
        pub const AIRING_TIME_MASK: i64 = 0xFFFF_FFFF << 24;
        pub const INTRO_DISABLE_MASK: i64 = 0x0100_0000_0000_0000;

        /// Skip-intro length in seconds (`viewer & 0xFF`).
        pub fn skip_intro_length(viewer: i64) -> i64 {
            viewer & INTRO_MASK
        }

        /// Episode number of the next episode to air (`(viewer & 0xFFFF00) / 16^2`).
        pub fn next_episode_to_air(viewer: i64) -> i64 {
            (viewer & AIRING_EPISODE_MASK) >> 8
        }

        /// Airing time of the next episode, Unix seconds (`(viewer & 0x0FFFFFFFF000000) / 16^6`).
        pub fn next_episode_airing_at(viewer: i64) -> i64 {
            (viewer & AIRING_TIME_MASK) >> 24
        }

        pub fn skip_intro_disabled(viewer: i64) -> bool {
            viewer & INTRO_DISABLE_MASK == INTRO_DISABLE_MASK
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manga_item_flags_round_trip_through_the_unified_layout() {
        for x in 0..0x20_0000_i64 {
            // manga-legal: no bit above the display bit, none of 0x80, 0x400..0x800 range is used by manga
            if x & 0x1880 != 0 || x & 0x400 != 0 {
                continue;
            }
            assert_eq!(manga_item_flags_from_unified(manga_item_flags_to_unified(x)), x, "{x:#x}");
        }
        assert_eq!(manga_item_flags_to_unified(chapter::SORTING_NUMBER), 0x200);
        assert_eq!(manga_item_flags_to_unified(chapter::SORTING_UPLOAD_DATE), 0x400);
        assert_eq!(manga_item_flags_to_unified(chapter::SORTING_ALPHABET), 0x600);
        assert_eq!(manga_item_flags_to_unified(chapter::DISPLAY_NUMBER | chapter::SORT_ASC), 0x100001);
    }

    #[test]
    fn set_flag_follows_the_kotlin_formula() {
        assert_eq!(set_flag(0b1111, 0b0000, 0b0110), 0b1001);
        assert_eq!(set_flag(0, 0b0100, 0b0110), 0b0100);
        assert_eq!(set_flag(0b0100, 0, 0b0110), 0);
    }

    #[test]
    fn set_all_chapter_flags_from_zero_matches_defaults() {
        // SetMangaChapterFlags.awaitSetAllFlags: rebuild from 0 through the six masks.
        let mut v = 0;
        v = set_flag(v, chapter::SORT_ASC, chapter::SORT_DIR_MASK);
        v = set_flag(v, chapter::SHOW_READ, chapter::UNREAD_MASK);
        v = set_flag(v, chapter::SHOW_DOWNLOADED, chapter::DOWNLOADED_MASK);
        v = set_flag(v, chapter::SHOW_BOOKMARKED, chapter::BOOKMARKED_MASK);
        v = set_flag(v, chapter::SORTING_NUMBER, chapter::SORTING_MASK);
        v = set_flag(v, chapter::DISPLAY_NUMBER, chapter::DISPLAY_MASK);
        assert_eq!(v, 0x1 | 0x4 | 0x8 | 0x20 | 0x100 | 0x100000);
        // Setting the same masks back to zero clears them without touching other bits.
        let cleared = set_flag(v, 0, chapter::UNREAD_MASK | chapter::DISPLAY_MASK);
        assert_eq!(cleared, 0x1 | 0x8 | 0x20 | 0x100);
    }

    #[test]
    fn chapter_masks_are_disjoint_from_their_flags() {
        assert_eq!(chapter::SHOW_UNREAD & !chapter::UNREAD_MASK, 0);
        assert_eq!(chapter::SHOW_NOT_DOWNLOADED & !chapter::DOWNLOADED_MASK, 0);
        assert_eq!(chapter::SHOW_NOT_BOOKMARKED & !chapter::BOOKMARKED_MASK, 0);
        assert_eq!(chapter::SORTING_ALPHABET & !chapter::SORTING_MASK, 0);
    }

    #[test]
    fn library_sort_round_trips_every_type_and_direction() {
        let types = [0, 4, 8, 12, 16, 20, 24, 28, 32, 60];
        assert_eq!(types, [
            library_sort::ALPHABETICAL,
            library_sort::LAST_READ,
            library_sort::LAST_UPDATE,
            library_sort::UNREAD_COUNT,
            library_sort::TOTAL_CHAPTERS,
            library_sort::LATEST_CHAPTER,
            library_sort::CHAPTER_FETCH_DATE,
            library_sort::DATE_ADDED,
            library_sort::TRACKER_MEAN,
            library_sort::RANDOM,
        ]);
        for t in types {
            for asc in [true, false] {
                let v = library_sort::encode(t, asc);
                assert_eq!(library_sort::sort_type(v), t, "type {t}");
                assert_eq!(library_sort::is_ascending(v), asc, "type {t}");
            }
        }
    }

    #[test]
    fn library_sort_default_and_unknown_type() {
        assert_eq!(library_sort::DEFAULT, 0x40);
        assert_eq!(library_sort::sort_type(library_sort::DEFAULT), library_sort::ALPHABETICAL);
        // Bits 3..5 set without a matching type (0b111000) fall back to Alphabetical.
        assert_eq!(library_sort::sort_type(0b0011_1000), library_sort::ALPHABETICAL);
        assert!(!library_sort::is_ascending(0));
    }

    #[test]
    fn viewer_lookups_mask_first_and_fall_back_to_default() {
        assert_eq!(viewer::reading_mode(0), viewer::MODE_DEFAULT);
        assert_eq!(viewer::reading_mode(4), viewer::MODE_WEBTOON);
        assert_eq!(viewer::reading_mode(0x18 | 5), viewer::MODE_CONTINUOUS_VERTICAL);
        assert_eq!(viewer::reading_mode(6), viewer::MODE_DEFAULT);
        assert_eq!(viewer::reading_mode(7), viewer::MODE_DEFAULT);
        assert_eq!(viewer::orientation(0x18 | 1), viewer::ORIENTATION_LANDSCAPE);
        assert_eq!(viewer::orientation(0x30), viewer::ORIENTATION_REVERSE_PORTRAIT);
        assert_eq!(viewer::orientation(0x38), viewer::ORIENTATION_DEFAULT);
    }

    #[test]
    fn with_sort_replaces_only_the_sort_bits() {
        let flags = library_sort::encode(library_sort::RANDOM, false) | 0b1000_0000_0000 | 0b1;
        let updated = library_sort::with_sort(flags, library_sort::LAST_READ, true);
        assert_eq!(updated, library_sort::encode(library_sort::LAST_READ, true) | 0b1000_0000_0000 | 0b1);
        assert_eq!(library_sort::with_sort(0, library_sort::RANDOM, true), library_sort::RANDOM | library_sort::ASCENDING);
    }

    #[test]
    fn serialized_sorts_round_trip_and_fall_back_to_the_default() {
        for (name, t) in library_sort::NAMES {
            for dir in ["ASCENDING", "DESCENDING"] {
                let s = format!("{name},{dir}");
                assert_eq!(library_sort::to_serialized(library_sort::from_serialized(&s)), s);
                assert_eq!(library_sort::sort_type(library_sort::from_serialized(&s)), t);
            }
        }
        assert_eq!(library_sort::from_serialized(""), library_sort::DEFAULT);
        assert_eq!(library_sort::from_serialized("RANDOM"), library_sort::DEFAULT, "no direction");
        assert_eq!(library_sort::to_serialized(library_sort::from_serialized("NOPE,ASCENDING")), "ALPHABETICAL,ASCENDING");
        assert_eq!(library_sort::to_serialized(library_sort::from_serialized("DATE_ADDED,OTHER")), "DATE_ADDED,DESCENDING");
    }

    #[test]
    fn item_layout_is_the_anime_superset_and_manga_mask_is_its_manga_groups() {
        assert_eq!(item::MANGA_MASK, 0x10_067F);
        assert_eq!(item::SORTING_NUMBER, 0x200);
        assert_eq!(item::SORTING_MASK, 0x600);
        assert_eq!(item::SHOW_UNDONE | item::SHOW_DONE, item::UNDONE_MASK);
        for (flag, mask) in [
            (item::SHOW_NOT_DOWNLOADED, item::DOWNLOADED_MASK),
            (item::SHOW_NOT_BOOKMARKED, item::BOOKMARKED_MASK),
            (item::SHOW_NOT_FILLERMARKED, item::FILLERMARKED_MASK),
            (item::SORTING_ALPHABET, item::SORTING_MASK),
            (item::SHOW_NOT_PREVIEWS, item::PREVIEWS_MASK),
            (item::SHOW_NOT_SUMMARIES, item::SUMMARIES_MASK),
        ] {
            assert_eq!(flag & !mask, 0);
        }
        // every manga-legal chapter_flags value lands inside MANGA_MASK
        for x in 0..0x20_0000_i64 {
            if x & !(0x7F | chapter::SORTING_MASK | chapter::DISPLAY_MASK) != 0 {
                continue;
            }
            assert_eq!(manga_item_flags_to_unified(x) & !item::MANGA_MASK, 0, "{x:#x}");
        }
        assert_eq!(season::SORT_MASK, 0xE000);
        assert_eq!(viewer::anime::INTRO_MASK, 0xFF);
        assert_eq!(viewer::manga::MODE_WEBTOON, 4);
    }
}
