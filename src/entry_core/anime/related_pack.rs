//! Related-anime rows (T-39a-030, T-39a-031): how the relation groups are packed and how one related item looks. Upstream:
//! `RelatedAnimeRows.kt` (`packRows`, the small and multi split, `RelatedAnimeItem`). Groups of at most three items share
//! 3-slot rows; larger groups get their own horizontal scroller. No GTK here: the thin view is `related_view`.

use kuro_prefs::library::LibraryDisplayMode;

/// Slots in one packed row, and the largest group that is packed (`MAX_ITEMS_PER_ROW`).
pub const MAX_ITEMS_PER_ROW: usize = 3;

/// Width of one related item in a horizontal scroller, and the widest a packed item may grow (`RELATED_ITEM_WIDTH`).
pub const RELATED_ITEM_WIDTH: i32 = 96;

/// Cover alpha of a related item that is already in the library (`BrowseFavoriteCoverAlpha`).
pub const FAVORITE_COVER_ALPHA: f64 = 0.34;

/// Which section a group goes to: `Small` groups are packed, `Multi` groups scroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Small,
    Multi,
}

/// A group of `size` items is small when it fills at most one packed row (E-13: size 3 is small, size 4 is multi).
pub fn section(size: usize) -> Section {
    if size <= MAX_ITEMS_PER_ROW { Section::Small } else { Section::Multi }
}

/// The group indices of the small and of the multi sections, each in the stored order.
pub fn partition(sizes: &[usize]) -> (Vec<usize>, Vec<usize>) {
    let mut small = Vec::new();
    let mut multi = Vec::new();
    for (index, &size) in sizes.iter().enumerate() {
        match section(size) {
            Section::Small => small.push(index),
            Section::Multi => multi.push(index),
        }
    }
    (small, multi)
}

/// `packRows`: the group indices of each packed row. The count of rows can exceed `ceil(total / max)` (D-12, kept as a
/// parity quirk). Empty input, or groups with no items at all, give no rows (the Kotlin would divide by zero).
pub fn pack_rows(sizes: &[usize], max_per_row: usize) -> Vec<Vec<usize>> {
    let total: usize = sizes.iter().sum();
    if total == 0 || max_per_row == 0 {
        return Vec::new();
    }
    let row_count = total.div_ceil(max_per_row);
    let base = total / row_count;
    let remainder = total % row_count;

    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut filled = 0;
    let mut cap = base + usize::from(remainder > 0);
    for (index, &size) in sizes.iter().enumerate() {
        if !current.is_empty() && filled + size > cap {
            rows.push(std::mem::take(&mut current));
            filled = 0;
            cap = base + usize::from(rows.len() < remainder);
            if cap < size {
                cap = max_per_row;
            }
        }
        current.push(index);
        filled += size;
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows
}

/// Where a group sits in its packed row: its first slot, and whether it is aligned to the start, the end or the middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

/// The first slot of each group in a row, from the group sizes (the running fold of the Kotlin).
pub fn start_slots(sizes: &[usize]) -> Vec<usize> {
    let mut acc = 0;
    sizes
        .iter()
        .map(|size| {
            let start = acc;
            acc += size;
            start
        })
        .collect()
}

/// The alignment of a group that starts at `start` and spans `size` slots: the first group starts, a group that ends
/// on the last slot ends, and any other is centred.
pub fn align(start: usize, size: usize) -> Align {
    let end = (start + size).saturating_sub(1);
    if start == 0 {
        Align::Start
    } else if end == MAX_ITEMS_PER_ROW - 1 {
        Align::End
    } else {
        Align::Center
    }
}

/// How one related item is drawn (`RelatedAnimeItem`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemStyle {
    pub show_title: bool,
    /// The title line limit, when the title is shown.
    pub title_max_lines: Option<u32>,
    pub cover_alpha: f64,
    /// The in-library badge shows on the cover, lit when the item is in the library.
    pub in_library_badge: bool,
}

/// Comfortable and list modes show a three-line title, compact shows a title, cover-only shows none. An item in the
/// library dims its cover to 0.34 and carries the in-library badge.
pub fn item_style(mode: LibraryDisplayMode, in_library: bool) -> ItemStyle {
    let (show_title, title_max_lines) = match mode {
        LibraryDisplayMode::ComfortableGrid | LibraryDisplayMode::List => (true, Some(3)),
        LibraryDisplayMode::CompactGrid => (true, None),
        LibraryDisplayMode::CoverOnlyGrid => (false, None),
    };
    ItemStyle {
        show_title,
        title_max_lines,
        cover_alpha: if in_library { FAVORITE_COVER_ALPHA } else { 1.0 },
        in_library_badge: in_library,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kotlin_quirk_gives_three_rows_for_2_2_1() {
        // T-R1 (D-12): total 5, two rows by count, but the overflow closes a third row
        assert_eq!(pack_rows(&[2, 2, 1], 3), vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn empty_and_zero_size_input_packs_to_nothing() {
        // T-R2: the guard replaces the Kotlin division by zero
        assert!(pack_rows(&[], 3).is_empty());
        assert!(pack_rows(&[0], 3).is_empty());
    }

    #[test]
    fn four_singles_fill_two_rows_of_two() {
        // T-R5
        assert_eq!(pack_rows(&[1, 1, 1, 1], 2), vec![vec![0, 1], vec![2, 3]]);
    }

    #[test]
    fn size_three_is_small_and_size_four_is_multi() {
        // T-R4
        assert_eq!(section(3), Section::Small);
        assert_eq!(section(4), Section::Multi);
        assert_eq!(partition(&[3, 4, 1]), (vec![0, 2], vec![1]));
    }

    #[test]
    fn groups_that_fit_one_row_share_it_and_the_row_fills_to_three() {
        assert_eq!(pack_rows(&[1, 2], 3), vec![vec![0, 1]]);
        assert_eq!(pack_rows(&[3], 3), vec![vec![0]]);
    }

    #[test]
    fn slot_starts_and_alignment_follow_the_kotlin_rules() {
        assert_eq!(start_slots(&[1, 2]), vec![0, 1]);
        assert_eq!(align(0, 1), Align::Start);
        assert_eq!(align(2, 1), Align::End);
        assert_eq!(align(1, 1), Align::Center);
        assert_eq!(align(0, 3), Align::Start);
    }

    #[test]
    fn display_modes_pick_title_and_badges() {
        let comfy = item_style(LibraryDisplayMode::ComfortableGrid, false);
        assert!(comfy.show_title);
        assert_eq!(comfy.title_max_lines, Some(3));
        assert_eq!(item_style(LibraryDisplayMode::List, false), comfy);
        let compact = item_style(LibraryDisplayMode::CompactGrid, false);
        assert_eq!((compact.show_title, compact.title_max_lines), (true, None));
        let cover = item_style(LibraryDisplayMode::CoverOnlyGrid, false);
        assert!(!cover.show_title);
    }

    #[test]
    fn a_library_item_is_dimmed_and_badged() {
        let fav = item_style(LibraryDisplayMode::CompactGrid, true);
        assert_eq!(fav.cover_alpha, 0.34);
        assert!(fav.in_library_badge);
        let other = item_style(LibraryDisplayMode::CompactGrid, false);
        assert_eq!(other.cover_alpha, 1.0);
        assert!(!other.in_library_badge);
    }
}
