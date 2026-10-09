//! Episode selection state and gestures (T-39a-016). Upstream: `AnimeScreen.kt` (`onEpisodeItemClick`, lines 1140-1151;
//! back handling, lines 362-368) and behaviour section 2. Selection is keyed by episode id. The caller owns the list
//! order and passes the ids of the processed list where a rule needs them.

use std::collections::HashSet;

/// The result of a plain click on a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// Nothing is selected: open the episode.
    Open(i64),
    /// Selection is active: the row was toggled and is now `selected`.
    Toggled { id: i64, selected: bool },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    ids: HashSet<i64>,
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

    /// Long press: toggles the row and enters selection (`onEpisodeSelected(item, !selected, true, true)`).
    pub fn long_press(&mut self, id: i64) -> bool {
        toggle(&mut self.ids, id)
    }

    /// Plain click. A selected row is toggled off, a row clicked while selection is active is toggled on, and any other
    /// click opens the episode.
    pub fn click(&mut self, id: i64) -> Click {
        if self.ids.contains(&id) || self.is_active() {
            Click::Toggled {
                id,
                selected: toggle(&mut self.ids, id),
            }
        } else {
            Click::Open(id)
        }
    }

    /// Back or Escape. Selection is cleared first and the key is consumed (`true`). With nothing selected it returns
    /// `false` and the caller navigates up.
    pub fn back(&mut self) -> bool {
        if self.is_active() {
            self.ids.clear();
            true
        } else {
            false
        }
    }

    /// `onAllEpisodeSelected(true)`: selects exactly the ids of `list`.
    pub fn select_all(&mut self, list: &[i64]) {
        self.ids = list.iter().copied().collect();
    }

    /// `onAllEpisodeSelected(false)`: the Cancel button and the back gesture.
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// `onInvertSelection`: flips each id of `list`.
    pub fn invert(&mut self, list: &[i64]) {
        for id in list {
            toggle(&mut self.ids, *id);
        }
    }
}

/// Flips membership and returns the new state.
fn toggle(ids: &mut HashSet<i64>, id: i64) -> bool {
    if ids.remove(&id) {
        false
    } else {
        ids.insert(id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_press_selects_that_row_and_enters_selection() {
        let mut s = Selection::default();
        assert!(s.long_press(7));
        assert!(s.is_active() && s.contains(7));
        assert!(!s.long_press(7));
        assert!(!s.is_active());
    }

    #[test]
    fn click_opens_when_nothing_is_selected() {
        let mut s = Selection::default();
        assert_eq!(s.click(3), Click::Open(3));
        assert!(!s.is_active());
    }

    #[test]
    fn click_toggles_while_selecting() {
        let mut s = Selection::default();
        s.long_press(1);
        assert_eq!(s.click(2), Click::Toggled { id: 2, selected: true });
        assert_eq!(s.click(1), Click::Toggled { id: 1, selected: false });
        assert_eq!(s.click(2), Click::Toggled { id: 2, selected: false });
        assert!(!s.is_active());
    }

    #[test]
    fn back_clears_the_selection_before_navigating_up() {
        let mut s = Selection::default();
        s.long_press(1);
        assert!(s.back());
        assert!(!s.is_active());
        assert!(!s.back());
    }

    #[test]
    fn select_all_invert_and_cancel() {
        let list = [10, 20, 30];
        let mut s = Selection::default();
        s.select_all(&list);
        assert_eq!(s.count_in(&list), 3);
        s.invert(&[20, 40]);
        assert_eq!(s.count_in(&list), 2);
        assert!(s.contains(40));
        s.clear();
        assert!(!s.is_active());
    }
}
