//! The header bar of a source listing: back, the source name, the shared search toggle, the display-mode button with its
//! two radio items (grid and list), and the overflow menu (T-40a-002). The rules (menu items, icon, overflow entries) are in
//! `header`. The display mode is cut from the settings: a radio selection changes the grid for this session only.

use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::library::LibraryDisplayMode;

use super::header::{DISPLAY_MENU, OverflowItem, display_label, overflow_items, overflow_label};
use super::prefs::BrowsePrefs;
use crate::widgets::view_options::{Item, Pick, Section, ViewOptions};

/// The action name of the display-mode radio group.
const DISPLAY: &str = "display";

pub struct HeaderDeps {
    pub source_name: String,
    pub prefs: Rc<BrowsePrefs>,
    pub is_local: bool,
    /// The source has settings of its own.
    pub configurable: bool,
    pub on_back: Rc<dyn Fn()>,
    /// The page's search toggle (`ui::search_row`); it goes at the end of the row, left of the other actions.
    pub search: gtk::ToggleButton,
    /// A display mode was chosen and already stored.
    pub on_display_mode: Rc<dyn Fn(LibraryDisplayMode)>,
    pub on_overflow: Rc<dyn Fn(OverflowItem)>,
}

#[allow(dead_code)] // kept for the shape of the type; not built or read yet
pub struct SourceHeader {
    pub bar: gtk::HeaderBar,
    pub search: gtk::ToggleButton,
    pub display: ViewOptions,
    pub overflow: Option<gtk::MenuButton>,
}

pub fn build(deps: HeaderDeps) -> SourceHeader {
    let bar = crate::ui::header_bar();
    bar.set_decoration_layout(Some(&crate::ui::decoration(false)));
    bar.set_title_widget(Some(&gtk::Label::builder().label(&deps.source_name).css_classes(["title"]).ellipsize(gtk::pango::EllipsizeMode::End).build()));

    bar.pack_start(&back_button(Rc::clone(&deps.on_back)));

    let overflow = overflow_menu(&deps);
    if let Some(menu) = &overflow {
        bar.pack_end(menu);
    }

    let display = display_menu(&deps);
    bar.pack_end(&display.widget);

    let search = deps.search.clone();
    bar.pack_end(&search);
    crate::ui::no_sticky_focus(&bar);

    SourceHeader { bar, search, display, overflow }
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

/// The display-mode button: the app's one view-options menu with a single radio group, grid or list. Choosing one stores
/// it and calls `on_display_mode`.
fn display_menu(deps: &HeaderDeps) -> ViewOptions {
    let items = DISPLAY_MENU.iter().map(|mode| Item::radio(DISPLAY, format!("{mode:?}"), display_label(*mode))).collect();
    let menu = ViewOptions::new(&tr(MsgKey::action_display_mode), &[Section::new("View", items)]);
    menu.set_radio(DISPLAY, &format!("{:?}", deps.prefs.display_mode()));
    let on_mode = Rc::clone(&deps.on_display_mode);
    menu.connect(move |pick| {
        if let Pick::Radio(_, value) = pick
            && let Some(mode) = DISPLAY_MENU.into_iter().find(|m| format!("{m:?}") == value)
        {
            on_mode(mode);
        }
    });
    menu
}

fn overflow_menu(deps: &HeaderDeps) -> Option<gtk::MenuButton> {
    let entries = overflow_items(deps.is_local, deps.configurable)
        .into_iter()
        .map(|item| {
            let on_overflow = Rc::clone(&deps.on_overflow);
            crate::ui::MenuEntry::new(overflow_label(item), move || on_overflow(item))
        })
        .collect();
    crate::ui::menu_button(vec![entries])
}

#[cfg(test)]
pub(crate) mod tests {
    use kuro_platform::kind::Kind;
    use std::cell::RefCell;

    use kuro_prefs::store::PrefStore;

    use super::*;

    fn deps(prefs: Rc<BrowsePrefs>, log: &Rc<RefCell<Vec<String>>>, local: bool, configurable: bool) -> HeaderDeps {
        let (back, mode, overflow) = (Rc::clone(log), Rc::clone(log), Rc::clone(log));
        HeaderDeps {
            source_name: "Source".into(),
            prefs,
            is_local: local,
            configurable,
            on_back: Rc::new(move || back.borrow_mut().push("back".into())),
            search: gtk::ToggleButton::new(),
            on_display_mode: Rc::new(move |m| mode.borrow_mut().push(format!("mode {m:?}"))),
            on_overflow: Rc::new(move |i| overflow.borrow_mut().push(format!("overflow {i:?}"))),
        }
    }

    fn header_shows_the_two_radios_and_writes_the_chosen_mode() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let prefs = Rc::new(BrowsePrefs::new(PrefStore::in_memory(), Kind::Manga));
        let header = build(deps(Rc::clone(&prefs), &log, false, true));
        assert_eq!(header.display.radio(DISPLAY).as_deref(), Some("CompactGrid"), "compact grid is selected by default");

        header.display.group().activate_action(DISPLAY, Some(&"List".to_variant()));
        assert_eq!(*log.borrow(), ["mode List"]);

        assert!(header.search.is_ancestor(&header.bar), "the search toggle is in the header");
    }

    pub(crate) fn smoke() {
        header_shows_the_two_radios_and_writes_the_chosen_mode();
    }
}
