//! Layout and FAB rules of the anime entry screen (T-39a-035, T-39a-037). Upstream: `AnimeScreen.kt` (FAB, lines 432-462
//! and 779-804; large and small layouts), `LazyListState.kt:22-30` (`shouldExpandFAB`), behaviour sections 1.2, 1.3, 9.

use kuro_i18n::MsgKey;

/// Two panes (info and actions left, list right) at or above `TWO_PANE_MIN_WIDTH`, one column below it.
pub use crate::entry_core::constants::{TWO_PANE_MIN_WIDTH, two_pane};

/// The width of the bottom action bar: the whole window narrow, half of it in the two-pane layout.
pub fn bottom_bar_width(window_width: i32) -> Option<i32> {
    two_pane(window_width).then_some(window_width / 2)
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

/// The opacity of the title and of the header bar background in the narrow layout (T-39a-036, upstream `AnimeScreen.kt`
/// 385-389): the title is opaque once the first item has scrolled out of view; the background is opaque when the first item
/// is not visible or the list has scrolled. The two-pane layout is always opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderFade {
    pub title: f64,
    pub background: f64,
}

pub fn header_fade(narrow: bool, first_visible: bool, scrolled: bool) -> HeaderFade {
    let alpha = |opaque: bool| if opaque { 1.0 } else { 0.0 };
    HeaderFade {
        title: alpha(!narrow || !first_visible),
        background: alpha(!narrow || !first_visible || scrolled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bottom_bar_is_full_width_narrow_and_half_width_wide() {
        assert_eq!(bottom_bar_width(360), None);
        assert_eq!(bottom_bar_width(899), None);
        assert_eq!(bottom_bar_width(1000), Some(500));
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

    #[test]
    fn the_narrow_header_fades_in_as_the_first_item_scrolls_away() {
        let top = header_fade(true, true, false);
        assert_eq!((top.title, top.background), (0.0, 0.0));
        let nudged = header_fade(true, true, true);
        assert_eq!((nudged.title, nudged.background), (0.0, 1.0), "the background shows on any scroll, the title waits");
        let away = header_fade(true, false, true);
        assert_eq!((away.title, away.background), (1.0, 1.0));
    }

    #[test]
    fn the_two_pane_header_is_always_opaque() {
        assert_eq!(header_fade(false, true, false), HeaderFade { title: 1.0, background: 1.0 });
    }
}
