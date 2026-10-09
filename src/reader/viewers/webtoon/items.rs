//! Webtoon item list (T-44a-031, B§5.5): the shared flow with no insert pages and no reversal.

use crate::reader::viewers::items::build_flow;
use crate::reader::viewers::model::{ChapterSet, ItemKey, Transition};

/// Builds the webtoon list. Returns the rows and the `Next` transition (the preload destination).
pub fn build_webtoon_items(set: &ChapterSet, force: bool) -> (Vec<ItemKey>, Transition) {
    build_flow(set, force, |pages| pages.into_iter().map(ItemKey::Page).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::viewers::items::tests::{label, snap};
    use crate::reader::viewers::model::LoadState;

    #[test]
    fn webtoon_lists_have_the_pager_gating_and_no_inserts() {
        let set = ChapterSet {
            curr: snap(2, 2.0, LoadState::Loaded, 2),
            prev: Some(snap(1, 1.0, LoadState::Loaded, 3)),
            next: Some(snap(3, 3.0, LoadState::Loading, 0)),
        };
        let (items, next) = build_webtoon_items(&set, false);
        // The Prev gate is off (no gap, loaded neighbour); the Next gate is on (not loaded).
        assert_eq!(label(&items), ["p1:1", "p1:2", "p2:0", "p2:1", "N(2,3)"]);
        assert_eq!(next.to.map(|k| k.id), Some(3));
    }

    #[test]
    fn a_forced_list_keeps_both_transitions() {
        let set = ChapterSet { curr: snap(2, 2.0, LoadState::Loaded, 1), prev: None, next: None };
        let (items, _) = build_webtoon_items(&set, true);
        assert_eq!(label(&items), ["P(2,-)", "p2:0", "N(2,-)"]);
    }
}
