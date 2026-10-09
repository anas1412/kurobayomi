//! The Browse tab: six pages in Aniyomi's order behind a tab strip, the capacity-1 channel that switches them, the
//! shared search entry for the extensions pages, and the header actions (T-40a-001).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

use super::glue::{PageChannel, debounce_search, follow_pages, keep_alive};
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

        let entries = vec![tr(MsgKey::action_filter), tr(MsgKey::label_extension_repos)];
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
    }

    fn shell() -> (BrowseShell, Log) {
        let log = Log { routes: Rc::default(), queries: Rc::default() };
        let (routes, queries) = (Rc::clone(&log.routes), Rc::clone(&log.queries));
        let pages = BrowsePage::ALL.iter().map(|p| (*p, gtk::Label::new(Some(&p.title())).upcast::<gtk::Widget>())).collect();
        let deps = ShellDeps {
            pages,
            on_route: Rc::new(move |route| routes.borrow_mut().push(route)),
            on_query: Rc::new(move |page, text| queries.borrow_mut().push((page, text))),
            on_filter: Rc::new(|_| {}),
            on_repos: Rc::new(|_| {}),
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

    pub(crate) fn smoke() {
        six_pages_in_order_and_the_extension_requests_select_pages_3_and_2();
        reselecting_opens_global_search_for_the_visible_kind();
        the_search_entry_follows_the_extensions_pages_and_waits_250_ms();
    }
}
