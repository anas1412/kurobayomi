//! Search toolbar logic (Kotlin `SearchToolbar`, T-36a-039): when a query is submitted, when reset shows, and when the
//! entry takes focus on open. The GTK entry is built by `toolbar`.

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

/// Consumers debounce filtering by this long; the toolbar itself does not debounce (behaviour 5.1).
pub const SEARCH_DEBOUNCE_MILLIS: u64 = 250;

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

/// The search part of the app bar: the entry, and the reset button beside it.
pub struct SearchToolbar {
    /// Holds the entry and the reset button. Hidden when search is disabled.
    pub widget: gtk::Box,
    pub entry: gtk::Entry,
    pub reset: gtk::Button,
}

impl SearchToolbar {
    /// Opens the toolbar on `query`. The entry takes focus (and so the keyboard) only when the query was empty.
    pub fn open(&self, query: &str) {
        self.entry.set_text(query);
        if focus_on_open(query) {
            self.entry.grab_focus();
        } else {
            self.entry.set_position(-1);
        }
    }
}

/// A search toolbar for the app bar. Enter runs `on_search` for a non-blank query and then drops focus. The reset
/// button (`action_reset`) shows for a non-empty query, clears it and puts the focus back in the entry. With
/// `enabled` false nothing shows at all.
pub fn toolbar(on_search: impl Fn(&str) + 'static, placeholder: Option<&str>, enabled: bool) -> SearchToolbar {
    let placeholder = placeholder.map(str::to_string).unwrap_or_else(|| tr(MsgKey::action_search_hint));
    let entry = gtk::Entry::builder().placeholder_text(placeholder).hexpand(true).primary_icon_name("system-search-symbolic").build();
    let reset_text = tr(MsgKey::action_reset);
    let reset = gtk::Button::builder().icon_name("edit-clear-symbolic").tooltip_text(reset_text.as_str()).name("action_reset").css_classes(["flat"]).visible(false).build();
    let widget = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    widget.append(&entry);
    widget.append(&reset);
    widget.set_visible(search_action(Some(""), enabled) != SearchAction::Hidden);
    entry.connect_activate(move |e| {
        if let Some(query) = submit(&e.text()) {
            on_search(query);
        }
        if let Some(window) = e.root().and_then(|r| r.downcast::<gtk::Window>().ok()) {
            gtk::prelude::GtkWindowExt::set_focus(&window, None::<&gtk::Widget>);
        }
    });
    {
        let reset = reset.clone();
        entry.connect_changed(move |e| reset.set_visible(search_action(Some(&e.text()), true) == SearchAction::Reset));
    }
    {
        let entry = entry.clone();
        reset.connect_clicked(move |_| {
            entry.set_text("");
            entry.grab_focus();
        });
    }
    SearchToolbar { widget, entry, reset }
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

    #[test]
    fn the_reset_label_is_the_action_reset_string() {
        assert_eq!(tr(MsgKey::action_reset), "Reset");
    }

    #[test]
    fn debounce_is_250_ms() {
        assert_eq!(SEARCH_DEBOUNCE_MILLIS, 250);
    }
}
