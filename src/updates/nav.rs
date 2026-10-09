//! Where History and Updates sit in the navigation (T-42a-021, T-42a-025): the sidebar index per nav style, the
//! back action of a tab that was moved into More (`fromMore`), and what choosing the tab again does.

use kuro_prefs::ui::{NavStyle, Tab};

/// The index of the More slot, which holds the tab the nav style moved out of the bar.
pub const MORE_INDEX: u8 = 5;

/// The sidebar index of `tab` for `style`: its place among the visible tabs, or [`MORE_INDEX`] when the style moves
/// it into More. History is 5, 3 or 2 and Updates is 5, 2 or 1.
pub fn sidebar_index(tab: Tab, style: NavStyle) -> u8 {
    if style.more_tab() == tab {
        return MORE_INDEX;
    }
    style.tabs().iter().position(|t| *t == tab).map_or(MORE_INDEX, |i| i as u8)
}

pub fn history_sidebar_index(style: NavStyle) -> u8 {
    sidebar_index(Tab::History, style)
}

pub fn updates_sidebar_index(style: NavStyle) -> u8 {
    sidebar_index(Tab::Updates, style)
}

/// `fromMore`: the tab was reached through More, so its header shows a back button that returns to More.
pub fn from_more(tab: Tab, style: NavStyle) -> bool {
    style.more_tab() == tab
}

/// What choosing a sidebar destination again does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reselect {
    /// History: resume the newest row of the visible sub-tab.
    ResumeHistory,
    /// Updates: open the Downloads page.
    OpenDownloads,
    Nothing,
}

/// `nav_id` is the spec id of the destination; `already_visible` is whether it was showing when it was chosen.
pub fn reselect_action(nav_id: &str, already_visible: bool) -> Reselect {
    match (nav_id, already_visible) {
        ("history", true) => Reselect::ResumeHistory,
        ("updates", true) => Reselect::OpenDownloads,
        _ => Reselect::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_history_index_is_5_3_or_2_per_style() {
        assert_eq!(history_sidebar_index(NavStyle::MOVE_HISTORY_TO_MORE), 5);
        assert_eq!(history_sidebar_index(NavStyle::MOVE_BROWSE_TO_MORE), 3);
        assert_eq!(history_sidebar_index(NavStyle::MOVE_UPDATES_TO_MORE), 2);
        assert_eq!(history_sidebar_index(NavStyle::MOVE_MANGA_TO_MORE), 2);
    }

    #[test]
    fn nav_updates_index_is_5_2_or_1_per_style() {
        assert_eq!(updates_sidebar_index(NavStyle::MOVE_UPDATES_TO_MORE), 5);
        assert_eq!(updates_sidebar_index(NavStyle::MOVE_HISTORY_TO_MORE), 2);
        assert_eq!(updates_sidebar_index(NavStyle::MOVE_BROWSE_TO_MORE), 2);
        assert_eq!(updates_sidebar_index(NavStyle::MOVE_MANGA_TO_MORE), 1);
    }

    #[test]
    fn nav_back_button_only_for_the_tab_moved_into_more() {
        assert!(from_more(Tab::History, NavStyle::MOVE_HISTORY_TO_MORE));
        assert!(!from_more(Tab::History, NavStyle::MOVE_BROWSE_TO_MORE));
        assert!(from_more(Tab::Updates, NavStyle::MOVE_UPDATES_TO_MORE));
        assert!(!from_more(Tab::Updates, NavStyle::MOVE_HISTORY_TO_MORE));
    }

    #[test]
    fn nav_reselect_resumes_history_and_opens_downloads_from_updates() {
        assert_eq!(reselect_action("history", true), Reselect::ResumeHistory);
        assert_eq!(reselect_action("updates", true), Reselect::OpenDownloads);
        assert_eq!(reselect_action("history", false), Reselect::Nothing);
        assert_eq!(reselect_action("library", true), Reselect::Nothing);
    }
}
