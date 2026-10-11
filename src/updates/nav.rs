//! Where History and Updates sit in the navigation (T-42a-021, T-42a-025): the sidebar index per nav style, the
//! back action of a tab that was moved into More (`fromMore`), and what choosing the tab again does.

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
    fn nav_reselect_resumes_history_and_opens_downloads_from_updates() {
        assert_eq!(reselect_action("history", true), Reselect::ResumeHistory);
        assert_eq!(reselect_action("updates", true), Reselect::OpenDownloads);
        assert_eq!(reselect_action("history", false), Reselect::Nothing);
        assert_eq!(reselect_action("library", true), Reselect::Nothing);
    }
}
