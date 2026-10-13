//! Summary expansion of the episode rows (T-39a-012). A summary shows three lines when collapsed and all of them when
//! expanded. The state is kept by episode id, so a row that is recycled or rebuilt keeps its expansion (E-6, T-R3).
//! Upstream: `AnimeEpisodeListItem.kt` (`expandSummary`). The font and line height are the row's text sizes.

use std::collections::HashSet;

/// Lines a collapsed summary shows.
pub const COLLAPSED_LINES: i32 = 3;

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

    #[cfg(test)]
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

}
