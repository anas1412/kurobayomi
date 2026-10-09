//! Navigation rules of a source's browse page: what Back does, which filters a search starts from, and the last-used
//! source write (T-40b-036, T-40b-039). Pure: the page runs the effects.

/// `LocalMangaSource.HELP_URL`: where the Help actions of a browse page go, for the local source and every other one.
pub const LOCAL_HELP_URL: &str = "https://aniyomi.org/help/guides/local-manga/";

/// What Back does on a browse page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackAction {
    /// An active search with text in the toolbar: clear the text first and stay (B-EC-10).
    ClearQuery,
    /// Leave the page.
    Leave,
}

pub fn back_action(search_active: bool, toolbar_text: &str) -> BackAction {
    if search_active && !toolbar_text.is_empty() { BackAction::ClearQuery } else { BackAction::Leave }
}

/// Which filters a search starts from (B§4.6.1, as `search` in the Kotlin model).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterSeed {
    /// Starting from Popular or Latest: the source's default filters.
    SourceDefaults,
    /// A search is already running: keep its filters unless new ones are given.
    Keep,
}

pub fn filter_seed(running_search: bool) -> FilterSeed {
    if running_search { FilterSeed::Keep } else { FilterSeed::SourceDefaults }
}

/// The source id to store as the last-used catalogue, or `None` when incognito applies to that source (B-EC-17). The
/// page calls this once per open, not on each listing change (T-40b-039).
pub fn last_used_write(incognito: bool, source_id: i64) -> Option<i64> {
    (!incognito).then_some(source_id)
}

/// Where an error-state action opens a url, when it does: both Help actions open the local source guide (B-EC-14).
pub fn action_url(action: super::primitives::EmptyAction) -> Option<&'static str> {
    use super::primitives::EmptyAction;
    matches!(action, EmptyAction::Help | EmptyAction::LocalHelp).then_some(LOCAL_HELP_URL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_help_actions_open_the_local_source_guide_and_the_others_open_nothing() {
        use super::super::primitives::EmptyAction;
        assert_eq!(action_url(EmptyAction::LocalHelp), Some(LOCAL_HELP_URL));
        assert_eq!(action_url(EmptyAction::Help), Some(LOCAL_HELP_URL));
        assert_eq!(action_url(EmptyAction::Retry), None);
        assert_eq!(action_url(EmptyAction::OpenInWebView), None);
        assert!(LOCAL_HELP_URL.starts_with("https://") && LOCAL_HELP_URL.ends_with("local-manga/"));
    }

    #[test]
    fn back_clears_an_active_search_text_before_leaving() {
        assert_eq!(back_action(true, "one piece"), BackAction::ClearQuery);
        assert_eq!(back_action(true, ""), BackAction::Leave);
        assert_eq!(back_action(false, "one piece"), BackAction::Leave);
    }

    #[test]
    fn a_search_from_popular_or_latest_starts_from_the_defaults() {
        assert_eq!(filter_seed(false), FilterSeed::SourceDefaults);
        assert_eq!(filter_seed(true), FilterSeed::Keep);
    }

    #[test]
    fn the_last_used_source_is_written_unless_incognito_applies() {
        assert_eq!(last_used_write(false, 7), Some(7));
        assert_eq!(last_used_write(true, 7), None);
    }
}
