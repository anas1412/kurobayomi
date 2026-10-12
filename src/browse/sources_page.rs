//! The Sources tab: language header rows, then source rows with a Latest button (T-40a-007, T-40a-005, T-40b-003). Sources
//! are not pinned or hidden here: the language filter decides which rows show. `sources::source_list` decides what is listed; this file draws it and routes the clicks.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use gtk::prelude::*;
use kuro_domain::source::browse::BrowseQuery;
use kuro_domain::source::model::Source;
use kuro_i18n::{MsgKey, tr};
use tokio::sync::watch;

use super::glue::{follow_ready, keep_alive};
use super::icons::{IconCache, SourceIcon};
use super::sources::{ListInput, LOCAL_SOURCE_ID, Section, SourceRow, language_line, section_title, shows_latest, source_list};
use super::view::{list_row, loading_sources_row, source_row_with};

/// The installed sources and the preferences that shape the list, as one snapshot of the models.
#[derive(Clone, Debug, Default)]
pub struct SourcesSnapshot {
    pub sources: Vec<Source>,
    /// `source_languages`.
    pub enabled_languages: BTreeSet<String>,
    /// Decimal ids of sources the list leaves out. The page never writes it (the hide toggle is cut); the caller passes
    /// none.
    pub disabled: BTreeSet<String>,
    /// Decimal ids of sources the list shows under Pinned. The page never writes it (the pin toggle is cut); the caller
    /// passes none.
    pub pinned: BTreeSet<String>,
    /// `last_catalogue_source`, -1 for none.
    pub last_used: i64,
    /// Icon files of the extensions that ship one, by source id.
    pub icon_files: BTreeMap<i64, String>,
}

/// What a click on the page does.
pub struct SourcesNav {
    /// A row opens its popular listing; the Latest button opens its latest listing.
    pub open_listing: Rc<dyn Fn(i64, BrowseQuery)>,
}

struct Inner {
    stack: gtk::Stack,
    column: gtk::Box,
    state: RefCell<State>,
    nav: SourcesNav,
}

struct State {
    snapshot: SourcesSnapshot,
    ready: bool,
    icons: IconCache,
}

/// Cloning shares the page; the widget keeps it alive.
#[derive(Clone)]
pub struct SourcesPage {
    inner: Rc<Inner>,
    root: gtk::Box,
}

impl SourcesPage {
    /// `ready` is `SourceRegistry::subscribe_initialized()`: the page shows a spinner until it is true and fills in
    /// when it turns true, without being reopened.
    pub fn new(nav: SourcesNav, ready: watch::Receiver<bool>) -> Self {
        let column = crate::ui::column();
        let scroller = crate::ui::scrolled(&column);
        let empty = sources_empty();
        let stack = gtk::Stack::new();
        stack.add_named(&loading_sources_row(), Some("spinner"));
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&scroller, Some("list"));
        let inner = Rc::new(Inner {
            stack,
            column,
            state: RefCell::new(State {
                snapshot: SourcesSnapshot { last_used: -1, ..SourcesSnapshot::default() },
                ready: false,
                icons: IconCache::default(),
            }),
            nav,
        });
        // the root owns the state; the state holds only the stack inside it, never the root
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        inner.stack.set_vexpand(true);
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

    /// A new snapshot of the models: re-renders the list.
    pub fn update(&self, snapshot: SourcesSnapshot) {
        self.inner.state.borrow_mut().snapshot = snapshot;
        self.inner.render();
    }
}

impl Inner {
    fn render(self: &Rc<Self>) {
        let rows = {
            let state = self.state.borrow();
            if !state.ready {
                self.stack.set_visible_child_name("spinner");
                return;
            }
            let snapshot = &state.snapshot;
            source_list(&ListInput {
                sources: &snapshot.sources,
                enabled_languages: &snapshot.enabled_languages,
                disabled: &snapshot.disabled,
                pinned: &snapshot.pinned,
                last_used: snapshot.last_used,
            })
        };
        while let Some(child) = self.column.first_child() {
            self.column.remove(&child);
        }
        if rows.is_empty() {
            self.stack.set_visible_child_name("empty");
            return;
        }
        let mut section = Section::LastUsed;
        let mut list: Option<gtk::ListBox> = None;
        for row in rows {
            match row {
                SourceRow::Header(next) => {
                    self.column.append(&header_label(&section_title(&next)));
                    list = None;
                    section = next;
                }
                SourceRow::Item(source) => {
                    let row = self.row(&section, source);
                    section_list(&self.column, &mut list).append(&row);
                }
            }
        }
        self.stack.set_visible_child_name("list");
    }

    fn row(self: &Rc<Self>, section: &Section, source: Source) -> gtk::ListBoxRow {
        let (icon, file) = {
            let mut state = self.state.borrow_mut();
            let file = state.snapshot.icon_files.get(&source.id).cloned();
            let icon = state.icons.get(source.id, false, source.is_stub, file.is_some(), source.id == LOCAL_SOURCE_ID);
            (icon, file)
        };
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        if shows_latest(&source) {
            let latest = gtk::Button::with_label(&tr(MsgKey::latest));
            latest.add_css_class("flat");
            let (weak, id) = (Rc::downgrade(self), source.id);
            latest.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    (inner.nav.open_listing)(id, BrowseQuery::Latest);
                }
            });
            actions.append(&latest);
        }

        let language = language_line(section, &source);
        let picture = crate::recipe_sources::icon_path(source.id);
        let row = source_row_with(&source.name, language.as_deref(), icon, file.as_deref(), picture.as_deref(), source.is_stub, crate::browse::errors::is_failing(source.id), Some(actions.upcast_ref()));
        let (open, id) = (Rc::downgrade(self), source.id);
        list_row(
            &row,
            move || {
                if let Some(inner) = open.upgrade() {
                    (inner.nav.open_listing)(id, BrowseQuery::Popular);
                }
            },
            || {},
        )
    }
}

pub(super) fn header_label(text: &str) -> gtk::Label {
    crate::ui::heading(text)
}

/// The boxed list of the current section, started under its heading on first use. The anime Sources tab uses it too.
pub(super) fn section_list(column: &gtk::Box, list: &mut Option<gtk::ListBox>) -> gtk::ListBox {
    list.get_or_insert_with(|| {
        let list = crate::ui::boxed_list();
        column.append(&list);
        list
    })
    .clone()
}

/// The empty Sources tab: no source is listed yet, so its one action opens the repository list.
pub(super) fn sources_empty() -> gtk::Box {
    let repos = gtk::Button::with_label(&tr(MsgKey::label_extension_repos));
    repos.connect_clicked(move |_| super::mount::open_repos());
    super::look::empty_state("system-search-symbolic", &tr(MsgKey::source_empty_screen), Some(&repos)).0
}

/// The `SourceIcon` a row resolves to, for callers that need to know without building the page.
pub fn icon_of(source: &Source, has_custom_icon: bool) -> SourceIcon {
    super::icons::resolve(source.is_stub, has_custom_icon, source.id == LOCAL_SOURCE_ID)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn source(id: i64, name: &str, lang: &str, latest: bool) -> Source {
        let mut s = Source::new(id, name, lang);
        s.supports_latest = latest;
        s
    }

    fn snapshot() -> SourcesSnapshot {
        SourcesSnapshot {
            sources: vec![source(1, "Alpha", "en", true), source(2, "Beta", "en", false), source(3, "Gamma", "ja", false)],
            enabled_languages: ["en".to_owned(), "ja".to_owned()].into(),
            last_used: -1,
            ..SourcesSnapshot::default()
        }
    }

    fn nav(log: &Rc<RefCell<Vec<String>>>) -> SourcesNav {
        let open = Rc::clone(log);
        SourcesNav { open_listing: Rc::new(move |id, q| open.borrow_mut().push(format!("open {id} {q:?}"))) }
    }

    fn labels(column: &gtk::Box) -> Vec<String> {
        let mut out = Vec::new();
        let mut child = column.first_child();
        while let Some(widget) = child {
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                out.push(label.label().to_string());
            } else if let Some(list) = widget.downcast_ref::<gtk::ListBox>() {
                let mut row = list.first_child();
                while let Some(r) = row {
                    out.push("row".into());
                    row = r.next_sibling();
                }
            } else {
                out.push("row".into());
            }
            child = widget.next_sibling();
        }
        out
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

    fn the_page_waits_for_the_registry_then_lists_headers_and_rows() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (tx, rx) = watch::channel(false);
        let page = SourcesPage::new(nav(&log), rx);
        page.update(snapshot());
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("spinner"), "no list before the registry is ready");
        tx.send_replace(true);
        let context = gtk::glib::MainContext::default();
        let start = std::time::Instant::now();
        while page.inner.stack.visible_child_name().as_deref() != Some("list") && start.elapsed().as_secs() < 2 {
            while context.iteration(false) {}
        }
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("list"), "initialisation finishing updates the open page");
        // English (Alpha, Beta), then Japanese (Gamma): headers carry the display names
        let texts = labels(&page.inner.column);
        assert_eq!(texts.iter().filter(|t| t.as_str() == "row").count(), 3);
        assert_eq!(texts.first().map(String::as_str), Some("English"));
        assert!(texts.iter().any(|t| t == "English") && texts.iter().any(|t| *t == crate::browse::names::localized_display_name("ja")), "{texts:?}");
    }

    /// T-40b-085 `pre_init_sources_spinner`: a registry that is not initialised keeps the list loading, however many
    /// snapshots arrive, until it says it is ready (D-SS-01).
    fn pre_init_sources_spinner() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (tx, rx) = watch::channel(false);
        let page = SourcesPage::new(nav(&log), rx);
        for _ in 0..3 {
            page.update(snapshot());
            let context = gtk::glib::MainContext::default();
            while context.iteration(false) {}
            assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("spinner"), "still loading before initialisation");
        }
        assert!(page.inner.column.first_child().is_none(), "no row is drawn before the registry is ready");
        tx.send_replace(true);
        let context = gtk::glib::MainContext::default();
        let start = std::time::Instant::now();
        while page.inner.stack.visible_child_name().as_deref() != Some("list") && start.elapsed().as_secs() < 2 {
            while context.iteration(false) {}
        }
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("list"));
    }

    fn latest_shows_only_for_sources_that_support_it_and_no_row_has_a_pin_button() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (_tx, rx) = watch::channel(true);
        let page = SourcesPage::new(nav(&log), rx);
        page.update(snapshot());
        let mut all = Vec::new();
        buttons(page.inner.column.upcast_ref(), &mut all);
        let latest = tr(MsgKey::latest);
        let latest_buttons: Vec<_> = all.iter().filter(|b| b.label().as_deref() == Some(latest.as_str())).collect();
        assert_eq!(latest_buttons.len(), 1, "only Alpha supports latest");
        assert!(!all.iter().any(|b| b.icon_name().as_deref() == Some("view-pin-symbolic")), "sources are not pinned from the list");
        latest_buttons[0].emit_clicked();
        assert_eq!(log.borrow().first().map(String::as_str), Some("open 1 Latest"));
    }

    fn a_stub_row_says_not_installed_in_the_error_colour() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (_tx, rx) = watch::channel(true);
        let page = SourcesPage::new(nav(&log), rx);
        let mut stub = source(9, "Gone", "en", false);
        stub.is_stub = true;
        page.update(SourcesSnapshot { sources: vec![stub, source(1, "Alpha", "en", false)], ..snapshot() });
        fn errors(w: &gtk::Widget, out: &mut Vec<String>) {
            if let Some(label) = w.downcast_ref::<gtk::Label>() {
                if label.has_css_class("error") {
                    out.push(label.label().to_string());
                }
            }
            let mut child = w.first_child();
            while let Some(c) = child {
                errors(&c, out);
                child = c.next_sibling();
            }
        }
        let mut found = Vec::new();
        errors(page.inner.column.upcast_ref(), &mut found);
        assert_eq!(found, [tr(MsgKey::not_installed)], "only the stub row carries it");
    }

    fn no_sources_shows_the_empty_screen_and_a_stub_gets_the_warning_icon() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (_tx, rx) = watch::channel(true);
        let page = SourcesPage::new(nav(&log), rx);
        page.update(SourcesSnapshot { last_used: -1, ..SourcesSnapshot::default() });
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("empty"));
        assert_eq!(icon_of(&Source { is_stub: true, ..source(9, "Gone", "en", false) }, false), SourceIcon::Warning);
    }

    pub(crate) fn smoke() {
        the_page_waits_for_the_registry_then_lists_headers_and_rows();
        pre_init_sources_spinner();
        latest_shows_only_for_sources_that_support_it_and_no_row_has_a_pin_button();
        a_stub_row_says_not_installed_in_the_error_colour();
        no_sources_shows_the_empty_screen_and_a_stub_gets_the_warning_icon();
    }
}
