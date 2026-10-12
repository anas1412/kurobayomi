//! The header bar of a source listing: back, the source name, the search toggle, the display-mode button with its
//! two radio items (grid and list), and the overflow menu (T-40a-002). The rules (menu items, icon, overflow entries) are in
//! `header`. The display mode is cut from the settings: a radio selection changes the grid for this session only.

use std::rc::Rc;
use kuro_platform::kind::Kind;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::library::LibraryDisplayMode;

use super::header::{DISPLAY_MENU, OverflowItem, display_button_icon, display_label, overflow_items, overflow_label, selected_radio};
use super::prefs::BrowsePrefs;

pub struct HeaderDeps {
    pub source_name: String,
    pub prefs: Rc<BrowsePrefs>,
    pub is_local: bool,
    /// The source has settings of its own.
    pub configurable: bool,
    pub on_back: Rc<dyn Fn()>,
    /// The search toggle changed.
    pub on_search: Rc<dyn Fn(bool)>,
    /// A display mode was chosen and already stored.
    pub on_display_mode: Rc<dyn Fn(LibraryDisplayMode)>,
    pub on_overflow: Rc<dyn Fn(OverflowItem)>,
}

pub struct SourceHeader {
    pub bar: gtk::HeaderBar,
    pub search: gtk::ToggleButton,
    pub display: gtk::MenuButton,
    pub overflow: gtk::MenuButton,
    pub radios: Vec<gtk::CheckButton>,
}

pub fn build(deps: HeaderDeps) -> SourceHeader {
    let bar = crate::ui::header_bar();
    bar.set_decoration_layout(Some(&crate::ui::decoration(false)));
    bar.set_title_widget(Some(&gtk::Label::builder().label(&deps.source_name).css_classes(["title"]).ellipsize(gtk::pango::EllipsizeMode::End).build()));

    bar.pack_start(&back_button(Rc::clone(&deps.on_back)));

    let overflow = overflow_menu(&deps);
    bar.pack_end(&overflow);

    let (display, radios) = display_menu(&deps);
    bar.pack_end(&display);

    let search = gtk::ToggleButton::new();
    search.set_icon_name("system-search-symbolic");
    search.set_tooltip_text(Some(&tr(MsgKey::action_search)));
    let on_search = Rc::clone(&deps.on_search);
    search.connect_toggled(move |button| on_search(button.is_active()));
    bar.pack_end(&search);
    crate::ui::no_sticky_focus(&bar);

    SourceHeader { bar, search, display, overflow, radios }
}

/// The header of a page that has nothing but its title and a way back: the missing-source page (T-41a-008).
pub fn stub_header(title: &str, on_back: Rc<dyn Fn()>) -> gtk::HeaderBar {
    let bar = crate::ui::header_bar();
    bar.set_decoration_layout(Some(&crate::ui::decoration(false)));
    bar.set_title_widget(Some(&gtk::Label::builder().label(title).css_classes(["title"]).ellipsize(gtk::pango::EllipsizeMode::End).build()));
    bar.pack_start(&back_button(on_back));
    crate::ui::no_sticky_focus(&bar);
    bar
}

/// The Back button of both headers: the translated tooltip, which is also its accessible name.
fn back_button(on_back: Rc<dyn Fn()>) -> gtk::Button {
    let back = crate::ui::icon_button("go-previous-symbolic", &tr(MsgKey::action_webview_back));
    back.connect_clicked(move |_| on_back());
    back
}

/// The display-mode button and its popover of two radio items. Choosing one stores it, updates the icon and calls
/// `on_display_mode`.
fn display_menu(deps: &HeaderDeps) -> (gtk::MenuButton, Vec<gtk::CheckButton>) {
    let mode = deps.prefs.display_mode();
    let button = gtk::MenuButton::new();
    button.set_icon_name(display_button_icon(mode));
    button.set_tooltip_text(Some(&tr(MsgKey::action_display_mode)));
    let popover = gtk::Popover::new();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let mut radios: Vec<gtk::CheckButton> = Vec::new();
    for item in DISPLAY_MENU {
        let radio = gtk::CheckButton::with_label(&display_label(item));
        if let Some(first) = radios.first() {
            radio.set_group(Some(first));
        }
        radios.push(radio.clone());
        column.append(&radio);
    }
    // set after grouping, so selecting the current mode does not fire the change
    if let Some(index) = selected_radio(mode) {
        radios[index].set_active(true);
    }
    for (radio, item) in radios.iter().zip(DISPLAY_MENU) {
        let (on_mode, button, popover) = (Rc::clone(&deps.on_display_mode), button.clone(), popover.clone());
        radio.connect_toggled(move |radio| {
            if !radio.is_active() {
                return;
            }
            button.set_icon_name(display_button_icon(item));
            popover.popdown();
            on_mode(item);
        });
    }
    popover.set_child(Some(&column));
    button.set_popover(Some(&popover));
    (button, radios)
}

fn overflow_menu(deps: &HeaderDeps) -> gtk::MenuButton {
    let items = overflow_items(deps.is_local, deps.configurable);
    let labels: Vec<String> = items.iter().map(|item| overflow_label(*item)).collect();
    let on_overflow = Rc::clone(&deps.on_overflow);
    let button = crate::ui::menu_button("view-more-symbolic", &labels, move |label| {
        if let Some(item) = items.iter().find(|item| overflow_label(**item) == label) {
            on_overflow(*item);
        }
    });
    button.set_tooltip_text(Some(&tr(MsgKey::action_menu_overflow_description)));
    button
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use kuro_prefs::store::PrefStore;

    use super::*;

    fn deps(prefs: Rc<BrowsePrefs>, log: &Rc<RefCell<Vec<String>>>, local: bool, configurable: bool) -> HeaderDeps {
        let (back, search, mode, overflow) = (Rc::clone(log), Rc::clone(log), Rc::clone(log), Rc::clone(log));
        HeaderDeps {
            source_name: "Source".into(),
            prefs,
            is_local: local,
            configurable,
            on_back: Rc::new(move || back.borrow_mut().push("back".into())),
            on_search: Rc::new(move |on| search.borrow_mut().push(format!("search {on}"))),
            on_display_mode: Rc::new(move |m| mode.borrow_mut().push(format!("mode {m:?}"))),
            on_overflow: Rc::new(move |i| overflow.borrow_mut().push(format!("overflow {i:?}"))),
        }
    }

    fn header_shows_the_two_radios_and_writes_the_chosen_mode() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let prefs = Rc::new(BrowsePrefs::new(PrefStore::in_memory(), Kind::Manga));
        let header = build(deps(Rc::clone(&prefs), &log, false, true));
        assert_eq!(header.radios.len(), 2, "the popover has two items");
        assert!(header.radios[0].is_active(), "compact grid is selected by default");
        assert_eq!(header.display.icon_name().as_deref(), Some("view-grid-symbolic"));

        header.radios[1].set_active(true);
        assert_eq!(header.display.icon_name().as_deref(), Some("view-list-symbolic"));
        assert_eq!(*log.borrow(), ["mode List"]);

        header.search.set_active(true);
        assert_eq!(log.borrow().last().map(String::as_str), Some("search true"));
    }

    pub(crate) fn smoke() {
        header_shows_the_two_radios_and_writes_the_chosen_mode();
    }
}
