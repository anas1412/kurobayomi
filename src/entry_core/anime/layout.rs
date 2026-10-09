//! Layout and FAB rules of the anime entry screen (T-39a-035, T-39a-037). Upstream: `AnimeScreen.kt` (FAB, lines 432-462
//! and 779-804; large and small layouts), `LazyListState.kt:22-30` (`shouldExpandFAB`), behaviour sections 1.2, 1.3, 9.

use kuro_i18n::MsgKey;

/// Two panes (info and actions left, list right) at or above this window width, one column below it.
pub const TWO_PANE_MIN_WIDTH: i32 = 900;

pub fn two_pane(window_width: i32) -> bool {
    window_width >= TWO_PANE_MIN_WIDTH
}

/// The FAB state. The rule uses the unfiltered list for both the visibility and the label, so a filter never hides the
/// button. This resolves the upstream inconsistency recorded in behaviour section 9.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fab {
    pub visible: bool,
    /// True when any episode is seen (label Resume), false for Start.
    pub resume: bool,
}

impl Fab {
    pub fn label(self) -> MsgKey {
        if self.resume {
            MsgKey::action_resume
        } else {
            MsgKey::action_start
        }
    }
}

/// `seen` is the seen flag of every episode of the unfiltered list. The FAB is visible when one is unseen and nothing is
/// selected.
pub fn fab(seen: &[bool], any_selected: bool) -> Fab {
    Fab {
        visible: seen.iter().any(|s| !s) && !any_selected,
        resume: seen.iter().any(|s| *s),
    }
}

/// `shouldExpandFAB`: expanded (label shown) at the top of the list, after a backward scroll, or when nothing is left to
/// scroll forward to.
pub fn fab_expanded(first_visible: usize, first_offset: i32, last_scrolled_backward: bool, can_scroll_forward: bool) -> bool {
    (first_visible == 0 && first_offset == 0) || last_scrolled_backward || !can_scroll_forward
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_panes_start_at_900_px() {
        assert!(!two_pane(899));
        assert!(two_pane(900));
        assert!(!two_pane(360));
    }

    #[test]
    fn fab_visible_when_an_unseen_episode_exists_and_nothing_is_selected() {
        assert!(fab(&[true, false], false).visible);
        assert!(!fab(&[true, true], false).visible);
        assert!(!fab(&[false, false], true).visible);
        assert!(!fab(&[], false).visible);
    }

    #[test]
    fn fab_label_is_resume_once_any_episode_is_seen() {
        assert_eq!(fab(&[true, false], false).label(), MsgKey::action_resume);
        assert_eq!(fab(&[false, false], false).label(), MsgKey::action_start);
    }

    #[test]
    fn fab_expands_at_the_top_and_after_a_backward_scroll() {
        assert!(fab_expanded(0, 0, false, true));
        assert!(!fab_expanded(0, 5, false, true));
        assert!(fab_expanded(3, 0, true, true));
        assert!(fab_expanded(3, 0, false, false));
        assert!(!fab_expanded(3, 0, false, true));
    }
}
