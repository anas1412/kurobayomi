//! Top toolbar of the manga page (T-38a-019): the title and background fade, the overflow menu order and the action
//! mode menu. Upstream: `presentation/entries/components/EntryToolbar.kt` and `MangaScreen.kt:295-308, 567-568`.
//! Pure: the widget reads these and sets the opacity or the menu items.

/// The overflow entries in the order they are shown (behaviour: Refresh, Edit categories, Migrate, Share, Settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overflow {
    Refresh,
    EditCategories,
    Migrate,
    Share,
    Settings,
}

/// Refresh is always there. The rest depend on the entry and its source.
pub fn overflow_items(favorite: bool, http_source: bool, configurable_source: bool) -> Vec<Overflow> {
    let mut items = vec![Overflow::Refresh];
    if favorite {
        items.push(Overflow::EditCategories);
        items.push(Overflow::Migrate);
    }
    if http_source {
        items.push(Overflow::Share);
    }
    if configurable_source {
        items.push(Overflow::Settings);
    }
    items
}

/// The title is opaque only in the small layout, once the first item has scrolled off. The large layout is always
/// opaque.
pub fn title_opaque(small_layout: bool, first_item_visible: bool) -> bool {
    !small_layout || !first_item_visible
}

/// The background is opaque in the small layout once the first item is off screen or the list has scrolled.
pub fn background_opaque(small_layout: bool, first_item_visible: bool, scrolled: bool) -> bool {
    !small_layout || !first_item_visible || scrolled
}

/// The action mode menu (selection count above zero) offers only select all and select inverse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionMenu {
    Normal,
    Selection,
}

pub fn action_menu(selected_visible: usize) -> ActionMenu {
    if selected_visible > 0 { ActionMenu::Selection } else { ActionMenu::Normal }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_order_and_conditions() {
        assert_eq!(overflow_items(false, false, false), vec![Overflow::Refresh]);
        assert_eq!(
            overflow_items(true, true, true),
            vec![Overflow::Refresh, Overflow::EditCategories, Overflow::Migrate, Overflow::Share, Overflow::Settings]
        );
        assert_eq!(overflow_items(false, true, false), vec![Overflow::Refresh, Overflow::Share]);
    }

    #[test]
    fn title_and_background_fade_only_in_the_small_layout() {
        assert!(!title_opaque(true, true), "small, first item visible: transparent");
        assert!(title_opaque(true, false), "small, first item gone: opaque");
        assert!(title_opaque(false, true), "large layout is always opaque");
        assert!(!background_opaque(true, true, false));
        assert!(background_opaque(true, true, true), "scrolled makes it opaque");
        assert!(background_opaque(false, true, false));
    }

    #[test]
    fn selection_switches_the_menu() {
        assert_eq!(action_menu(0), ActionMenu::Normal);
        assert_eq!(action_menu(2), ActionMenu::Selection);
    }
}
