//! The active category index, the random-entry pick and the "continue" target (behaviour 2.8). Pure; the UI passes
//! the displayed list and an RNG.

use rand::Rng;
use rand::seq::IndexedRandom;

/// Clamps the persisted page index to the displayed categories. With no pages the index is 0.
pub fn clamp_page(index: i64, pages: usize) -> usize {
    if pages == 0 { 0 } else { index.clamp(0, pages as i64 - 1) as usize }
}

/// "Open random entry": one uniformly chosen item of the current category, `None` when it is empty.
pub fn random_item<'a, T>(items: &'a [T], rng: &mut (impl Rng + ?Sized)) -> Option<&'a T> {
    items.choose(rng)
}

/// The continue button's entry: the first pending item of the list, or the last one when the entry is sorted
/// descending (`findLast` in `ChapterGetNextUnread`). `list` is in the entry's own sort order.
pub fn continue_target<T>(list: &[T], descending: bool, is_pending: impl Fn(&T) -> bool) -> Option<&T> {
    if descending { list.iter().rev().find(|item| is_pending(item)) } else { list.iter().find(|item| is_pending(item)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::ChaCha8Rng;

    #[test]
    fn the_index_is_clamped_and_zero_for_no_pages() {
        assert_eq!(clamp_page(5, 3), 2);
        assert_eq!(clamp_page(-1, 3), 0);
        assert_eq!(clamp_page(7, 0), 0);
        assert_eq!(clamp_page(1, 3), 1);
    }

    #[test]
    fn random_item_is_none_for_an_empty_category_and_a_member_otherwise() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let empty: Vec<i32> = vec![];
        assert_eq!(random_item(&empty, &mut rng), None);
        let items = [1, 2, 3];
        assert!(items.contains(random_item(&items, &mut rng).unwrap()));
    }

    #[test]
    fn continue_takes_the_first_pending_unless_sorted_descending() {
        let list = [(1, true), (2, false), (3, false), (4, true)];
        let pending = |x: &(i32, bool)| !x.1;
        assert_eq!(continue_target(&list, false, pending), Some(&(2, false)));
        assert_eq!(continue_target(&list, true, pending), Some(&(3, false)));
        let done = [(1, true)];
        assert_eq!(continue_target(&done, false, pending), None);
    }
}
