//! Top toolbar of the manga page (T-38a-019): the title and background fade, the overflow menu order and the action
//! mode menu. Upstream: `presentation/entries/components/EntryToolbar.kt` and `MangaScreen.kt:295-308, 567-568`.
//! Pure: the widget reads these and sets the opacity or the menu items.

use crate::entry_core::detail::EntryState;
use crate::entry_core::overflow_menu;
use crate::entry_core::view::{selection_counts, view};

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

/// What the header bar of a ready page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarState {
    pub menu: ActionMenu,
    /// Every selected chapter, the ones a filter hides included (Back and Escape read this).
    pub selected_total: usize,
    /// The selected chapters the list shows: the count in the action mode title.
    pub selected_visible: usize,
    /// The Download menu is there for a manga that is not local.
    pub download_menu: bool,
    /// A filter is on: the Filter button is tinted (the forced downloaded-only filter counts).
    pub filter_active: bool,
    /// The overflow entries, in order (`overflow_menu`).
    pub overflow: Vec<overflow_menu::Item>,
}

/// The header bar state of `page`. `http_source` and `configurable_source` come from the source, which the page does
/// not hold: Share needs an HTTP source and Settings a configurable one.
pub fn bar_state(page: &EntryState, http_source: bool, configurable_source: bool) -> BarState {
    let ids: Vec<i64> = page.selection.ids().collect();
    let shown = view(&page.chapters, page.manga.flags, page.downloaded_only, page.manga.local);
    let (selected_total, selected_visible) = selection_counts(&ids, &shown);
    BarState {
        menu: action_menu(selected_visible),
        selected_total,
        selected_visible,
        download_menu: !page.manga.local,
        filter_active: page.manga.flags.filter_active(page.downloaded_only),
        overflow: overflow_menu::items(overflow_menu::Entry {
            favorite: page.manga.favorite,
            has_intro_length: false,
            http_source,
            configurable_source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_core::chapter::ChapterRow;
    use crate::entry_core::detail::{LoadContext, LoadState, MangaRow, SourceInfo, finish_load};
    use crate::entry_core::flags::ChapterFlags;

    fn page(flags: i64, local: bool, favorite: bool, rows: Vec<ChapterRow>) -> Box<EntryState> {
        let manga = MangaRow {
            id: 1,
            next_update: 0,
            fetch_interval: 0,
            source: 1,
            url: String::new(),
            title: "t".into(),
            author: None,
            artist: None,
            description: None,
            genres: vec![],
            status: 0,
            thumbnail_url: None,
            cover_last_modified: 0,
            initialized: true,
            favorite,
            local,
            flags: ChapterFlags(flags),
        };
        match finish_load(Some(manga), rows, SourceInfo::default(), &LoadContext::default()).state {
            LoadState::Ready(page) => page,
            _ => unreachable!(),
        }
    }

    #[test]
    fn the_bar_follows_the_page() {
        let rows = vec![ChapterRow::new(1, "a", 1.0), ChapterRow { read: true, ..ChapterRow::new(2, "b", 2.0) }];
        let mut p = page(0x200, false, true, rows.clone());
        let bar = bar_state(&p, false, false);
        assert_eq!((bar.menu, bar.download_menu, bar.filter_active), (ActionMenu::Normal, true, false));
        assert_eq!(bar.overflow, vec![overflow_menu::Item::Refresh, overflow_menu::Item::EditCategories, overflow_menu::Item::Migrate]);
        // the Download menu is not offered for a local manga, and a library filter tints the Filter button
        let local = page(0x200 | kuro_db::flags::item::SHOW_UNDONE, true, true, rows);
        let bar = bar_state(&local, true, true);
        assert!(!bar.download_menu && bar.filter_active);
        assert_eq!(bar.overflow, vec![overflow_menu::Item::Refresh, overflow_menu::Item::EditCategories, overflow_menu::Item::Migrate, overflow_menu::Item::Share, overflow_menu::Item::OpenInBrowser, overflow_menu::Item::Settings]);
        // a selection on a visible row switches to the action mode; one the filter hides counts in the total only
        p.selection.set_all(&[1, 2], true);
        let bar = bar_state(&p, false, false);
        assert_eq!((bar.menu, bar.selected_total, bar.selected_visible), (ActionMenu::Selection, 2, 2));
        let mut hidden = page(0x200 | kuro_db::flags::item::SHOW_UNDONE, false, true, vec![ChapterRow { read: true, ..ChapterRow::new(2, "b", 2.0) }]);
        hidden.selection.set_all(&[2], true);
        let bar = bar_state(&hidden, false, false);
        assert_eq!((bar.menu, bar.selected_total, bar.selected_visible), (ActionMenu::Normal, 1, 0));
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
