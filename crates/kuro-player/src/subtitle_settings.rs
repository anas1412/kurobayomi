//! Subtitle settings container (area 46 `SubtitleSettingsPanel.kt`, T-46b-032). One layout on the
//! desktop: a header with the title and a close control, then three pages. Escape and the close
//! control both close the panel through the shared host (`hosts::close_top`).

use crate::enums::Panels;
use crate::hosts::{Layer, close_top};
use crate::state::PlayerState;

/// Header title of the container.
pub const TITLE: &str = "Subtitles settings";

/// The three pages, in tab order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Typography,
    Colors,
    Miscellaneous,
}

impl Page {
    pub const ALL: [Self; 3] = [Self::Typography, Self::Colors, Self::Miscellaneous];

    pub fn title(self) -> &'static str {
        match self {
            Self::Typography => "Typography",
            Self::Colors => "Colors",
            Self::Miscellaneous => "Miscellaneous",
        }
    }
}

/// The page the container shows. It starts on Typography each time it opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubtitleSettings {
    page: Page,
}

impl SubtitleSettings {
    pub fn page(&self) -> Page {
        self.page
    }

    pub fn select(&mut self, page: Page) {
        self.page = page;
    }
}

/// Closes the container when it is the open panel. Escape and the close control both call this.
/// Returns whether it closed something.
pub fn close(state: &mut PlayerState) -> bool {
    if state.panel != Panels::SubtitleSettings {
        return false;
    }
    close_top(state) == Some(Layer::Panel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::{Dialogs, Sheets};

    #[test]
    fn three_pages_in_order_and_typography_first() {
        assert_eq!(
            Page::ALL.map(Page::title),
            ["Typography", "Colors", "Miscellaneous"]
        );
        assert_eq!(SubtitleSettings::default().page(), Page::Typography);
    }

    #[test]
    fn selecting_a_page_changes_only_the_page() {
        let mut s = SubtitleSettings::default();
        s.select(Page::Miscellaneous);
        assert_eq!(s.page(), Page::Miscellaneous);
    }

    #[test]
    fn escape_and_close_both_close_the_panel() {
        let mut state = PlayerState {
            panel: Panels::SubtitleSettings,
            ..PlayerState::default()
        };
        assert!(close(&mut state));
        assert_eq!(state.panel, Panels::None);
        assert!(!close(&mut state));
    }

    #[test]
    fn a_sheet_on_top_closes_first_and_other_panels_are_left_alone() {
        let mut state = PlayerState {
            panel: Panels::SubtitleSettings,
            sheet: Sheets::More,
            dialog: Dialogs::EpisodeList,
            ..PlayerState::default()
        };
        assert!(!close(&mut state));
        assert_eq!(state.sheet, Sheets::None);
        assert_eq!(state.panel, Panels::SubtitleSettings);
        assert_eq!(state.dialog, Dialogs::EpisodeList);
        let mut other = PlayerState {
            panel: Panels::VideoFilters,
            ..PlayerState::default()
        };
        assert!(!close(&mut other));
        assert_eq!(other.panel, Panels::VideoFilters);
    }
}
