//! The source header bar: the display-mode button and its two radio items, and the overflow entries (T-40a-002).
//! The mode is held for the session only: the stored display-mode setting is cut (`prefs::BrowsePrefs::display_mode`).

use kuro_i18n::{MsgKey, tr};
use kuro_prefs::library::LibraryDisplayMode;

/// The two radio items of the display-mode popover, in order: grid and list. The comfortable grid is cut (two modes only).
pub const DISPLAY_MENU: [LibraryDisplayMode; 2] = [LibraryDisplayMode::CompactGrid, LibraryDisplayMode::List];

/// Overflow entries of the source header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowItem {
    /// The local source's help page. Left out while no Kurobayomi help page exists.
    Help,
    /// Any other source: open its site in the web view.
    OpenInWebView,
    /// Only for sources that have settings.
    Settings,
}

/// Overflow of a source header: help for the local source, web view for others, and settings when configurable.
pub fn overflow_items(is_local: bool, configurable: bool) -> Vec<OverflowItem> {
    overflow_items_with(is_local, configurable, super::nav::help_url().is_some())
}

/// `has_help` is false while there is no help page: the local source then has no Help entry.
fn overflow_items_with(is_local: bool, configurable: bool, has_help: bool) -> Vec<OverflowItem> {
    let mut items = match (is_local, has_help) {
        (true, true) => vec![OverflowItem::Help],
        (true, false) => vec![],
        (false, _) => vec![OverflowItem::OpenInWebView],
    };
    if configurable {
        items.push(OverflowItem::Settings);
    }
    items
}

/// The label of a radio item of the display-mode popover.
pub fn display_label(mode: LibraryDisplayMode) -> String {
    tr(match mode {
        LibraryDisplayMode::ComfortableGrid => MsgKey::action_display_comfortable_grid,
        LibraryDisplayMode::CompactGrid => MsgKey::action_display_grid,
        LibraryDisplayMode::List => MsgKey::action_display_list,
        LibraryDisplayMode::CoverOnlyGrid => MsgKey::action_display_cover_only_grid,
    })
}

/// The label of an overflow entry.
pub fn overflow_label(item: OverflowItem) -> String {
    tr(match item {
        OverflowItem::Help => MsgKey::label_help,
        OverflowItem::OpenInWebView => MsgKey::action_open_in_web_view,
        OverflowItem::Settings => MsgKey::label_settings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_entry_has_a_label() {
        for mode in DISPLAY_MENU {
            assert!(!display_label(mode).is_empty());
        }
        for item in [OverflowItem::Help, OverflowItem::OpenInWebView, OverflowItem::Settings] {
            assert!(!overflow_label(item).is_empty());
        }
        let labels: Vec<String> = DISPLAY_MENU.iter().map(|m| display_label(*m)).collect();
        assert_eq!(labels.len(), 2);
        assert!(labels[0] != labels[1], "the two radio items read differently");
    }

    #[test]
    fn the_popover_has_exactly_two_items_in_order() {
        assert_eq!(DISPLAY_MENU, [LibraryDisplayMode::CompactGrid, LibraryDisplayMode::List]);
    }

    #[test]
    fn overflow_has_help_for_local_and_web_view_otherwise_plus_settings_when_configurable() {
        assert_eq!(overflow_items_with(true, false, true), vec![OverflowItem::Help]);
        assert_eq!(overflow_items_with(false, false, true), vec![OverflowItem::OpenInWebView]);
        assert_eq!(overflow_items_with(false, true, true), vec![OverflowItem::OpenInWebView, OverflowItem::Settings]);
        // no help page yet: the local source has no Help entry, every other list is unchanged
        assert!(overflow_items_with(true, false, false).is_empty());
        assert_eq!(overflow_items_with(true, true, false), vec![OverflowItem::Settings]);
        assert!(!overflow_items(true, false).contains(&OverflowItem::Help));
    }
}
