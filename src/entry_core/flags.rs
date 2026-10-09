//! Chapter flag word (`Manga.chapterFlags`) for the details page: typed reads, masked writes and the
//! filter setters. Bit layout comes from `kuro_db::flags::chapter`; TriState rules from `kuro_domain::flags`.
//! Upstream: `Manga.kt:37-103`, `SetMangaChapterFlags.kt:11-70`, `TriState.kt`.

use kuro_db::flags::chapter as c;
use kuro_db::flags::set_flag;
use kuro_domain::flags::tri;
use kuro_domain::items::sorter::{SortKey, chapter_key};
use kuro_prefs::tristate::TriState;

/// A masked write: `new = (old & !mask) | (flag & mask)`.
pub type Write = (i64, i64);

/// The chapter flag word of one manga.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChapterFlags(pub i64);

impl ChapterFlags {
    /// `(self & !mask) | (flag & mask)`. Bits outside `mask` never change.
    pub fn with_masked(self, flag: i64, mask: i64) -> Self {
        Self(set_flag(self.0, flag, mask))
    }

    /// The sort key (`sorting`). The 2-bit field always maps to one of the four keys.
    pub fn sort_key(self) -> SortKey {
        chapter_key(self.0 & c::SORTING_MASK)
    }

    /// `sortDescending()`: true when the direction bit is 0.
    pub fn sort_descending(self) -> bool {
        self.0 & c::SORT_DIR_MASK == c::SORT_DESC
    }

    /// `displayMode`: true for the chapter-number display, false for the name display.
    pub fn display_number(self) -> bool {
        self.0 & c::DISPLAY_MASK == c::DISPLAY_NUMBER
    }

    /// `unreadFilter`: raw 0x2 is `ENABLED_IS`, raw 0x4 is `ENABLED_NOT`, anything else is `DISABLED`.
    pub fn unread(self) -> TriState {
        tri(self.0 & c::UNREAD_MASK, c::SHOW_UNREAD, c::SHOW_READ)
    }

    /// `bookmarkedFilter`.
    pub fn bookmarked(self) -> TriState {
        tri(
            self.0 & c::BOOKMARKED_MASK,
            c::SHOW_BOOKMARKED,
            c::SHOW_NOT_BOOKMARKED,
        )
    }

    /// `downloadedFilter`. The app-wide downloaded-only preference forces `ENABLED_IS` without touching the bits.
    pub fn downloaded(self, downloaded_only: bool) -> TriState {
        if downloaded_only {
            TriState::ENABLED_IS
        } else {
            tri(
                self.0 & c::DOWNLOADED_MASK,
                c::SHOW_DOWNLOADED,
                c::SHOW_NOT_DOWNLOADED,
            )
        }
    }

    /// `chaptersFiltered()`: any of the three filters is not `DISABLED` (the forced downloaded state counts).
    pub fn filter_active(self, downloaded_only: bool) -> bool {
        self.unread() != TriState::DISABLED
            || self.downloaded(downloaded_only) != TriState::DISABLED
            || self.bookmarked() != TriState::DISABLED
    }

    /// The masked writes for "sort by `mode`" (`awaitSetSortingModeOrFlipOrder`). Same key: flip the direction.
    /// New key: set the key and make it ascending (two writes, applied in order).
    pub fn sort_writes(self, mode: i64) -> Vec<Write> {
        if self.0 & c::SORTING_MASK == mode {
            let dir = if self.sort_descending() {
                c::SORT_ASC
            } else {
                c::SORT_DESC
            };
            vec![(dir, c::SORT_DIR_MASK)]
        } else {
            vec![(mode, c::SORTING_MASK), (c::SORT_ASC, c::SORT_DIR_MASK)]
        }
    }
}

/// Masked write for a two-bit filter group: `DISABLED` clears the group, the other states set `is` / `not`.
pub fn filter_write(state: TriState, is: i64, not: i64, mask: i64) -> Write {
    let flag = match state {
        TriState::DISABLED => 0,
        TriState::ENABLED_IS => is,
        TriState::ENABLED_NOT => not,
    };
    (flag, mask)
}

/// `setUnreadFilter` (MSM:878-925).
pub fn unread_write(state: TriState) -> Write {
    filter_write(state, c::SHOW_UNREAD, c::SHOW_READ, c::UNREAD_MASK)
}

/// `setDownloadedFilter`.
pub fn downloaded_write(state: TriState) -> Write {
    filter_write(
        state,
        c::SHOW_DOWNLOADED,
        c::SHOW_NOT_DOWNLOADED,
        c::DOWNLOADED_MASK,
    )
}

/// `setBookmarkedFilter`.
pub fn bookmarked_write(state: TriState) -> Write {
    filter_write(
        state,
        c::SHOW_BOOKMARKED,
        c::SHOW_NOT_BOOKMARKED,
        c::BOOKMARKED_MASK,
    )
}

/// `scanlatorFilterActive`: some excluded scanlator is also one the chapters have (E-43). A stale excluded name, one
/// no chapter carries, does not tint the filter row.
pub fn scanlator_filter_active(available: &[String], excluded: &std::collections::BTreeSet<String>) -> bool {
    available.iter().any(|name| excluded.contains(name))
}

/// "Set as default": stores the groups of `flags` as the library's default chapter flags, and returns the text of the
/// "Updated default chapter settings" message (`ChapterSettingsDialog`, `SetMangaDefaultChapterFlags`).
pub fn save_as_default(store: &kuro_prefs::store::PrefStore, flags: ChapterFlags) -> String {
    kuro_domain::entries::defaults::set_chapter_settings_default(store, flags.0);
    kuro_i18n::tr(kuro_i18n::MsgKey::chapter_settings_updated)
}

/// `setDisplayMode` (MSM:931-949).
pub fn display_write(number: bool) -> Write {
    (
        if number {
            c::DISPLAY_NUMBER
        } else {
            c::DISPLAY_NAME
        },
        c::DISPLAY_MASK,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_as_default_stores_the_flags_and_says_so() {
        let store = kuro_prefs::store::PrefStore::in_memory();
        let flags = ChapterFlags(c::SORTING_NUMBER | c::SORT_ASC | c::SHOW_UNREAD | c::DISPLAY_NUMBER);
        assert_eq!(save_as_default(&store, flags), "Updated default chapter settings");
        let stored = ChapterFlags(kuro_domain::entries::defaults::chapter_default_word(&store));
        assert_eq!(stored, flags, "a new manga opens with what was saved");
    }

    #[test]
    fn the_scanlator_filter_is_active_on_an_intersection_only() {
        let available = vec!["A".to_owned(), "B".to_owned()];
        let set = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect::<std::collections::BTreeSet<_>>();
        assert!(scanlator_filter_active(&available, &set(&["B", "Gone"])));
        assert!(!scanlator_filter_active(&available, &set(&["Gone"])), "a stale name is kept but does not count");
        assert!(!scanlator_filter_active(&available, &set(&[])));
        assert!(!scanlator_filter_active(&[], &set(&["A"])));
    }

    #[test]
    fn flag_masks_are_disjoint() {
        let masks = [
            c::SORT_DIR_MASK,
            c::UNREAD_MASK,
            c::DOWNLOADED_MASK,
            c::BOOKMARKED_MASK,
            c::SORTING_MASK,
            c::DISPLAY_MASK,
        ];
        for (i, a) in masks.iter().enumerate() {
            for b in &masks[i + 1..] {
                assert_eq!(a & b, 0, "masks {a:#x} and {b:#x} overlap");
            }
        }
    }

    #[test]
    fn with_masked_matches_the_formula_and_keeps_outside_bits() {
        assert_eq!(ChapterFlags(0x1A).with_masked(0x8, 0x18).0, 0x0A);
        // Bits 10-19 and 21-63 set; the write touches only bits 0-1 and 20.
        let outside = !(0x3 | (1 << 20));
        let high = outside;
        let out = ChapterFlags(high)
            .with_masked(0x1 | (1 << 20), 0x3 | (1 << 20))
            .0;
        assert_eq!(
            out & outside,
            high & outside,
            "bits outside the mask changed"
        );
        assert_eq!(out & (0x3 | (1 << 20)), 0x1 | (1 << 20));
    }

    #[test]
    fn masked_flag_matches_kotlin_setflag() {
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed as i64
        };
        for _ in 0..64 {
            let (old, flag, mask) = (next(), next(), next());
            let expected = (old & !mask) | (flag & mask);
            assert_eq!(ChapterFlags(old).with_masked(flag, mask).0, expected);
        }
    }

    #[test]
    fn default_is_source_descending_no_filters_name_display() {
        let f = ChapterFlags::default();
        assert_eq!(f.sort_key(), SortKey::Source);
        assert!(f.sort_descending());
        assert_eq!(f.unread(), TriState::DISABLED);
        assert_eq!(f.downloaded(false), TriState::DISABLED);
        assert_eq!(f.bookmarked(), TriState::DISABLED);
        assert!(!f.display_number());
        assert!(!f.filter_active(false));
    }

    #[test]
    fn sort_key_and_direction_bits() {
        assert_eq!(ChapterFlags(0x100).sort_key(), SortKey::Number);
        assert_eq!(ChapterFlags(0x200).sort_key(), SortKey::UploadDate);
        assert_eq!(ChapterFlags(0x300).sort_key(), SortKey::Alphabet);
        assert!(!ChapterFlags(0x1).sort_descending());
        assert!(ChapterFlags(0x0).sort_descending());
    }

    #[test]
    fn tri_state_mapping_for_filters() {
        assert_eq!(ChapterFlags(0x2).unread(), TriState::ENABLED_IS);
        assert_eq!(ChapterFlags(0x4).unread(), TriState::ENABLED_NOT);
        assert_eq!(
            ChapterFlags(0x6).unread(),
            TriState::DISABLED,
            "both bits set is invalid"
        );
        assert_eq!(ChapterFlags(0x20).bookmarked(), TriState::ENABLED_IS);
        assert_eq!(ChapterFlags(0x40).bookmarked(), TriState::ENABLED_NOT);
        assert_eq!(ChapterFlags(0x8).downloaded(false), TriState::ENABLED_IS);
        assert_eq!(ChapterFlags(0x10).downloaded(false), TriState::ENABLED_NOT);
    }

    #[test]
    fn downloaded_only_forces_enabled_is_without_changing_bits() {
        let f = ChapterFlags(0x10);
        assert_eq!(f.downloaded(true), TriState::ENABLED_IS);
        assert_eq!(f.0, 0x10);
        assert!(f.filter_active(true));
        assert!(!ChapterFlags(0).filter_active(false));
    }

    #[test]
    fn first_sort_selection_is_ascending_and_same_key_flips() {
        let f = ChapterFlags(0); // source, descending
        assert_eq!(f.sort_writes(0x100), vec![(0x100, 0x300), (0x1, 0x1)]);
        let asc = ChapterFlags(0x1); // source, ascending: same key, so flip to descending
        assert_eq!(asc.sort_writes(0x0), vec![(0x0, 0x1)]);
        let same = ChapterFlags(0x100);
        assert_eq!(
            same.sort_writes(0x100),
            vec![(0x1, 0x1)],
            "number descending flips to ascending"
        );
        let same_asc = ChapterFlags(0x101);
        assert_eq!(
            same_asc.sort_writes(0x100),
            vec![(0x0, 0x1)],
            "number ascending flips to descending"
        );
    }

    #[test]
    fn filter_setters_map_tri_states_to_flags() {
        assert_eq!(unread_write(TriState::DISABLED), (0, c::UNREAD_MASK));
        assert_eq!(
            unread_write(TriState::ENABLED_IS),
            (c::SHOW_UNREAD, c::UNREAD_MASK)
        );
        assert_eq!(
            downloaded_write(TriState::ENABLED_NOT),
            (c::SHOW_NOT_DOWNLOADED, c::DOWNLOADED_MASK)
        );
        assert_eq!(
            bookmarked_write(TriState::ENABLED_NOT),
            (c::SHOW_NOT_BOOKMARKED, c::BOOKMARKED_MASK)
        );
        assert_eq!(display_write(true), (c::DISPLAY_NUMBER, c::DISPLAY_MASK));
        assert_eq!(display_write(false), (c::DISPLAY_NAME, c::DISPLAY_MASK));
    }
}
