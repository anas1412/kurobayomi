//! What a source's content area shows: spinner, error, empty, stub or the items in the chosen display mode
//! (T-40a-003). The readiness gate (T-40b-003) is the first rule: nothing says "no source" before the registry loaded.

use kuro_prefs::library::LibraryDisplayMode;

use super::primitives::{EmptyAction, ToastSpec, error_actions, error_toast};

/// The inputs of one decision, read from the screen model.
#[derive(Clone, Copy, Debug)]
pub struct ContentInput<'a> {
    /// `SourceRegistry::is_initialized()`.
    pub initialized: bool,
    /// The source is not installed.
    pub stub: bool,
    pub is_local: bool,
    /// A first load is in flight.
    pub loading: bool,
    pub items: usize,
    /// Error of the first page or a refresh.
    pub refresh_error: Option<&'a str>,
    /// Error of a later page.
    pub append_error: Option<&'a str>,
    pub display: LibraryDisplayMode,
}

/// What to draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentPlan {
    /// Centred spinner while the registry or the first page loads.
    Spinner,
    /// Source not installed: its name and back navigation only.
    Stub,
    /// No items and an error: the message and its actions.
    Error { message: String, actions: Vec<EmptyAction> },
    /// No items and no error: `empty_screen` (D-UI-2).
    Empty,
    /// Items in the given display mode, with a retry toast when a later page failed.
    Items { mode: LibraryDisplayMode, toast: Option<ToastSpec> },
}

/// Refresh errors win over append errors (B§2.2).
pub fn pick_error<'a>(refresh: Option<&'a str>, append: Option<&'a str>) -> Option<&'a str> {
    refresh.or(append)
}

/// Picks what the content area shows.
pub fn content_plan(input: &ContentInput) -> ContentPlan {
    if !input.initialized {
        return ContentPlan::Spinner;
    }
    if input.stub {
        return ContentPlan::Stub;
    }
    let error = pick_error(input.refresh_error, input.append_error);
    if input.items == 0 {
        return match error {
            Some(message) => ContentPlan::Error { message: message.to_owned(), actions: error_actions(input.is_local) },
            None if input.loading => ContentPlan::Spinner,
            None => ContentPlan::Empty,
        };
    }
    ContentPlan::Items { mode: input.display, toast: error.map(error_toast) }
}

/// Whether the error state offers this action in the given plan (used by the view for accessible names).
pub fn offers(plan: &ContentPlan, action: EmptyAction) -> bool {
    matches!(plan, ContentPlan::Error { actions, .. } if actions.contains(&action))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::primitives::ToastAction;

    fn input<'a>() -> ContentInput<'a> {
        ContentInput {
            initialized: true,
            stub: false,
            is_local: false,
            loading: false,
            items: 0,
            refresh_error: None,
            append_error: None,
            display: LibraryDisplayMode::CompactGrid,
        }
    }

    #[test]
    fn nothing_says_no_source_before_the_registry_is_ready() {
        let mut i = input();
        i.initialized = false;
        assert_eq!(content_plan(&i), ContentPlan::Spinner);
        i.items = 0;
        i.stub = true;
        assert_eq!(content_plan(&i), ContentPlan::Spinner);
    }

    #[test]
    fn first_load_with_no_items_shows_the_spinner() {
        let mut i = input();
        i.loading = true;
        assert_eq!(content_plan(&i), ContentPlan::Spinner);
    }

    #[test]
    fn empty_without_error_is_the_empty_screen() {
        assert_eq!(content_plan(&input()), ContentPlan::Empty);
    }

    #[test]
    fn a_refresh_error_wins_over_an_append_error() {
        assert_eq!(pick_error(Some("refresh"), Some("append")), Some("refresh"));
        assert_eq!(pick_error(None, Some("append")), Some("append"));
        assert_eq!(pick_error(None, None), None);
    }

    #[test]
    fn items_with_an_error_keep_the_grid_and_show_a_retry_toast_that_stays() {
        let mut i = input();
        i.items = 3;
        i.append_error = Some("page 2 failed");
        match content_plan(&i) {
            ContentPlan::Items { mode, toast: Some(toast) } => {
                assert_eq!(mode, LibraryDisplayMode::CompactGrid);
                assert_eq!(toast.message, "page 2 failed");
                assert_eq!(toast.action, Some(ToastAction::Retry));
                assert_eq!(toast.duration, crate::browse::primitives::ToastDuration::Indefinite);
            }
            other => panic!("unexpected plan {other:?}"),
        }
    }

    #[test]
    fn no_items_with_an_error_offers_retry_web_view_and_help_for_a_web_source() {
        let mut i = input();
        i.refresh_error = Some("offline");
        let plan = content_plan(&i);
        assert_eq!(
            plan,
            ContentPlan::Error {
                message: "offline".into(),
                actions: error_actions(false),
            }
        );
        assert!(offers(&plan, EmptyAction::Retry));
    }

    #[test]
    fn a_local_source_error_offers_only_the_local_help_guide_when_there_is_one() {
        let mut i = input();
        i.is_local = true;
        i.refresh_error = Some("no folder");
        match content_plan(&i) {
            ContentPlan::Error { actions, .. } => {
                assert_eq!(actions, error_actions(true));
                assert!(!actions.is_empty(), "a local error state always offers something");
            }
            other => panic!("unexpected plan {other:?}"),
        }
    }

    #[test]
    fn a_stub_shows_only_the_not_installed_state() {
        let mut i = input();
        i.stub = true;
        i.items = 5;
        assert_eq!(content_plan(&i), ContentPlan::Stub);
    }

    #[test]
    fn each_display_mode_dispatches_to_its_own_items_view() {
        for mode in LibraryDisplayMode::ALL {
            let mut i = input();
            i.items = 1;
            i.display = mode;
            assert_eq!(content_plan(&i), ContentPlan::Items { mode, toast: None });
        }
    }
}
