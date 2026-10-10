//! Summary expansion of the episode rows (T-39a-012). A summary shows three lines when collapsed and all of them when
//! expanded. The state is kept by episode id, so a row that is recycled or rebuilt keeps its expansion (E-6, T-R3).
//! Upstream: `AnimeEpisodeListItem.kt` (`expandSummary`). The font and line height are the row's text sizes.

use std::collections::HashSet;

/// Lines a collapsed summary shows.
pub const COLLAPSED_LINES: i32 = 3;

/// Font size of a summary, in pixels, and its line height.
pub const SUMMARY_FONT_PX: u32 = 10;
pub const SUMMARY_LINE_HEIGHT_PX: u32 = 11;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SummaryExpansion {
    open: HashSet<i64>,
}

impl SummaryExpansion {
    pub fn is_expanded(&self, episode_id: i64) -> bool {
        self.open.contains(&episode_id)
    }

    /// A click toggles one summary, and only while no episode is selected (a selecting click belongs to the selection).
    /// Returns whether the state changed.
    pub fn click(&mut self, episode_id: i64, any_selected: bool) -> bool {
        if any_selected {
            return false;
        }
        if !self.open.remove(&episode_id) {
            self.open.insert(episode_id);
        }
        true
    }

    /// The line limit of a summary: `None` when expanded (no limit), else three lines.
    pub fn lines(&self, episode_id: i64) -> Option<i32> {
        if self.is_expanded(episode_id) { None } else { Some(COLLAPSED_LINES) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_summary_is_three_lines_until_it_is_clicked() {
        let mut e = SummaryExpansion::default();
        assert_eq!(e.lines(1), Some(3));
        assert!(e.click(1, false));
        assert_eq!(e.lines(1), None);
        assert!(e.click(1, false));
        assert_eq!(e.lines(1), Some(3), "a second click collapses it again");
    }

    #[test]
    fn a_click_while_selecting_does_nothing() {
        let mut e = SummaryExpansion::default();
        assert!(!e.click(2, true));
        assert!(!e.is_expanded(2));
    }

    #[test]
    fn expansion_is_kept_per_episode_id_across_rebuilds() {
        // T-R3: the key is the id, so another row's click does not move it
        let mut e = SummaryExpansion::default();
        e.click(10, false);
        e.click(11, false);
        e.click(11, false);
        assert!(e.is_expanded(10));
        assert!(!e.is_expanded(11));
        assert_eq!((COLLAPSED_LINES, SUMMARY_FONT_PX, SUMMARY_LINE_HEIGHT_PX), (3, 10, 11));
    }
}
