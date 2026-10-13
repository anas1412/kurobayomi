//! Bit layouts of `entries.item_flags` (`item`), `entries.season_flags` (`season`) and `entries.viewer` (`viewer`). The
//! sort of `categories.flags` is `kuro_platform::sort`; the Aniyomi manga `chapter_flags` wire layout is translated at
//! the backup edge only (`kuro_backup::flags`). Values are copied from Aniyomi `Manga.kt`, `ReadingMode.kt` and
//! `ReaderOrientation.kt`. Every mask is applied before comparing, so stray high bits are ignored.

/// `(value & !mask) | (flag & mask)`: `FlagWithMask.plus` and `SetMangaChapterFlags.setFlag`.
pub const fn set_flag(value: i64, flag: i64, mask: i64) -> i64 {
    (value & !mask) | (flag & mask)
}

/// `entries.item_flags` for BOTH kinds: the anime superset layout (docs/design/unified-model.md section 0, item 5).
/// A manga row only ever carries the groups of [`MANGA_MASK`]. The Aniyomi manga `chapter_flags` layout differs only
/// in the two sorting bits (0x300 there, 0x600 here).
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
    fn set_flag_follows_the_kotlin_formula() {
        assert_eq!(set_flag(0b1111, 0b0000, 0b0110), 0b1001);
        assert_eq!(set_flag(0, 0b0100, 0b0110), 0b0100);
        assert_eq!(set_flag(0b0100, 0, 0b0110), 0);
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
        assert_eq!(season::SORT_MASK, 0xE000);
        assert_eq!(viewer::anime::INTRO_MASK, 0xFF);
        assert_eq!(viewer::manga::MODE_WEBTOON, 4);
    }
}
