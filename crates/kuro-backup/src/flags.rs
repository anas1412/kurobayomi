//! The one place the manga `chapterFlags` wire layout meets the stored item-flag layout (`kuro_db::flags::item`). The
//! wire keeps the Aniyomi values: the two sorting bits sit at 0x300 there and at 0x600 in the stored word. Anime
//! `episodeFlags` already is the stored word and passes through unchanged.

use kuro_db::flags::{item, manga_item_flags_from_unified, manga_item_flags_to_unified};

/// The manga `chapterFlags` groups: sort direction, done and bookmark filters, sorting, display.
pub const MANGA_WIRE_MASK: i64 = 0x1 | 0x6 | 0x18 | 0x60 | 0x300 | 0x100000;

/// The stored `item_flags` of a restored manga: the wire word masked to its groups, sorting moved up one place.
pub const fn chapter_flags_from_wire(x: i64) -> i64 {
    manga_item_flags_to_unified(x & MANGA_WIRE_MASK)
}

/// The wire `chapterFlags` of a stored manga `item_flags`: groups a manga never carries are dropped.
pub const fn chapter_flags_to_wire(item_flags: i64) -> i64 {
    manga_item_flags_from_unified(item_flags & item::MANGA_MASK)
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
    fn a_wire_word_round_trips_and_stray_bits_are_dropped() {
        for x in [0, 1, 0x6, 0x18, 0x60, 0x100, 0x200, 0x300, 0x100001, MANGA_WIRE_MASK] {
            assert_eq!(chapter_flags_to_wire(chapter_flags_from_wire(x)), x, "{x:#x}");
        }
        assert_eq!(chapter_flags_from_wire(0x80 | 0x4000), 0, "bits outside the manga groups are not stored");
        assert_eq!(chapter_flags_to_wire(0x80 | 0x1), 0x1, "anime-only stored bits are not written to a manga");
    }
}
