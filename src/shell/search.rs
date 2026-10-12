//! Search toolbar logic (Kotlin `SearchToolbar`, T-36a-039): when a query is submitted, when reset shows, and when the
//! entry takes focus on open.

/// The query to submit on Enter, or `None` for a blank query (blank never searches).
pub fn submit(query: &str) -> Option<&str> {
    if query.trim().is_empty() { None } else { Some(query) }
}

/// The reset control shows only for a non-empty query.
pub fn reset_visible(query: &str) -> bool {
    !query.is_empty()
}

/// The keyboard opens with the entry only when the query was empty at open time.
pub fn focus_on_open(query_at_open: &str) -> bool {
    query_at_open.is_empty()
}

/// The search action in the bar: hidden when search is disabled, the search icon when closed, reset when there is text,
/// and nothing while open but empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchAction {
    Hidden,
    Open,
    Reset,
    Nothing,
}

/// `query` is `None` when search is closed (behaviour 5.6).
pub fn search_action(query: Option<&str>, enabled: bool) -> SearchAction {
    match (enabled, query) {
        (false, _) => SearchAction::Hidden,
        (true, None) => SearchAction::Open,
        (true, Some(q)) if reset_visible(q) => SearchAction::Reset,
        (true, Some(_)) => SearchAction::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_queries_do_not_search() {
        assert_eq!(submit(""), None);
        assert_eq!(submit("   "), None);
        assert_eq!(submit(" frieren "), Some(" frieren "));
    }

    #[test]
    fn reset_only_for_a_non_empty_query() {
        assert!(!reset_visible(""));
        assert!(reset_visible("a"));
    }

    #[test]
    fn focus_opens_the_entry_only_for_an_empty_query() {
        assert!(focus_on_open(""));
        assert!(!focus_on_open("old"));
    }

    #[test]
    fn search_action_follows_the_toolbar_states() {
        assert_eq!(search_action(None, false), SearchAction::Hidden);
        assert_eq!(search_action(None, true), SearchAction::Open);
        assert_eq!(search_action(Some(""), true), SearchAction::Nothing);
        assert_eq!(search_action(Some("x"), true), SearchAction::Reset);
    }

    #[test]
    fn a_disabled_search_hides_the_action_whatever_the_query() {
        assert_eq!(search_action(Some("x"), false), SearchAction::Hidden);
        assert_eq!(search_action(Some(""), false), SearchAction::Hidden);
    }

    #[test]
    fn reset_follows_the_text_and_the_toolbar_never_hides_it_while_enabled() {
        assert_eq!(search_action(Some("a"), true), SearchAction::Reset);
        assert_eq!(search_action(Some(""), true), SearchAction::Nothing);
    }

}
