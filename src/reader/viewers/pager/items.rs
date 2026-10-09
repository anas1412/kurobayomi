//! Pager item list (T-44a-021, T-44a-022, T-44a-030): the shared flow of `items.rs`, the preprocessed dual-page
//! inserts of the current chapter (B§5.3 step 5, B§5.6) and the right-to-left reversal (B§5.3 step 10).

use std::collections::BTreeMap;

use crate::reader::viewers::ViewerKind;
use crate::reader::viewers::items::build_flow;
use crate::reader::viewers::model::{ChapterSet, ItemKey, PageRef, Transition};

/// The result of one build: the rows, the item to jump to when the last page had a split, and the `Next`
/// transition.
pub struct PagerList {
    pub items: Vec<ItemKey>,
    pub jump: Option<ItemKey>,
    pub next: Transition,
}

/// Builds the pager list (B§5.3). `preprocessed` holds the split pages of neighbour chapters, keyed by parent index;
/// it is consumed here (cleared whatever the current chapter is), as `preprocessed = {}` does upstream.
pub fn build_pager_items(set: &ChapterSet, force: bool, preprocessed: &mut BTreeMap<u32, PageRef>, kind: ViewerKind) -> PagerList {
    let pending = std::mem::take(preprocessed);
    let mut jump = None;
    let (mut items, next) = build_flow(set, force, |pages| {
        let Some(last_index) = pages.last().map(|p| p.index) else { return Vec::new() };
        let mut list = pages;
        for (&key, insert) in pending.iter().rev() {
            if key == last_index {
                jump = Some(ItemKey::Page(insert.clone()));
            }
            // A split for a page past the end cannot be placed; upstream would throw here.
            if (key as usize) < list.len() {
                list.insert(key as usize + 1, insert.clone());
            }
        }
        list.into_iter().map(ItemKey::Page).collect()
    });
    if kind == ViewerKind::RightToLeft {
        items.reverse();
    }
    PagerList { items, jump, next }
}

/// Places a dual-page split next to the page at `current` (B§5.6). Returns `false` when the insert would repeat
/// (R2L guard), would sit before an existing insert, or has no slot.
pub fn place_insert(items: &mut Vec<ItemKey>, current: usize, kind: ViewerKind, insert: PageRef) -> bool {
    let place = if kind == ViewerKind::RightToLeft { current } else { current + 1 };
    if kind == ViewerKind::RightToLeft && place >= 1 && is_insert(items.get(place - 1)) {
        return false;
    }
    if place >= items.len() || is_insert(items.get(place)) {
        return false;
    }
    items.insert(place, ItemKey::Page(insert));
    true
}

fn is_insert(item: Option<&ItemKey>) -> bool {
    matches!(item, Some(ItemKey::Page(p)) if p.is_insert())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::viewers::items::tests::{label, snap};
    use crate::reader::viewers::model::{ChapterSnap, LoadState, PageKind};

    fn set(curr: ChapterSnap, prev: Option<ChapterSnap>, next: Option<ChapterSnap>) -> ChapterSet {
        ChapterSet { curr, prev, next }
    }

    fn loaded(id: i64, number: f64, pages: u32) -> ChapterSnap {
        snap(id, number, LoadState::Loaded, pages)
    }

    #[test]
    fn default_settings_give_the_observed_golden_list() {
        // Prev tail, Prev transition, current pages, Next transition, Next head (B§5.3).
        let s = set(loaded(2, 2.0, 3), Some(loaded(1, 1.0, 5)), Some(loaded(3, 3.0, 4)));
        let list = build_pager_items(&s, true, &mut BTreeMap::new(), ViewerKind::LeftToRight);
        assert_eq!(label(&list.items), ["p1:3", "p1:4", "P(2,1)", "p2:0", "p2:1", "p2:2", "N(2,3)", "p3:0", "p3:1"]);
        assert!(list.jump.is_none());
        assert_eq!(list.next.to.as_ref().map(|k| k.id), Some(3));
    }

    #[test]
    fn the_end_of_the_book_keeps_a_terminal_next_transition() {
        let s = set(loaded(2, 2.0, 2), Some(loaded(1, 1.0, 2)), None);
        let list = build_pager_items(&s, true, &mut BTreeMap::new(), ViewerKind::LeftToRight);
        assert_eq!(label(&list.items).last().map(String::as_str), Some("N(2,-)"));
    }

    #[test]
    fn right_to_left_reverses_the_whole_list() {
        let s = set(loaded(2, 2.0, 2), None, Some(loaded(3, 3.0, 2)));
        let ltr = build_pager_items(&s, true, &mut BTreeMap::new(), ViewerKind::LeftToRight);
        let rtl = build_pager_items(&s, true, &mut BTreeMap::new(), ViewerKind::RightToLeft);
        let mut back = rtl.items.clone();
        back.reverse();
        assert_eq!(back, ltr.items);
        assert_eq!(label(&rtl.items)[0], "p3:1");
    }

    #[test]
    fn an_empty_loaded_chapter_does_not_panic_and_still_has_transitions() {
        let s = set(loaded(2, 2.0, 0), None, None);
        let mut pre = BTreeMap::new();
        pre.insert(0, PageRef::insert_of(&loaded(2, 2.0, 1).page(0)));
        let list = build_pager_items(&s, true, &mut pre, ViewerKind::LeftToRight);
        assert_eq!(label(&list.items), ["P(2,-)", "N(2,-)"]);
        assert!(pre.is_empty());
    }

    #[test]
    fn a_split_of_the_last_page_places_the_insert_after_it_and_asks_for_a_jump() {
        let curr = loaded(2, 2.0, 3);
        let s = set(curr.clone(), None, Some(loaded(3, 3.0, 1)));
        let mut pre = BTreeMap::new();
        pre.insert(2, PageRef::insert_of(&curr.page(2)));
        let list = build_pager_items(&s, false, &mut pre, ViewerKind::LeftToRight);
        assert_eq!(label(&list.items), ["P(2,-)", "p2:0", "p2:1", "p2:2", "i2:2", "p3:0"]);
        assert!(matches!(&list.jump, Some(ItemKey::Page(p)) if p.kind == PageKind::Insert { parent_index: 2 }));
    }

    #[test]
    fn a_split_past_the_end_is_dropped_instead_of_panicking() {
        let curr = loaded(2, 2.0, 2);
        let mut pre = BTreeMap::new();
        pre.insert(7, PageRef::insert_of(&curr.page(7)));
        let list = build_pager_items(&set(curr, None, None), true, &mut pre, ViewerKind::LeftToRight);
        assert_eq!(label(&list.items), ["P(2,-)", "p2:0", "p2:1", "N(2,-)"]);
    }

    #[test]
    fn insert_placement_follows_the_direction() {
        let base = vec![ItemKey::Page(loaded(2, 2.0, 3).page(0)), ItemKey::Page(loaded(2, 2.0, 3).page(1))];
        let insert = PageRef::insert_of(&loaded(2, 2.0, 3).page(0));
        let mut ltr = base.clone();
        assert!(place_insert(&mut ltr, 0, ViewerKind::LeftToRight, insert.clone()));
        assert_eq!(label(&ltr), ["p2:0", "i2:0", "p2:1"]);
        let mut rtl = base.clone();
        assert!(place_insert(&mut rtl, 1, ViewerKind::RightToLeft, insert.clone()));
        assert_eq!(label(&rtl), ["p2:0", "i2:0", "p2:1"]);
    }

    #[test]
    fn right_to_left_refuses_an_insert_right_after_another_insert() {
        let curr = loaded(2, 2.0, 3);
        let insert = PageRef::insert_of(&curr.page(1));
        let mut items = vec![ItemKey::Page(curr.page(0)), ItemKey::Page(insert.clone()), ItemKey::Page(curr.page(1))];
        assert!(!place_insert(&mut items, 2, ViewerKind::RightToLeft, insert));
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn no_insert_goes_next_to_an_existing_insert_or_past_the_end() {
        let curr = loaded(2, 2.0, 2);
        let insert = PageRef::insert_of(&curr.page(0));
        let mut items = vec![ItemKey::Page(curr.page(0)), ItemKey::Page(insert.clone())];
        assert!(!place_insert(&mut items, 0, ViewerKind::LeftToRight, insert.clone()));
        let mut at_end = vec![ItemKey::Page(curr.page(0))];
        assert!(!place_insert(&mut at_end, 0, ViewerKind::LeftToRight, insert));
        assert_eq!(at_end.len(), 1);
    }
}
