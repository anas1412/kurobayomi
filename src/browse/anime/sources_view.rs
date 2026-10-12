//! The anime Sources tab (T-41b-005): the section headers and source rows, the inline error row with Retry. Sources are not
//! pinned or hidden from the list. A thin view over `sources::SourcesTab` and its row model; every decision (which rows,
//! in which order, where a press leads) is made there. The shared row and icon widgets are the manga Sources tab's. The
//! screen lane registers this page and feeds it snapshots.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_domain::source::model::Source;
use kuro_i18n::{MsgKey, tr};
use tokio::sync::watch;

use super::sources::{ListPrefs, SourcesTab, TabRoute, anime_section_title, enabled_source_rows, row_route};
use crate::browse::glue::{follow_ready, keep_alive};
use crate::browse::icons::IconCache;
use crate::browse::primitives::{EmptyAction, ToastSpec};
use crate::browse::sources::{LOCAL_SOURCE_ID, Section, SourceRow, language_line, shows_latest};
use crate::browse::sources_page::{header_label, section_list, sources_empty};
use crate::browse::view::{list_row, loading_sources_row, source_row_with};

/// What a press on the page does.
pub struct AnimeSourcesNav {
    /// A row or a Latest button: where it leads (`row_route`).
    pub route: Rc<dyn Fn(TabRoute)>,
    /// Retry on the inline error row: the caller subscribes to the source watch again.
    pub retry: Rc<dyn Fn()>,
    /// The toast of a failure after the first list.
    pub toast: Rc<dyn Fn(ToastSpec)>,
}

struct State {
    tab: SourcesTab,
    sources: Vec<Source>,
    prefs: ListPrefs,
    icon_files: BTreeMap<i64, String>,
    ready: bool,
    icons: IconCache,
}

struct Inner {
    stack: gtk::Stack,
    column: gtk::Box,
    scroller: gtk::ScrolledWindow,
    state: RefCell<State>,
    nav: AnimeSourcesNav,
}

/// Cloning shares the page; the widget keeps it alive.
#[derive(Clone)]
pub struct AnimeSourcesPage {
    inner: Rc<Inner>,
    root: gtk::Box,
}

impl AnimeSourcesPage {
    /// `ready` is `SourceRegistry::subscribe_initialized()`: the page shows a spinner until it is true.
    pub fn new(nav: AnimeSourcesNav, ready: watch::Receiver<bool>) -> Self {
        let column = crate::ui::column();
        let scroller = crate::ui::scrolled(&column);
        let empty = sources_empty();
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.add_named(&loading_sources_row(), Some("spinner"));
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("error"));
        stack.add_named(&scroller, Some("list"));
        let inner = Rc::new(Inner {
            stack,
            column,
            scroller,
            state: RefCell::new(State {
                tab: SourcesTab::new(),
                sources: Vec::new(),
                prefs: ListPrefs { last_used: -1, ..ListPrefs::default() },
                icon_files: BTreeMap::new(),
                ready: false,
                icons: IconCache::default(),
            }),
            nav,
        });
        // global search and the filter are in the Browse header (`shell`), so the page has no header of its own
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&inner.stack);
        keep_alive(&root, &inner);
        let weak = Rc::downgrade(&inner);
        follow_ready(ready, move |ready| {
            let Some(inner) = weak.upgrade() else { return false };
            inner.state.borrow_mut().ready = ready;
            inner.render();
            true
        });
        Self { inner, root }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// A new emission of the sources watch: replaces the rows and clears an error.
    pub fn update(&self, sources: Vec<Source>, prefs: ListPrefs, icon_files: BTreeMap<i64, String>) {
        {
            let mut state = self.inner.state.borrow_mut();
            let rows = enabled_source_rows(&sources, &prefs);
            state.tab.emit(rows);
            (state.sources, state.prefs, state.icon_files) = (sources, prefs, icon_files);
        }
        self.inner.render();
    }

    /// The watch failed: an inline error row with Retry before the first list, a toast after it (E-SRC-02).
    pub fn fail(&self) {
        let toast = self.inner.state.borrow_mut().tab.fail();
        if let Some(toast) = toast {
            (self.inner.nav.toast)(toast);
        }
        self.inner.render();
    }

    /// The vertical scroll offset, so a page pushed over this one can hand it back on Back (T-41b-005).
    pub fn scroll_position(&self) -> f64 {
        self.inner.scroller.vadjustment().value()
    }

    pub fn set_scroll_position(&self, value: f64) {
        self.inner.scroller.vadjustment().set_value(value);
    }
}

impl Inner {
    fn render(self: &Rc<Self>) {
        let scroll = self.scroller.vadjustment().value();
        let page = {
            let state = self.state.borrow();
            if state.tab.inline_error {
                "error"
            } else if !state.ready || state.tab.loading {
                "spinner"
            } else if state.tab.is_empty() {
                "empty"
            } else {
                "list"
            }
        };
        if page == "error" {
            self.show_error();
        }
        if page == "list" {
            self.fill();
            // a rebuild resets the offset: put it back, so a refresh of the rows never jumps the list
            self.scroller.vadjustment().set_value(scroll);
        }
        self.stack.set_visible_child_name(page);
    }

    /// The inline error row: the error text and Retry. Retry clears the error, shows the spinner and asks the caller to
    /// subscribe again.
    fn show_error(self: &Rc<Self>) {
        let Some(slot) = self.stack.child_by_name("error").and_downcast::<gtk::Box>() else { return };
        while let Some(child) = slot.first_child() {
            slot.remove(&child);
        }
        let label = gtk::Label::builder().label(tr(MsgKey::internal_error)).wrap(true).margin_top(24).build();
        let retry = gtk::Button::with_label(&EmptyAction::Retry.label());
        retry.set_halign(gtk::Align::Center);
        retry.set_margin_top(8);
        let weak = Rc::downgrade(self);
        retry.connect_clicked(move |_| {
            let Some(inner) = weak.upgrade() else { return };
            inner.state.borrow_mut().tab.retry();
            inner.render();
            (inner.nav.retry)();
        });
        slot.append(&label);
        slot.append(&retry);
    }

    fn fill(self: &Rc<Self>) {
        while let Some(child) = self.column.first_child() {
            self.column.remove(&child);
        }
        let (rows, locale) = {
            let state = self.state.borrow();
            (state.tab.rows.clone(), state.prefs.locale.clone())
        };
        let mut section = Section::LastUsed;
        let mut list: Option<gtk::ListBox> = None;
        for row in rows {
            match row {
                SourceRow::Header(next) => {
                    self.column.append(&header_label(&anime_section_title(&next, &locale)));
                    list = None;
                    section = next;
                }
                SourceRow::Item(source) => {
                    let row = self.row(&section, source);
                    section_list(&self.column, &mut list).append(&row);
                }
            }
        }
    }

    fn row(self: &Rc<Self>, section: &Section, source: Source) -> gtk::ListBoxRow {
        let (icon, file) = {
            let mut state = self.state.borrow_mut();
            let file = state.icon_files.get(&source.id).cloned();
            let icon = state.icons.get(source.id, false, source.is_stub, file.is_some(), source.id == LOCAL_SOURCE_ID);
            (icon, file)
        };
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        if shows_latest(&source) {
            let latest = gtk::Button::with_label(&tr(MsgKey::latest));
            latest.add_css_class("flat");
            let (route, id) = (Rc::clone(&self.nav.route), source.id);
            latest.connect_clicked(move |_| route(row_route(id, true)));
            actions.append(&latest);
        }

        let language = language_line(section, &source);
        let row = source_row_with(&source.name, language.as_deref(), icon, file.as_deref(), source.is_stub, crate::browse::errors::is_failing(source.id), Some(actions.upcast_ref()));
        let (route, id) = (Rc::clone(&self.nav.route), source.id);
        list_row(&row, move || route(row_route(id, false)), || {})
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::browse::anime::fixtures::worked_example;
    use crate::browse::primitives::ToastDuration;
    use kuro_domain::source::browse::BrowseQuery;

    fn prefs() -> ListPrefs {
        ListPrefs {
            enabled_langs: ["en".to_owned(), "all".to_owned(), "ar".to_owned(), String::new()].into(),
            last_used: -1,
            locale: "en_US.UTF-8".into(),
            ..ListPrefs::default()
        }
    }

    fn page(log: &Rc<RefCell<Vec<String>>>, ready: bool) -> (AnimeSourcesPage, watch::Sender<bool>) {
        let (route, retry, toast) = (Rc::clone(log), Rc::clone(log), Rc::clone(log));
        let nav = AnimeSourcesNav {
            route: Rc::new(move |to| route.borrow_mut().push(format!("route {to:?}"))),
            retry: Rc::new(move || retry.borrow_mut().push("retry".into())),
            toast: Rc::new(move |spec| toast.borrow_mut().push(format!("toast {} {:?}", spec.message, spec.duration))),
        };
        let (tx, rx) = watch::channel(ready);
        (AnimeSourcesPage::new(nav, rx), tx)
    }

    fn buttons(root: &gtk::Widget, out: &mut Vec<gtk::Button>) {
        if let Some(button) = root.downcast_ref::<gtk::Button>() {
            out.push(button.clone());
        }
        let mut child = root.first_child();
        while let Some(widget) = child {
            buttons(&widget, out);
            child = widget.next_sibling();
        }
    }

    fn all_buttons(page: &AnimeSourcesPage) -> Vec<gtk::Button> {
        let mut found = Vec::new();
        buttons(page.widget().upcast_ref(), &mut found);
        found
    }

    fn headers(page: &AnimeSourcesPage) -> Vec<String> {
        let mut out = Vec::new();
        let mut child = page.inner.column.first_child();
        while let Some(widget) = child {
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                out.push(label.label().to_string());
            }
            child = widget.next_sibling();
        }
        out
    }

    /// Global search and the filter live in the Browse header (`shell`); the page repeats neither.
    fn the_page_has_no_header_of_its_own() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (page, _tx) = page(&log, true);
        let named = |key: MsgKey| all_buttons(&page).iter().any(|b| b.tooltip_text().as_deref() == Some(tr(key).as_str()));
        assert!(!named(MsgKey::action_global_search) && !named(MsgKey::action_filter));
    }

    fn the_list_uses_the_anime_section_order_and_a_row_and_latest_press_open_the_browse_page() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (page, _tx) = page(&log, true);
        let mut sources = worked_example();
        sources[1].supports_latest = true;
        page.update(sources.clone(), prefs(), BTreeMap::new());
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("list"));
        // the headers are the model's sections in the model's order, named in English, with Multi first
        let expected: Vec<String> = enabled_source_rows(&sources, &prefs())
            .iter()
            .filter_map(|row| match row {
                SourceRow::Header(section) => Some(anime_section_title(section, "en_US.UTF-8")),
                SourceRow::Item(_) => None,
            })
            .collect();
        assert_eq!(headers(&page), expected);
        assert_eq!(expected.first().map(String::as_str), Some("Multi"));
        let latest = tr(MsgKey::latest);
        let found = all_buttons(&page);
        let latest_buttons: Vec<_> = found.iter().filter(|b| b.label().as_deref() == Some(latest.as_str())).collect();
        assert_eq!(latest_buttons.len(), 1, "only the source that supports it");
        latest_buttons[0].emit_clicked();
        assert_eq!(*log.borrow(), [format!("route {:?}", row_route(11, true))]);
        assert_eq!(row_route(11, false), TabRoute::Browse { source_id: 11, query: BrowseQuery::Popular });
    }

    fn a_failure_before_the_first_list_is_an_inline_error_and_retry_asks_again() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (page, _tx) = page(&log, true);
        page.fail();
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("error"));
        let found = all_buttons(&page);
        let retry = found.iter().find(|b| b.label().as_deref() == Some(EmptyAction::Retry.label().as_str())).expect("retry");
        retry.emit_clicked();
        assert_eq!(*log.borrow(), ["retry"]);
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("spinner"), "Retry waits for the next emission");
    }

    fn a_failure_after_the_first_list_keeps_the_rows_and_toasts() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (page, _tx) = page(&log, true);
        page.update(worked_example(), prefs(), BTreeMap::new());
        page.fail();
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("list"));
        assert_eq!(*log.borrow(), [format!("toast {} {:?}", tr(MsgKey::internal_error), ToastDuration::Seconds(4))]);
    }

    fn the_page_waits_for_the_registry_and_an_empty_list_says_so() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (page, tx) = page(&log, false);
        page.update(Vec::new(), prefs(), BTreeMap::new());
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("spinner"));
        tx.send_replace(true);
        let context = gtk::glib::MainContext::default();
        let start = std::time::Instant::now();
        while page.inner.stack.visible_child_name().as_deref() != Some("empty") && start.elapsed().as_secs() < 2 {
            while context.iteration(false) {}
        }
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("empty"));
    }

    fn a_refresh_of_the_rows_keeps_the_scroll_offset() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (page, _tx) = page(&log, true);
        let window = gtk::Window::new();
        window.set_default_size(300, 80);
        window.set_child(Some(page.widget()));
        window.present();
        page.update(worked_example(), prefs(), BTreeMap::new());
        let context = gtk::glib::MainContext::default();
        while context.iteration(false) {}
        let adjustment = page.inner.scroller.vadjustment();
        let target = (adjustment.upper() - adjustment.page_size()).min(40.0);
        page.set_scroll_position(target);
        page.update(worked_example(), prefs(), BTreeMap::new());
        assert_eq!(page.scroll_position(), target, "the list is rebuilt at the same offset");
        window.close();
    }

    pub(crate) fn smoke() {
        the_page_has_no_header_of_its_own();
        the_list_uses_the_anime_section_order_and_a_row_and_latest_press_open_the_browse_page();
        a_failure_before_the_first_list_is_an_inline_error_and_retry_asks_again();
        a_failure_after_the_first_list_keeps_the_rows_and_toasts();
        the_page_waits_for_the_registry_and_an_empty_list_says_so();
        a_refresh_of_the_rows_keeps_the_scroll_offset();
    }
}
