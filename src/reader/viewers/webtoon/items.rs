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

    /// T-44a-079: the webtoon item lists, compared with the golden file.
    #[test]
    fn the_golden_webtoon_item_lists_match() {
        let scenario = |about: &str, force: bool, set: &ChapterSet| {
            let (items, next) = build_webtoon_items(set, force);
            serde_json::json!({ "about": about, "force": force, "items": label(&items), "next_to": next.to.map(|k| k.id) })
        };
        let loaded = |id: i64, number: f64, pages: u32| snap(id, number, LoadState::Loaded, pages);
        let all = ChapterSet { curr: loaded(2, 2.0, 2), prev: Some(loaded(1, 1.0, 3)), next: Some(loaded(3, 3.0, 3)) };
        let waiting = ChapterSet { curr: loaded(2, 2.0, 2), prev: Some(loaded(1, 1.0, 3)), next: Some(snap(3, 3.0, LoadState::Loading, 0)) };
        let alone = ChapterSet { curr: loaded(2, 2.0, 1), prev: None, next: None };
        let golden = serde_json::json!({
            "loaded_neighbours_forced": scenario("both neighbours loaded, transitions forced", true, &all),
            "loaded_neighbours_plain": scenario("both neighbours loaded, no gap, always-show off", false, &all),
            "next_still_loading": scenario("the next chapter is loading: its gate stays", false, &waiting),
            "end_of_book": scenario("one chapter", false, &alone),
        });
        crate::reader::golden::check("webtoon_items.json", &golden);
    }
}
