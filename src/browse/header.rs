//! The source header bar: the display-mode button and its three radio items, and the overflow entries (T-40a-002).
//! The stored mode itself lives in the preferences (`prefs::BrowsePrefs::display_mode`).

use kuro_i18n::{MsgKey, tr};
use kuro_prefs::library::LibraryDisplayMode;

/// The three radio items of the display-mode popover, in order: comfortable grid, grid (compact), list (B§2.12).
pub const DISPLAY_MENU: [LibraryDisplayMode; 3] =
    [LibraryDisplayMode::ComfortableGrid, LibraryDisplayMode::CompactGrid, LibraryDisplayMode::List];

/// Icon of the display-mode button: a list when the mode is List, a grid otherwise.
pub fn display_button_icon(mode: LibraryDisplayMode) -> &'static str {
    match mode {
        LibraryDisplayMode::List => "view-list-symbolic",
        _ => "view-grid-symbolic",
    }
}

/// The selected radio item. Cover-only grid is not in the popover, so no item is selected for it.
pub fn selected_radio(mode: LibraryDisplayMode) -> Option<usize> {
    DISPLAY_MENU.iter().position(|m| *m == mode)
}

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
        assert_eq!(labels.len(), 3);
        assert!(labels[0] != labels[1] && labels[1] != labels[2], "the three radio items read differently");
    }

    #[test]
    fn the_popover_has_exactly_three_items_in_order() {
        assert_eq!(
            DISPLAY_MENU,
            [LibraryDisplayMode::ComfortableGrid, LibraryDisplayMode::CompactGrid, LibraryDisplayMode::List]
        );
    }

    #[test]
    fn the_button_shows_a_list_only_in_list_mode() {
        assert_eq!(display_button_icon(LibraryDisplayMode::List), "view-list-symbolic");
        assert_eq!(display_button_icon(LibraryDisplayMode::CompactGrid), "view-grid-symbolic");
        assert_eq!(display_button_icon(LibraryDisplayMode::ComfortableGrid), "view-grid-symbolic");
        assert_eq!(display_button_icon(LibraryDisplayMode::CoverOnlyGrid), "view-grid-symbolic");
    }

    #[test]
    fn the_selected_radio_follows_the_mode() {
        assert_eq!(selected_radio(LibraryDisplayMode::CompactGrid), Some(1));
        assert_eq!(selected_radio(LibraryDisplayMode::List), Some(2));
        assert_eq!(selected_radio(LibraryDisplayMode::CoverOnlyGrid), None);
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
