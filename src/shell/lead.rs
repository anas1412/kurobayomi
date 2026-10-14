//! The open tab's search toggle and ⋮ menu in the sidebar's header (user, 2026-10-14). Every tab page leads its own
//! header with them (`ui::pack_lead`), which is where they stay in a narrow window. While the sidebar shows a tab page,
//! both move into the sidebar's header around the app name: the search before it, the menu after it. Switching tabs,
//! pushing a page or narrowing the window puts them back first.

use std::cell::RefCell;

use gtk::prelude::*;
use gtk::Box as GBox;

thread_local! {
    /// The two places in the sidebar's header: before the app name (search) and after it (menu).
    static HOSTS: (GBox, GBox) = (GBox::new(gtk::Orientation::Horizontal, 0), GBox::new(gtk::Orientation::Horizontal, 0));
    /// The slots now in the sidebar's header, each with the lead box it came from and whether it goes first there.
    static AWAY: RefCell<Vec<(gtk::Widget, GBox, bool)>> = const { RefCell::new(Vec::new()) };
}

/// The sidebar header's places for the search (before the app name) and the menu (after it).
pub fn hosts() -> (GBox, GBox) {
    HOSTS.with(|(search, menu)| (search.clone(), menu.clone()))
}

/// The lead box of a tab page: in the header that starts the page.
fn lead_of(page: &gtk::Widget) -> Option<GBox> {
    fn find(widget: &gtk::Widget) -> Option<GBox> {
        if widget.has_css_class(crate::ui::LEAD) {
            return widget.clone().downcast().ok();
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            if let Some(found) = find(&c) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }
    let header = page.first_child().filter(|w| w.is::<gtk::HeaderBar>())?;
    find(&header)
}

/// Puts the slots of `page` (the visible tab page, when one is on screen) in the sidebar's header when `wide`, and
/// anything there before back into its own page.
pub fn place(page: Option<&gtk::Widget>, wide: bool) {
    for (slot, home, first) in AWAY.with(|away| std::mem::take(&mut *away.borrow_mut())) {
        if let Some(parent) = slot.parent().and_downcast::<GBox>() {
            parent.remove(&slot);
        }
        if first { home.prepend(&slot) } else { home.append(&slot) }
        // the page being left still fades out on screen: it must not flash its slots back in its own header
        home.set_visible(!wide);
    }
    let Some(lead) = page.and_then(lead_of) else { return };
    // an empty slot box still takes its spacing in the header, so while the sidebar holds the slots every page's box is
    // hidden, the page on screen and the one fading out alike: the Anime | Manga switch then keeps its place across a
    // change of tab
    lead.set_visible(!wide);
    if !wide {
        return;
    }
    let (search_host, menu_host) = hosts();
    for (class, host, first) in [(crate::ui::LEAD_SEARCH, search_host, true), (crate::ui::LEAD_MENU, menu_host, false)] {
        let mut child = lead.first_child();
        while let Some(slot) = child {
            child = slot.next_sibling();
            if slot.has_css_class(class) {
                lead.remove(&slot);
                host.append(&slot);
                AWAY.with(|away| away.borrow_mut().push((slot, lead.clone(), first)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slots_go_to_the_sidebar_when_wide_and_come_back_in_order() {
        crate::testing::with_display(|| {
            let page = GBox::new(gtk::Orientation::Vertical, 0);
            let header = gtk::HeaderBar::new();
            page.append(&header);
            let (search, menu) = (gtk::ToggleButton::new(), gtk::Button::new());
            crate::ui::pack_lead(&header, Some(search.upcast_ref()), Some(menu.upcast_ref()));
            let lead = lead_of(page.upcast_ref()).expect("the lead box");
            place(Some(page.upcast_ref()), true);
            let (search_host, menu_host) = hosts();
            assert!(search.is_ancestor(&search_host) && menu.is_ancestor(&menu_host), "both in the sidebar's header");
            assert!(lead.first_child().is_none());
            place(Some(page.upcast_ref()), false);
            assert!(search.is_ancestor(&lead) && menu.is_ancestor(&lead), "back home");
            assert!(lead.first_child().expect("a slot").has_css_class(crate::ui::LEAD_SEARCH), "the search first again");
            place(Some(page.upcast_ref()), true);
            let other = GBox::new(gtk::Orientation::Vertical, 0);
            place(Some(other.upcast_ref()), true);
            assert!(search.is_ancestor(&lead) && !lead.is_visible(), "a page left while wide keeps its slots hidden");
            place(Some(page.upcast_ref()), true);
            assert!(!lead.is_visible(), "the page on screen has its empty box hidden too, so no page leaves a gap");
            place(Some(page.upcast_ref()), false);
            assert!(lead.is_visible(), "shown again once it is the page on screen");
            place(None, true);
            assert!(search.is_ancestor(&lead), "no page on screen: nothing moves");
        });
    }
}
