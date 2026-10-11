//! The anime browse page's rules: the gate, the listing sentinels and restart rule, the chips, toolbar and Back, the
//! display mode, the overflow menu, the WebView and help links, and the error text (T-41b-010, T-41b-011, T-41b-015,
//! T-41b-016, T-41b-017, T-41b-018, T-41b-023, T-41b-042, T-41b-044). Ports `BrowseAnimeSourceScreenModel` and the
//! toolbar and menu rules. The GTK page is not built here: these say what the page shows and what each action does.
//! The body states (`content_plan`), the append-error toast (`error_toast`) and the grid columns (`grid_columns`) are
//! the parent module's and are used as they are.

use kuro_domain::source::browse::BrowseQuery;
use kuro_i18n::{Arg, MsgKey, trf};
use kuro_source::SourceError;
use kuro_source::messages::source_error_message;

use crate::browse::nav::help_url;

/// The stub page's line: "Source not installed: LABEL".
pub fn not_installed_text(label: &str) -> String {
    trf(MsgKey::source_not_installed, &[("arg1", Arg::Str(label))])
}

/// The chips of the browse page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chip {
    Popular,
    Latest,
    Filter,
}

/// The chips shown and the one selected. Popular is always shown, Latest only when the source supports it, and Filter
/// only when the source defines filters. A search selects Filter, and the selection is `None` when Filter is not shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chips {
    pub shown: Vec<Chip>,
    pub selected: Option<Chip>,
}

pub fn chips(supports_latest: bool, has_filters: bool, listing: &BrowseQuery) -> Chips {
    let mut shown = vec![Chip::Popular];
    if supports_latest {
        shown.push(Chip::Latest);
    }
    if has_filters {
        shown.push(Chip::Filter);
    }
    let wanted = match listing {
        BrowseQuery::Popular => Chip::Popular,
        BrowseQuery::Latest => Chip::Latest,
        BrowseQuery::Search(_) => Chip::Filter,
    };
    let selected = shown.contains(&wanted).then_some(wanted);
    Chips { shown, selected }
}

/// What a chip press does. Popular and Latest reset the filters to the source defaults and switch the listing. Filter
/// opens the filter sheet and leaves the listing as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChipPress {
    Switch(BrowseQuery),
    OpenSheet,
}

pub fn press(chip: Chip) -> ChipPress {
    match chip {
        Chip::Popular => ChipPress::Switch(BrowseQuery::Popular),
        Chip::Latest => ChipPress::Switch(BrowseQuery::Latest),
        Chip::Filter => ChipPress::OpenSheet,
    }
}

/// What Back does on the browse page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    /// Clear the toolbar text and stay on the page.
    ClearToolbarText,
    /// Leave the page.
    Leave,
}

/// Back clears the toolbar text first when the listing was not typed by the user and the toolbar holds a query;
/// otherwise it leaves the page (E-BR-05).
pub fn back(is_user_query: bool, toolbar_query: Option<&str>) -> Back {
    if !is_user_query && toolbar_query.is_some() { Back::ClearToolbarText } else { Back::Leave }
}

/// The browse overflow menu. "Open in WebView" is offered for a non-local source. "Help" is offered for the local
/// source only when a help page exists, and none does (`help_url`). "Settings" is offered for a configurable source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overflow {
    OpenInWebView,
    Help,
    Settings,
}

pub fn overflow(is_local: bool, configurable: bool) -> Vec<Overflow> {
    let mut items = Vec::new();
    if !is_local {
        items.push(Overflow::OpenInWebView);
    } else if help_url().is_some() {
        items.push(Overflow::Help);
    }
    if configurable {
        items.push(Overflow::Settings);
    }
    items
}

/// Where a link goes: an external page through `gtk::UriLauncher`, or the WebView page for a source's home URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    External(String),
    WebView { url: String, source_name: String },
}

/// "Open in WebView" opens the source's home URL with its name. A missing or empty home URL opens nothing.
pub fn webview_link(home_url: Option<&str>, source_name: &str) -> Option<Link> {
    home_url
        .filter(|url| !url.is_empty())
        .map(|url| Link::WebView { url: url.to_owned(), source_name: source_name.to_owned() })
}

/// The help page as an external link. There is none in this app, so this is `None` (see `help_url`).
pub fn help_link() -> Option<Link> {
    help_url().map(|url| Link::External(url.to_owned()))
}

/// The text a browse page shows for a failed call: the formatted message, as the Kotlin `ExceptionFormatter` gives it.
/// Global search shows the raw text instead, which is not this function.
pub fn browse_error_text(error: &SourceError, online: bool) -> String {
    source_error_message(error, online)
}

/// Opens an external page in the default browser through `gtk::UriLauncher`. A launch that fails is logged, not shown.
pub fn open_external(url: &str, parent: Option<&gtk::Window>) {
    gtk::UriLauncher::new(url).launch(parent, gtk::gio::Cancellable::NONE, |result| {
        if let Err(error) = result {
            tracing::warn!(%error, "could not open an external link");
        }
    });
}

#[cfg(test)]
use kuro_prefs::library::LibraryDisplayMode;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::content::{ContentInput, ContentPlan, content_plan};
    use crate::browse::grid::{GridColumns, grid_columns};
    use crate::browse::primitives::{EmptyAction, ToastAction, ToastDuration, error_actions, error_toast};
    

    fn search(text: &str) -> BrowseQuery {
        BrowseQuery::Search(text.to_owned())
    }

    #[test]
    fn popular_is_always_shown_and_latest_and_filter_depend_on_the_source() {
        let full = chips(true, true, &BrowseQuery::Popular);
        assert_eq!(full.shown, [Chip::Popular, Chip::Latest, Chip::Filter]);
        assert_eq!(full.selected, Some(Chip::Popular));
        let bare = chips(false, false, &search("x"));
        assert_eq!(bare.shown, [Chip::Popular]);
        assert_eq!(bare.selected, None, "a search selects Filter, which is not shown");
    }

    #[test]
    fn a_search_selects_filter_and_latest_selects_latest() {
        assert_eq!(chips(true, true, &search("x")).selected, Some(Chip::Filter));
        assert_eq!(chips(true, true, &BrowseQuery::Latest).selected, Some(Chip::Latest));
    }

    #[test]
    fn popular_and_latest_switch_the_listing_and_filter_opens_the_sheet() {
        assert_eq!(press(Chip::Popular), ChipPress::Switch(BrowseQuery::Popular));
        assert_eq!(press(Chip::Latest), ChipPress::Switch(BrowseQuery::Latest));
        assert_eq!(press(Chip::Filter), ChipPress::OpenSheet);
    }

    #[test]
    fn back_clears_the_toolbar_text_first_for_a_non_user_search_and_otherwise_leaves() {
        assert_eq!(back(false, Some("abc")), Back::ClearToolbarText);
        assert_eq!(back(true, Some("abc")), Back::Leave, "a search the user typed leaves the page");
        assert_eq!(back(false, None), Back::Leave);
    }

    #[test]
    fn a_zero_column_preference_is_adaptive_and_a_positive_one_is_that_many_columns() {
        assert_eq!(grid_columns(800, 0, 0), GridColumns::Adaptive { cell_px: 128 });
        assert_eq!(grid_columns(800, 0, 3), GridColumns::Fixed(3), "a landscape window reads the landscape key");
        assert!(matches!(grid_columns(800, -1, 0), GridColumns::Adaptive { .. }), "values below 1 are adaptive");
    }

    #[test]
    fn the_overflow_menu_offers_the_webview_for_remote_sources_and_settings_for_configurable_ones() {
        assert_eq!(overflow(false, true), [Overflow::OpenInWebView, Overflow::Settings]);
        assert_eq!(overflow(false, false), [Overflow::OpenInWebView]);
        let local_help = if help_url().is_some() { vec![Overflow::Help] } else { vec![] };
        assert_eq!(overflow(true, false), local_help);
    }

    #[test]
    fn a_webview_link_needs_a_home_url_and_the_help_link_follows_the_project_help_page() {
        assert_eq!(
            webview_link(Some("https://anime.example/"), "Ex"),
            Some(Link::WebView { url: "https://anime.example/".into(), source_name: "Ex".into() })
        );
        assert_eq!(webview_link(None, "Ex"), None);
        assert_eq!(webview_link(Some(""), "Ex"), None);
        assert_eq!(help_link().is_some(), help_url().is_some());
    }

    #[test]
    fn source_errors_read_as_the_browse_text_for_each_variant() {
        assert_eq!(browse_error_text(&SourceError::NotInstalled(3), true), "Source not found");
        assert_eq!(browse_error_text(&SourceError::Http(404), true), "HTTP 404, check website in WebView");
        assert_eq!(browse_error_text(&SourceError::UnknownHost { host: "x.test".into() }, false), "No Internet connection");
        assert_eq!(browse_error_text(&SourceError::UnknownHost { host: "x.test".into() }, true), "Couldn't reach x.test");
    }

    #[test]
    fn a_failed_first_page_is_the_error_state_and_a_failed_later_page_is_an_indefinite_toast_with_retry() {
        let failed = ContentInput {
            initialized: true,
            stub: false,
            is_local: false,
            loading: false,
            items: 0,
            refresh_error: Some("HTTP 503, check website in WebView"),
            append_error: None,
            display: LibraryDisplayMode::CompactGrid,
        };
        assert_eq!(
            content_plan(&failed),
            ContentPlan::Error { message: "HTTP 503, check website in WebView".into(), actions: error_actions(false) }
        );
        let appended = ContentInput { items: 25, refresh_error: None, append_error: Some("Couldn't reach x.test"), ..failed };
        let toast = error_toast("Couldn't reach x.test");
        assert_eq!(content_plan(&appended), ContentPlan::Items { mode: LibraryDisplayMode::CompactGrid, toast: Some(toast.clone()) });
        assert_eq!(toast.action, Some(ToastAction::Retry));
        assert_eq!(toast.duration, ToastDuration::Indefinite);
        assert!(EmptyAction::Retry.label().len() > 0);
    }

    #[test]
    fn source_error_texts_follow_the_formatter_for_each_kind() {
        assert_eq!(browse_error_text(&SourceError::Http(503), true), "HTTP 503, check website in WebView");
        assert_eq!(browse_error_text(&SourceError::Transport("reset".into()), false), "No Internet connection");
        assert_eq!(browse_error_text(&SourceError::Transport("reset".into()), true), "reset", "a transport error keeps its text online");
        assert_eq!(browse_error_text(&SourceError::EmptyPageList, true), "No pages found");
        assert_eq!(browse_error_text(&SourceError::Unsupported("search"), true), "not supported: search", "other errors keep their message");
    }
}
