//! Navigation rules of a source's browse page: what Back does, which filters a search starts from, and the last-used
//! source write (T-40b-036, T-40b-039). Pure: the page runs the effects.

/// Where the Help actions of a browse page go. `kuro-local` owns the answer: no Kurobayomi help page exists yet, so
/// it is `None` and the Help actions are omitted. The Aniyomi guide is never opened (T-14a-047).
pub fn help_url() -> Option<&'static str> {
    kuro_local::ux::LOCAL_HELP_URL
}

/// Opens an external page in the default browser through `gtk::UriLauncher`. A launch that fails is logged, not shown.
pub fn open_external(url: &str, parent: Option<&gtk::Window>) {
    gtk::UriLauncher::new(url).launch(parent, gtk::gio::Cancellable::NONE, |result| {
        if let Err(error) = result {
            tracing::warn!(%error, "could not open an external link");
        }
    });
}

/// The web address of a browsed entry for Open in browser: an `http` or `https` url as it is, or a path on the source's
/// site joined to `home`. `None` when neither gives a web address.
pub fn entry_web_url(home: Option<&str>, url: &str) -> Option<String> {
    let url = url.trim();
    let web = |text: &str| {
        let lower = text.to_ascii_lowercase();
        lower.starts_with("http://") || lower.starts_with("https://")
    };
    if web(url) {
        return Some(url.to_owned());
    }
    let home = home.map(str::trim).filter(|home| web(home))?;
    if url.is_empty() {
        return None;
    }
    Some(format!("{}/{}", home.trim_end_matches('/'), url.trim_start_matches('/')))
}

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

/// Where an error-state action opens a url, when it does: both Help actions open the help page (B-EC-14), if there is one.
pub fn action_url(action: super::primitives::EmptyAction) -> Option<&'static str> {
    use super::primitives::EmptyAction;
    matches!(action, EmptyAction::Help | EmptyAction::LocalHelp).then(help_url).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_opens_its_own_web_address_or_its_path_on_the_source_site() {
        assert_eq!(entry_web_url(None, " https://a.test/m/1 ").as_deref(), Some("https://a.test/m/1"));
        assert_eq!(entry_web_url(Some("https://a.test/"), "/m/1").as_deref(), Some("https://a.test/m/1"));
        assert_eq!(entry_web_url(Some("https://a.test"), "m/1").as_deref(), Some("https://a.test/m/1"));
        assert_eq!(entry_web_url(None, "/m/1"), None, "a path without a site");
        assert_eq!(entry_web_url(Some("file:///x"), "/m/1"), None, "only a web site");
        assert_eq!(entry_web_url(Some("https://a.test"), ""), None);
    }

    #[test]
    fn no_action_opens_an_aniyomi_guide_and_the_others_open_nothing() {
        use super::super::primitives::EmptyAction;
        for action in [EmptyAction::LocalHelp, EmptyAction::Help, EmptyAction::Retry] {
            assert!(action_url(action).is_none_or(|url| !url.contains("aniyomi.org")), "{action:?}");
        }
        assert_eq!(action_url(EmptyAction::Retry), None);
        assert_eq!(action_url(EmptyAction::Help), help_url());
        assert_eq!(action_url(EmptyAction::LocalHelp), help_url());
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
}
