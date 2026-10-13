//! The overflow menu ("⋮") of the manga page and the anime page, one for both (T-39a-075, T-39a-057). Upstream:
//! `EntryToolbar.kt` (the order: change intro length, Refresh, edit categories, Migrate, Share, Open in browser, Settings)
//! and the screens' conditions: a favorite for the category item, Migrate and the intro length; episodes for the intro
//! length; an HTTP source for Share and Open in browser; a configurable source for Settings. The model is pure; the
//! popover, the address copy and the browser launch are the shared widgets the two pages call.

use gtk::prelude::*;
use kuro_i18n::{Arg, MsgKey, tr, trf};

use crate::entry_core::page_shell::plain_row;

/// Whether Open in browser may launch this address: an `http` or `https` one only.
pub fn can_launch(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// The overflow entries, in the order they are shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// The skip-intro length of an anime's episodes.
    ChangeIntro,
    Refresh,
    EditCategories,
    Migrate,
    Share,
    OpenInBrowser,
    Settings,
}

/// What the menu reads from the entry and its source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub favorite: bool,
    /// The entry has an intro length to change: an anime that lists episodes, not seasons.
    pub has_intro_length: bool,
    pub http_source: bool,
    pub configurable_source: bool,
}

/// The items the menu shows for `entry`. Refresh is always there.
pub fn items(entry: Entry) -> Vec<Item> {
    let mut items = Vec::new();
    if entry.favorite && entry.has_intro_length {
        items.push(Item::ChangeIntro);
    }
    items.push(Item::Refresh);
    if entry.favorite {
        items.push(Item::EditCategories);
        items.push(Item::Migrate);
    }
    if entry.http_source {
        items.push(Item::Share);
        items.push(Item::OpenInBrowser);
    }
    if entry.configurable_source {
        items.push(Item::Settings);
    }
    items
}

/// The row label of an item.
pub fn label(item: Item) -> String {
    tr(match item {
        Item::ChangeIntro => MsgKey::action_change_intro_length,
        Item::Refresh => MsgKey::action_webview_refresh,
        Item::EditCategories => MsgKey::action_edit_categories,
        Item::Migrate => MsgKey::action_migrate,
        Item::Share => MsgKey::action_share,
        Item::OpenInBrowser => MsgKey::action_open_in_browser,
        Item::Settings => MsgKey::action_settings,
    })
}

/// The menu as a popover of flat rows, built each time it opens so the items follow the entry. Refresh is greyed while
/// one runs (one refresh at a time). `on_pick` runs with the chosen item.
pub fn popover(entry: Entry, refreshing: bool, on_pick: impl Fn(Item) + Clone + 'static) -> gtk::Popover {
    let popover = gtk::Popover::new();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
    crate::ui::margins(&column, 6);
    for item in items(entry) {
        let on_pick = on_pick.clone();
        let row = plain_row(&label(item), &popover, move |_| on_pick(item));
        row.set_sensitive(item != Item::Refresh || !refreshing);
        column.append(&row);
    }
    popover.set_child(Some(&column));
    popover
}

/// Share: copies the entry's address to the clipboard and returns the notice to show ("Copied to clipboard: …"), or
/// `None` when there is no display.
pub fn share(url: &str) -> Option<String> {
    let display = gtk::gdk::Display::default()?;
    display.clipboard().set_text(url);
    Some(trf(MsgKey::copied_to_clipboard, &[("arg1", Arg::Str(url))]))
}

/// Open in browser: the entry's address in the desktop's browser. Only a web address launches; a failed launch is a
/// toast, never a panic.
pub fn open_in_browser(url: &str) {
    if can_launch(url) {
        crate::ui::open_link(url, Some(crate::ui::window().upcast_ref()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(favorite: bool, has_intro_length: bool, http_source: bool, configurable_source: bool) -> Entry {
        Entry { favorite, has_intro_length, http_source, configurable_source }
    }

    #[test]
    fn a_plain_entry_offers_refresh_only() {
        assert_eq!(items(entry(false, true, false, false)), vec![Item::Refresh]);
        assert_eq!(items(entry(false, false, false, false)), vec![Item::Refresh]);
    }

    #[test]
    fn a_favorite_with_an_intro_length_offers_the_library_items_in_android_order() {
        assert_eq!(
            items(entry(true, true, true, true)),
            vec![Item::ChangeIntro, Item::Refresh, Item::EditCategories, Item::Migrate, Item::Share, Item::OpenInBrowser, Item::Settings],
        );
    }

    #[test]
    fn a_manga_and_a_season_entry_never_offer_the_intro_length() {
        assert_eq!(items(entry(true, false, false, false)), vec![Item::Refresh, Item::EditCategories, Item::Migrate]);
        assert_eq!(
            items(entry(true, false, true, true)),
            vec![Item::Refresh, Item::EditCategories, Item::Migrate, Item::Share, Item::OpenInBrowser, Item::Settings],
        );
    }

    #[test]
    fn settings_shows_only_for_a_configurable_source_and_share_and_open_only_for_http() {
        assert_eq!(items(entry(false, true, false, true)), vec![Item::Refresh, Item::Settings]);
        assert_eq!(items(entry(false, true, true, false)), vec![Item::Refresh, Item::Share, Item::OpenInBrowser]);
    }

    #[test]
    fn only_web_addresses_launch() {
        assert!(can_launch("https://a.example/x") && can_launch("HTTP://a.example"));
        assert!(!can_launch("/relative/path") && !can_launch("file:///x") && !can_launch(""));
    }
}
