//! The two language filters: the Sources language filter (one switch per language, T-40a-009) and the extension language
//! page (one switch per language, T-40a-020). Both write `source_languages` through the same toggle. The language filter
//! is the only way to hide sources: there are no per-source hide or pin toggles. Ports `MangaSourcesFilterScreen` and
//! `MangaExtensionFilterScreen` with their screen models; the rules are pure and the widgets are thin.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_domain::source::model::Source;
use kuro_domain::source::toggles::toggle_language;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::pref::Pref;

use super::glue::keep_alive;
use super::names::{case_insensitive_cmp, compare_languages, localized_display_name};

/// One source of a language group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCheck {
    pub id: i64,
    pub name: String,
}

/// One language and the sources under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LangGroup {
    pub lang: String,
    pub title: String,
    /// The switch: the language is in `source_languages`.
    pub enabled: bool,
    pub sources: Vec<SourceCheck>,
}

/// `GetLanguagesWithMangaSources`: sources sorted by name ignoring case, grouped by language, the groups sorted by
/// `(language off, language order)`. Every source of a language is listed; the language switch decides whether it shows.
pub fn sources_filter_groups(sources: &[Source], enabled: &BTreeSet<String>) -> Vec<LangGroup> {
    // Only online sources are listed (E-SF-04): a stub and the local source are not HTTP sources.
    let mut sorted: Vec<&Source> = sources.iter().filter(|s| !s.is_stub && s.id != super::sources::LOCAL_SOURCE_ID).collect();
    sorted.sort_by(|a, b| case_insensitive_cmp(&a.name, &b.name));
    let mut groups: Vec<LangGroup> = Vec::new();
    for source in sorted {
        let check = SourceCheck { id: source.id, name: source.name.clone() };
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
pub fn filter_screen(ready: bool, failed: bool, sources: &[Source], enabled: &BTreeSet<String>) -> FilterScreen {
    match (ready, failed) {
        (false, _) => FilterScreen::Loading,
        (true, true) => FilterScreen::Error,
        (true, false) => FilterScreen::Success(sources_filter_groups(sources, enabled)),
    }
}

/// What an error does: the page shows `internal_error` as a toast and pops itself (E-SF-05).
pub fn error_pops_the_page(screen: &FilterScreen) -> bool {
    *screen == FilterScreen::Error
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

/// A stack of spinner, empty message and list. The page shows the spinner until its models arrive.
fn page_stack(empty_text: &str) -> (gtk::Stack, gtk::Box) {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroller = gtk::ScrolledWindow::builder().child(&column).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build();
    let stack = gtk::Stack::new();
    stack.add_named(&crate::ui::loading(crate::ui::LoadSize::Page), Some("loading"));
    stack.add_named(&super::look::empty_state("view-list-symbolic", empty_text, None).0, Some("empty"));
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

/// The Sources language filter page. It reads `source_languages` when it draws and writes it when a switch is flipped, so
/// the stored set is the one truth. Cloning shares the page.
#[derive(Clone)]
pub struct SourcesFilterPage {
    inner: Rc<SourcesInner>,
    root: gtk::Box,
}

impl SourcesFilterPage {
    /// `languages` is `source_languages`.
    pub fn new(languages: Pref<BTreeSet<String>>) -> Self {
        let (stack, column) = page_stack(&tr(MsgKey::source_filter_empty_screen));
        let inner = Rc::new(SourcesInner {
            languages,
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
        let screen = filter_screen(self.ready.get(), self.failed.get(), &self.sources.borrow(), &self.languages.get());
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
            let (weak, lang) = (Rc::downgrade(self), group.lang.clone());
            let (header, _) = switch_row(&group.title, group.enabled, move |on| {
                if let Some(inner) = weak.upgrade() {
                    let mut set = inner.languages.get();
                    if set.contains(&lang) != on {
                        toggle_language(&mut set, &lang);
                        write(&inner.languages, set);
                    }
                }
            });
            self.column.append(&header);
        }
        self.stack.set_visible_child_name("list");
    }
}

#[cfg(test)]
mod pure_tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn groups_put_enabled_languages_first_and_sources_by_name_in_each() {
        let sources = vec![
            Source::new(1, "Zeta", "en"),
            Source::new(2, "alpha", "en"),
            Source::new(3, "Mid", "fr"),
            Source::new(4, "Other", "de"),
        ];
        let groups = sources_filter_groups(&sources, &set(&["en", "fr"]));
        let langs: Vec<&str> = groups.iter().map(|g| g.lang.as_str()).collect();
        assert_eq!(langs, ["en", "fr", "de"], "the off language goes last");
        assert_eq!(groups[0].sources.iter().map(|s| s.id).collect::<Vec<_>>(), [2, 1], "names ignore case");
        assert!(groups[0].enabled && !groups[2].enabled);
    }

    #[test]
    fn no_sources_means_no_groups() {
        assert!(sources_filter_groups(&[], &set(&["en"])).is_empty());
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;

    fn prefs() -> Pref<BTreeSet<String>> {
        PrefStore::in_memory().pref("source_languages", BTreeSet::from(["en".to_owned()]))
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

    fn the_language_switches_write_the_set_and_a_switched_off_language_has_no_list() {
        let languages = prefs();
        let page = SourcesFilterPage::new(languages.clone());
        page.set_sources(vec![Source::new(11, "Alpha", "en"), Source::new(12, "Beta", "en"), Source::new(21, "Gamma", "fr")]);
        let top = rows(&page.inner.column);
        assert_eq!(top.len(), 2, "one switch row per language, and no per-source rows");
        let fr_switch = find::<gtk::Switch>(&top[1]).expect("fr switch");
        fr_switch.emit_by_name::<bool>("state-set", &[&true]);
        assert!(languages.get().contains("fr"), "the language joins source_languages");
        let en_switch = find::<gtk::Switch>(&top[0]).expect("en switch");
        en_switch.emit_by_name::<bool>("state-set", &[&false]);
        assert!(!languages.get().contains("en"), "the language leaves source_languages");
        assert!(find::<gtk::CheckButton>(page.inner.column.upcast_ref()).is_none(), "no source checkboxes: the language filter is the only filter");
    }

    fn no_sources_shows_the_source_filter_empty_screen() {
        let page = SourcesFilterPage::new(prefs());
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("loading"));
        page.set_sources(Vec::new());
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("empty"));
        let label = find::<gtk::Label>(page.inner.stack.visible_child().as_ref().expect("child")).expect("label");
        assert_eq!(label.label().as_str(), tr(MsgKey::source_filter_empty_screen));
    }

    fn the_filter_page_shows_loading_then_the_groups_and_an_error_toasts_and_pops_once() {
        let page = SourcesFilterPage::new(prefs());
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

    pub(crate) fn smoke() {
        the_language_switches_write_the_set_and_a_switched_off_language_has_no_list();
        no_sources_shows_the_source_filter_empty_screen();
        the_filter_page_shows_loading_then_the_groups_and_an_error_toasts_and_pops_once();
    }

    #[test]
    fn the_filter_page_states_follow_readiness_then_failure_then_the_groups() {
        let sources = [Source::new(1, "Zed", "en")];
        let enabled = BTreeSet::from(["en".to_owned()]);
        assert_eq!(filter_screen(false, false, &sources, &enabled), FilterScreen::Loading);
        assert_eq!(filter_screen(false, true, &sources, &enabled), FilterScreen::Loading, "loading until the registry is ready");
        assert_eq!(filter_screen(true, true, &sources, &enabled), FilterScreen::Error);
        assert!(error_pops_the_page(&FilterScreen::Error), "E-SF-05: an error shows the toast and pops the page");
        assert!(!error_pops_the_page(&FilterScreen::Loading));
        match filter_screen(true, false, &sources, &enabled) {
            FilterScreen::Success(groups) => assert_eq!(groups.len(), 1),
            other => panic!("a success has groups: {other:?}"),
        }
        assert_eq!(filter_screen(true, false, &[], &enabled), FilterScreen::Success(vec![]), "an empty success is the empty screen");
    }
}
