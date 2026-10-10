//! The extension details page: the header (112 px icon, name, package, version, language and NSFW age rating, uninstall,
//! incognito switch), the obsolete banner, the source rows with their switches and settings buttons, and the toolbar
//! with the repository link and the enable all, disable all and clear cookies actions (T-40a-021, T-40a-022). The
//! rules are in `ext_details`; this file draws them and writes the preferences.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_domain::source::toggles::flip_disabled;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::pref::Pref;

use super::dialogs::{ButtonStyle, DialogSlot, DialogSpec};
use super::ext_details::{ExtDetails, cleared_log, clear_cookies, debug_text, detail_rows, repo_link, set_all_enabled, stripped_pkg};
use super::glue::keep_alive;
use super::icons::{DETAILS_ICON_PX, resolve};
use super::names::localized_display_name;
use super::view::icon_image_sized;

/// What the page needs from the app.
pub struct DetailsDeps {
    /// `incognito_extensions` of the kind: the package names whose history is paused.
    pub incognito: Pref<BTreeSet<String>>,
    /// `hidden_catalogues` of the kind: the disabled source ids, as strings.
    pub disabled: Pref<BTreeSet<String>>,
    /// The Uninstall button.
    pub uninstall: Rc<dyn Fn()>,
    /// The settings button of a configurable source.
    pub open_source_prefs: Rc<dyn Fn(i64)>,
    /// The open-repo action.
    pub open_url: Rc<dyn Fn(&str)>,
    /// Removes the cookies of one base url and returns how many it dropped.
    pub clear_cookies: Rc<dyn Fn(&str) -> usize>,
    /// A short message at the bottom of the window.
    pub toast: Rc<dyn Fn(String)>,
}

struct Inner {
    deps: DetailsDeps,
    stack: gtk::Stack,
    column: gtk::Box,
    toolbar: gtk::Box,
    dialogs: DialogSlot,
    details: RefCell<Option<ExtDetails>>,
    /// The switch of each source row, by source id, for enable all and disable all.
    switches: RefCell<Vec<(i64, gtk::Switch)>>,
    repo_button: gtk::Button,
}

/// Cloning shares the page; the widget keeps it alive.
#[derive(Clone)]
pub struct ExtDetailsPage {
    inner: Rc<Inner>,
    root: gtk::Box,
}

impl ExtDetailsPage {
    pub fn new(deps: DetailsDeps) -> Self {
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.set_margin_start(16);
        toolbar.set_margin_end(16);
        toolbar.set_margin_top(8);
        toolbar.append(&gtk::Label::builder().label(tr(MsgKey::label_extension_info)).hexpand(true).xalign(0.0).css_classes(["title-4"]).build());
        let repo_button = gtk::Button::from_icon_name("web-browser-symbolic");
        repo_button.set_tooltip_text(Some(&tr(MsgKey::action_open_repo)));
        repo_button.set_visible(false);
        toolbar.append(&repo_button);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroller = gtk::ScrolledWindow::builder().child(&column).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build();
        let empty = gtk::Label::new(Some(&tr(MsgKey::empty_screen)));
        empty.set_valign(gtk::Align::Center);
        empty.set_vexpand(true);
        let stack = gtk::Stack::new();
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&scroller, Some("details"));
        stack.set_vexpand(true);
        stack.set_visible_child_name("empty");

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&toolbar);
        root.append(&stack);
        let inner = Rc::new(Inner {
            deps,
            stack,
            column,
            toolbar,
            dialogs: DialogSlot::new(),
            details: RefCell::new(None),
            switches: RefCell::new(Vec::new()),
            repo_button,
        });
        keep_alive(&root, &inner);
        inner.add_overflow();
        let weak = Rc::downgrade(&inner);
        inner.repo_button.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.open_repo();
            }
        });
        Self { inner, root }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// The extension to show. `None` shows `empty_screen` (the extension is not installed any more).
    pub fn set_details(&self, details: Option<ExtDetails>) {
        *self.inner.details.borrow_mut() = details;
        self.inner.render();
    }

    /// Enable all (`true`) or disable all (`false`): every source id of the extension changes in one write.
    pub fn set_all(&self, enable: bool) {
        self.inner.set_all(enable);
    }

    /// Clears the cookies of the extension's sources and logs how many went. Nothing else shows.
    pub fn clear_cookies(&self) {
        self.inner.clear_cookies();
    }
}

impl Inner {
    fn add_overflow(self: &Rc<Self>) {
        let entries = vec![tr(MsgKey::action_enable_all), tr(MsgKey::action_disable_all), tr(MsgKey::pref_clear_cookies)];
        let weak = Rc::downgrade(self);
        let menu = crate::ui::menu_button("view-more-symbolic", &entries, move |label| {
            let Some(inner) = weak.upgrade() else { return };
            if label == tr(MsgKey::action_enable_all) {
                inner.set_all(true);
            } else if label == tr(MsgKey::action_disable_all) {
                inner.set_all(false);
            } else {
                inner.clear_cookies();
            }
        });
        self.toolbar.append(&menu);
    }

    fn open_repo(&self) {
        let link = self.details.borrow().as_ref().and_then(|d| repo_link(d.repo_url.as_deref()));
        if let Some(link) = link {
            (self.deps.open_url)(&link);
        }
    }

    fn set_all(&self, enable: bool) {
        let Some(ids) = self.details.borrow().as_ref().map(|d| d.sources.iter().map(|s| s.id).collect::<Vec<_>>()) else { return };
        let next = set_all_enabled(&self.deps.disabled.get(), &ids, enable);
        if let Err(error) = self.deps.disabled.set(next) {
            tracing::warn!(%error, "the enabled sources could not be stored");
            return;
        }
        for (_, switch) in self.switches.borrow().iter() {
            switch.set_active(enable);
        }
    }

    fn clear_cookies(&self) {
        let Some(sources) = self.details.borrow().as_ref().map(|d| d.sources.clone()) else { return };
        let (urls, cleared) = clear_cookies(&sources, |url| (self.deps.clear_cookies)(url));
        tracing::info!("{}", cleared_log(cleared, &urls));
    }

    fn render(self: &Rc<Self>) {
        while let Some(child) = self.column.first_child() {
            self.column.remove(&child);
        }
        self.switches.borrow_mut().clear();
        let Some(details) = self.details.borrow().clone() else {
            self.repo_button.set_visible(false);
            self.stack.set_visible_child_name("empty");
            return;
        };
        self.repo_button.set_visible(repo_link(details.repo_url.as_deref()).is_some());
        if details.obsolete {
            let banner = gtk::Label::builder().label(tr(MsgKey::obsolete_extension_message)).wrap(true).xalign(0.0).build();
            banner.add_css_class("error");
            banner.set_margin_start(16);
            banner.set_margin_end(16);
            banner.set_margin_top(12);
            self.column.append(&banner);
        }
        self.column.append(&self.header(&details));
        self.column.append(&self.incognito_row(&details.pkg));
        self.column.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        for row in detail_rows(&details.sources, &self.deps.disabled.get()) {
            self.column.append(&self.source_row(&row));
        }
        self.stack.set_visible_child_name("details");
    }

    fn header(self: &Rc<Self>, d: &ExtDetails) -> gtk::Box {
        let header = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let top = gtk::Box::new(gtk::Orientation::Vertical, 4);
        top.set_margin_start(16);
        top.set_margin_end(16);
        top.set_margin_top(16);
        top.set_halign(gtk::Align::Fill);
        let icon = icon_image_sized(resolve(false, d.icon_file.is_some(), false), d.icon_file.as_deref(), DETAILS_ICON_PX);
        icon.set_halign(gtk::Align::Center);
        top.append(&icon);
        top.append(&gtk::Label::builder().label(&d.name).css_classes(["title-2"]).wrap(true).justify(gtk::Justification::Center).build());
        top.append(&gtk::Label::builder().label(stripped_pkg(&d.pkg)).css_classes(["caption"]).build());
        // a click anywhere on the icon block copies the debug text
        let (weak, text) = (Rc::downgrade(self), debug_text(d));
        let copy = gtk::GestureClick::new();
        copy.connect_released(move |gesture, _, _, _| {
            let (Some(inner), Some(widget)) = (weak.upgrade(), gesture.widget()) else { return };
            widget.clipboard().set_text(&text);
            (inner.deps.toast)(tr(MsgKey::copied_to_clipboard_plain));
        });
        top.add_controller(copy);
        header.append(&top);

        let info = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        info.set_homogeneous(false);
        info.set_halign(gtk::Align::Center);
        info.append(&info_text(&d.version_name, &tr(MsgKey::ext_info_version), None));
        info.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        info.append(&info_text(&localized_display_name(&d.lang), &tr(MsgKey::ext_info_language), None));
        if d.nsfw {
            info.append(&gtk::Separator::new(gtk::Orientation::Vertical));
            let weak = Rc::downgrade(self);
            let age = info_text(&tr(MsgKey::ext_nsfw_short), &format!("{} \u{24D8}", tr(MsgKey::ext_info_age_rating)), Some("error"));
            let click = gtk::GestureClick::new();
            click.connect_released(move |_, _, _, _| {
                if let Some(inner) = weak.upgrade() {
                    inner.show_age_rating();
                }
            });
            age.add_controller(click);
            info.append(&age);
        }
        header.append(&info);

        let uninstall = gtk::Button::with_label(&tr(MsgKey::ext_uninstall));
        uninstall.set_margin_start(16);
        uninstall.set_margin_end(16);
        let weak = Rc::downgrade(self);
        uninstall.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                (inner.deps.uninstall)();
            }
        });
        header.append(&uninstall);
        header
    }

    fn show_age_rating(&self) {
        let spec = DialogSpec {
            title: tr(MsgKey::ext_info_age_rating),
            body: Some(tr(MsgKey::ext_nsfw_warning)),
            buttons: vec![(tr(MsgKey::action_ok), ButtonStyle::Suggested)],
        };
        self.dialogs.show(&self.column, spec, |_| {});
    }

    fn incognito_row(self: &Rc<Self>, pkg: &str) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        row.set_margin_start(16);
        row.set_margin_end(16);
        row.set_margin_top(12);
        row.set_margin_bottom(12);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        text.append(&gtk::Label::builder().label(tr(MsgKey::pref_incognito_mode)).xalign(0.0).build());
        text.append(&gtk::Label::builder().label(tr(MsgKey::pref_incognito_mode_extension_summary)).xalign(0.0).css_classes(["dim-label"]).wrap(true).build());
        row.append(&text);
        let switch = gtk::Switch::builder().active(self.deps.incognito.get().contains(pkg)).valign(gtk::Align::Center).build();
        let (weak, pkg) = (Rc::downgrade(self), pkg.to_owned());
        switch.connect_state_set(move |switch, on| {
            if let Some(inner) = weak.upgrade() {
                let mut set = inner.deps.incognito.get();
                if on { set.insert(pkg.clone()) } else { set.remove(&pkg) };
                if let Err(error) = inner.deps.incognito.set(set) {
                    tracing::warn!(%error, "the incognito extensions could not be stored");
                }
            }
            switch.set_state(on);
            gtk::glib::Propagation::Stop
        });
        row.append(&switch);
        row
    }

    fn source_row(self: &Rc<Self>, row: &super::ext_details::DetailRow) -> gtk::Box {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        widget.set_margin_start(16);
        widget.set_margin_end(16);
        widget.set_margin_top(8);
        widget.set_margin_bottom(8);
        widget.append(&gtk::Label::builder().label(row.title()).hexpand(true).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build());
        let id = row.row.id;
        if row.configurable {
            let settings = gtk::Button::from_icon_name("emblem-system-symbolic");
            settings.add_css_class("flat");
            settings.set_tooltip_text(Some(&tr(MsgKey::label_settings)));
            let weak = Rc::downgrade(self);
            settings.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    (inner.deps.open_source_prefs)(id);
                }
            });
            widget.append(&settings);
        }
        // the switch only shows the state; the row is what toggles it
        let switch = gtk::Switch::builder().active(row.row.enabled).valign(gtk::Align::Center).can_target(false).focusable(false).build();
        widget.append(&switch);
        self.switches.borrow_mut().push((id, switch.clone()));
        let weak = Rc::downgrade(self);
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, _, _| {
            let Some(inner) = weak.upgrade() else { return };
            let mut disabled = inner.deps.disabled.get();
            let now_disabled = flip_disabled(&mut disabled, id);
            if let Err(error) = inner.deps.disabled.set(disabled) {
                tracing::warn!(%error, "the enabled sources could not be stored");
                return;
            }
            switch.set_active(!now_disabled);
        });
        widget.add_controller(click);
        widget
    }
}

/// A primary line over a secondary one, centred. `class` styles the primary line.
fn info_text(primary: &str, secondary: &str, class: Option<&str>) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let top = gtk::Label::new(Some(primary));
    if let Some(class) = class {
        top.add_css_class(class);
    }
    column.append(&top);
    column.append(&gtk::Label::builder().label(secondary).css_classes(["caption", "dim-label"]).build());
    column
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::Cell;

    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::browse::ext_details::DetailSource;

    struct Rig {
        page: ExtDetailsPage,
        incognito: Pref<BTreeSet<String>>,
        disabled: Pref<BTreeSet<String>>,
        uninstalled: Rc<Cell<u32>>,
        opened: Rc<RefCell<Vec<String>>>,
        prefs_for: Rc<RefCell<Vec<i64>>>,
        toasts: Rc<RefCell<Vec<String>>>,
        cookie_calls: Rc<RefCell<Vec<String>>>,
    }

    fn rig() -> Rig {
        let store = PrefStore::in_memory();
        let incognito = store.pref("incognito_extensions", BTreeSet::<String>::new());
        let disabled = store.pref("hidden_catalogues", BTreeSet::<String>::new());
        let (uninstalled, opened, prefs_for, toasts, cookie_calls) = (Rc::new(Cell::new(0)), Rc::default(), Rc::default(), Rc::default(), Rc::default());
        let deps = DetailsDeps {
            incognito: incognito.clone(),
            disabled: disabled.clone(),
            uninstall: {
                let n = Rc::clone(&uninstalled);
                Rc::new(move || n.set(n.get() + 1))
            },
            open_source_prefs: {
                let log: Rc<RefCell<Vec<i64>>> = Rc::clone(&prefs_for);
                Rc::new(move |id| log.borrow_mut().push(id))
            },
            open_url: {
                let log: Rc<RefCell<Vec<String>>> = Rc::clone(&opened);
                Rc::new(move |url| log.borrow_mut().push(url.to_owned()))
            },
            clear_cookies: {
                let log: Rc<RefCell<Vec<String>>> = Rc::clone(&cookie_calls);
                Rc::new(move |url| {
                    log.borrow_mut().push(url.to_owned());
                    2
                })
            },
            toast: {
                let log: Rc<RefCell<Vec<String>>> = Rc::clone(&toasts);
                Rc::new(move |text| log.borrow_mut().push(text))
            },
        };
        Rig { page: ExtDetailsPage::new(deps), incognito, disabled, uninstalled, opened, prefs_for, toasts, cookie_calls }
    }

    fn details(nsfw: bool, obsolete: bool) -> ExtDetails {
        let source = |id: i64, name: &str, configurable: bool, base: &str| DetailSource {
            id,
            name: name.into(),
            lang: "en".into(),
            label_as_name: true,
            configurable,
            base_url: Some(base.into()),
        };
        ExtDetails {
            name: "Foo".into(),
            pkg: "eu.kanade.tachiyomi.extension.en.foo".into(),
            lang: "en".into(),
            version_name: "1.4.12".into(),
            lib_version: Some(1.4),
            version_code: 12,
            nsfw,
            has_update: false,
            obsolete,
            shared: false,
            repo_url: Some("https://raw.githubusercontent.com/user/repo/main/index.min.json".into()),
            icon_file: None,
            sources: vec![source(1, "Alpha", true, "https://a.test"), source(2, "Beta", false, "https://a.test"), source(3, "Gamma", false, "https://g.test")],
        }
    }

    fn find_all<T: IsA<gtk::Widget>>(widget: &gtk::Widget, out: &mut Vec<T>) {
        if let Some(found) = widget.downcast_ref::<T>() {
            out.push(found.clone());
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            find_all(&w, out);
            child = w.next_sibling();
        }
    }

    fn all<T: IsA<gtk::Widget>>(widget: &impl IsA<gtk::Widget>) -> Vec<T> {
        let mut out = Vec::new();
        find_all(widget.upcast_ref(), &mut out);
        out
    }

    fn labels(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
        all::<gtk::Label>(widget).into_iter().map(|l| l.label().to_string()).collect()
    }

    fn click(widget: &impl IsA<gtk::Widget>) {
        let list = widget.observe_controllers();
        let gesture = (0..list.n_items()).filter_map(|i| list.item(i)).find_map(|c| c.downcast::<gtk::GestureClick>().ok()).expect("click");
        gesture.emit_by_name::<()>("released", &[&1_i32, &0.0_f64, &0.0_f64]);
    }

    fn the_header_shows_the_facts_the_banner_and_the_age_rating_dialog() {
        let rig = rig();
        assert_eq!(rig.page.inner.stack.visible_child_name().as_deref(), Some("empty"), "no extension: empty_screen");
        rig.page.set_details(Some(details(true, true)));
        let text = labels(&rig.page.inner.column);
        for expected in ["Foo", "en.foo", "1.4.12", &tr(MsgKey::ext_nsfw_short), &tr(MsgKey::obsolete_extension_message), &tr(MsgKey::pref_incognito_mode)] {
            assert!(text.iter().any(|t| t == expected), "{expected} is on the page: {text:?}");
        }
        let icons = all::<gtk::Image>(&rig.page.inner.column);
        assert!(icons.iter().any(|i| i.pixel_size() == DETAILS_ICON_PX), "the header icon is 112 px");

        // the age rating opens the NSFW dialog; OK closes it
        let age = all::<gtk::Box>(&rig.page.inner.column).into_iter().find(|b| labels(b).contains(&tr(MsgKey::ext_nsfw_short)) && b.observe_controllers().n_items() > 0).expect("age rating");
        click(&age);
        let window = rig.page.inner.dialogs.current().expect("the NSFW dialog is open");
        assert!(labels(&window.child().expect("child")).contains(&tr(MsgKey::ext_nsfw_warning)));
        crate::browse::dialogs::buttons_of(&window)[0].emit_clicked();
        assert!(rig.page.inner.dialogs.current().is_none());

        // without NSFW and without the obsolete flag there is neither
        rig.page.set_details(Some(details(false, false)));
        let text = labels(&rig.page.inner.column);
        assert!(!text.contains(&tr(MsgKey::ext_nsfw_short)) && !text.contains(&tr(MsgKey::obsolete_extension_message)));
    }

    fn a_header_click_copies_the_debug_text_and_toasts() {
        let rig = rig();
        rig.page.set_details(Some(details(false, false)));
        let top = all::<gtk::Box>(&rig.page.inner.column).into_iter().find(|b| labels(b).contains(&"Foo".to_owned()) && b.observe_controllers().n_items() > 0).expect("icon block");
        click(&top);
        assert_eq!(*rig.toasts.borrow(), [tr(MsgKey::copied_to_clipboard_plain)]);
    }

    fn the_repo_action_shows_only_with_a_link_and_opens_the_github_page() {
        let rig = rig();
        rig.page.set_details(Some(details(false, false)));
        assert!(rig.page.inner.repo_button.is_visible());
        rig.page.inner.repo_button.emit_clicked();
        assert_eq!(*rig.opened.borrow(), ["https://github.com/user/repo"]);
        let mut none = details(false, false);
        none.repo_url = None;
        rig.page.set_details(Some(none));
        assert!(!rig.page.inner.repo_button.is_visible(), "no repo, no action");
    }

    fn the_incognito_switch_writes_the_package_into_the_set() {
        let rig = rig();
        rig.page.set_details(Some(details(false, false)));
        let switch = all::<gtk::Switch>(&rig.page.inner.column).into_iter().find(|s| s.can_target()).expect("incognito switch");
        switch.emit_by_name::<bool>("state-set", &[&true]);
        assert!(rig.incognito.get().contains("eu.kanade.tachiyomi.extension.en.foo"));
        switch.emit_by_name::<bool>("state-set", &[&false]);
        assert!(rig.incognito.get().is_empty());
    }

    fn source_rows_toggle_on_a_click_and_enable_all_and_disable_all_write_every_id() {
        let rig = rig();
        rig.page.set_details(Some(details(false, false)));
        let rows: Vec<gtk::Box> = {
            let mut out = Vec::new();
            let mut child = rig.page.inner.column.first_child();
            while let Some(w) = child {
                child = w.next_sibling();
                if let Some(b) = w.downcast_ref::<gtk::Box>() {
                    if labels(b) == ["Alpha"] || labels(b) == ["Beta"] || labels(b) == ["Gamma"] {
                        out.push(b.clone());
                    }
                }
            }
            out
        };
        assert_eq!(rows.len(), 3, "one row per source, in label order");
        click(&rows[1]);
        assert_eq!(rig.disabled.get(), BTreeSet::from(["2".to_owned()]), "a click disables; the id is a string");
        let switch = all::<gtk::Switch>(&rows[1])[0].clone();
        assert!(!switch.is_active());
        click(&rows[1]);
        assert!(rig.disabled.get().is_empty());

        rig.page.set_all(false);
        assert_eq!(rig.disabled.get().len(), 3, "disable all acts on every id of the extension");
        assert!(rows.iter().all(|r| !all::<gtk::Switch>(r)[0].is_active()));
        rig.page.set_all(true);
        assert!(rig.disabled.get().is_empty());
        assert!(rows.iter().all(|r| all::<gtk::Switch>(r)[0].is_active()));

        // only the configurable source has the settings button, and it opens that source's preferences
        let with_button: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| !all::<gtk::Button>(*r).is_empty()).map(|(i, _)| i).collect();
        assert_eq!(with_button, [0], "Alpha is the configurable one");
        all::<gtk::Button>(&rows[0])[0].emit_clicked();
        assert_eq!(*rig.prefs_for.borrow(), [1]);
    }

    fn clearing_cookies_goes_through_each_distinct_base_url_once() {
        let rig = rig();
        rig.page.set_details(Some(details(false, false)));
        rig.page.clear_cookies();
        assert_eq!(*rig.cookie_calls.borrow(), ["https://a.test", "https://g.test"]);
        assert!(rig.toasts.borrow().is_empty(), "there is no feedback beyond the log");
    }

    fn the_uninstall_button_calls_back_and_a_vanished_extension_shows_the_empty_screen() {
        let rig = rig();
        rig.page.set_details(Some(details(false, false)));
        let button = all::<gtk::Button>(&rig.page.inner.column).into_iter().find(|b| b.label().as_deref() == Some(tr(MsgKey::ext_uninstall).as_str())).expect("uninstall");
        button.emit_clicked();
        assert_eq!(rig.uninstalled.get(), 1);
        rig.page.set_details(None);
        assert_eq!(rig.page.inner.stack.visible_child_name().as_deref(), Some("empty"));
    }

    pub(crate) fn smoke() {
        the_header_shows_the_facts_the_banner_and_the_age_rating_dialog();
        a_header_click_copies_the_debug_text_and_toasts();
        the_repo_action_shows_only_with_a_link_and_opens_the_github_page();
        the_incognito_switch_writes_the_package_into_the_set();
        source_rows_toggle_on_a_click_and_enable_all_and_disable_all_write_every_id();
        clearing_cookies_goes_through_each_distinct_base_url_once();
        the_uninstall_button_calls_back_and_a_vanished_extension_shows_the_empty_screen();
    }
}
