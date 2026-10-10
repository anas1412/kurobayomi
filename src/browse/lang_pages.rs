//! The two language filters: Show/hide sources (a language switch per language and a checkbox per source, T-40a-009) and
//! the extension language page (one switch per language, T-40a-020). Both write `source_languages` through the same
//! toggle; the sources page also writes the hidden source ids, as strings (D§5). Ports `MangaSourcesFilterScreen` and
//! `MangaExtensionFilterScreen` with their screen models; the rules are pure and the widgets are thin.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_domain::source::model::Source;
use kuro_domain::source::toggles::{flip_disabled, toggle_language};
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::pref::Pref;

use super::extensions::Ext;
use super::glue::keep_alive;
use super::names::{case_insensitive_cmp, compare_languages, localized_display_name};
use super::view::spinner_row;

/// One source checkbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCheck {
    pub id: i64,
    pub name: String,
    /// Checked when the id string is not in `hidden_catalogues` (B§4).
    pub checked: bool,
}

/// One language header and the sources under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LangGroup {
    pub lang: String,
    pub title: String,
    /// The header switch: the language is in `source_languages`.
    pub enabled: bool,
    pub sources: Vec<SourceCheck>,
}

/// `GetLanguagesWithMangaSources`: sources sorted by `(hidden, name ignoring case)`, grouped by language, the groups
/// sorted by `(language off, language order)`. The page shows the sources of an enabled language only.
pub fn sources_filter_groups(sources: &[Source], enabled: &BTreeSet<String>, disabled: &BTreeSet<String>) -> Vec<LangGroup> {
    let hidden = |s: &Source| disabled.contains(&s.id.to_string());
    // Only online sources are listed (E-SF-04): a stub and the local source are not HTTP sources.
    let mut sorted: Vec<&Source> = sources.iter().filter(|s| !s.is_stub && s.id != super::sources::LOCAL_SOURCE_ID).collect();
    sorted.sort_by(|a, b| hidden(a).cmp(&hidden(b)).then_with(|| case_insensitive_cmp(&a.name, &b.name)));
    let mut groups: Vec<LangGroup> = Vec::new();
    for source in sorted {
        let check = SourceCheck { id: source.id, name: source.name.clone(), checked: !hidden(source) };
        match groups.iter_mut().find(|g| g.lang == source.lang) {
            Some(group) => group.sources.push(check),
            None => groups.push(LangGroup {
                lang: source.lang.clone(),
                title: localized_display_name(&source.lang),
                enabled: enabled.contains(&source.lang),
                sources: vec![check],
            }),
        }
    }
    groups.sort_by(|a, b| (!a.enabled).cmp(&(!b.enabled)).then_with(|| compare_languages(&a.lang, &b.lang)));
    groups
}

/// The states of the Sources filter page (B§3.1): loading until the registry is ready, an error when the list failed, and
/// the groups otherwise (an empty success shows `source_filter_empty_screen`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterScreen {
    Loading,
    Error,
    Success(Vec<LangGroup>),
}

/// The state the page is in (E-SF-05 for the error). `ready` is the registry's readiness; `failed` is a failed load.
pub fn filter_screen(ready: bool, failed: bool, sources: &[Source], enabled: &BTreeSet<String>, disabled: &BTreeSet<String>) -> FilterScreen {
    match (ready, failed) {
        (false, _) => FilterScreen::Loading,
        (true, true) => FilterScreen::Error,
        (true, false) => FilterScreen::Success(sources_filter_groups(sources, enabled, disabled)),
    }
}

/// What an error does: the page shows `internal_error` as a toast and pops itself (E-SF-05).
pub fn error_pops_the_page(screen: &FilterScreen) -> bool {
    *screen == FilterScreen::Error
}

/// One switch of the extension language page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LangRow {
    pub lang: String,
    pub title: String,
    pub enabled: bool,
}

/// `GetMangaExtensionLanguages`: the language of every source of every extension (the extension's own language when it
/// has no sources), each once, enabled languages first and each part in language order.
pub fn extension_language_rows(exts: &[Ext], enabled: &BTreeSet<String>) -> Vec<LangRow> {
    let mut langs: BTreeSet<String> = BTreeSet::new();
    for ext in exts {
        if ext.sources.is_empty() {
            langs.insert(ext.lang.clone().unwrap_or_default());
        } else {
            langs.extend(ext.sources.iter().map(|s| s.lang.clone()));
        }
    }
    let mut rows: Vec<LangRow> =
        langs.into_iter().map(|lang| LangRow { title: localized_display_name(&lang), enabled: enabled.contains(&lang), lang }).collect();
    rows.sort_by(|a, b| (!a.enabled).cmp(&(!b.enabled)).then_with(|| compare_languages(&a.lang, &b.lang)));
    rows
}

/// A row with a title and a switch. `on_toggle` runs with the new position when the user flips it.
fn switch_row(title: &str, active: bool, on_toggle: impl Fn(bool) + 'static) -> (gtk::Box, gtk::Switch) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.set_margin_start(16);
    row.set_margin_end(16);
    row.set_margin_top(8);
    row.set_margin_bottom(8);
    row.append(&gtk::Label::builder().label(title).hexpand(true).xalign(0.0).build());
    let switch = gtk::Switch::builder().active(active).valign(gtk::Align::Center).build();
    switch.connect_state_set(move |switch, state| {
        on_toggle(state);
        switch.set_state(state);
        gtk::glib::Propagation::Stop
    });
    row.append(&switch);
    (row, switch)
}

fn write(pref: &Pref<BTreeSet<String>>, set: BTreeSet<String>) {
    if let Err(error) = pref.set(set) {
        tracing::warn!(key = pref.key(), %error, "a browse filter could not be stored");
    }
}

fn empty_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_valign(gtk::Align::Center);
    label.set_vexpand(true);
    label
}

/// A stack of spinner, empty message and list. The page shows the spinner until its models arrive.
fn page_stack(empty_text: &str) -> (gtk::Stack, gtk::Box) {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroller = gtk::ScrolledWindow::builder().child(&column).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build();
    let stack = gtk::Stack::new();
    stack.add_named(&spinner_row(), Some("loading"));
    stack.add_named(&empty_label(empty_text), Some("empty"));
    stack.add_named(&scroller, Some("list"));
    stack.set_vexpand(true);
    stack.set_visible_child_name("loading");
    (stack, column)
}

fn clear(column: &gtk::Box) {
    while let Some(child) = column.first_child() {
        column.remove(&child);
    }
}

// ---------- Show/hide sources ----------

struct SourcesInner {
    languages: Pref<BTreeSet<String>>,
    hidden: Pref<BTreeSet<String>>,
    stack: gtk::Stack,
    column: gtk::Box,
    sources: RefCell<Vec<Source>>,
    /// The registry's readiness and whether the list failed (B§3.1): the page is Loading until it is ready.
    ready: Cell<bool>,
    failed: Cell<bool>,
    /// What the Error state does: the `internal_error` toast and a pop of the page (E-SF-05). Runs once.
    on_error: RefCell<Option<Rc<dyn Fn()>>>,
    errored: Cell<bool>,
}

/// The Show/hide sources page. It reads `source_languages` and the hidden ids when it draws and writes them when a
/// switch or a checkbox is flipped, so the stored sets are the one truth. Cloning shares the page.
#[derive(Clone)]
pub struct SourcesFilterPage {
    inner: Rc<SourcesInner>,
    root: gtk::Box,
}

impl SourcesFilterPage {
    /// `languages` is `source_languages`; `hidden` is `hidden_catalogues` or `hidden_anime_catalogues`.
    pub fn new(languages: Pref<BTreeSet<String>>, hidden: Pref<BTreeSet<String>>) -> Self {
        let (stack, column) = page_stack(&tr(MsgKey::source_filter_empty_screen));
        let inner = Rc::new(SourcesInner {
            languages,
            hidden,
            stack,
            column,
            sources: RefCell::new(Vec::new()),
            ready: Cell::new(false),
            failed: Cell::new(false),
            on_error: RefCell::new(None),
            errored: Cell::new(false),
        });
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&inner.stack);
        keep_alive(&root, &inner);
        Self { inner, root }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// The installed online sources, replacing the list: the registry is ready and the load succeeded.
    pub fn set_sources(&self, sources: Vec<Source>) {
        self.set_state(true, false, sources);
    }

    /// The page's state as B§3.1 has it: Loading until `ready`, Error when the load `failed` (the error hook runs),
    /// Success with the groups of `sources` otherwise.
    pub fn set_state(&self, ready: bool, failed: bool, sources: Vec<Source>) {
        *self.inner.sources.borrow_mut() = sources;
        self.inner.ready.set(ready);
        self.inner.failed.set(failed);
        self.inner.render();
    }

    /// What the Error state does (E-SF-05): show the `internal_error` toast and pop the page.
    pub fn set_on_error(&self, on_error: Rc<dyn Fn()>) {
        *self.inner.on_error.borrow_mut() = Some(on_error);
    }
}

impl SourcesInner {
    fn render(self: &Rc<Self>) {
        clear(&self.column);
        let screen = filter_screen(self.ready.get(), self.failed.get(), &self.sources.borrow(), &self.languages.get(), &self.hidden.get());
        if error_pops_the_page(&screen) {
            self.stack.set_visible_child_name("empty");
            // the hook runs after the borrow of the slot ends: it pops the page, which may drop this one
            let hook = self.on_error.borrow().clone();
            if let Some(hook) = hook.filter(|_| !self.errored.replace(true)) {
                hook();
            }
            return;
        }
        let groups = match screen {
            FilterScreen::Loading => {
                self.stack.set_visible_child_name("loading");
                return;
            }
            FilterScreen::Error => return,
            FilterScreen::Success(groups) => groups,
        };
        if groups.is_empty() {
            self.stack.set_visible_child_name("empty");
            return;
        }
        for group in groups {
            let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
            list.set_visible(group.enabled);
            let (weak, lang, shown) = (Rc::downgrade(self), group.lang.clone(), list.clone());
            let (header, _) = switch_row(&group.title, group.enabled, move |on| {
                if let Some(inner) = weak.upgrade() {
                    let mut set = inner.languages.get();
                    if set.contains(&lang) != on {
                        toggle_language(&mut set, &lang);
                        write(&inner.languages, set);
                    }
                    shown.set_visible(on);
                }
            });
            self.column.append(&header);
            for source in group.sources {
                list.append(&self.check_row(&source));
            }
            self.column.append(&list);
        }
        self.stack.set_visible_child_name("list");
    }

    fn check_row(self: &Rc<Self>, source: &SourceCheck) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        row.set_margin_start(32);
        row.set_margin_end(16);
        row.set_margin_top(8);
        row.set_margin_bottom(8);
        row.append(&gtk::Label::builder().label(&source.name).hexpand(true).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build());
        let check = gtk::CheckButton::new();
        check.set_active(source.checked);
        // the row is the click target; the box is a picture of the state
        check.set_can_target(false);
        row.append(&check);
        let (weak, id) = (Rc::downgrade(self), source.id);
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, _, _| {
            let Some(inner) = weak.upgrade() else { return };
            let mut hidden = inner.hidden.get();
            let now_hidden = flip_disabled(&mut hidden, id);
            write(&inner.hidden, hidden);
            check.set_active(!now_hidden);
        });
        row.add_controller(click);
        row
    }
}

// ---------- extension languages ----------

struct ExtLangInner {
    languages: Pref<BTreeSet<String>>,
    toast: Rc<dyn Fn(String)>,
    stack: gtk::Stack,
    column: gtk::Box,
}

/// The extension language page: one switch per language of the extension lists. A failed language fetch toasts
/// `internal_error`. Cloning shares the page.
#[derive(Clone)]
pub struct ExtensionLanguagesPage {
    inner: Rc<ExtLangInner>,
    root: gtk::Box,
}

impl ExtensionLanguagesPage {
    pub fn new(languages: Pref<BTreeSet<String>>, toast: Rc<dyn Fn(String)>) -> Self {
        let (stack, column) = page_stack(&tr(MsgKey::empty_screen));
        let inner = Rc::new(ExtLangInner { languages, toast, stack, column });
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&inner.stack);
        keep_alive(&root, &inner);
        Self { inner, root }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// The extensions whose languages are offered, replacing the list. An empty list shows `empty_screen`.
    pub fn set_exts(&self, exts: &[Ext]) {
        let inner = &self.inner;
        clear(&inner.column);
        let rows = extension_language_rows(exts, &inner.languages.get());
        if rows.is_empty() {
            inner.stack.set_visible_child_name("empty");
            return;
        }
        for row in rows {
            let (weak, lang) = (Rc::downgrade(inner), row.lang.clone());
            let (widget, _) = switch_row(&row.title, row.enabled, move |on| {
                let Some(inner) = weak.upgrade() else { return };
                let mut set = inner.languages.get();
                if set.contains(&lang) != on {
                    toggle_language(&mut set, &lang);
                    write(&inner.languages, set);
                }
            });
            inner.column.append(&widget);
        }
        inner.stack.set_visible_child_name("list");
    }

    /// The language fetch failed: the toast says `internal_error` and the page shows its empty message.
    pub fn fetch_failed(&self) {
        (self.inner.toast)(tr(MsgKey::internal_error));
        self.inner.stack.set_visible_child_name("empty");
    }
}

#[cfg(test)]
mod pure_tests {
    use super::*;
    use crate::browse::extensions::{ExtSource, ExtState};

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn groups_put_enabled_languages_first_and_hidden_sources_last_in_each() {
        let sources = vec![
            Source::new(1, "Zeta", "en"),
            Source::new(2, "alpha", "en"),
            Source::new(3, "Mid", "fr"),
            Source::new(4, "Other", "de"),
        ];
        let groups = sources_filter_groups(&sources, &set(&["en", "fr"]), &set(&["2"]));
        let langs: Vec<&str> = groups.iter().map(|g| g.lang.as_str()).collect();
        assert_eq!(langs, ["en", "fr", "de"], "the off language goes last");
        assert_eq!(groups[0].sources.iter().map(|s| s.id).collect::<Vec<_>>(), [1, 2], "the hidden source sorts after the shown one");
        assert!(groups[0].sources[0].checked && !groups[0].sources[1].checked, "checked is not being in the hidden set");
        assert!(groups[0].enabled && !groups[2].enabled);
    }

    #[test]
    fn no_sources_means_no_groups() {
        assert!(sources_filter_groups(&[], &set(&["en"]), &set(&[])).is_empty());
    }

    fn ext(lang: Option<&str>, sources: &[(i64, &str)]) -> Ext {
        Ext {
            name: "E".into(),
            pkg: "p".into(),
            version: String::new(),
            lang: lang.map(str::to_owned),
            nsfw: false,
            sources: sources.iter().map(|(id, l)| ExtSource { id: *id, name: "S".into(), lang: (*l).into(), base_url: None }).collect(),
            state: ExtState::Available,
            icon_file: None,
        }
    }

    #[test]
    fn extension_languages_come_from_the_sources_or_the_extension_itself_once_each() {
        let exts = vec![ext(Some("en"), &[(1, "ja"), (2, "ja"), (3, "de")]), ext(Some("fr"), &[]), ext(None, &[])];
        let rows = extension_language_rows(&exts, &set(&["de"]));
        let langs: Vec<&str> = rows.iter().map(|r| r.lang.as_str()).collect();
        assert_eq!(langs[0], "de", "the enabled language is first");
        assert_eq!(langs.len(), 4, "ja once, de, fr and the empty language");
        assert!(langs.contains(&"") && langs.contains(&"fr") && langs.contains(&"ja"));
        assert!(!langs.contains(&"en"), "the extension's own language is not used when it has sources");
        assert!(rows[0].enabled && !rows[1].enabled);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::browse::extensions::{ExtSource, ExtState};

    fn prefs() -> (Pref<BTreeSet<String>>, Pref<BTreeSet<String>>) {
        let store = PrefStore::in_memory();
        (store.pref("source_languages", BTreeSet::from(["en".to_owned()])), store.pref("hidden_catalogues", BTreeSet::new()))
    }

    fn rows(column: &gtk::Box) -> Vec<gtk::Widget> {
        let mut out = Vec::new();
        let mut child = column.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            out.push(widget);
        }
        out
    }

    fn find<T: IsA<gtk::Widget>>(widget: &gtk::Widget) -> Option<T> {
        if let Some(found) = widget.downcast_ref::<T>() {
            return Some(found.clone());
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            if let Some(found) = find::<T>(&w) {
                return Some(found);
            }
            child = w.next_sibling();
        }
        None
    }

    fn the_language_switch_and_the_source_checkbox_write_their_sets_as_strings() {
        let (languages, hidden) = prefs();
        let page = SourcesFilterPage::new(languages.clone(), hidden.clone());
        page.set_sources(vec![Source::new(11, "Alpha", "en"), Source::new(12, "Beta", "en"), Source::new(21, "Gamma", "fr")]);
        let top = rows(&page.inner.column);
        assert_eq!(top.len(), 4, "a header and a list for each of two languages");
        // en is on: its list shows; fr is off: its list is hidden
        assert!(top[1].is_visible() && !top[3].is_visible());

        let fr_switch = find::<gtk::Switch>(&top[2]).expect("fr switch");
        fr_switch.emit_by_name::<bool>("state-set", &[&true]);
        assert!(languages.get().contains("fr"), "the language joins source_languages");
        assert!(top[3].is_visible(), "its sources appear");

        let en_switch = find::<gtk::Switch>(&top[0]).expect("en switch");
        en_switch.emit_by_name::<bool>("state-set", &[&false]);
        assert!(!languages.get().contains("en"));
        assert!(!top[1].is_visible());

        let first = rows(top[1].downcast_ref::<gtk::Box>().expect("list"))[0].clone();
        let check = find::<gtk::CheckButton>(&first).expect("checkbox");
        assert!(check.is_active(), "not hidden means checked");
        let list = first.observe_controllers();
        let click = (0..list.n_items()).filter_map(|i| list.item(i)).find_map(|c| c.downcast::<gtk::GestureClick>().ok()).expect("click");
        click.emit_by_name::<()>("released", &[&1_i32, &0.0_f64, &0.0_f64]);
        assert_eq!(hidden.get(), BTreeSet::from(["11".to_owned()]), "the id is stored as a string");
        assert!(!check.is_active());
        click.emit_by_name::<()>("released", &[&1_i32, &0.0_f64, &0.0_f64]);
        assert!(hidden.get().is_empty(), "a second click shows it again");
    }

    fn language_toggle_keeps_checkboxes() {
        let (languages, hidden) = prefs();
        hidden.set(BTreeSet::from(["12".to_owned()])).expect("hidden");
        let page = SourcesFilterPage::new(languages.clone(), hidden.clone());
        page.set_sources(vec![Source::new(11, "Alpha", "en"), Source::new(12, "Beta", "en")]);
        let header = rows(&page.inner.column).remove(0);
        let switch = find::<gtk::Switch>(&header).expect("switch");
        switch.emit_by_name::<bool>("state-set", &[&false]);
        switch.emit_by_name::<bool>("state-set", &[&true]);
        assert_eq!(hidden.get(), BTreeSet::from(["12".to_owned()]), "switching a language off and on leaves the source checkboxes as they were");
        assert!(languages.get().contains("en"));
    }

    fn no_sources_shows_the_source_filter_empty_screen() {
        let (languages, hidden) = prefs();
        let page = SourcesFilterPage::new(languages, hidden);
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("loading"));
        page.set_sources(Vec::new());
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("empty"));
        let label = find::<gtk::Label>(page.inner.stack.visible_child().as_ref().expect("child")).expect("label");
        assert_eq!(label.label().as_str(), tr(MsgKey::source_filter_empty_screen));
    }

    fn the_filter_page_shows_loading_then_the_groups_and_an_error_toasts_and_pops_once() {
        let (languages, hidden) = prefs();
        let page = SourcesFilterPage::new(languages, hidden);
        let popped: Rc<RefCell<Vec<String>>> = Rc::default();
        let log = Rc::clone(&popped);
        page.set_on_error(Rc::new(move || log.borrow_mut().push(tr(MsgKey::internal_error))));
        page.set_state(false, false, vec![Source::new(1, "Alpha", "en")]);
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("loading"), "not ready: Loading");
        page.set_state(true, false, vec![Source::new(1, "Alpha", "en")]);
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("list"), "ready: the groups");
        page.set_state(true, true, Vec::new());
        assert_eq!(*popped.borrow(), [tr(MsgKey::internal_error)], "an error shows the toast and pops the page");
        page.set_state(true, true, Vec::new());
        assert_eq!(popped.borrow().len(), 1, "once");
    }

    fn ext(lang: &str) -> Ext {
        Ext {
            name: "E".into(),
            pkg: format!("p.{lang}"),
            version: String::new(),
            lang: Some(lang.into()),
            nsfw: false,
            sources: vec![ExtSource { id: 1, name: "S".into(), lang: lang.into(), base_url: None }],
            state: ExtState::Available,
            icon_file: None,
        }
    }

    fn the_extension_language_switches_write_the_set_and_a_failed_fetch_toasts_internal_error() {
        let (languages, _) = prefs();
        let toasts: Rc<RefCell<Vec<String>>> = Rc::default();
        let log = Rc::clone(&toasts);
        let page = ExtensionLanguagesPage::new(languages.clone(), Rc::new(move |text| log.borrow_mut().push(text)));
        page.set_exts(&[ext("en"), ext("ja")]);
        let list = rows(&page.inner.column);
        assert_eq!(list.len(), 2);
        let ja = find::<gtk::Switch>(&list[1]).expect("switch");
        ja.emit_by_name::<bool>("state-set", &[&true]);
        assert_eq!(languages.get(), BTreeSet::from(["en".to_owned(), "ja".to_owned()]));
        let en = find::<gtk::Switch>(&list[0]).expect("switch");
        en.emit_by_name::<bool>("state-set", &[&false]);
        assert_eq!(languages.get(), BTreeSet::from(["ja".to_owned()]));

        page.set_exts(&[]);
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("empty"), "no languages: the empty screen");
        assert!(toasts.borrow().is_empty());
        page.fetch_failed();
        assert_eq!(*toasts.borrow(), [tr(MsgKey::internal_error)]);
    }

    pub(crate) fn smoke() {
        the_language_switch_and_the_source_checkbox_write_their_sets_as_strings();
        language_toggle_keeps_checkboxes();
        no_sources_shows_the_source_filter_empty_screen();
        the_filter_page_shows_loading_then_the_groups_and_an_error_toasts_and_pops_once();
        the_extension_language_switches_write_the_set_and_a_failed_fetch_toasts_internal_error();
    }

    #[test]
    fn the_filter_page_states_follow_readiness_then_failure_then_the_groups() {
        let sources = [Source::new(1, "Zed", "en")];
        let (enabled, disabled) = (BTreeSet::from(["en".to_owned()]), BTreeSet::new());
        assert_eq!(filter_screen(false, false, &sources, &enabled, &disabled), FilterScreen::Loading);
        assert_eq!(filter_screen(false, true, &sources, &enabled, &disabled), FilterScreen::Loading, "loading until the registry is ready");
        assert_eq!(filter_screen(true, true, &sources, &enabled, &disabled), FilterScreen::Error);
        assert!(error_pops_the_page(&FilterScreen::Error), "E-SF-05: an error shows the toast and pops the page");
        assert!(!error_pops_the_page(&FilterScreen::Loading));
        match filter_screen(true, false, &sources, &enabled, &disabled) {
            FilterScreen::Success(groups) => assert_eq!(groups.len(), 1),
            other => panic!("a success has groups: {other:?}"),
        }
        assert_eq!(filter_screen(true, false, &[], &enabled, &disabled), FilterScreen::Success(vec![]), "an empty success is the empty screen");
    }
}
