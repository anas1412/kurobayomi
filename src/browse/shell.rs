//! The Browse tab: the anime and manga source lists behind one header row, with the global search entry and the header
//! actions (T-40a-001, docs/design/other-tabs.md). The tab has no extensions or migrate pages: a source comes with its
//! repository link, and the library is the only place that migrates.
//!
//! The header has the Anime | Manga switch at the start, the search entry in the middle and the filter and the menu at
//! the end. Enter in the search entry runs a global search of the visible kind, and so does re-selecting the tab.

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

use super::glue::keep_alive;
use super::keys::{BrowseShortcut, EscapeStep, browse_shortcut, escape_step};
use super::route::{BrowseKind, BrowsePage, BrowseRoute, reselect_route};

/// What the tab needs from the app.
pub struct ShellDeps {
    /// The two source pages, anime then manga (`BrowsePage::AnimeSources`, `BrowsePage::MangaSources`).
    pub pages: Vec<(BrowsePage, gtk::Widget)>,
    /// A route to open: global search from the entry, or from re-selecting the tab.
    pub on_route: Rc<dyn Fn(BrowseRoute)>,
    /// The filter button, for the visible page: its kind's language filter.
    pub on_filter: Rc<dyn Fn(BrowsePage)>,
    /// The menu's repository entry, for the visible kind: where source repository links are added.
    pub on_repos: Rc<dyn Fn(BrowseKind)>,
    /// Escape with an empty search: back out of the Browse page.
    pub on_leave: Rc<dyn Fn()>,
}

/// The pages of the tab, in tab order: anime (switch 0) and manga (switch 1).
const SOURCE_PAGES: [BrowsePage; 2] = [BrowsePage::AnimeSources, BrowsePage::MangaSources];

struct Inner {
    stack: gtk::Stack,
    search: gtk::SearchEntry,
    kinds: [gtk::ToggleButton; 2],
    deps: ShellDeps,
    ready: Cell<bool>,
    /// Set while the switch is moved to match the page, so its handler does not select again.
    syncing: Cell<bool>,
}

/// The tab shell. `root` is the whole page, header included; `end` stays empty because the header is in `root`.
pub struct BrowseShell {
    pub root: gtk::Box,
    pub end: Vec<gtk::Widget>,
    inner: Rc<Inner>,
}

/// A row of linked toggle buttons that act as one switch.
fn switch(labels: &[String]) -> (gtk::Box, Vec<gtk::ToggleButton>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("linked");
    let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
    for label in labels {
        let button = gtk::ToggleButton::with_label(label);
        if let Some(first) = buttons.first() {
            button.set_group(Some(first));
        }
        row.append(&button);
        buttons.push(button);
    }
    (row, buttons)
}

impl BrowseShell {
    pub fn new(deps: ShellDeps) -> Self {
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_vexpand(true);
        let mut pages = deps.pages.iter().map(|(page, widget)| (*page, widget.clone())).collect::<Vec<_>>();
        pages.sort_by_key(|(page, _)| page.index());
        for (page, widget) in &pages {
            stack.add_titled(widget, Some(&page.index().to_string()), &page.title());
        }

        let (kind_row, kinds) = switch(&[tr(MsgKey::label_anime), tr(MsgKey::label_manga)]);
        let search = gtk::SearchEntry::new();
        search.set_width_chars(28);

        let header = crate::ui::header_bar();
        header.set_decoration_layout(Some(&crate::ui::decoration(false)));
        header.pack_start(&kind_row);
        header.set_title_widget(Some(&search));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&header);
        root.append(&stack);

        let inner = Rc::new(Inner {
            stack,
            search,
            kinds: [kinds[0].clone(), kinds[1].clone()],
            deps,
            ready: Cell::new(false),
            syncing: Cell::new(false),
        });
        keep_alive(&root, &inner);
        let end = inner.header_actions();
        for widget in end.iter().rev() {
            header.pack_end(widget);
        }
        crate::ui::no_sticky_focus(&header);

        for button in &inner.kinds {
            let weak = Rc::downgrade(&inner);
            button.connect_toggled(move |button| {
                if let Some(inner) = weak.upgrade()
                    && button.is_active()
                    && !inner.syncing.get()
                {
                    inner.select(inner.switched());
                }
            });
        }
        let weak = Rc::downgrade(&inner);
        inner.stack.connect_visible_child_name_notify(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.page_changed();
            }
        });
        // Enter searches every source of the visible kind (the global search is the way in, not a filter of the list)
        let (weak, search) = (Rc::downgrade(&inner), inner.search.clone());
        search.connect_activate(move |entry| {
            if let Some(inner) = weak.upgrade() {
                inner.search_globally(&entry.text());
            }
        });
        // Ctrl+F, Escape and Ctrl+1 and Ctrl+2 reach the shell from anywhere on the tab (T-40a-035)
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&inner);
        keys.connect_key_pressed(move |_, key, _, modifiers| match weak.upgrade() {
            Some(inner) if inner.key(key, modifiers) => glib::Propagation::Stop,
            _ => glib::Propagation::Proceed,
        });
        root.add_controller(keys);
        inner.ready.set(true);
        inner.page_changed();
        Self { root, end: Vec::new(), inner }
    }

    /// The tab that is showing.
    pub fn visible(&self) -> BrowsePage {
        self.inner.visible()
    }
}

impl Inner {
    fn visible(&self) -> BrowsePage {
        self.stack
            .visible_child_name()
            .and_then(|name| name.parse::<usize>().ok())
            .and_then(|index| BrowsePage::ALL.get(index).copied())
            .unwrap_or(BrowsePage::AnimeSources)
    }

    /// The page the switch points at.
    fn switched(&self) -> BrowsePage {
        SOURCE_PAGES[usize::from(self.kinds[1].is_active())]
    }

    /// One key of the tab. True when it was ours.
    fn key(&self, key: gtk::gdk::Key, modifiers: gtk::gdk::ModifierType) -> bool {
        if key == gtk::gdk::Key::Escape {
            return match escape_step(!self.search.text().is_empty()) {
                EscapeStep::CloseSearch => {
                    self.search.set_text("");
                    true
                }
                EscapeStep::Leave => {
                    (self.deps.on_leave)();
                    true
                }
            };
        }
        match browse_shortcut(key, modifiers) {
            Some(BrowseShortcut::FocusSearch) => {
                self.search.grab_focus();
                true
            }
            Some(BrowseShortcut::SubTab(n)) if (1..=2).contains(&n) => {
                self.select(SOURCE_PAGES[usize::from(n) - 1]);
                true
            }
            _ => false,
        }
    }

    fn select(&self, page: BrowsePage) {
        self.stack.set_visible_child_name(&page.index().to_string());
    }

    /// Follows the page now showing: the switch and the entry's hint.
    fn page_changed(&self) {
        if !self.ready.get() {
            return;
        }
        let page = self.visible();
        self.syncing.set(true);
        self.kinds[usize::from(page.kind() == BrowseKind::Manga)].set_active(true);
        self.syncing.set(false);
        self.search.set_placeholder_text(Some(&tr(MsgKey::action_global_search_hint)));
    }

    /// Enter in the entry: a global search of the visible kind for the text.
    fn search_globally(&self, text: &str) {
        (self.deps.on_route)(BrowseRoute::GlobalSearch { kind: self.visible().kind(), source_id: None, query: text.trim().to_owned() });
        self.search.set_text("");
    }

    fn header_actions(self: &Rc<Self>) -> Vec<gtk::Widget> {
        let filter = crate::ui::icon_button("view-sort-descending-symbolic", &tr(MsgKey::action_filter));
        let weak = Rc::downgrade(self);
        filter.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                (inner.deps.on_filter)(inner.visible());
            }
        });

        let entries = vec![tr(MsgKey::action_global_search), tr(MsgKey::label_extension_repos)];
        let weak = Rc::downgrade(self);
        let menu = crate::ui::menu_button("open-menu-symbolic", &entries, move |label| {
            let Some(inner) = weak.upgrade() else { return };
            if label == tr(MsgKey::action_global_search) {
                (inner.deps.on_route)(reselect_route(inner.visible()));
            } else {
                (inner.deps.on_repos)(inner.visible().kind());
            }
        });
        menu.set_tooltip_text(Some(&tr(MsgKey::action_menu)));
        vec![filter.upcast(), menu.upcast()]
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use super::*;

    struct Log {
        routes: Rc<RefCell<Vec<BrowseRoute>>>,
        left: Rc<RefCell<u32>>,
        repos: Rc<RefCell<Vec<BrowseKind>>>,
    }

    fn shell() -> (BrowseShell, Log) {
        let log = Log { routes: Rc::default(), left: Rc::default(), repos: Rc::default() };
        let (routes, left, repos) = (Rc::clone(&log.routes), Rc::clone(&log.left), Rc::clone(&log.repos));
        let pages = SOURCE_PAGES.iter().map(|p| (*p, gtk::Label::new(Some(&p.title())).upcast::<gtk::Widget>())).collect();
        let deps = ShellDeps {
            pages,
            on_route: Rc::new(move |route| routes.borrow_mut().push(route)),
            on_filter: Rc::new(|_| {}),
            on_repos: Rc::new(move |kind| repos.borrow_mut().push(kind)),
            on_leave: Rc::new(move || *left.borrow_mut() += 1),
        };
        (BrowseShell::new(deps), log)
    }

    fn walk(w: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
        let mut child = w.first_child();
        while let Some(c) = child {
            out.push(c.clone());
            walk(&c, out);
            child = c.next_sibling();
        }
    }

    /// The tab has the two source pages and nothing else: no extensions, no migrate, one search in the header.
    fn the_tab_has_only_the_two_source_pages_and_one_header_search() {
        let (shell, _log) = shell();
        let names: Vec<String> = shell
            .inner
            .stack
            .pages()
            .iter::<gtk::StackPage>()
            .map(|page| page.expect("page").name().map(|n| n.to_string()).unwrap_or_default())
            .collect();
        assert_eq!(names, ["0", "1"], "anime sources, manga sources");
        let header = shell.root.first_child().and_downcast::<gtk::HeaderBar>().expect("the header row comes first");
        let mut all = Vec::new();
        walk(header.upcast_ref(), &mut all);
        assert_eq!(all.iter().filter(|w| w.is::<gtk::SearchEntry>()).count(), 1, "one search, in the header");
        assert!(!all.iter().any(|w| w.downcast_ref::<gtk::Label>().is_some_and(|l| l.label() == "Browse")), "no page title");
        assert!(shell.end.is_empty(), "the header is part of the page");
    }

    fn enter_runs_a_global_search_of_the_visible_kind() {
        let (shell, log) = shell();
        shell.inner.select(BrowsePage::MangaSources);
        assert_eq!(shell.inner.search.placeholder_text().as_deref(), Some(tr(MsgKey::action_global_search_hint).as_str()));
        shell.inner.search.set_text("  berserk ");
        shell.inner.search.emit_by_name::<()>("activate", &[]);
        assert_eq!(
            log.routes.borrow().last(),
            Some(&BrowseRoute::GlobalSearch { kind: BrowseKind::Manga, source_id: None, query: "berserk".into() }),
            "the text is trimmed and every source of the kind is searched"
        );
        assert!(shell.inner.search.text().is_empty(), "the entry is cleared after the search");
    }

    fn the_switch_follows_the_page_and_ctrl_digits_pick_only_the_two_pages() {
        use gtk::gdk::{Key, ModifierType};
        let (shell, _log) = shell();
        let ctrl = ModifierType::CONTROL_MASK;
        assert!(shell.inner.key(Key::_2, ctrl));
        assert_eq!(shell.visible(), BrowsePage::MangaSources, "Ctrl+2 is the manga sources");
        assert!(shell.inner.kinds[1].is_active(), "the switch follows the page");
        assert!(shell.inner.key(Key::_1, ctrl));
        assert_eq!(shell.visible(), BrowsePage::AnimeSources);
        assert!(shell.inner.kinds[0].is_active());
        assert!(!shell.inner.key(Key::_3, ctrl), "there is no third page to pick");
        assert_eq!(shell.visible(), BrowsePage::AnimeSources);
        shell.inner.kinds[1].set_active(true);
        assert_eq!(shell.visible(), BrowsePage::MangaSources, "the switch picks the page");
    }

    fn ctrl_f_and_escape_drive_the_search_then_leave() {
        use gtk::gdk::{Key, ModifierType};
        let (shell, log) = shell();
        assert!(shell.inner.key(Key::f, ModifierType::CONTROL_MASK), "Ctrl+F goes to the search on every page");
        shell.inner.search.set_text("x");
        assert!(shell.inner.key(Key::Escape, ModifierType::empty()));
        assert!(shell.inner.search.text().is_empty(), "the first Escape clears the search");
        assert_eq!(*log.left.borrow(), 0);
        assert!(shell.inner.key(Key::Escape, ModifierType::empty()));
        assert_eq!(*log.left.borrow(), 1, "the second Escape leaves");
    }

    /// The header menu has two entries: global search, and the repository entry of the visible kind.
    fn the_header_menu_routes_by_its_label() {
        let (shell, log) = shell();
        let mut all = Vec::new();
        walk(shell.root.upcast_ref(), &mut all);
        let menu = all.iter().find_map(|w| w.downcast_ref::<gtk::MenuButton>().cloned()).expect("the header menu");
        let mut entries = Vec::new();
        walk(menu.popover().expect("the menu").upcast_ref(), &mut entries);
        let press = |key: MsgKey| {
            let label = tr(key);
            let entry = entries.iter().find_map(|w| w.downcast_ref::<gtk::Button>().filter(|b| b.label().as_deref() == Some(label.as_str())).cloned());
            entry.unwrap_or_else(|| panic!("no menu entry {label}")).emit_clicked();
        };
        shell.inner.select(BrowsePage::MangaSources);
        press(MsgKey::action_global_search);
        assert_eq!(log.routes.borrow().last(), Some(&BrowseRoute::GlobalSearch { kind: BrowseKind::Manga, source_id: None, query: String::new() }));
        press(MsgKey::label_extension_repos);
        shell.inner.select(BrowsePage::AnimeSources);
        press(MsgKey::label_extension_repos);
        assert_eq!(*log.repos.borrow(), [BrowseKind::Manga, BrowseKind::Anime], "the repository entry follows the visible kind");
    }

    pub(crate) fn smoke() {
        the_tab_has_only_the_two_source_pages_and_one_header_search();
        enter_runs_a_global_search_of_the_visible_kind();
        the_switch_follows_the_page_and_ctrl_digits_pick_only_the_two_pages();
        ctrl_f_and_escape_drive_the_search_then_leave();
        the_header_menu_routes_by_its_label();
    }
}
