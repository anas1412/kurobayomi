//! The one place the manga `chapterFlags` wire layout meets the stored item-flag layout (`kuro_db::flags::item`). The
//! wire keeps the Aniyomi values: the two sorting bits sit at 0x300 there and at 0x600 in the stored word. Anime
//! `episodeFlags` already is the stored word and passes through unchanged.

use kuro_db::flags::item;

/// The manga `chapterFlags` groups: sort direction, done and bookmark filters, sorting, display.
pub const MANGA_WIRE_MASK: i64 = 0x1 | 0x6 | 0x18 | 0x60 | 0x300 | 0x100000;

/// The stored `item_flags` of a restored manga: the wire word masked to its groups, sorting moved up one place.
pub const fn chapter_flags_from_wire(x: i64) -> i64 {
    let x = x & MANGA_WIRE_MASK;
    (x & !0x300) | ((x & 0x300) << 1)
}

/// The wire `chapterFlags` of a stored manga `item_flags`: groups a manga never carries are dropped.
pub const fn chapter_flags_to_wire(item_flags: i64) -> i64 {
    let x = item_flags & item::MANGA_MASK;
    (x & !0x600) | ((x & 0x600) >> 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorting_moves_between_the_layouts() {
        assert_eq!(chapter_flags_from_wire(0x100), 0x200);
        assert_eq!(chapter_flags_from_wire(0x300), 0x600);
        assert_eq!(chapter_flags_to_wire(0x600), 0x300);
        assert_eq!(chapter_flags_from_wire(0x100000 | 1), 0x100001);
    }

    #[test]
    fn every_manga_legal_wire_value_lands_inside_the_manga_groups_and_the_wire_groups_compose() {
        use kuro_db::flags::set_flag;
        for x in 0..0x20_0000_i64 {
            if x & !(0x7F | 0x300 | 0x10_0000) != 0 {
                continue;
            }
            assert_eq!(chapter_flags_from_wire(x) & !item::MANGA_MASK, 0, "{x:#x}");
        }
        // SetMangaChapterFlags.awaitSetAllFlags: the six groups from 0 in wire values, then stored.
        let mut wire = 0;
        for (flag, mask) in [(0x1, 0x1), (0x4, 0x6), (0x8, 0x18), (0x20, 0x60), (0x100, 0x300), (0x10_0000, 0x10_0000)] {
            wire = set_flag(wire, flag, mask);
        }
        assert_eq!(wire, 0x1 | 0x4 | 0x8 | 0x20 | 0x100 | 0x10_0000);
        assert_eq!(chapter_flags_from_wire(wire), 0x1 | 0x4 | 0x8 | 0x20 | 0x200 | 0x10_0000);
    }

    #[test]
    fn a_wire_word_round_trips_and_stray_bits_are_dropped() {
        for x in [0, 1, 0x6, 0x18, 0x60, 0x100, 0x200, 0x300, 0x100001, MANGA_WIRE_MASK] {
            assert_eq!(chapter_flags_to_wire(chapter_flags_from_wire(x)), x, "{x:#x}");
        }
        assert_eq!(chapter_flags_from_wire(0x80 | 0x4000), 0, "bits outside the manga groups are not stored");
        assert_eq!(chapter_flags_to_wire(0x80 | 0x1), 0x1, "anime-only stored bits are not written to a manga");
    }
}
