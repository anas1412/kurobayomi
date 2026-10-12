//! Loading, empty, error and toast primitives (T-40a-030, D§13). Pure data here; `view` turns them into widgets.

use kuro_i18n::{MsgKey, tr};

/// Padding above and below a spinner row, and the spinner's size (D§13).
pub const SPINNER_PADDING_PX: i32 = 16;
pub const SPINNER_SIZE_PX: i32 = 16;
/// An empty state shows at most this many action buttons.
pub const EMPTY_ACTION_LIMIT: usize = 3;

/// Actions an empty or error state can offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyAction {
    Retry,
    Help,
    /// The local source's help guide, the only action for `LocalMangaSource` (B§2.4).
    LocalHelp,
}

impl EmptyAction {
    /// The accessible name of the action.
    pub fn label(self) -> String {
        match self {
            EmptyAction::Retry => tr(MsgKey::action_retry),
            EmptyAction::Help => tr(MsgKey::label_help),
            EmptyAction::LocalHelp => tr(MsgKey::local_source_help_guide),
        }
    }

    /// GTK symbolic icon of the action (U§3.6 mapping).
    pub fn icon(self) -> &'static str {
        match self {
            EmptyAction::Retry => "view-refresh-symbolic",
            EmptyAction::Help | EmptyAction::LocalHelp => "help-about-symbolic",
        }
    }
}

/// The actions of an error empty state. A local source offers its help guide only; any other source offers retry, and
/// help after it. Help is left out while there is no help page to open, so a web source's error state is Retry alone, and
/// a local source then offers Retry, so its error state is never empty (decision: T-14a-047 forbids opening the Aniyomi
/// guide). "Open in WebView" lives in the page overflow menu only, where a login may be needed.
pub fn error_actions(is_local: bool) -> Vec<EmptyAction> {
    error_actions_with(is_local, super::nav::help_url().is_some())
}

fn error_actions_with(is_local: bool, has_help: bool) -> Vec<EmptyAction> {
    let mut actions = if is_local {
        vec![EmptyAction::LocalHelp]
    } else {
        vec![EmptyAction::Retry, EmptyAction::Help]
    };
    if !has_help {
        actions.retain(|a| !matches!(a, EmptyAction::Help | EmptyAction::LocalHelp));
    }
    if actions.is_empty() {
        actions.push(EmptyAction::Retry);
    }
    actions
}

/// The name of a source that is not installed: "name (LANG)", or the bare id when the name or the language is blank
/// (T-41a-008).
pub fn stub_source_label(name: &str, lang: &str, id: i64) -> String {
    if name.trim().is_empty() || lang.trim().is_empty() {
        id.to_string()
    } else {
        format!("{name} ({})", lang.to_uppercase())
    }
}

/// How long a toast stays up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastDuration {
    Seconds(u32),
    Indefinite,
}

/// The one action a toast can carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastAction {
    Retry,
}

/// A toast in the bottom centre of the window. It always has a dismiss button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastSpec {
    pub message: String,
    pub action: Option<ToastAction>,
    pub duration: ToastDuration,
}

/// The default toast length (3 s).
pub const DEFAULT_TOAST: ToastDuration = ToastDuration::Seconds(3);

/// A load error while items are on screen: a retry toast that does not hide itself (B§2.3).
pub fn error_toast(message: &str) -> ToastSpec {
    ToastSpec { message: message.to_owned(), action: Some(ToastAction::Retry), duration: ToastDuration::Indefinite }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stub_is_named_with_its_language_or_by_its_bare_id() {
        assert_eq!(stub_source_label("Demo", "en", 9), "Demo (EN)");
        assert_eq!(stub_source_label("Demo", "", 9), "9");
        assert_eq!(stub_source_label("  ", "en", 9), "9");
    }

    #[test]
    fn error_actions_for_a_web_source_are_retry_and_help_in_order() {
        assert_eq!(error_actions_with(false, true), vec![EmptyAction::Retry, EmptyAction::Help]);
    }

    #[test]
    fn a_local_source_error_offers_only_its_help_guide() {
        assert_eq!(error_actions_with(true, true), vec![EmptyAction::LocalHelp]);
    }

    #[test]
    fn without_a_help_page_the_help_actions_are_left_out() {
        assert_eq!(error_actions_with(false, false), vec![EmptyAction::Retry], "a web source's error state is retry alone");
        assert_eq!(error_actions_with(true, false), vec![EmptyAction::Retry], "a local error state is never empty");
        // kuro-local has no Kurobayomi help page yet, so the shipped lists have none either
        assert!(!error_actions(true).contains(&EmptyAction::LocalHelp));
        assert!(!error_actions(false).contains(&EmptyAction::Help));
    }

    #[test]
    fn no_empty_state_offers_more_than_three_actions() {
        assert!(error_actions(false).len() <= EMPTY_ACTION_LIMIT);
        assert!(error_actions(true).len() <= EMPTY_ACTION_LIMIT);
    }

    #[test]
    fn error_toast_is_retry_and_does_not_hide_itself() {
        let toast = error_toast("boom");
        assert_eq!(toast.message, "boom");
        assert_eq!(toast.action, Some(ToastAction::Retry));
        assert_eq!(toast.duration, ToastDuration::Indefinite);
    }

    #[test]
    fn the_default_toast_lasts_three_seconds() {
        assert_eq!(DEFAULT_TOAST, ToastDuration::Seconds(3));
    }

    #[test]
    fn spinner_metrics_match_the_design_sheet() {
        assert_eq!((SPINNER_PADDING_PX, SPINNER_SIZE_PX), (16, 16));
    }
}
