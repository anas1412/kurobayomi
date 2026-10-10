//! The Browse tab: six pages in Aniyomi's order behind a tab strip, the capacity-1 channel that switches them, the
//! shared search entry for the extensions pages, and the header actions (T-40a-001).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

use super::glue::{PageChannel, debounce_search, follow_pages, keep_alive};
use super::keys::{BrowseShortcut, EscapeStep, browse_shortcut, escape_step};
use super::route::{BrowseKind, BrowsePage, BrowseRoute, reselect_route};

/// What the tab needs from the app.
pub struct ShellDeps {
    /// The six pages, in `BrowsePage::ALL` order.
    pub pages: Vec<(BrowsePage, gtk::Widget)>,
    /// A route to open: global search from the header button, or from re-selecting the tab.
    pub on_route: Rc<dyn Fn(BrowseRoute)>,
    /// The debounced text of the shared search entry, for the visible extensions page.
    pub on_query: Rc<dyn Fn(BrowsePage, String)>,
    /// The filter button and the overflow's filter entry, for the visible page.
    pub on_filter: Rc<dyn Fn(BrowsePage)>,
    /// The overflow's extension repos entry, for the visible kind.
    pub on_repos: Rc<dyn Fn(BrowseKind)>,
    /// F5 and Ctrl+R on an extensions page: look for extensions again.
    pub on_refresh: Rc<dyn Fn(BrowsePage)>,
    /// Ctrl+U on an extensions page: update every extension that has an update.
    pub on_update_all: Rc<dyn Fn(BrowsePage)>,
    /// Escape with no search open: back out of the Browse page.
    pub on_leave: Rc<dyn Fn()>,
}

/// The tab label with its badge: the update count in brackets, only when above zero (T-41a-011).
pub fn tab_title(base: &str, updates: usize) -> String {
    if updates > 0 { format!("{base} ({updates})") } else { base.to_owned() }
}

struct Inner {
    stack: gtk::Stack,
    search_bar: gtk::Revealer,
    search: gtk::SearchEntry,
    channel: PageChannel,
    deps: ShellDeps,
    /// The text the visible extensions page was last given.
    sent: RefCell<String>,
    ready: Cell<bool>,
}

/// The tab shell. `root` is the page body; `end` are the header actions of the Browse screen.
pub struct BrowseShell {
    pub root: gtk::Box,
    pub end: Vec<gtk::Widget>,
    inner: Rc<Inner>,
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
        let switcher = gtk::StackSwitcher::builder().stack(&stack).halign(gtk::Align::Center).margin_top(6).margin_start(12).margin_end(12).build();
        // six tabs are wider than a narrow window: scroll sideways instead of forcing the window wide
        let strip = gtk::ScrolledWindow::builder().child(&switcher).vscrollbar_policy(gtk::PolicyType::Never).propagate_natural_width(true).build();

        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some(&tr(MsgKey::action_search_hint)));
        search.set_margin_start(12);
        search.set_margin_end(12);
        search.set_margin_top(6);
        let search_bar = gtk::Revealer::new();
        search_bar.set_child(Some(&search));

        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.append(&strip);
        root.append(&search_bar);
        root.append(&stack);

        let inner = Rc::new(Inner {
            stack,
            search_bar,
            search,
            channel: PageChannel::new(),
            deps,
            sent: RefCell::new(String::new()),
            ready: Cell::new(false),
        });
        keep_alive(&root, &inner);
        let end = inner.header_actions();

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
        Self { root, end, inner }
    }

    /// `show_extension()`: the manga extensions tab (page 3), through the channel of capacity one.
    pub fn show_extension(&self) {
        self.inner.channel.send(BrowsePage::extensions_of(BrowseKind::Manga));
    }

    /// `show_anime_extension()`: the anime extensions tab (page 2).
    pub fn show_anime_extension(&self) {
        self.inner.channel.send(BrowsePage::extensions_of(BrowseKind::Anime));
    }

    /// The badge of a tab: its title plus the count, or the plain title at zero.
    pub fn set_badge(&self, page: BrowsePage, updates: usize) {
        if let Some(child) = self.inner.stack.child_by_name(&page.index().to_string()) {
            self.inner.stack.page(&child).set_title(&tab_title(&page.title(), updates));
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

    /// One key of the tab. True when it was ours.
    fn key(&self, key: gtk::gdk::Key, modifiers: gtk::gdk::ModifierType) -> bool {
        if key == gtk::gdk::Key::Escape {
            return match escape_step(self.search_bar.reveals_child() && self.visible().is_extensions()) {
                EscapeStep::CloseSearch => {
                    self.search.set_text("");
                    self.search_bar.set_reveal_child(false);
                    true
                }
                EscapeStep::Leave => {
                    (self.deps.on_leave)();
                    true
                }
            };
        }
        match browse_shortcut(key, modifiers) {
            Some(BrowseShortcut::FocusSearch) if self.visible().is_extensions() => {
                self.search_bar.set_reveal_child(true);
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

    /// The search entry belongs to the extensions pages: it shows there and hands its text to the page now showing.
    fn page_changed(&self) {
        if !self.ready.get() {
            return;
        }
        let page = self.visible();
        self.search_bar.set_reveal_child(page.is_extensions());
        if page.is_extensions() {
            let text = self.search.text().to_string();
            *self.sent.borrow_mut() = text.clone();
            (self.deps.on_query)(page, text);
        }
    }

    fn query(&self, text: String) {
        let page = self.visible();
        if page.is_extensions() && *self.sent.borrow() != text {
            *self.sent.borrow_mut() = text.clone();
            (self.deps.on_query)(page, text);
        }
    }

    fn header_actions(self: &Rc<Self>) -> Vec<gtk::Widget> {
        let global = gtk::Button::from_icon_name("system-search-symbolic");
        global.set_tooltip_text(Some(&tr(MsgKey::action_global_search)));
        let weak = Rc::downgrade(self);
        global.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                (inner.deps.on_route)(reselect_route(inner.visible()));
            }
        });

        let filter = gtk::Button::from_icon_name("view-filter-symbolic");
        filter.set_tooltip_text(Some(&tr(MsgKey::action_filter)));
        let weak = Rc::downgrade(self);
        filter.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                (inner.deps.on_filter)(inner.visible());
            }
        });

        // Anime says "Extension stores" in the overflow (T-41a-011); manga keeps "Extension repos".
        let repos_key = match self.visible().kind() {
            BrowseKind::Anime => MsgKey::extensionStores,
            BrowseKind::Manga => MsgKey::label_extension_repos,
        };
        let entries = vec![tr(MsgKey::action_filter), tr(repos_key)];
        let weak = Rc::downgrade(self);
        let overflow = crate::ui::menu_button("view-more-symbolic", &entries, move |label| {
            let Some(inner) = weak.upgrade() else { return };
            if label == tr(MsgKey::action_filter) {
                (inner.deps.on_filter)(inner.visible());
            } else {
                (inner.deps.on_repos)(inner.visible().kind());
            }
        });
        vec![global.upcast(), filter.upcast(), overflow.upcast()]
    }
}

#[cfg(test)]
mod pure_tests {
    use super::tab_title;

    #[test]
    fn the_badge_shows_only_above_zero() {
        assert_eq!(tab_title("Extensions", 0), "Extensions");
        assert_eq!(tab_title("Extensions", 2), "Extensions (2)");
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
    }

    fn shell() -> (BrowseShell, Log) {
        let log = Log { routes: Rc::default(), queries: Rc::default(), left: Rc::default(), keyed: Rc::default() };
        let (routes, queries, left) = (Rc::clone(&log.routes), Rc::clone(&log.queries), Rc::clone(&log.left));
        let (refreshed, updated) = (Rc::clone(&log.keyed), Rc::clone(&log.keyed));
        let pages = BrowsePage::ALL.iter().map(|p| (*p, gtk::Label::new(Some(&p.title())).upcast::<gtk::Widget>())).collect();
        let deps = ShellDeps {
            pages,
            on_route: Rc::new(move |route| routes.borrow_mut().push(route)),
            on_query: Rc::new(move |page, text| queries.borrow_mut().push((page, text))),
            on_filter: Rc::new(|_| {}),
            on_repos: Rc::new(|_| {}),
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
        assert!(shell.inner.search_bar.reveals_child(), "the entry shows on an extensions page");
        log.queries.borrow_mut().clear();
        shell.inner.search.set_text("a");
        shell.inner.search.set_text("ab");
        assert!(log.queries.borrow().is_empty(), "nothing before the debounce");
        assert!(pump(|| !log.queries.borrow().is_empty(), Duration::from_secs(2)));
        assert_eq!(*log.queries.borrow(), [(BrowsePage::MangaExtensions, "ab".to_owned())], "one call with the final text");
        shell.inner.select(BrowsePage::MangaSources);
        assert!(!shell.inner.search_bar.reveals_child(), "and hides elsewhere");
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
        assert!(!shell.inner.key(Key::f, ctrl), "there is no search on a sources page");
        assert!(shell.inner.key(Key::_3, ctrl));
        shell.inner.search_bar.set_reveal_child(false);
        assert!(shell.inner.key(Key::f, ctrl));
        assert!(shell.inner.search_bar.reveals_child(), "Ctrl+F opens the search");
        assert!(shell.inner.key(Key::Escape, ModifierType::empty()));
        assert!(!shell.inner.search_bar.reveals_child(), "the first Escape closes the search");
        assert_eq!(*log.left.borrow(), 0);
        assert!(shell.inner.key(Key::Escape, ModifierType::empty()));
        assert_eq!(*log.left.borrow(), 1, "the second Escape leaves");
        // the same keys through the controller of the root
        let controllers = shell.root.observe_controllers();
        let key = (0..controllers.n_items()).filter_map(|i| controllers.item(i)).find_map(|c| c.downcast::<gtk::EventControllerKey>().ok()).expect("key controller");
        assert!(key.emit_by_name::<bool>("key-pressed", &[&Key::_2, &0_u32, &ctrl]));
        assert_eq!(shell.visible(), BrowsePage::MangaSources);
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

    fn the_update_count_badges_the_tab_title_only_above_zero() {
        let (shell, _log) = shell();
        let title = |page: BrowsePage| {
            let child = shell.inner.stack.child_by_name(&page.index().to_string()).expect("page");
            shell.inner.stack.page(&child).title().map(|t| t.to_string()).unwrap_or_default()
        };
        let base = BrowsePage::AnimeExtensions.title();
        shell.set_badge(BrowsePage::AnimeExtensions, 3);
        assert_eq!(title(BrowsePage::AnimeExtensions), format!("{base} (3)"));
        shell.set_badge(BrowsePage::AnimeExtensions, 0);
        assert_eq!(title(BrowsePage::AnimeExtensions), base);
    }

    pub(crate) fn smoke() {
        ctrl_f_escape_and_ctrl_digits_drive_the_tab();
        f5_ctrl_r_and_ctrl_u_act_on_the_extensions_pages_only();
        the_update_count_badges_the_tab_title_only_above_zero();
        six_pages_in_order_and_the_extension_requests_select_pages_3_and_2();
        reselecting_opens_global_search_for_the_visible_kind();
        the_search_entry_follows_the_extensions_pages_and_waits_250_ms();
    }
}
