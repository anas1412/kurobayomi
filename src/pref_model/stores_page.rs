//! The GTK page over `stores::Page`, for the anime extension stores (T-47b-009 to T-47b-018) and the manga extension
//! repos (T-47b-019): a spinner while loading, the empty text, the card list, the Add FAB that shrinks on scroll, the
//! add, remove and deep-link dialogs, and the header Refresh with its spinner. The page owns no rules: each user action
//! is an [`Event`] for the reducer, and the returned [`Effect`] runs on a worker through [`Ops`].

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use kuro_i18n::{Arg, MsgKey, tr, trf};

use super::stores::{self, Dialog, Effect, Entry, Event, Keyed, Page};
use super::stores_backend::{AddOutcome, Card, Ops};
use super::strings;
use super::widgets::dialog as dialog_widget;
use crate::screens::Built;

thread_local! {
    /// What runs after the stores or repos changed, so the available extensions are read again.
    static RELOAD: RefCell<Vec<Rc<dyn Fn()>>> = const { RefCell::new(Vec::new()) };
}

/// Registers a listener for "the list of available extensions must be loaded again". The extension browser registers
/// here; the add, remove and refresh flows call [`reload_extensions`] when they finish.
pub fn on_extensions_reload(listener: impl Fn() + 'static) {
    RELOAD.with(|l| l.borrow_mut().push(Rc::new(listener)));
}

/// Tells every listener that the available extensions must be loaded again.
pub fn reload_extensions() {
    let listeners: Vec<Rc<dyn Fn()>> = RELOAD.with(|l| l.borrow().clone());
    for listener in listeners {
        listener();
    }
}

/// The fixed texts of one page.
pub struct Texts {
    pub empty: String,
    pub add_title: String,
    /// A line above the URL entry of the add dialog.
    pub add_intro: Option<String>,
    pub input_label: String,
    /// The line above the read-only URL of the deep-link dialog.
    pub deep_link_intro: Option<String>,
    pub remove_title: String,
    pub remove_body: fn(&str, &str) -> String,
    pub open_website: String,
    pub copy: String,
    pub delete: String,
    pub refresh: String,
    pub add: String,
}

impl Texts {
    pub fn stores() -> Self {
        Self {
            empty: stores::empty_text(),
            add_title: tr(MsgKey::extensionStoresScreen_addStore_title),
            add_intro: None,
            input_label: tr(MsgKey::extensionStoresScreen_addStoreInput_inputLabel),
            deep_link_intro: Some(tr(MsgKey::extensionStoresScreen_addStoreDeeplink_bodyText)),
            remove_title: stores::remove_title(),
            remove_body: super::stores_backend::store_remove_body,
            open_website: tr(MsgKey::action_open_in_browser),
            copy: tr(MsgKey::action_copy_to_clipboard),
            delete: tr(MsgKey::action_delete),
            refresh: tr(MsgKey::action_webview_refresh),
            add: tr(MsgKey::action_add),
        }
    }

    pub fn repos() -> Self {
        let app = "Kurobayomi";
        Self {
            empty: tr(MsgKey::information_empty_repos),
            add_title: tr(MsgKey::action_add_repo),
            add_intro: Some(trf(MsgKey::action_add_repo_message, &[("arg1", Arg::Str(app))])),
            input_label: tr(MsgKey::label_add_repo_input),
            deep_link_intro: None,
            remove_title: tr(MsgKey::action_delete_repo),
            remove_body: super::stores_backend::repo_remove_body,
            open_website: tr(MsgKey::action_open_in_browser),
            copy: tr(MsgKey::action_copy_to_clipboard),
            delete: tr(MsgKey::action_delete),
            refresh: tr(MsgKey::action_webview_refresh),
            add: tr(MsgKey::action_add),
        }
    }
}

/// Whether the FAB shows its label: at the top of the list, after scrolling back, or when nothing is left to scroll to
/// (`shouldExpandFAB`).
pub fn fab_expanded_at(value: f64, previous: f64, page_size: f64, upper: f64) -> bool {
    let first_visible = usize::from(value > 0.0);
    crate::entry_core::anime::layout::fab_expanded(first_visible, 0, value < previous, value + page_size < upper - 1.0)
}

/// Opens a link in the default browser. A failure is logged and nothing else happens.
pub fn open_url(url: &str) {
    gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gio::Cancellable>, |result| {
        if let Err(err) = result {
            tracing::warn!(%err, "could not open the link");
        }
    });
}

fn copy_to_clipboard(text: &str) {
    if let Some(display) = gtk::gdk::Display::default() {
        display.clipboard().set_text(text);
    }
    crate::toast::notify(trf(MsgKey::copied_to_clipboard, &[("arg1", Arg::Str(text))]));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shown {
    Add,
    DeepLink,
    Remove,
}

/// The open dialog window and the parts that change while it is open.
struct DialogUi {
    shown: Shown,
    dialog: dialog_widget::Dialog,
    helper: Option<gtk::Label>,
}

fn shown_of(dialog: &Dialog) -> Shown {
    match dialog {
        Dialog::Add(_) => Shown::Add,
        Dialog::DeepLink(_) => Shown::DeepLink,
        Dialog::Remove { .. } => Shown::Remove,
    }
}

struct Inner<T: Keyed + Send + 'static> {
    ops: Arc<dyn Ops<T>>,
    texts: Texts,
    state: RefCell<Page<T>>,
    /// A deep link that arrived while loading; it opens as the confirm dialog once the list is there.
    queued_link: RefCell<Option<String>>,
    stack: gtk::Stack,
    list: gtk::ListBox,
    fab: gtk::Button,
    fab_label: gtk::Label,
    refresh: gtk::Button,
    spinner: gtk::Spinner,
    dialog: RefCell<Option<DialogUi>>,
}

/// A built page. Dropping it does not close the page; the widget owns the state.
pub struct StoresPage<T: Keyed + Send + 'static> {
    inner: Rc<Inner<T>>,
}

impl<T: Keyed + Send + 'static> StoresPage<T> {
    /// A deep link to add a store. While the list is loading it waits and opens when the list is ready (RACE-01).
    pub fn open_deep_link(&self, url: &str) {
        self.inner.open_deep_link(url);
    }
}

/// Builds the page. `subscribe` starts the live list and calls its argument with every emission.
pub fn build<T: Keyed + Send + 'static>(
    ops: Arc<dyn Ops<T>>,
    texts: Texts,
    subscribe: impl FnOnce(Box<dyn Fn(Vec<T>)>),
) -> (Built, StoresPage<T>) {
    super::style::install();
    let stack = gtk::Stack::new();
    let spinner_page = gtk::Spinner::builder().spinning(true).width_request(32).height_request(32).halign(gtk::Align::Center).valign(gtk::Align::Center).build();
    stack.add_named(&spinner_page, Some("loading"));
    stack.add_named(&crate::ui::status("dialog-information-symbolic", &texts.empty), Some("empty"));
    let list = crate::ui::boxed_list();
    let column = crate::ui::column();
    column.append(&list);
    let scrolled = crate::ui::scrolled(&column);
    stack.add_named(&scrolled, Some("list"));
    stack.set_visible_child_name("loading");

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&stack));
    let fab = crate::screens::fab(&overlay, &texts.add, "list-add-symbolic");
    fab.set_visible(false);
    let fab_label = fab.child().and_then(|c| c.last_child()).and_downcast::<gtk::Label>().expect("the FAB ends with its label");

    let refresh = crate::ui::icon_button("view-refresh-symbolic", &texts.refresh);
    refresh.set_sensitive(false);
    let spinner = gtk::Spinner::builder().spinning(true).visible(false).build();

    let inner = Rc::new(Inner {
        ops,
        texts,
        state: RefCell::new(Page::Loading),
        queued_link: RefCell::new(None),
        stack,
        list,
        fab: fab.clone(),
        fab_label,
        refresh: refresh.clone(),
        spinner: spinner.clone(),
        dialog: RefCell::new(None),
    });

    let weak = Rc::downgrade(&inner);
    fab.connect_clicked({
        let weak = weak.clone();
        move |_| {
            if let Some(page) = weak.upgrade() {
                page.dispatch(Event::Open(Dialog::Add(Entry::default())));
            }
        }
    });
    refresh.connect_clicked({
        let weak = weak.clone();
        move |_| {
            if let Some(page) = weak.upgrade() {
                page.dispatch(Event::Refresh);
            }
        }
    });
    let previous = Rc::new(Cell::new(0.0_f64));
    let adjustment = scrolled.vadjustment();
    let fab_label = inner.fab_label.clone();
    adjustment.connect_value_changed(move |a| {
        fab_label.set_visible(fab_expanded_at(a.value(), previous.get(), a.page_size(), a.upper()));
        previous.set(a.value());
    });

    {
        let weak = weak.clone();
        subscribe(Box::new(move |items| {
            if let Some(page) = weak.upgrade() {
                page.dispatch(Event::Loaded(items));
            }
        }));
    }

    // the widget owns the state; closures reach it through a weak reference
    let keep = Rc::clone(&inner);
    let root: gtk::Widget = overlay.clone().upcast();
    root.connect_destroy(move |_| drop(Rc::clone(&keep)));
    let built = Built { content: overlay.upcast(), end: vec![spinner.upcast(), refresh.upcast()] };
    (built, StoresPage { inner })
}

impl<T: Keyed + Send + 'static> Inner<T> {
    /// Applies one event, draws the result and runs the effect it asks for.
    fn dispatch(self: &Rc<Self>, event: Event<T>) {
        let was_loading = matches!(*self.state.borrow(), Page::Loading);
        let effect = stores::reduce(&mut self.state.borrow_mut(), event);
        self.render();
        if was_loading && !matches!(*self.state.borrow(), Page::Loading) {
            let queued = self.queued_link.borrow_mut().take();
            if let Some(url) = queued {
                self.open_deep_link(&url);
            }
        }
        self.run(effect);
    }

    fn open_deep_link(self: &Rc<Self>, url: &str) {
        if matches!(*self.state.borrow(), Page::Loading) {
            *self.queued_link.borrow_mut() = Some(url.to_owned());
            return;
        }
        self.dispatch(Event::Open(Dialog::DeepLink(Entry { input: url.to_owned(), ..Entry::default() })));
    }

    fn run(self: &Rc<Self>, effect: Effect) {
        match effect {
            Effect::None => {}
            Effect::Fetch(url) => {
                let (weak, ops) = (Rc::downgrade(self), self.ops.clone());
                glib::spawn_future_local(async move {
                    let outcome = gio::spawn_blocking(move || ops.add(&url)).await;
                    let Some(page) = weak.upgrade() else { return };
                    match outcome {
                        Ok(AddOutcome::Added) => {
                            page.dispatch(Event::AddSucceeded);
                            reload_extensions();
                        }
                        Ok(AddOutcome::Failed(text)) => page.dispatch(Event::AddFailed(text)),
                        Ok(AddOutcome::Notice(text)) => {
                            crate::toast::notify(text);
                            page.dispatch(Event::AddSucceeded);
                        }
                        Ok(AddOutcome::Conflict { old_name, new_name, replace }) => {
                            page.dispatch(Event::AddSucceeded);
                            page.show_conflict(&old_name, &new_name, replace);
                        }
                        Err(_) => page.dispatch(Event::AddFailed(stores::unknown_error())),
                    }
                });
            }
            Effect::Delete(key) => {
                let ops = self.ops.clone();
                glib::spawn_future_local(async move {
                    if gio::spawn_blocking(move || ops.remove(&key)).await.is_ok() {
                        reload_extensions();
                    }
                });
            }
            Effect::RefreshAll => {
                let (weak, ops) = (Rc::downgrade(self), self.ops.clone());
                glib::spawn_future_local(async move {
                    // a panic on the worker still ends the spinner
                    let _ = gio::spawn_blocking(move || ops.refresh()).await;
                    reload_extensions();
                    if let Some(page) = weak.upgrade() {
                        page.dispatch(Event::RefreshDone);
                    }
                });
            }
        }
    }

    /// Draws the page from the state: which of loading, empty and list shows, the cards, the FAB and header.
    fn render(self: &Rc<Self>) {
        let (cards, ready, refreshing) = match &*self.state.borrow() {
            Page::Loading => (Vec::new(), false, false),
            Page::Ready { items, refreshing, .. } => (items.iter().map(|i| self.ops.card(i)).collect::<Vec<_>>(), true, *refreshing),
        };
        self.stack.set_visible_child_name(match (ready, cards.is_empty()) {
            (false, _) => "loading",
            (true, true) => "empty",
            (true, false) => "list",
        });
        self.fab.set_visible(ready);
        self.refresh.set_sensitive(ready && !refreshing);
        self.spinner.set_visible(refreshing);
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        for card in &cards {
            self.list.append(&self.card_row(card));
        }
        self.sync_dialog();
    }

    fn card_row(self: &Rc<Self>, card: &Card) -> gtk::ListBoxRow {
        let actions = stores::card_actions(&card.website, card.discord.as_deref());
        let h = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        crate::ui::margins(&h, 10);
        let tag = gtk::Image::from_icon_name("tag-symbolic");
        h.append(&tag);
        h.append(&gtk::Label::builder().label(&card.name).xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).build());

        let globe = crate::ui::icon_button("web-browser-symbolic", &self.texts.open_website);
        globe.set_sensitive(actions.globe_enabled);
        let website = card.website.clone();
        globe.connect_clicked(move |_| open_url(&website));
        h.append(&globe);
        if let Some(link) = actions.discord {
            let discord = crate::ui::icon_button("user-available-symbolic", "Discord");
            discord.connect_clicked(move |_| open_url(&link));
            h.append(&discord);
        }
        let copy = crate::ui::icon_button("edit-copy-symbolic", &self.texts.copy);
        let text = card.copy.clone();
        copy.connect_clicked(move |_| copy_to_clipboard(&text));
        h.append(&copy);
        let delete = crate::ui::icon_button("user-trash-symbolic", &self.texts.delete);
        let (weak, key, name) = (Rc::downgrade(self), card.key.clone(), card.name.clone());
        delete.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.dispatch(Event::Open(Dialog::Remove { key: key.clone(), name: name.clone(), url: key.clone() }));
            }
        });
        h.append(&delete);
        gtk::ListBoxRow::builder().child(&h).activatable(false).build()
    }

    /// Makes the dialog window match the state's dialog: opens it, updates it in place or closes it.
    fn sync_dialog(self: &Rc<Self>) {
        let want = match &*self.state.borrow() {
            Page::Ready { dialog, .. } => dialog.clone(),
            Page::Loading => None,
        };
        let have = self.dialog.borrow().as_ref().map(|d| d.shown);
        match (&want, have) {
            (None, None) => {}
            (None, Some(_)) => self.close_dialog(),
            (Some(d), None) => self.open_dialog(d),
            (Some(d), Some(shown)) if shown_of(d) != shown => {
                self.close_dialog();
                self.open_dialog(d);
            }
            (Some(Dialog::Add(e) | Dialog::DeepLink(e)), Some(_)) => self.update_entry_dialog(e),
            (Some(Dialog::Remove { .. }), Some(_)) => {}
        }
    }

    fn close_dialog(&self) {
        let open = self.dialog.borrow_mut().take();
        if let Some(open) = open {
            open.dialog.finish();
        }
    }

    fn existing_keys(&self) -> Vec<String> {
        match &*self.state.borrow() {
            Page::Ready { items, .. } => items.iter().map(|i| i.key().to_owned()).collect(),
            Page::Loading => Vec::new(),
        }
    }

    /// Helper line, error style and the Add button of the open entry dialog.
    fn update_entry_dialog(&self, entry: &Entry) {
        let keys = self.existing_keys();
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        let open = self.dialog.borrow();
        let Some(open) = open.as_ref() else { return };
        let helper = stores::helper(entry, &keys);
        if let Some(label) = &open.helper {
            label.set_text(&stores::helper_text(&helper));
            if helper.is_error() { label.add_css_class("pref-error") } else { label.remove_css_class("pref-error") }
        }
        open.dialog.set_confirm_enabled(stores::add_enabled(entry, &keys));
        if let Some(confirm) = &open.dialog.confirm {
            confirm.set_label(&stores::add_label(entry));
        }
    }

    fn open_dialog(self: &Rc<Self>, dialog: &Dialog) {
        let weak = Rc::downgrade(self);
        let on_cancel = {
            let weak = weak.clone();
            move || {
                if let Some(page) = weak.upgrade() {
                    // the window closed itself; forget it before the reducer closes the dialog
                    page.dialog.borrow_mut().take();
                    page.dispatch(Event::Dismiss);
                }
            }
        };
        let parent = crate::ui::window();
        let ui = match dialog {
            Dialog::Add(entry) | Dialog::DeepLink(entry) => {
                let read_only = matches!(dialog, Dialog::DeepLink(_));
                let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
                let intro = if read_only { &self.texts.deep_link_intro } else { &self.texts.add_intro };
                if let Some(text) = intro {
                    body.append(&gtk::Label::builder().label(text).wrap(true).xalign(0.0).build());
                }
                let field = gtk::Entry::builder().text(&entry.input).placeholder_text(&self.texts.input_label).editable(!read_only).input_purpose(gtk::InputPurpose::Url).activates_default(false).build();
                field.update_property(&[gtk::accessible::Property::Label(&self.texts.input_label)]);
                body.append(&field);
                let helper = gtk::Label::builder().xalign(0.0).wrap(true).build();
                body.append(&helper);
                let on_confirm = {
                    let weak = weak.clone();
                    move |_: dialog_widget::Dialog| {
                        if let Some(page) = weak.upgrade() {
                            page.dispatch(Event::Submit);
                        }
                    }
                };
                let confirm_label = stores::add_label(entry);
                let shown = dialog_widget::show(&parent, &self.texts.add_title, &body, Some(confirm_label.as_str()), on_confirm, on_cancel);
                if !read_only {
                    let weak = weak.clone();
                    field.connect_changed(move |f| {
                        if let Some(page) = weak.upgrade() {
                            page.dispatch(Event::Type(f.text().to_string()));
                        }
                    });
                }
                // Enter presses Add when it is enabled
                if let Some(confirm) = shown.confirm.clone() {
                    field.connect_activate(move |_| {
                        if confirm.is_sensitive() {
                            confirm.emit_clicked();
                        }
                    });
                }
                field.grab_focus();
                DialogUi { shown: if read_only { Shown::DeepLink } else { Shown::Add }, dialog: shown, helper: Some(helper) }
            }
            Dialog::Remove { name, url, .. } => {
                let body = gtk::Label::builder().label((self.texts.remove_body)(name, url)).wrap(true).xalign(0.0).build();
                let on_confirm = {
                    let weak = weak.clone();
                    move |_: dialog_widget::Dialog| {
                        if let Some(page) = weak.upgrade() {
                            page.dispatch(Event::ConfirmRemove);
                        }
                    }
                };
                let ok = strings::ok();
                let shown = dialog_widget::show(&parent, &self.texts.remove_title, &body, Some(ok.as_str()), on_confirm, on_cancel);
                if let Some(confirm) = &shown.confirm {
                    confirm.remove_css_class("suggested-action");
                    confirm.add_css_class("destructive-action");
                }
                DialogUi { shown: Shown::Remove, dialog: shown, helper: None }
            }
        };
        *self.dialog.borrow_mut() = Some(ui);
        if let Dialog::Add(e) | Dialog::DeepLink(e) = dialog {
            self.update_entry_dialog(e);
        }
    }

    /// The signing-key conflict of a repo add: Replace swaps the old row for the new one, Cancel changes nothing.
    fn show_conflict(&self, old_name: &str, new_name: &str, replace: Box<dyn FnOnce() + Send>) {
        let body = gtk::Label::builder()
            .label(trf(MsgKey::action_replace_repo_message, &[("arg1", Arg::Str(new_name)), ("arg2", Arg::Str(old_name))]))
            .wrap(true)
            .xalign(0.0)
            .build();
        let replace = RefCell::new(Some(replace));
        let confirm = tr(MsgKey::action_replace_repo);
        dialog_widget::show(
            &crate::ui::window(),
            &tr(MsgKey::action_replace_repo_title),
            &body,
            Some(confirm.as_str()),
            move |dialog| {
                if let Some(replace) = replace.borrow_mut().take() {
                    glib::spawn_future_local(async move {
                        if gio::spawn_blocking(replace).await.is_ok() {
                            reload_extensions();
                        }
                    });
                }
                dialog.finish();
            },
            || {},
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_47b_018_the_fab_label_shows_at_the_top_and_after_scrolling_back() {
        // at the top
        assert!(fab_expanded_at(0.0, 0.0, 400.0, 1000.0));
        // scrolling forward in the middle: shrunk
        assert!(!fab_expanded_at(300.0, 200.0, 400.0, 1000.0));
        // scrolling back: expanded again
        assert!(fab_expanded_at(250.0, 300.0, 400.0, 1000.0));
        // nothing left to scroll to: expanded
        assert!(fab_expanded_at(600.0, 500.0, 400.0, 1000.0));
    }

    #[test]
    fn t_47b_011_reload_listeners_all_run() {
        let count = Rc::new(Cell::new(0));
        for _ in 0..2 {
            let count = count.clone();
            on_extensions_reload(move || count.set(count.get() + 1));
        }
        reload_extensions();
        assert_eq!(count.get(), 2);
        RELOAD.with(|l| l.borrow_mut().clear());
    }

    #[test]
    fn t_47b_016_the_texts_come_from_the_string_table() {
        let stores = Texts::stores();
        assert_eq!(stores.empty, "You haven't added an extension store yet");
        assert_eq!(stores.add_title, "Add extension store");
        assert_eq!(stores.input_label, "Store index URL");
        assert_eq!(stores.remove_title, "Remove extension store");
        assert_eq!(stores.refresh, "Refresh");
        let repos = Texts::repos();
        assert_eq!(repos.empty, "You have no repos set.");
        assert_eq!(repos.add_title, "Add repo");
        assert_eq!(repos.input_label, "Repo URL");
        assert!(repos.add_intro.as_deref().is_some_and(|t| t.contains("index.min.json")));
    }

    #[test]
    fn t_47b_017_the_remove_bodies_follow_the_kind() {
        assert_eq!((Texts::stores().remove_body)("A", "https://a"), "Do you wish to remove the extension store \"A\" (https://a)?");
        assert_eq!((Texts::repos().remove_body)("A", "https://a"), "Do you wish to delete the repo \"https://a\"?");
    }

    // ---------- widget check (needs a display) ----------

    #[derive(Clone, Debug)]
    struct Item(String);

    impl Keyed for Item {
        fn key(&self) -> &str {
            &self.0
        }
    }

    struct Fake {
        fail_with: Option<String>,
        added: std::sync::Mutex<Vec<String>>,
    }

    impl Ops<Item> for Fake {
        fn card(&self, item: &Item) -> Card {
            Card { key: item.0.clone(), name: item.0.clone(), website: String::new(), discord: None, copy: item.0.clone() }
        }
        fn add(&self, url: &str) -> AddOutcome {
            self.added.lock().unwrap().push(url.to_owned());
            match &self.fail_with {
                Some(text) => AddOutcome::Failed(text.clone()),
                None => AddOutcome::Added,
            }
        }
        fn refresh(&self) {}
        fn remove(&self, _key: &str) {}
    }

    fn pump_until(done: impl Fn() -> bool) {
        let context = glib::MainContext::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !done() && std::time::Instant::now() < deadline {
            context.iteration(false);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// GTK binds to the first thread that starts it, so the widget checks run in this one test. Run alone:
    /// `cargo test -p kurobayomi pref_model::stores_page -- --ignored`.
    #[test]
    #[ignore = "needs a display; run alone: cargo test -p kurobayomi pref_model::stores_page -- --ignored"]
    fn page_widgets_smoke() {
        if gtk::init().is_err() {
            eprintln!("skipped: no display for the stores page checks");
            return;
        }
        let app = gtk::Application::builder().application_id("io.github.blackbox.KurobayomiTest").flags(gio::ApplicationFlags::NON_UNIQUE).build();
        app.register(None::<&gio::Cancellable>).expect("register");
        let window = gtk::ApplicationWindow::new(&app);
        crate::ui::nav_init(&gtk::Stack::new(), &window);

        let ops = Arc::new(Fake { fail_with: Some("HTTP 500".into()), added: Default::default() });
        let feed: Rc<RefCell<Option<Box<dyn Fn(Vec<Item>)>>>> = Rc::new(RefCell::new(None));
        let sink = feed.clone();
        let (built, page) = build(ops.clone(), Texts::stores(), move |push| *sink.borrow_mut() = Some(push));
        let push = |items: Vec<Item>| (feed.borrow().as_ref().expect("subscribed"))(items);
        let inner = page.inner.clone();
        let shown = || inner.stack.visible_child_name().map(|n| n.to_string()).unwrap_or_default();

        assert_eq!(shown(), "loading", "a spinner until the first emission");
        assert!(!inner.fab.is_visible());
        push(vec![]);
        assert_eq!(shown(), "empty", "zero stores show the empty text");
        assert!(inner.fab.is_visible(), "with the FAB still there");
        push(vec![Item("https://a/index.json".into())]);
        assert_eq!(shown(), "list");
        assert!(inner.list.row_at_index(0).is_some() && inner.list.row_at_index(1).is_none());

        // the add dialog: Add is off while empty, on for a new URL, off again for a stored one
        inner.dispatch(Event::Open(Dialog::Add(Entry::default())));
        assert!(inner.dialog.borrow().is_some());
        {
            let open = inner.dialog.borrow();
            let open = open.as_ref().expect("open");
            assert!(!open.dialog.confirm.as_ref().expect("Add").is_sensitive());
            assert_eq!(open.helper.as_ref().expect("helper").text(), "*required");
        }
        inner.dispatch(Event::Type("https://a/index.json".into()));
        {
            let open = inner.dialog.borrow();
            let open = open.as_ref().expect("open");
            assert_eq!(open.helper.as_ref().expect("helper").text(), "This extension store already exists");
            assert!(!open.dialog.confirm.as_ref().expect("Add").is_sensitive());
        }
        inner.dispatch(Event::Type("https://b/index.json".into()));
        assert!(inner.dialog.borrow().as_ref().expect("open").dialog.confirm.as_ref().expect("Add").is_sensitive());

        // a failing add keeps the dialog, shows the error and enables Add again
        inner.dispatch(Event::Submit);
        assert_eq!(inner.dialog.borrow().as_ref().expect("open").dialog.confirm.as_ref().expect("Add").label().as_deref(), Some("Processing…"));
        pump_until(|| inner.dialog.borrow().as_ref().is_some_and(|d| d.helper.as_ref().is_some_and(|h| h.text() == "HTTP 500")));
        assert_eq!(ops.added.lock().unwrap().as_slice(), ["https://b/index.json"]);
        assert!(inner.dialog.borrow().as_ref().expect("still open").dialog.confirm.as_ref().expect("Add").is_sensitive());

        // Escape or Cancel closes it; the remove dialog opens from the state and closes on confirm
        inner.dispatch(Event::Dismiss);
        assert!(inner.dialog.borrow().is_none());
        inner.dispatch(Event::Open(Dialog::Remove { key: "https://a/index.json".into(), name: "A".into(), url: "https://a/index.json".into() }));
        assert!(inner.dialog.borrow().is_some());
        inner.dispatch(Event::ConfirmRemove);
        assert!(inner.dialog.borrow().is_none(), "the dialog closes at once");

        // a refresh shows the header spinner until it is done
        inner.dispatch(Event::Refresh);
        assert!(inner.spinner.is_visible());
        pump_until(|| !inner.spinner.is_visible());
        assert!(!inner.spinner.is_visible());
        assert_eq!(built.end.len(), 2, "spinner and Refresh in the header");
    }
}
