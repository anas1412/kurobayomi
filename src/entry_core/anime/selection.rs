//! Episode selection (T-39a-016, T-39b-016). Upstream: `AnimeScreenModel.kt:1369-1455` (`toggleSelection`,
//! `toggleAllSelection`, `invertSelection`), topic episode list section 5, and the click rules of `AnimeScreen.kt`.
//!
//! Selection is a set of episode ids plus two anchors. The anchors are indexes into the processed view (filtered and
//! sorted), `-1` when unset, as upstream keeps them. `toggle_one` takes that view; `select_all` and `invert` take the
//! full, unfiltered list, so hidden episodes are selected too (E-SEL-6, E-SEL-7). The engine never writes the view
//! back into the full list (T-SEL-6), and it has no GTK in it.

use std::collections::HashSet;
use std::ops::Range;


/// The result of a plain click on a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// Nothing is selected: open the episode.
    Open(i64),
    /// Selection is active: the row was toggled and is now `selected`.
    Toggled { id: i64, selected: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    ids: HashSet<i64>,
    anchors: [i64; 2],
}

impl Default for Selection {
    fn default() -> Self {
        Self { ids: HashSet::new(), anchors: [-1, -1] }
    }
}

impl Selection {
    /// `isAnyEpisodeSelected` over the whole selection.
    pub fn is_active(&self) -> bool {
        !self.ids.is_empty()
    }

    pub fn contains(&self, id: i64) -> bool {
        self.ids.contains(&id)
    }

    /// `selectedEpisodeCount`: how many ids of `list` are selected.
    pub fn count_in(&self, list: &[i64]) -> usize {
        list.iter().filter(|id| self.ids.contains(id)).count()
    }

    /// The two anchors as view indexes, `-1` when unset.
    pub fn anchors(&self) -> [i64; 2] {
        self.anchors
    }

    /// Leaves selection mode: every id goes and the anchors reset (`toggleAllSelection(false)`, E-SEL-5).
    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchors = [-1, -1];
    }

    /// `toggleSelection(item, selected, userSelected, fromLongPress)` over the processed `view`. Returns true when the
    /// selection changed. A request equal to the current state, or an id outside `view`, changes nothing.
    pub fn toggle_one(&mut self, view: &[i64], id: i64, selected: bool, user_selected: bool, from_long_press: bool) -> bool {
        let Some(idx) = view.iter().position(|v| *v == id).map(|i| i as i64) else {
            return false;
        };
        if self.ids.contains(&id) == selected {
            return false;
        }
        // firstSelection is read before the write (section 5.2, observed property 4)
        let first_selection = !view.iter().any(|v| self.ids.contains(v));
        if selected {
            self.ids.insert(id);
        } else {
            self.ids.remove(&id);
        }

        let [mut p0, mut p1] = self.anchors;
        if selected && user_selected && from_long_press {
            if first_selection {
                p0 = idx;
                p1 = idx;
            } else {
                // the exclusive ranges of section 5.2: the anchor moves to the tapped row, the in-between rows are selected
                let range: Range<i64> = if idx < p0 {
                    let r = (idx + 1)..p0;
                    p0 = idx;
                    r
                } else if idx > p1 {
                    let r = (p1 + 1)..idx;
                    p1 = idx;
                    r
                } else {
                    0..0
                };
                for i in range {
                    self.ids.insert(view[i as usize]);
                }
            }
        } else if user_selected && !from_long_press {
            if !selected {
                if idx == p0 {
                    p0 = self.first_selected(view);
                } else if idx == p1 {
                    p1 = self.last_selected(view);
                }
            } else if idx < p0 {
                p0 = idx;
            } else if idx > p1 {
                p1 = idx;
            }
        }
        self.anchors = [p0, p1];
        true
    }

    /// `toggleAllSelection(selected)`: every id of the full list (E-SEL-6). Anchors reset.
    pub fn select_all(&mut self, full: &[i64]) {
        for id in full {
            self.ids.insert(*id);
        }
        self.anchors = [-1, -1];
    }

    /// `invertSelection()`: flips every id of the full list (E-SEL-7). Anchors reset.
    pub fn invert(&mut self, full: &[i64]) {
        for id in full {
            if !self.ids.remove(id) {
                self.ids.insert(*id);
            }
        }
        self.anchors = [-1, -1];
    }

    /// Long press: `onEpisodeSelected(item, !selected, true, true)`.
    pub fn long_press(&mut self, view: &[i64], id: i64) -> bool {
        let selected = !self.contains(id);
        self.toggle_one(view, id, selected, true, true) && selected
    }

    /// Plain click. A selected row, or any click while selection is active, toggles the row; any other click opens
    /// the episode.
    pub fn click(&mut self, view: &[i64], id: i64) -> Click {
        if !(self.contains(id) || self.is_active()) {
            return Click::Open(id);
        }
        let selected = !self.contains(id);
        if self.toggle_one(view, id, selected, true, false) {
            Click::Toggled { id, selected }
        } else {
            Click::Open(id)
        }
    }

    /// A click with desktop modifiers (T-39a-055, desktop additions; upstream has touch only). Shift+click selects the range
    /// from the anchor, as a long press does; Ctrl+click toggles that one row, also when nothing is selected yet. With
    /// neither it is a plain `click`.
    pub fn click_with(&mut self, view: &[i64], id: i64, shift: bool, ctrl: bool) -> Click {
        let selected = !self.contains(id);
        if shift {
            self.long_press(view, id);
            Click::Toggled { id, selected }
        } else if ctrl {
            if self.toggle_one(view, id, selected, true, false) { Click::Toggled { id, selected } } else { Click::Open(id) }
        } else {
            self.click(view, id)
        }
    }

    /// Back or Escape. Selection is cleared first and the key is consumed (`true`). With nothing selected it returns
    /// `false` and the caller navigates up.
    pub fn back(&mut self) -> bool {
        if self.is_active() {
            self.clear();
            true
        } else {
            false
        }
    }

    fn first_selected(&self, view: &[i64]) -> i64 {
        view.iter().position(|v| self.ids.contains(v)).map_or(-1, |i| i as i64)
    }

    fn last_selected(&self, view: &[i64]) -> i64 {
        view.iter().rposition(|v| self.ids.contains(v)).map_or(-1, |i| i as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: [i64; 6] = [10, 20, 30, 40, 50, 60];

    /// Taps the ids in order (no long press): only the second anchor is set when none exists (E-SEL-3).
    fn sel(ids: &[i64]) -> Selection {
        let mut s = Selection::default();
        for id in ids {
            s.toggle_one(&VIEW, *id, true, true, false);
        }
        s
    }

    /// A long press on the first id sets both anchors, then taps extend the second one.
    fn span(ids: &[i64]) -> Selection {
        let mut s = Selection::default();
        s.toggle_one(&VIEW, ids[0], true, true, true);
        for id in &ids[1..] {
            s.toggle_one(&VIEW, *id, true, true, false);
        }
        s
    }

    fn selected_in_view(s: &Selection) -> Vec<i64> {
        VIEW.iter().copied().filter(|id| s.contains(*id)).collect()
    }

    #[test]
    fn first_long_press_sets_both_anchors_on_the_row() {
        let mut s = Selection::default();
        assert!(s.toggle_one(&VIEW, 30, true, true, true));
        assert_eq!(s.anchors(), [2, 2]);
        assert_eq!(selected_in_view(&s), vec![30]);
    }

    #[test]
    fn long_press_below_the_span_selects_the_rows_between() {
        let mut s = span(&[30]);
        assert!(s.toggle_one(&VIEW, 60, true, true, true));
        assert_eq!(selected_in_view(&s), vec![30, 40, 50, 60]);
        assert_eq!(s.anchors(), [2, 5]);
    }

    #[test]
    fn long_press_above_the_span_selects_the_rows_between() {
        let mut s = span(&[40]);
        assert!(s.toggle_one(&VIEW, 10, true, true, true));
        assert_eq!(selected_in_view(&s), vec![10, 20, 30, 40]);
        assert_eq!(s.anchors(), [0, 3]);
    }

    #[test]
    fn long_press_inside_the_span_selects_only_itself() {
        let mut s = sel(&[10, 60]);
        assert!(s.toggle_one(&VIEW, 30, true, true, true));
        assert_eq!(selected_in_view(&s), vec![10, 30, 60]);
        assert_eq!(s.anchors(), [-1, 5], "the tapped row is inside the span, so no anchor moves");
    }

    #[test]
    fn a_plain_tap_moves_an_anchor_but_selects_nothing_between() {
        let mut s = sel(&[20, 30]);
        assert!(s.toggle_one(&VIEW, 50, true, true, false));
        assert_eq!(selected_in_view(&s), vec![20, 30, 50]);
        assert_eq!(s.anchors(), [-1, 4]);
    }

    #[test]
    fn deselecting_the_first_anchor_moves_it_to_the_first_selected_row() {
        let mut s = span(&[20, 30, 40]);
        assert!(s.toggle_one(&VIEW, 20, false, true, false));
        assert_eq!(s.anchors(), [2, 3]);
    }

    #[test]
    fn deselecting_the_last_anchor_moves_it_to_the_last_selected_row() {
        let mut s = span(&[20, 30, 40]);
        assert!(s.toggle_one(&VIEW, 40, false, true, false));
        assert_eq!(s.anchors(), [1, 2]);
    }

    #[test]
    fn deselecting_the_only_row_resets_only_the_first_anchor() {
        // section 5.2 changes pos[0] or pos[1], never both, on a tap: the second anchor stays on the old row
        let mut s = span(&[20]);
        assert!(s.toggle_one(&VIEW, 20, false, true, false));
        assert_eq!(s.anchors(), [-1, 1]);
        assert!(!s.is_active());
    }

    #[test]
    fn a_tap_with_no_anchor_sets_only_the_second_anchor() {
        let mut s = Selection::default();
        assert!(s.toggle_one(&VIEW, 40, true, true, false));
        assert_eq!(s.anchors(), [-1, 3]);
    }

    #[test]
    fn requests_that_match_the_state_or_name_an_unknown_id_change_nothing() {
        let mut s = sel(&[20]);
        assert!(!s.toggle_one(&VIEW, 20, true, true, false));
        assert!(!s.toggle_one(&VIEW, 30, false, true, false));
        assert!(!s.toggle_one(&VIEW, 999, true, true, false));
        assert_eq!(selected_in_view(&s), vec![20]);
    }

    #[test]
    fn programmatic_changes_do_not_move_anchors() {
        let mut s = sel(&[20]);
        let before = s.anchors();
        assert!(s.toggle_one(&VIEW, 50, true, false, false));
        assert_eq!(s.anchors(), before);
    }

    #[test]
    fn select_all_acts_on_the_full_list_and_resets_anchors() {
        let full = [10, 20, 30, 40, 50, 60, 70];
        let mut s = sel(&[20]);
        s.select_all(&full);
        assert_eq!(s.count_in(&full), 7);
        assert_eq!(s.anchors(), [-1, -1]);
    }

    #[test]
    fn invert_flips_the_full_list() {
        let full = [1, 2, 3];
        let mut s = Selection::default();
        s.toggle_one(&[1], 1, true, true, false);
        s.invert(&full);
        assert!(!s.contains(1) && s.contains(2) && s.contains(3));
        assert_eq!(s.anchors(), [-1, -1]);
    }

    #[test]
    fn ids_and_anchor_indexes_stay_consistent_over_a_random_walk() {
        // deterministic LCG walk: every anchor stays in range or -1, and selected ids are always real rows
        let mut state: u64 = 0x9e37_79b9;
        let mut next = move || {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as usize
        };
        let mut s = Selection::default();
        for _ in 0..500 {
            let id = VIEW[next() % VIEW.len()];
            let selected = next() % 2 == 0;
            let long = next() % 3 == 0;
            s.toggle_one(&VIEW, id, selected, true, long);
            for a in s.anchors() {
                assert!(a == -1 || (0..VIEW.len() as i64).contains(&a));
            }
            assert!(s.ids.iter().all(|i| VIEW.contains(i)));
        }
    }

    #[test]
    fn click_opens_when_nothing_is_selected_and_toggles_while_selecting() {
        let mut s = Selection::default();
        assert_eq!(s.click(&VIEW, 10), Click::Open(10));
        s.long_press(&VIEW, 10);
        assert_eq!(s.click(&VIEW, 20), Click::Toggled { id: 20, selected: true });
        assert_eq!(s.click(&VIEW, 10), Click::Toggled { id: 10, selected: false });
    }

    #[test]
    fn back_clears_the_selection_before_navigating_up() {
        let mut s = Selection::default();
        s.long_press(&VIEW, 10);
        assert!(s.back());
        assert!(!s.is_active());
        assert!(!s.back());
    }

    #[test]
    fn shift_click_selects_the_range_from_the_anchor() {
        let mut s = span(&[20]);
        assert_eq!(s.click_with(&VIEW, 50, true, false), Click::Toggled { id: 50, selected: true });
        assert_eq!(selected_in_view(&s), vec![20, 30, 40, 50]);
    }

    #[test]
    fn ctrl_click_toggles_one_row_even_with_nothing_selected() {
        let mut s = Selection::default();
        assert_eq!(s.click_with(&VIEW, 30, false, true), Click::Toggled { id: 30, selected: true });
        assert_eq!(s.click_with(&VIEW, 50, false, true), Click::Toggled { id: 50, selected: true });
        assert_eq!(selected_in_view(&s), vec![30, 50], "no range between them");
        assert_eq!(s.click_with(&VIEW, 30, false, true), Click::Toggled { id: 30, selected: false });
        assert_eq!(selected_in_view(&s), vec![50]);
    }

    #[test]
    fn a_click_without_modifiers_still_opens_when_nothing_is_selected() {
        let mut s = Selection::default();
        assert_eq!(s.click_with(&VIEW, 30, false, false), Click::Open(30));
    }
}
