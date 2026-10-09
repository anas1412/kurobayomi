//! Port of `LibraryFlagsTest.kt` (T-08a-002 and T-07a-035) plus the flag-algebra properties.

use kuro_domain::kind::EntryKind;
use kuro_domain::library::display_mode::LibraryDisplayMode;
use kuro_domain::library::flags::{add_unmasked, contains, merge};
use kuro_domain::library::sort::{Direction, LibrarySort, SORT_MASK, SortType};

fn sort(ty: SortType, dir: Direction) -> LibrarySort {
    LibrarySort { ty, dir }
}

/// `Long + Flag` for a sort: the mask-aware merge of the sort flag.
fn plus(value: i64, other: LibrarySort) -> i64 {
    merge(value, other.flag(), SORT_MASK)
}

#[test]
fn check_the_amount_of_flags() {
    assert_eq!(LibraryDisplayMode::ALL.len(), 4);
    assert_eq!(SortType::types(EntryKind::Manga).len(), 10);
    assert_eq!(Direction::ALL.len(), 2);
    assert_eq!(SortType::types(EntryKind::Anime).len(), 11);
}

#[test]
fn flag_plus_operator_library_sort() {
    let manga_current = sort(SortType::LastActivity, Direction::Ascending);
    let manga_new = sort(SortType::DateAdded, Direction::Ascending);
    assert_eq!(plus(manga_current.flag(), manga_new), 0b0101_1100);

    let anime_current = sort(SortType::LastActivity, Direction::Ascending);
    let anime_new = sort(SortType::DateAdded, Direction::Ascending);
    assert_eq!(plus(anime_current.flag(), anime_new), 0b0101_1100);
}

#[test]
fn flag_plus_operator() {
    assert_eq!(sort(SortType::DateAdded, Direction::Ascending).flag(), 0b0101_1100);
}

#[test]
fn flag_plus_operator_with_old_flag_as_base() {
    let current = sort(SortType::UnreadCount, Direction::Descending);
    assert_eq!(current.flag(), 0b0000_1100);
    let new = sort(SortType::DateAdded, Direction::Ascending);
    let merged = plus(current.flag(), new);
    assert_eq!(merged, 0b0101_1100);
    assert_ne!(merged, current.flag());

    // Upstream slip, kept as written: the anime half adds the new sort to itself.
    let anime_new = sort(SortType::DateAdded, Direction::Ascending);
    assert_eq!(plus(anime_new.flag(), anime_new), 0b0101_1100);
}

#[test]
fn default_flags() {
    assert_eq!(add_unmasked(SortType::Alphabetical.flag(), Direction::Ascending.flag()), 0b0100_0000);
    assert_eq!(LibrarySort::DEFAULT.flag(), 0b0100_0000);
}

#[test]
fn plain_or_and_mask_aware_merge_differ_on_the_masked_bits() {
    assert_eq!(add_unmasked(0b0000_1100, 0b0101_1100), 0b0101_1100);
    assert_eq!(merge(0b0000_1100, 0b0101_1100, SORT_MASK), 0b0101_1100);
    assert_eq!(merge(0b0100_0100, 0b0000_1100, SORT_MASK), 0b0000_1100, "a masked merge replaces, a plain OR keeps the old bit");
    assert_eq!(add_unmasked(0b0100_0100, 0b0000_1100), 0b0100_1100);
}

#[test]
fn contains_compares_only_the_masked_bits() {
    assert!(contains(0b0101_1100, 0x1C, SortType::types(EntryKind::Manga)[7].flag() | 0x3C));
    assert!(contains(0x5C, 0x40, Direction::Ascending.flag()));
    assert!(!contains(0x1C, 0x40, Direction::Ascending.flag()));
}

/// Deterministic xorshift, so the property runs the same inputs every time without a dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> i64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 as i64
    }
}

#[test]
fn merge_is_idempotent_and_keeps_bits_outside_the_mask_for_random_inputs() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..10_000 {
        let (value, flag, mask) = (rng.next(), rng.next(), rng.next());
        let once = merge(value, flag, mask);
        assert_eq!(merge(once, flag, mask), once, "idempotent for {value:#x} {flag:#x} {mask:#x}");
        assert_eq!(once & !mask, value & !mask, "outside the mask nothing moves");
        assert_eq!(once & mask, flag & mask, "inside the mask the flag wins");
    }
}
