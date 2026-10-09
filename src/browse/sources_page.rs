//! The Sources tab: language header rows, then source rows with Latest and pin buttons (T-40a-007, T-40a-005,
//! T-40b-003). `sources::source_list` decides what is listed; this file draws it and routes the clicks.

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
use super::sources::{
    ListInput, LOCAL_SOURCE_ID, OptionsEffect, Section, SourceRow, is_pinned, language_line, options_actions, options_effect, options_label,
    options_title, pin_button_style, section_title, shows_latest, source_list,
};
use super::view::{attach_row_gestures, source_row, spinner_row};

/// The installed sources and the preferences that shape the list, as one snapshot of the models.
#[derive(Clone, Debug, Default)]
pub struct SourcesSnapshot {
    pub sources: Vec<Source>,
    /// `source_languages`.
    pub enabled_languages: BTreeSet<String>,
    /// `hidden_catalogues`, decimal ids.
    pub disabled: BTreeSet<String>,
    /// `pinned_catalogues`, decimal ids.
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
    /// The pin button, and the Pin and Unpin rows of the options popover.
    pub toggle_pin: Rc<dyn Fn(i64)>,
    /// The Disable row of the options popover.
    pub toggle_disabled: Rc<dyn Fn(i64)>,
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
        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroller = gtk::ScrolledWindow::builder().child(&column).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build();
        let empty = gtk::Label::new(Some(&tr(MsgKey::source_empty_screen)));
        empty.set_valign(gtk::Align::Center);
        empty.set_vexpand(true);
        let stack = gtk::Stack::new();
        stack.add_named(&spinner_row(), Some("spinner"));
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
        for row in rows {
            match row {
                SourceRow::Header(next) => {
                    self.column.append(&header_label(&section_title(&next)));
                    section = next;
                }
                SourceRow::Item(source) => self.column.append(&self.row(&section, source)),
            }
        }
        self.stack.set_visible_child_name("list");
    }

    fn row(self: &Rc<Self>, section: &Section, source: Source) -> gtk::Box {
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
        actions.append(&self.pin_button(&source));

        let language = language_line(section, &source);
        let row = source_row(&source.name, language.as_deref(), icon, file.as_deref(), source.is_stub, Some(actions.upcast_ref()));
        let (open, options, id) = (Rc::downgrade(self), Rc::downgrade(self), source.id);
        let anchor = row.downgrade();
        attach_row_gestures(
            &row,
            move || {
                if let Some(inner) = open.upgrade() {
                    (inner.nav.open_listing)(id, BrowseQuery::Popular);
                }
            },
            move || {
                if let (Some(inner), Some(anchor)) = (options.upgrade(), anchor.upgrade()) {
                    inner.options_popover(&anchor, &source);
                }
            },
        );
        row
    }

    /// The options of a source (long press, right click or the menu key): a popover titled with its visual name and a
    /// row for Pin or Unpin and, except for the local source, Disable. A choice closes the popover, then applies.
    fn options_popover(self: &Rc<Self>, row: &gtk::Box, source: &Source) -> gtk::Popover {
        let popover = gtk::Popover::new();
        popover.set_parent(row);
        let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
        column.append(&gtk::Label::builder().label(options_title(source)).xalign(0.0).css_classes(["heading"]).margin_bottom(4).build());
        for action in options_actions(source) {
            let button = gtk::Button::with_label(&options_label(action));
            button.add_css_class("flat");
            let (weak, popover, id) = (Rc::downgrade(self), popover.clone(), source.id);
            button.connect_clicked(move |_| {
                popover.popdown();
                if let Some(inner) = weak.upgrade() {
                    match options_effect(action) {
                        OptionsEffect::TogglePin => (inner.nav.toggle_pin)(id),
                        OptionsEffect::ToggleDisabled => (inner.nav.toggle_disabled)(id),
                    }
                }
            });
            column.append(&button);
        }
        popover.set_child(Some(&column));
        popover.connect_closed(|popover| popover.unparent());
        // a row that is not inside a window has no surface to show the popover on
        if row.root().is_some() {
            popover.popup();
        }
        popover
    }

    /// Filled and full strength when pinned, outline at the secondary alpha (78 %) otherwise; named `action_pin` or `action_unpin`.
    fn pin_button(self: &Rc<Self>, source: &Source) -> gtk::Button {
        let pinned = is_pinned(source);
        let (filled, alpha) = pin_button_style(pinned);
        let button = gtk::Button::from_icon_name("view-pin-symbolic");
        button.add_css_class("flat");
        if filled {
            button.add_css_class("pinned");
        }
        button.set_opacity(alpha);
        let name = tr(if pinned { MsgKey::action_unpin } else { MsgKey::action_pin });
        button.set_tooltip_text(Some(&name));
        button.update_property(&[gtk::accessible::Property::Label(&name)]);
        let (weak, id) = (Rc::downgrade(self), source.id);
        button.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                (inner.nav.toggle_pin)(id);
            }
        });
        button
    }
}

fn header_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.add_css_class("heading");
    label.set_margin_start(16);
    label.set_margin_end(16);
    label.set_margin_top(16);
    label.set_margin_bottom(8);
    label
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
            pinned: ["2".to_owned()].into(),
            last_used: -1,
            ..SourcesSnapshot::default()
        }
    }

    fn nav(log: &Rc<RefCell<Vec<String>>>) -> SourcesNav {
        let (open, disable, pin) = (Rc::clone(log), Rc::clone(log), Rc::clone(log));
        SourcesNav {
            open_listing: Rc::new(move |id, q| open.borrow_mut().push(format!("open {id} {q:?}"))),
            toggle_pin: Rc::new(move |id| pin.borrow_mut().push(format!("pin {id}"))),
            toggle_disabled: Rc::new(move |id| disable.borrow_mut().push(format!("disable {id}"))),
        }
    }

    fn labels(column: &gtk::Box) -> Vec<String> {
        let mut out = Vec::new();
        let mut child = column.first_child();
        while let Some(widget) = child {
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                out.push(label.label().to_string());
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
        // Pinned (Beta), then English (Alpha), then Japanese (Gamma): headers carry the display names
        let texts = labels(&page.inner.column);
        assert_eq!(texts.iter().filter(|t| t.as_str() == "row").count(), 3);
        assert_eq!(texts.first().map(String::as_str), Some("Pinned"));
        assert!(texts.iter().any(|t| t == "English") && texts.iter().any(|t| *t == crate::browse::names::localized_display_name("ja")), "{texts:?}");
    }

    fn latest_shows_only_for_sources_that_support_it_and_pin_buttons_are_named() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (_tx, rx) = watch::channel(true);
        let page = SourcesPage::new(nav(&log), rx);
        page.update(snapshot());
        let mut all = Vec::new();
        buttons(page.inner.column.upcast_ref(), &mut all);
        let latest = tr(MsgKey::latest);
        let latest_buttons: Vec<_> = all.iter().filter(|b| b.label().as_deref() == Some(latest.as_str())).collect();
        assert_eq!(latest_buttons.len(), 1, "only Alpha supports latest");
        let pins: Vec<_> = all.iter().filter(|b| b.icon_name().as_deref() == Some("view-pin-symbolic")).collect();
        assert_eq!(pins.len(), 3);
        let unpin = tr(MsgKey::action_unpin);
        let pin = tr(MsgKey::action_pin);
        let tips: Vec<String> = pins.iter().filter_map(|b| b.tooltip_text().map(|t| t.to_string())).collect();
        assert_eq!(tips.iter().filter(|t| **t == unpin).count(), 1, "Beta is pinned: one row shows the unpin name");
        assert_eq!(tips.iter().filter(|t| **t == pin).count(), 2);
        let filled = pins.iter().find(|b| b.has_css_class("pinned")).expect("filled pin");
        assert_eq!(filled.opacity(), 1.0);
        let outline = pins.iter().find(|b| !b.has_css_class("pinned")).expect("outline pin");
        assert!((outline.opacity() - 0.78).abs() < 0.005, "dimmed to the secondary alpha");

        latest_buttons[0].emit_clicked();
        pins[0].emit_clicked();
        let log = log.borrow();
        assert_eq!(log[0], "open 1 Latest");
        assert!(log[1].starts_with("pin "));
    }

    fn long_press_opens_the_options_popover_titled_with_the_visual_name_and_a_choice_applies() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (_tx, rx) = watch::channel(true);
        let page = SourcesPage::new(nav(&log), rx);
        let mut local = source(LOCAL_SOURCE_ID, "Local source", "", false);
        local.is_stub = false;
        page.update(SourcesSnapshot { sources: vec![source(1, "Alpha", "en", true), local], ..snapshot() });
        let anchor = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let window = gtk::Window::new();
        window.set_child(Some(&anchor));
        window.present();

        let popover = page.inner.options_popover(&anchor, &source(1, "Alpha", "en", true));
        let mut found = Vec::new();
        buttons(popover.child().expect("child").upcast_ref(), &mut found);
        let names: Vec<String> = found.iter().filter_map(|b| b.label().map(|l| l.to_string())).collect();
        assert_eq!(names, [tr(MsgKey::action_pin), tr(MsgKey::action_disable)]);
        let title = popover.child().and_then(|c| c.first_child()).and_downcast::<gtk::Label>().expect("title");
        assert_eq!(title.label().as_str(), "Alpha (EN)");
        found[1].emit_clicked();
        assert_eq!(log.borrow().last().map(String::as_str), Some("disable 1"));
        assert!(!popover.is_visible(), "the popover closes when a row is chosen");

        let mut pinned = source(2, "Beta", "en", false);
        pinned.pin = kuro_domain::source::pin::Pins::PINNED;
        let popover = page.inner.options_popover(&anchor, &pinned);
        let mut found = Vec::new();
        buttons(popover.child().expect("child").upcast_ref(), &mut found);
        assert_eq!(found[0].label().as_deref(), Some(tr(MsgKey::action_unpin).as_str()), "a pinned source offers Unpin");
        found[0].emit_clicked();
        assert_eq!(log.borrow().last().map(String::as_str), Some("pin 2"));

        let popover = page.inner.options_popover(&anchor, &source(LOCAL_SOURCE_ID, "Local source", "", false));
        let mut found = Vec::new();
        buttons(popover.child().expect("child").upcast_ref(), &mut found);
        assert_eq!(found.len(), 1, "the local source cannot be disabled");
        window.close();
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
        latest_shows_only_for_sources_that_support_it_and_pin_buttons_are_named();
        long_press_opens_the_options_popover_titled_with_the_visual_name_and_a_choice_applies();
        a_stub_row_says_not_installed_in_the_error_colour();
        no_sources_shows_the_empty_screen_and_a_stub_gets_the_warning_icon();
    }
}
