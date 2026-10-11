//! The Browse tab: six pages in Aniyomi's order behind one header row and a section switch, the capacity-1 channel that
//! switches them, the shared search entry, and the header actions (T-40a-001, docs/design/other-tabs.md).
//!
//! The header has the Anime | Manga switch at the start, the search entry in the middle and the visible page's actions
//! at the end; the Sources | Extensions | Migrate switch sits under it. The two switches pick one of the six pages. On
//! an extensions page the entry filters the list as you type; on the other pages Enter runs a global search.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

use super::glue::{PageChannel, debounce_search, follow_pages, keep_alive};
use super::keys::{BrowseShortcut, EscapeStep, browse_shortcut, escape_step};
use super::route::{BrowseKind, BrowsePage, BrowseRoute, reselect_route};
use crate::library::layout::{SPACE_S, SPACE_XS};

/// What the tab needs from the app.
pub struct ShellDeps {
    /// The six pages, in `BrowsePage::ALL` order.
    pub pages: Vec<(BrowsePage, gtk::Widget)>,
    /// Header widgets of one page, shown at the end of the header while that page is visible (the extension refresh).
    pub page_actions: Vec<(BrowsePage, gtk::Widget)>,
    /// A route to open: global search from the entry, or from re-selecting the tab.
    pub on_route: Rc<dyn Fn(BrowseRoute)>,
    /// The debounced text of the shared search entry, for the visible extensions page.
    pub on_query: Rc<dyn Fn(BrowsePage, String)>,
    /// The filter button, for the visible page.
    pub on_filter: Rc<dyn Fn(BrowsePage)>,
    /// The menu's extension repos entry, for the visible kind.
    pub on_repos: Rc<dyn Fn(BrowseKind)>,
    /// F5 and Ctrl+R on an extensions page: look for extensions again.
    pub on_refresh: Rc<dyn Fn(BrowsePage)>,
    /// Ctrl+U and the menu on an extensions page: update every extension that has an update.
    pub on_update_all: Rc<dyn Fn(BrowsePage)>,
    /// Escape with an empty search: back out of the Browse page.
    pub on_leave: Rc<dyn Fn()>,
}

/// The badge of the Extensions switch, in words: `1 update`, `3 updates`; none at zero.
pub fn updates_badge(updates: usize) -> Option<String> {
    match updates {
        0 => None,
        1 => Some("1 update".to_owned()),
        n => Some(format!("{n} updates")),
    }
}

/// The three sections of the switch under the header, in tab order.
const SECTIONS: [MsgKey; 3] = [MsgKey::label_sources, MsgKey::label_extensions, MsgKey::label_migration];

/// The page of a kind (0 anime, 1 manga) and a section (0 sources, 1 extensions, 2 migrate).
fn page_at(kind: usize, section: usize) -> BrowsePage {
    BrowsePage::ALL[section * 2 + kind]
}

struct Inner {
    stack: gtk::Stack,
    search: gtk::SearchEntry,
    kinds: [gtk::ToggleButton; 2],
    sections: [gtk::ToggleButton; 3],
    /// The worded update badge on the Extensions switch, for the visible kind.
    badge: gtk::Label,
    /// The update count of each kind's extensions page (anime, manga).
    updates: Cell<[usize; 2]>,
    actions: gtk::Stack,
    channel: PageChannel,
    deps: ShellDeps,
    /// The text the visible extensions page was last given.
    sent: RefCell<String>,
    ready: Cell<bool>,
    /// Set while the switches are moved to match the page, so their handlers do not select again.
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
        let (section_row, sections) = switch(&SECTIONS.map(tr));
        section_row.set_halign(gtk::Align::Center);
        section_row.set_margin_top(SPACE_S);
        section_row.set_margin_start(SPACE_S);
        section_row.set_margin_end(SPACE_S);
        // the Extensions switch carries the update badge of the visible kind
        let badge = super::look::badge("", true);
        badge.set_visible(false);
        let extensions_label = gtk::Box::new(gtk::Orientation::Horizontal, SPACE_XS + 2);
        extensions_label.append(&gtk::Label::new(Some(&tr(MsgKey::label_extensions))));
        extensions_label.append(&badge);
        sections[1].set_child(Some(&extensions_label));

        let search = gtk::SearchEntry::new();
        search.set_width_chars(28);

        let actions = gtk::Stack::new();
        actions.set_hhomogeneous(false);
        actions.add_named(&gtk::Box::new(gtk::Orientation::Horizontal, 0), Some("none"));
        for (page, widget) in &deps.page_actions {
            actions.add_named(widget, Some(&page.index().to_string()));
        }

        let header = gtk::HeaderBar::new();
        header.set_decoration_layout(Some(&crate::ui::decoration(false)));
        header.pack_start(&kind_row);
        header.set_title_widget(Some(&search));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&header);
        root.append(&section_row);
        root.append(&stack);

        let inner = Rc::new(Inner {
            stack,
            search,
            kinds: [kinds[0].clone(), kinds[1].clone()],
            sections: [sections[0].clone(), sections[1].clone(), sections[2].clone()],
            badge,
            updates: Cell::new([0, 0]),
            actions,
            channel: PageChannel::new(),
            deps,
            sent: RefCell::new(String::new()),
            ready: Cell::new(false),
            syncing: Cell::new(false),
        });
        keep_alive(&root, &inner);
        let end = inner.header_actions();
        for widget in end.iter().rev() {
            header.pack_end(widget);
        }
        header.pack_end(&inner.actions);
        crate::ui::no_sticky_focus(&header);

        for button in inner.kinds.iter().chain(&inner.sections) {
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
        follow_pages(&inner.channel, move |page| match weak.upgrade() {
            Some(inner) => {
                inner.select(page);
                true
            }
            None => false,
        });
        let weak = Rc::downgrade(&inner);
        inner.stack.connect_visible_child_name_notify(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.page_changed();
            }
        });
        let weak = Rc::downgrade(&inner);
        debounce_search(&inner.search, move |text| {
            if let Some(inner) = weak.upgrade() {
                inner.query(text);
            }
        });
        // Enter outside the extensions pages searches every source of the visible kind
        let (weak, search) = (Rc::downgrade(&inner), inner.search.clone());
        search.connect_activate(move |entry| {
            if let Some(inner) = weak.upgrade() {
                inner.search_globally(&entry.text());
            }
        });
        // Ctrl+F, Escape and Ctrl+1..6 reach the shell from anywhere on the tab (T-40a-035)
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

    /// `show_extension()`: the manga extensions tab (page 3), through the channel of capacity one.
    pub fn show_extension(&self) {
        self.inner.channel.send(BrowsePage::extensions_of(BrowseKind::Manga));
    }

    /// `show_anime_extension()`: the anime extensions tab (page 2).
    pub fn show_anime_extension(&self) {
        self.inner.channel.send(BrowsePage::extensions_of(BrowseKind::Anime));
    }

    /// The update count of an extensions page (T-41a-011): the Extensions switch shows the count of the visible kind
    /// in words, only above zero.
    pub fn set_badge(&self, page: BrowsePage, updates: usize) {
        if page.is_extensions() {
            let mut counts = self.inner.updates.get();
            counts[usize::from(page.kind() == BrowseKind::Manga)] = updates;
            self.inner.updates.set(counts);
            self.inner.show_badge();
        }
    }

    /// The tab that is showing.
    pub fn visible(&self) -> BrowsePage {
        self.inner.visible()
    }

    /// Re-selecting the Browse tab opens global search for the visible kind (D-UI-1).
    pub fn reselect(&self) {
        (self.inner.deps.on_route)(reselect_route(self.inner.visible()));
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

    /// The page the two switches point at.
    fn switched(&self) -> BrowsePage {
        let kind = usize::from(self.kinds[1].is_active());
        let section = self.sections.iter().position(gtk::ToggleButton::is_active).unwrap_or(0);
        page_at(kind, section)
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
            Some(BrowseShortcut::RefreshExtensions) if self.visible().is_extensions() => {
                (self.deps.on_refresh)(self.visible());
                true
            }
            Some(BrowseShortcut::UpdateAll) if self.visible().is_extensions() => {
                (self.deps.on_update_all)(self.visible());
                true
            }
            Some(BrowseShortcut::SubTab(n)) => {
                self.select(BrowsePage::ALL[usize::from(n) - 1]);
                true
            }
            _ => false,
        }
    }

    fn select(&self, page: BrowsePage) {
        self.stack.set_visible_child_name(&page.index().to_string());
    }

    /// Follows the page now showing: the switches, the badge, the header actions, and the entry, whose text goes to an
    /// extensions page.
    fn page_changed(&self) {
        if !self.ready.get() {
            return;
        }
        let page = self.visible();
        self.syncing.set(true);
        self.kinds[usize::from(page.kind() == BrowseKind::Manga)].set_active(true);
        self.sections[page.index() / 2].set_active(true);
        self.syncing.set(false);
        self.show_badge();
        let name = page.index().to_string();
        self.actions.set_visible_child_name(if self.actions.child_by_name(&name).is_some() { &name } else { "none" });
        self.search.set_placeholder_text(Some(&tr(if page.is_extensions() { MsgKey::action_search_hint } else { MsgKey::action_global_search_hint })));
        if page.is_extensions() {
            let text = self.search.text().to_string();
            *self.sent.borrow_mut() = text.clone();
            (self.deps.on_query)(page, text);
        }
    }

    fn show_badge(&self) {
        let count = self.updates.get()[usize::from(self.visible().kind() == BrowseKind::Manga)];
        let text = updates_badge(count);
        self.badge.set_visible(text.is_some());
        self.badge.set_label(text.as_deref().unwrap_or(""));
    }

    fn query(&self, text: String) {
        let page = self.visible();
        if page.is_extensions() && *self.sent.borrow() != text {
            *self.sent.borrow_mut() = text.clone();
            (self.deps.on_query)(page, text);
        }
    }

    /// Enter in the entry: on a sources or migrate page, a global search of the visible kind for the text.
    fn search_globally(&self, text: &str) {
        let page = self.visible();
        if page.is_extensions() {
            return;
        }
        (self.deps.on_route)(BrowseRoute::GlobalSearch { kind: page.kind(), source_id: None, query: text.trim().to_owned() });
        self.search.set_text("");
    }

    fn header_actions(self: &Rc<Self>) -> Vec<gtk::Widget> {
        let filter = crate::ui::icon_button("view-filter-symbolic", &tr(MsgKey::action_filter));
        let weak = Rc::downgrade(self);
        filter.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                (inner.deps.on_filter)(inner.visible());
            }
        });

        // anime packages come from "Extension stores" (T-41a-011), manga ones from "Extension repos"
        let entries = vec![tr(MsgKey::action_global_search), tr(MsgKey::ext_update_all), tr(MsgKey::extensionStores), tr(MsgKey::label_extension_repos)];
        let weak = Rc::downgrade(self);
        let menu = crate::ui::menu_button("open-menu-symbolic", &entries, move |label| {
            let Some(inner) = weak.upgrade() else { return };
            let kind = inner.visible().kind();
            if label == tr(MsgKey::action_global_search) {
                (inner.deps.on_route)(reselect_route(inner.visible()));
            } else if label == tr(MsgKey::ext_update_all) {
                (inner.deps.on_update_all)(BrowsePage::extensions_of(kind));
            } else if label == tr(MsgKey::extensionStores) {
                (inner.deps.on_repos)(BrowseKind::Anime);
            } else {
                (inner.deps.on_repos)(BrowseKind::Manga);
            }
        });
        menu.set_tooltip_text(Some(&tr(MsgKey::action_menu)));
        vec![filter.upcast(), menu.upcast()]
    }
}

#[cfg(test)]
mod pure_tests {
    use super::updates_badge;

    #[test]
    fn the_badge_shows_only_above_zero() {
        assert_eq!(updates_badge(0), None);
        assert_eq!(updates_badge(1).as_deref(), Some("1 update"));
        assert_eq!(updates_badge(3).as_deref(), Some("3 updates"));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn pump(mut done: impl FnMut() -> bool, limit: Duration) -> bool {
        let context = gtk::glib::MainContext::default();
        let start = Instant::now();
        while start.elapsed() < limit {
            while context.iteration(false) {}
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        done()
    }

    struct Log {
        routes: Rc<RefCell<Vec<BrowseRoute>>>,
        queries: Rc<RefCell<Vec<(BrowsePage, String)>>>,
        left: Rc<RefCell<u32>>,
        keyed: Rc<RefCell<Vec<String>>>,
        repos: Rc<RefCell<Vec<BrowseKind>>>,
    }

    fn shell() -> (BrowseShell, Log) {
        let log = Log { routes: Rc::default(), queries: Rc::default(), left: Rc::default(), keyed: Rc::default(), repos: Rc::default() };
        let repos = Rc::clone(&log.repos);
        let (routes, queries, left) = (Rc::clone(&log.routes), Rc::clone(&log.queries), Rc::clone(&log.left));
        let (refreshed, updated) = (Rc::clone(&log.keyed), Rc::clone(&log.keyed));
        let pages = BrowsePage::ALL.iter().map(|p| (*p, gtk::Label::new(Some(&p.title())).upcast::<gtk::Widget>())).collect();
        let deps = ShellDeps {
            pages,
            page_actions: vec![(BrowsePage::MangaExtensions, gtk::Button::from_icon_name("view-refresh-symbolic").upcast())],
            on_route: Rc::new(move |route| routes.borrow_mut().push(route)),
            on_query: Rc::new(move |page, text| queries.borrow_mut().push((page, text))),
            on_filter: Rc::new(|_| {}),
            on_repos: Rc::new(move |kind| repos.borrow_mut().push(kind)),
            on_refresh: Rc::new(move |page| refreshed.borrow_mut().push(format!("refresh {page:?}"))),
            on_update_all: Rc::new(move |page| updated.borrow_mut().push(format!("update {page:?}"))),
            on_leave: Rc::new(move || *left.borrow_mut() += 1),
        };
        (BrowseShell::new(deps), log)
    }

    fn six_pages_in_order_and_the_extension_requests_select_pages_3_and_2() {
        let (shell, _log) = shell();
        let names: Vec<String> = shell
            .inner
            .stack
            .pages()
            .iter::<gtk::StackPage>()
            .map(|page| page.expect("page").name().map(|n| n.to_string()).unwrap_or_default())
            .collect();
        assert_eq!(names, ["0", "1", "2", "3", "4", "5"], "anime sources, manga sources, anime ext, manga ext, anime migrate, manga migrate");
        shell.show_extension();
        assert!(pump(|| shell.visible() == BrowsePage::MangaExtensions, Duration::from_secs(2)));
        shell.show_anime_extension();
        assert!(pump(|| shell.visible() == BrowsePage::AnimeExtensions, Duration::from_secs(2)));
        // two requests before the loop runs: only the newest is kept
        shell.show_anime_extension();
        shell.show_extension();
        assert!(pump(|| shell.visible() == BrowsePage::MangaExtensions, Duration::from_secs(2)));
    }

    fn reselecting_opens_global_search_for_the_visible_kind() {
        let (shell, log) = shell();
        shell.inner.select(BrowsePage::MangaSources);
        shell.reselect();
        assert_eq!(
            log.routes.borrow().last(),
            Some(&BrowseRoute::GlobalSearch { kind: BrowseKind::Manga, source_id: None, query: String::new() })
        );
        shell.inner.select(BrowsePage::AnimeMigrate);
        shell.reselect();
        assert_eq!(log.routes.borrow().last().map(BrowseRoute::screen_id), Some("global_search"));
        assert!(matches!(log.routes.borrow().last(), Some(BrowseRoute::GlobalSearch { kind: BrowseKind::Anime, .. })));
    }

    fn the_search_entry_follows_the_extensions_pages_and_waits_250_ms() {
        let (shell, log) = shell();
        shell.inner.select(BrowsePage::MangaExtensions);
        assert_eq!(shell.inner.search.placeholder_text().as_deref(), Some(tr(MsgKey::action_search_hint).as_str()), "it filters the extensions here");
        log.queries.borrow_mut().clear();
        shell.inner.search.set_text("a");
        shell.inner.search.set_text("ab");
        assert!(log.queries.borrow().is_empty(), "nothing before the debounce");
        assert!(pump(|| !log.queries.borrow().is_empty(), Duration::from_secs(2)));
        assert_eq!(*log.queries.borrow(), [(BrowsePage::MangaExtensions, "ab".to_owned())], "one call with the final text");
        shell.inner.select(BrowsePage::MangaSources);
        assert_eq!(shell.inner.search.placeholder_text().as_deref(), Some(tr(MsgKey::action_global_search_hint).as_str()), "and searches every source elsewhere");
        shell.inner.select(BrowsePage::AnimeExtensions);
        assert_eq!(log.queries.borrow().last(), Some(&(BrowsePage::AnimeExtensions, "ab".to_owned())), "the new page gets the text");
    }

    fn ctrl_f_escape_and_ctrl_digits_drive_the_tab() {
        use gtk::gdk::{Key, ModifierType};
        let (shell, log) = shell();
        let ctrl = ModifierType::CONTROL_MASK;
        assert!(shell.inner.key(Key::_4, ctrl));
        assert_eq!(shell.visible(), BrowsePage::MangaExtensions, "Ctrl+4 is the fourth tab");
        assert!(shell.inner.key(Key::_1, ctrl));
        assert_eq!(shell.visible(), BrowsePage::AnimeSources);
        assert!(shell.inner.key(Key::f, ctrl), "Ctrl+F goes to the search on every page");
        shell.inner.search.set_text("x");
        assert!(shell.inner.key(Key::Escape, ModifierType::empty()));
        assert!(shell.inner.search.text().is_empty(), "the first Escape clears the search");
        assert_eq!(*log.left.borrow(), 0);
        assert!(shell.inner.key(Key::Escape, ModifierType::empty()));
        assert_eq!(*log.left.borrow(), 1, "the second Escape leaves");
        // the same keys through the controller of the root
        let controllers = shell.root.observe_controllers();
        let key = (0..controllers.n_items()).filter_map(|i| controllers.item(i)).find_map(|c| c.downcast::<gtk::EventControllerKey>().ok()).expect("key controller");
        assert!(key.emit_by_name::<bool>("key-pressed", &[&Key::_2, &0_u32, &ctrl]));
        assert_eq!(shell.visible(), BrowsePage::MangaSources);
    }

    /// docs/design/other-tabs.md: one header row (kind switch, search, actions) with no title, the section switch under
    /// it, and the two switches always naming the page that shows.
    fn one_header_row_and_two_switches_pick_the_page() {
        let (shell, log) = shell();
        let header = shell.root.first_child().and_downcast::<gtk::HeaderBar>().expect("the header row comes first");
        let titled = |w: &gtk::Widget| w.downcast_ref::<gtk::Label>().is_some_and(|l| l.label() == "Browse");
        let mut all = Vec::new();
        fn walk(w: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
            let mut child = w.first_child();
            while let Some(c) = child {
                out.push(c.clone());
                walk(&c, out);
                child = c.next_sibling();
            }
        }
        walk(header.upcast_ref(), &mut all);
        assert!(!all.iter().any(titled), "no page title");
        assert_eq!(all.iter().filter(|w| w.is::<gtk::SearchEntry>()).count(), 1, "one search, in the header");
        assert!(shell.end.is_empty(), "the header is part of the page");

        shell.inner.kinds[1].set_active(true);
        assert_eq!(shell.visible(), BrowsePage::MangaSources);
        shell.inner.sections[1].set_active(true);
        assert_eq!(shell.visible(), BrowsePage::MangaExtensions);
        assert_eq!(shell.inner.actions.visible_child_name().as_deref(), Some("3"), "the page's own header action shows");
        shell.inner.kinds[0].set_active(true);
        assert_eq!(shell.visible(), BrowsePage::AnimeExtensions);
        assert_eq!(shell.inner.actions.visible_child_name().as_deref(), Some("none"));
        shell.inner.select(BrowsePage::MangaMigrate);
        assert!(shell.inner.kinds[1].is_active() && shell.inner.sections[2].is_active(), "the switches follow a page chosen elsewhere");

        // Enter on a sources or migrate page searches every source of the visible kind
        shell.inner.search.set_text("berserk");
        shell.inner.search.emit_by_name::<()>("activate", &[]);
        assert_eq!(log.routes.borrow().last(), Some(&BrowseRoute::GlobalSearch { kind: BrowseKind::Manga, source_id: None, query: "berserk".into() }));
        assert!(shell.inner.search.text().is_empty());
    }

    fn f5_ctrl_r_and_ctrl_u_act_on_the_extensions_pages_only() {
        use gtk::gdk::{Key, ModifierType};
        let (shell, log) = shell();
        let (ctrl, none) = (ModifierType::CONTROL_MASK, ModifierType::empty());
        assert!(!shell.inner.key(Key::u, ctrl), "Ctrl+U is not ours on a sources page");
        assert!(!shell.inner.key(Key::F5, none));
        assert!(log.keyed.borrow().is_empty());
        shell.inner.select(BrowsePage::MangaExtensions);
        assert!(shell.inner.key(Key::u, ctrl));
        assert!(shell.inner.key(Key::F5, none));
        assert!(shell.inner.key(Key::r, ctrl));
        assert_eq!(*log.keyed.borrow(), ["update MangaExtensions", "refresh MangaExtensions", "refresh MangaExtensions"]);
    }

    fn the_extensions_switch_says_the_update_count_in_words_for_the_visible_kind() {
        let (shell, _log) = shell();
        shell.set_badge(BrowsePage::AnimeExtensions, 3);
        shell.set_badge(BrowsePage::AnimeExtensions, 0);
        shell.set_badge(BrowsePage::MangaExtensions, 2);
        shell.inner.select(BrowsePage::AnimeSources);
        assert!(!shell.inner.badge.is_visible(), "anime has none");
        shell.inner.select(BrowsePage::MangaSources);
        assert!(shell.inner.badge.is_visible());
        assert_eq!(shell.inner.badge.label(), "2 updates");
    }

    /// The header menu dispatches by label: Global search, Update all, Extension stores (anime) and Extension repos
    /// (manga).
    fn the_header_menu_entries_route_by_their_label() {
        fn buttons(w: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
            let mut child = w.first_child();
            while let Some(c) = child {
                out.push(c.clone());
                buttons(&c, out);
                child = c.next_sibling();
            }
        }
        let (shell, log) = shell();
        let mut all = Vec::new();
        buttons(shell.root.upcast_ref(), &mut all);
        let menu = all.iter().find_map(|w| w.downcast_ref::<gtk::MenuButton>().cloned()).expect("the header menu");
        let mut entries = Vec::new();
        buttons(menu.popover().expect("the menu").upcast_ref(), &mut entries);
        let press = |key: MsgKey| {
            let label = tr(key);
            let entry = entries.iter().find_map(|w| w.downcast_ref::<gtk::Button>().filter(|b| b.label().as_deref() == Some(label.as_str())).cloned());
            entry.unwrap_or_else(|| panic!("no menu entry {label}")).emit_clicked();
        };
        shell.inner.select(BrowsePage::MangaSources);
        press(MsgKey::action_global_search);
        assert_eq!(log.routes.borrow().last(), Some(&BrowseRoute::GlobalSearch { kind: BrowseKind::Manga, source_id: None, query: String::new() }));
        press(MsgKey::ext_update_all);
        assert_eq!(log.keyed.borrow().last().map(String::as_str), Some("update MangaExtensions"), "update all for the visible kind");
        shell.inner.select(BrowsePage::AnimeMigrate);
        press(MsgKey::ext_update_all);
        assert_eq!(log.keyed.borrow().last().map(String::as_str), Some("update AnimeExtensions"));
        press(MsgKey::extensionStores);
        press(MsgKey::label_extension_repos);
        assert_eq!(*log.repos.borrow(), [BrowseKind::Anime, BrowseKind::Manga], "stores are anime's, repos manga's, whatever the visible kind");
    }

    pub(crate) fn smoke() {
        one_header_row_and_two_switches_pick_the_page();
        ctrl_f_escape_and_ctrl_digits_drive_the_tab();
        f5_ctrl_r_and_ctrl_u_act_on_the_extensions_pages_only();
        the_extensions_switch_says_the_update_count_in_words_for_the_visible_kind();
        the_header_menu_entries_route_by_their_label();
        six_pages_in_order_and_the_extension_requests_select_pages_3_and_2();
        reselecting_opens_global_search_for_the_visible_kind();
        the_search_entry_follows_the_extensions_pages_and_waits_250_ms();
    }
}
