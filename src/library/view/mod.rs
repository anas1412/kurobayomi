//! The library view: toolbar with title and count, search, category tabs, the cover grid of the active page, the
//! selection bar and the empty states. Pipeline work runs on the worker (`worker`); this file applies its replies.
//! Borrows of the shared state are never held across a widget call, because widget calls can re-enter the handlers.

mod card;
mod dialog;
mod sheet;
mod tabs;

use std::cell::{Cell as FlagCell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{Orientation as O, gdk, gio, glib};
use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;
use kuro_db::handle::Db;
use kuro_db::schema_anime::FetchType;
use kuro_domain::category::service::CategoryService;
use kuro_domain::kind::EntryKind;
use kuro_domain::library::active::clamp_page;
use kuro_domain::library::columns::columns_for_width;
use kuro_domain::library::model::LibraryRow;
use kuro_domain::library::page::{LibraryItem, toolbar_title};
use kuro_domain::library::sort::LibrarySort;
use kuro_i18n::{MsgKey, tr};
use kuro_domain::library::search::{SEARCH_DEBOUNCE, empty_page, shows_global_search};
use kuro_prefs::library::LibraryPrefs;

use crate::library::actions;
use crate::library::cells::{self, Cell};
use crate::library::category_dialog::CategoryDialog;
use crate::library::display_tab;
use crate::library::keys;
use crate::library::layout;
use crate::library::filter_tab;
use crate::library::load;
use crate::library::random;
use crate::library::query::{Built, Facts};
use crate::library::select::Selection;
use crate::library::worker::{Gate, Job, Reply, Worker};

/// What the view reads from the app: storage, preferences and the lookups that live outside storage.
pub struct Deps {
    pub kind: EntryKind,
    pub db: Arc<Db>,
    pub prefs: Rc<LibraryPrefs>,
    pub categories: Rc<CategoryService>,
    pub facts: Facts,
    pub downloaded_only: Rc<dyn Fn() -> bool>,
    pub logged_in: Rc<dyn Fn() -> BTreeSet<i64>>,
    /// The logged-in trackers as (id, name), for the Filter tab's tracker rows.
    pub tracker_names: Rc<dyn Fn() -> Vec<(i64, String)>>,
    /// `pref_always_use_external_player`, read when the continue button is clicked.
    pub always_external: Rc<dyn Fn() -> bool>,
    /// Whether the chapter or episode with this id is on disk; the entry's downloaded filter reads it.
    pub item_downloaded: Rc<dyn Fn(i64) -> bool>,
}

/// What the continue button asks the app to open: `item` is the chapter or episode id of `entry`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resume {
    pub entry: i64,
    pub item: i64,
    /// Play it in the external player (anime only; the reader ignores it).
    pub external: bool,
}

/// Navigation out of the library: open an entry, resume it, search globally, edit the categories.
pub struct Nav {
    pub open: Rc<dyn Fn(i64)>,
    pub resume: Rc<dyn Fn(Resume)>,
    /// Queues the downloads of the selected entries: how many, from the Download menu.
    pub download: Rc<dyn Fn(Vec<i64>, actions::DownloadAmount)>,
    /// Starts removing the selected entries from the library.
    pub delete: Rc<dyn Fn(Vec<i64>)>,
    pub global_search: Rc<dyn Fn(&str)>,
    pub edit_categories: Rc<dyn Fn()>,
    /// Told when selection mode starts or ends; the shell hides its sidebar while it is on (B-1.4).
    pub selecting: Rc<dyn Fn(bool)>,
    /// Starts an update pass of one category (`Some(id)`); `false` when a pass of this kind is already running.
    pub update: Rc<dyn Fn(Option<i64>) -> bool>,
    /// Whether a pass of this kind is running now; the refresh button shows it.
    pub update_running: Rc<dyn Fn() -> bool>,
}

/// The shared state of one library view. Lives in an `Rc<RefCell<_>>` that handlers reach through a `Weak`.
struct Inner {
    deps: Deps,
    nav: Nav,
    worker: Worker,
    gate: Gate,
    built: Option<Built>,
    /// The active tab by index, used when `active` is not on the current pages.
    page: usize,
    /// The active tab by category id; this is what the app remembers across launches.
    active: Option<i64>,
    query: Option<String>,
    search_turn: u64,
    selection: Selection,
    /// The last value sent to `Nav::selecting`.
    selecting_shown: bool,
    /// Told on every render whether a filter (or the forced downloaded-only view) is active (T-37a-056).
    on_filters: Option<Rc<dyn Fn(bool)>>,
    last_resume: HashMap<i64, Instant>,
    store: gio::ListStore,
    title: gtk::Label,
    /// The toolbar; the selection header replaces it while entries are selected (U-1.1).
    header: gtk::Box,
    sel_header: gtk::Box,
    download_button: gtk::MenuButton,
    search: gtk::SearchEntry,
    tabs: tabs::TabStrip,
    scroller: gtk::ScrolledWindow,
    grid: gtk::GridView,
    /// List mode shows this over the same store instead of the grid (T-37a-021).
    list: gtk::ListView,
    /// Starts an update of the active category; busy while a pass of this kind runs (T-26a-102).
    refresh_button: gtk::Button,
    empty: gtk::Box,
    empty_label: gtk::Label,
    global_button: gtk::Button,
    bar: gtk::Revealer,
    bar_label: gtk::Label,
}

/// Handle to the library view. The widget is `widget()`; `refresh()` reruns the pipeline after a setting or data
/// change.
#[derive(Clone)]
pub struct LibraryView {
    inner: Rc<RefCell<Inner>>,
    root: gtk::Box,
    /// The overlay around `root` that carries the library toast (T-37a-062).
    shell: gtk::Overlay,
}

impl LibraryView {
    pub fn new(deps: Deps, nav: Nav) -> std::io::Result<Self> {
        let (replies_tx, replies_rx) = async_channel::unbounded::<Reply>();
        let worker = Worker::spawn(replies_tx)?;
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();

        let title = gtk::Label::new(None);
        title.add_css_class("title-2");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some(&tr(MsgKey::action_search_hint)));
        search.set_hexpand(true);
        let header = gtk::Box::new(O::Horizontal, 12);
        header.set_margin_start(16);
        header.set_margin_end(16);
        header.set_margin_top(8);
        header.append(&title);
        header.append(&search);
        let refresh_button = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh_button.set_tooltip_text(Some(&tr(MsgKey::action_update_category)));
        header.append(&refresh_button);

        let tabs = tabs::TabStrip::new();
        let grid = gtk::GridView::new(
            Some(gtk::NoSelection::new(Some(store.clone()))),
            Some(gtk::SignalListItemFactory::new()),
        );
        grid.set_vexpand(true);
        grid.set_single_click_activate(false);
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(store.clone()))),
            Some(gtk::SignalListItemFactory::new()),
        );
        list.set_vexpand(true);
        list.set_single_click_activate(false);
        let scroller = gtk::ScrolledWindow::builder().child(&grid).vexpand(true).build();

        let empty_label = gtk::Label::new(None);
        empty_label.add_css_class("dim-label");
        let global_button = gtk::Button::new();
        global_button.set_visible(false);
        let empty = gtk::Box::new(O::Vertical, 12);
        empty.set_valign(gtk::Align::Center);
        empty.set_vexpand(true);
        empty.append(&empty_label);
        empty.append(&global_button);

        let bar_label = gtk::Label::new(None);
        bar_label.set_hexpand(true);
        bar_label.set_xalign(0.0);
        bar_label.add_css_class("title-2");
        let sel_header = gtk::Box::new(O::Horizontal, 12);
        sel_header.set_margin_start(16);
        sel_header.set_margin_end(16);
        sel_header.set_margin_top(8);
        sel_header.set_visible(false);
        sel_header.append(&bar_label);
        let download_button = gtk::MenuButton::builder().label(tr(MsgKey::action_download)).build();
        let action_bar = gtk::ActionBar::new();
        let bar = gtk::Revealer::new();
        bar.set_transition_type(gtk::RevealerTransitionType::SlideUp);
        bar.set_child(Some(&action_bar));

        let root = gtk::Box::new(O::Vertical, 0);
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.append(&header);
        root.append(&sel_header);
        root.append(&tabs.scroller);
        root.append(&scroller);
        root.append(&empty);
        root.append(&bar);

        let inner = Rc::new(RefCell::new(Inner {
            deps,
            nav,
            worker,
            gate: Gate::default(),
            built: None,
            page: 0,
            active: None,
            query: None,
            search_turn: 0,
            selection: Selection::default(),
            selecting_shown: false,
            on_filters: None,
            last_resume: HashMap::new(),
            store,
            title,
            header,
            sel_header,
            download_button,
            search,
            tabs,
            scroller,
            grid,
            list,
            refresh_button,
            empty,
            empty_label,
            global_button,
            bar,
            bar_label,
        }));
        let shell = crate::library::toast::attach(&root);
        let view = Self { inner: inner.clone(), root, shell };
        // a resize across a column boundary or the 900 px breakpoint changes the count without a data change
        let (weak, seen) = (Rc::downgrade(&inner), FlagCell::new((-1, -1)));
        let scroller = inner.borrow().scroller.clone();
        scroller.add_tick_callback(move |scroller, _| {
            let size = (window_width(scroller), scroller.height());
            let before = seen.replace(size);
            if before != size {
                if let Some(inner) = weak.upgrade() {
                    if before.0 != size.0 {
                        apply_columns(&inner);
                    }
                    // List rows divide the page height, so a new height or list size rebuilds them
                    if is_list_mode(&inner.borrow()) {
                        render(&inner);
                    }
                }
            }
            glib::ControlFlow::Continue
        });
        view.wire(&replies_rx);
        view.refresh();
        Ok(view)
    }

    /// The widget to place in the page.
    pub fn widget(&self) -> gtk::Widget {
        self.shell.clone().upcast()
    }

    /// Reruns the pipeline with the current preferences and query.
    pub fn refresh(&self) {
        submit(&self.inner);
    }

    /// Sets the search text from outside (B-1.3): the entry shows it and the library filters at once, without the
    /// typing debounce. An empty text clears the search.
    pub fn set_query(&self, text: &str) {
        let search = {
            let mut st = self.inner.borrow_mut();
            st.search_turn += 1;
            st.query = (!text.is_empty()).then(|| text.to_owned());
            st.search.clone()
        };
        search.set_text(text);
        submit(&self.inner);
    }

    /// "Open random entry": one uniformly chosen entry of the active page; a toast when the page is empty.
    pub fn open_random(&self) {
        let (id, open) = {
            let st = self.inner.borrow();
            let page = st.built.as_ref().and_then(|b| b.pages.get(active_index(&st)));
            let items = page.map_or(&[][..], |p| p.items.as_slice());
            let nonce = u64::from(kuro_domain::library::order::new_seed().unsigned_abs());
            let id = random::pick_index(items.len(), nonce).and_then(|i| items.get(i)).map(|item| item.row.id);
            (id, st.nav.open.clone())
        };
        match id {
            Some(id) => open(id),
            None => crate::library::toast::notify(random::EMPTY_TOAST),
        }
    }

    /// Whether entries are selected (selection mode).
    pub fn selection_active(&self) -> bool {
        !self.inner.borrow().selection.is_empty()
    }

    /// Registers what runs on every render with whether a filter is active, and runs it once with the current value.
    pub fn on_filters_active(&self, listener: impl Fn(bool) + 'static) {
        let active = {
            let mut st = self.inner.borrow_mut();
            st.on_filters = Some(Rc::new(listener));
            st.built.as_ref().is_some_and(|b| b.has_active_filters)
        };
        self.notify_filters(active);
    }

    fn notify_filters(&self, active: bool) {
        let listener = self.inner.borrow().on_filters.clone();
        if let Some(listener) = listener {
            listener(active);
        }
    }

    /// Opens the library settings sheet: Filter, Sort and Display (B-1.2, T-37a-038, T-37a-039).
    pub fn open_settings(&self) {
        open_settings(&self.inner);
    }

    fn wire(&self, replies: &async_channel::Receiver<Reply>) {
        let weak = Rc::downgrade(&self.inner);

        // the grid and the list share one binder; List mode shows the list (T-37a-021)
        let (grid_factory, list_factory) = {
            let st = self.inner.borrow();
            (st.grid.factory(), st.list.factory())
        };
        for factory in [grid_factory, list_factory].into_iter().flatten() {
            let Ok(factory) = factory.downcast::<gtk::SignalListItemFactory>() else { continue };
            let weak_bind = weak.clone();
            factory.connect_bind(move |_, obj| {
                if let Some(item) = obj.downcast_ref::<gtk::ListItem>() {
                    bind_item(&weak_bind, item);
                }
            });
        }
        self.wire_refresh(&weak);

        let weak_search = weak.clone();
        let search = self.inner.borrow().search.clone();
        search.connect_search_changed(move |entry| {
            let Some(inner) = weak_search.upgrade() else { return };
            let text = entry.text().to_string();
            let turn = {
                let mut st = inner.borrow_mut();
                st.search_turn += 1;
                st.search_turn
            };
            let weak_later = weak_search.clone();
            glib::timeout_add_local_once(SEARCH_DEBOUNCE, move || {
                let Some(inner) = weak_later.upgrade() else { return };
                if inner.borrow().search_turn != turn {
                    return;
                }
                let query = (!text.is_empty()).then_some(text);
                if inner.borrow().query == query {
                    return;
                }
                inner.borrow_mut().query = query;
                submit(&inner);
            });
        });

        let key = gtk::EventControllerKey::new();
        let weak_key = weak.clone();
        key.connect_key_pressed(move |_, keyval, _, modifiers| {
            let Some(inner) = weak_key.upgrade() else { return glib::Propagation::Proceed };
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let (has_selection, typing) = {
                let st = inner.borrow();
                (!st.selection.is_empty(), st.search.has_focus() || st.search.focus_child().is_some())
            };
            let Some(shortcut) = keys::shortcut(keyval, ctrl, has_selection, typing) else { return glib::Propagation::Proceed };
            match shortcut {
                keys::Shortcut::SelectAll => select_with(&inner, |sel, page| sel.select_all(page)),
                keys::Shortcut::Invert => select_with(&inner, |sel, page| sel.invert(page)),
                keys::Shortcut::DeleteSelection => selected_action(&inner, |nav| nav.delete.clone()),
                keys::Shortcut::Tab(step) => {
                    let target = {
                        let st = inner.borrow();
                        let count = st.built.as_ref().map_or(0, |b| b.pages.len());
                        (active_index(&st) as i64 + step).clamp(0, (count as i64 - 1).max(0)) as usize
                    };
                    select_tab(&inner, target);
                }
            }
            glib::Propagation::Stop
        });
        self.root.add_controller(key);
        // Escape runs in the capture phase so it clears the selection before the search entry sees it
        let esc = gtk::EventControllerKey::new();
        esc.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak_esc = weak.clone();
        esc.connect_key_pressed(move |_, keyval, _, _| {
            if keyval != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            let Some(inner) = weak_esc.upgrade() else { return glib::Propagation::Proceed };
            let (has_selection, search) = {
                let st = inner.borrow();
                (!st.selection.is_empty(), st.search.clone())
            };
            match keys::escape(has_selection, !search.text().is_empty()) {
                keys::Escape::ClearSelection => select_with(&inner, |sel, _| sel.clear()),
                keys::Escape::ClearSearch => search.set_text(""),
                keys::Escape::Ignore => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        });
        self.root.add_controller(esc);

        self.wire_bar(&weak);
        self.watch_prefs();

        let weak_reply = weak.clone();
        let replies = replies.clone();
        glib::spawn_future_local(async move {
            while let Ok(reply) = replies.recv().await {
                let Some(inner) = weak_reply.upgrade() else { break };
                apply_reply(&inner, reply);
            }
        });
    }

    /// Reloads the library when a preference that shapes it changes (T-37a-004). The store reports changes on a
    /// channel, which this polls on the main context; the task ends with the view.
    fn watch_prefs(&self) {
        let (changes, kind) = {
            let st = self.inner.borrow();
            (st.deps.prefs.store().subscribe(), st.deps.kind)
        };
        let weak = Rc::downgrade(&self.inner);
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let Some(inner) = weak.upgrade() else { return glib::ControlFlow::Break };
            let mut relevant = false;
            while let Ok(changed) = changes.try_recv() {
                relevant |= changed.key.as_deref().is_none_or(|key| load::affects_library(kind, key));
            }
            if relevant {
                submit(&inner);
            }
            glib::ControlFlow::Continue
        });
    }

    /// The refresh button (T-26a-102): starts a pass of the active category, says so in a toast, and stays busy
    /// while a pass of this kind runs.
    fn wire_refresh(&self, weak: &Weak<RefCell<Inner>>) {
        let button = self.inner.borrow().refresh_button.clone();
        sync_refresh_button(&self.inner);
        let weak_click = weak.clone();
        button.connect_clicked(move |_| {
            let Some(inner) = weak_click.upgrade() else { return };
            let (update, category) = {
                let st = inner.borrow();
                let page = st.built.as_ref().and_then(|b| b.pages.get(active_index(&st)));
                (st.nav.update.clone(), page.map(|p| p.category.id))
            };
            let started = update(category);
            crate::library::toast::notify(actions::update_toast(actions::UpdateScope::Category, !started));
            sync_refresh_button(&inner);
        });
        // a pass that ends, or one started elsewhere, changes the button without a click
        let weak_poll = weak.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            let Some(inner) = weak_poll.upgrade() else { return glib::ControlFlow::Break };
            sync_refresh_button(&inner);
            glib::ControlFlow::Continue
        });
    }

    /// The selection header (count, select all, invert, cancel) and the action bar (change categories, mark,
    /// download, delete).
    fn wire_bar(&self, weak: &Weak<RefCell<Inner>>) {
        let kind = self.inner.borrow().deps.kind;
        let (read_label, unread_label) = match kind {
            EntryKind::Manga => (tr(MsgKey::action_mark_as_read), tr(MsgKey::action_mark_as_unread)),
            EntryKind::Anime => (tr(MsgKey::action_mark_as_seen), tr(MsgKey::action_mark_as_unseen)),
        };
        type Action = Box<dyn Fn(&Rc<RefCell<Inner>>)>;
        let connect = |button: &gtk::Button, action: Action| {
            let weak = weak.clone();
            button.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    action(&inner);
                }
            });
        };
        let (sel_header, download_button) = {
            let st = self.inner.borrow();
            (st.sel_header.clone(), st.download_button.clone())
        };
        let header_buttons: Vec<(String, Action)> = vec![
            (tr(MsgKey::action_select_all), Box::new(|inner: &Rc<RefCell<Inner>>| select_with(inner, |sel, page| sel.select_all(page)))),
            (tr(MsgKey::action_select_inverse), Box::new(|inner: &Rc<RefCell<Inner>>| select_with(inner, |sel, page| sel.invert(page)))),
            (tr(MsgKey::action_cancel), Box::new(|inner: &Rc<RefCell<Inner>>| select_with(inner, |sel, _| sel.clear()))),
        ];
        for (label, action) in header_buttons {
            let button = gtk::Button::with_label(&label);
            connect(&button, action);
            sel_header.append(&button);
        }
        let bar_box = self.inner.borrow().bar.child().and_then(|w| w.downcast::<gtk::ActionBar>().ok());
        if let Some(bar_box) = bar_box {
            let buttons: Vec<(String, Action)> = vec![
                (tr(MsgKey::action_move_category), Box::new(open_categories)),
                (read_label, Box::new(|inner: &Rc<RefCell<Inner>>| mark(inner, true))),
                (unread_label, Box::new(|inner: &Rc<RefCell<Inner>>| mark(inner, false))),
                (tr(MsgKey::action_delete), Box::new(|inner: &Rc<RefCell<Inner>>| selected_action(inner, |nav| nav.delete.clone()))),
            ];
            for (label, action) in buttons {
                let button = gtk::Button::with_label(&label);
                connect(&button, action);
                bar_box.pack_start(&button);
            }
            // the Download menu: the next 1, 5, 10 or 25 items, or every unviewed one (T-37a-028)
            let menu = gtk::Box::new(O::Vertical, 2);
            let popover = gtk::Popover::builder().child(&menu).build();
            for (amount, label) in actions::download_menu(kind) {
                let item = gtk::Button::with_label(&label);
                let (weak_item, popover_item) = (weak.clone(), popover.clone());
                item.connect_clicked(move |_| {
                    popover_item.popdown();
                    if let Some(inner) = weak_item.upgrade() {
                        selected_download(&inner, amount);
                    }
                });
                menu.append(&item);
            }
            download_button.set_popover(Some(&popover));
            bar_box.pack_start(&download_button);
        }
        let weak_global = weak.clone();
        let global = self.inner.borrow().global_button.clone();
        global.connect_clicked(move |_| {
            let Some(inner) = weak_global.upgrade() else { return };
            let query = inner.borrow().query.clone();
            if let Some(query) = query {
                let open = inner.borrow().nav.global_search.clone();
                open(&query);
            }
        });
    }
}

/// Disables the refresh button while a pass of this kind runs.
fn sync_refresh_button(inner: &Rc<RefCell<Inner>>) {
    let (button, running) = {
        let st = inner.borrow();
        (st.refresh_button.clone(), st.nav.update_running.clone())
    };
    button.set_sensitive(!running());
}

/// Sends the current settings to the worker as a new generation.
fn submit(inner: &Rc<RefCell<Inner>>) {
    let mut st = inner.borrow_mut();
    let generation = st.gate.issue();
    let downloaded_only = (st.deps.downloaded_only)();
    let logged_in = (st.deps.logged_in)();
    let request = load::request(&st.deps.prefs, st.deps.kind, downloaded_only, &logged_in, st.query.clone());
    let job = Job { generation, db: st.deps.db.clone(), request, facts: st.deps.facts.clone() };
    if !st.worker.submit(job) {
        tracing::error!("the library worker has stopped");
    }
}

fn apply_reply(inner: &Rc<RefCell<Inner>>, reply: Reply) {
    {
        let mut st = inner.borrow_mut();
        if !st.gate.is_current(reply.generation) {
            return;
        }
        match reply.result {
            Ok(built) => {
                // ids that left the library drop out of the selection; the counts follow the new rows (T-37a-059)
                st.selection.refresh(&built.library);
                st.built = Some(built);
            }
            Err(e) => {
                tracing::error!(error = %e, "loading the library failed");
                return;
            }
        }
    }
    render(inner);
}

/// Index of the active tab: the remembered category when it is still shown, else the clamped stored index.
fn active_index(st: &Inner) -> usize {
    let pages = st.built.as_ref().map_or(&[][..], |b| b.pages.as_slice());
    if let Some(id) = st.active {
        if let Some(index) = pages.iter().position(|p| p.category.id == id) {
            return index;
        }
    }
    clamp_page(st.page as i64, pages.len())
}

/// Redraws the toolbar, tabs, grid, empty state and selection bar from the current state.
fn render(inner: &Rc<RefCell<Inner>>) {
    let weak = Rc::downgrade(inner);
    let view = {
        let st = inner.borrow();
        let pages = st.built.as_ref().map_or(&[][..], |b| b.pages.as_slice());
        let index = active_index(&st);
        let search_active = st.query.is_some();
        let tabs_on = layout::tabs_visible(st.deps.prefs.category_tabs.get(), search_active);
        let show_tabs = tabs_on && pages.len() > 1;
        let default_label = tr(MsgKey::label_default);
        let default_title = match st.deps.kind {
            EntryKind::Anime => tr(MsgKey::label_anime_library),
            EntryKind::Manga => tr(MsgKey::label_manga_library),
        };
        let show_counts = st.deps.prefs.category_number_of_items.get() || search_active;
        let labels: Vec<String> = if show_tabs {
            pages
                .iter()
                .map(|p| layout::tab_label(p.category.display_name(&default_label), show_counts.then_some(p.items.len())))
                .collect()
        } else {
            Vec::new()
        };
        let title = match pages.get(index) {
            Some(page) => {
                let distinct = st.built.as_ref().map_or(0, |b| b.distinct_count);
                let (title, count) = toolbar_title(
                    tabs_on,
                    st.deps.prefs.category_number_of_items.get(),
                    &default_title,
                    page.category.display_name(&default_label),
                    page.items.len(),
                    distinct,
                );
                match count {
                    Some(n) => format!("{title} ({n})"),
                    None => title,
                }
            }
            None => default_title.clone(),
        };
        let items: Vec<LibraryItem> = pages.get(index).map_or_else(Vec::new, |p| p.items.clone());
        let grid_cells = cells::cells(&items, st.query.as_deref());
        let global_label = cells::global_label(st.query.as_deref().unwrap_or_default());
        let (empty_text, show_global) = if items.is_empty() && st.built.is_some() {
            let filters = st.built.as_ref().is_some_and(|b| b.has_active_filters);
            let library_empty = st.built.as_ref().is_none_or(|b| b.is_library_empty());
            let text = empty_page(search_active, filters, library_empty).text();
            (text, shows_global_search(search_active, true))
        } else {
            ("", false)
        };
        let hidden = st.selection.hidden_count(&items);
        let selected = st.selection.len();
        let bar_text = match (selected, hidden) {
            (0, _) => String::new(),
            (n, 0) => format!("{n} selected"),
            (n, h) => format!("{n} selected ({h} hidden by the current view)"),
        };
        Rendered {
            title,
            labels,
            index,
            items,
            grid_cells,
            empty_text,
            show_global,
            global_label,
            bar_text,
            bar_open: !st.selection.is_empty(),
            download_visible: actions::download_visible(st.selection.rows()),
            filters_active: st.built.as_ref().is_some_and(|b| b.has_active_filters),
        }
    };

    let (title_label, tabs, store, scroller, empty, empty_label, global, bar, bar_label, header, sel_header, download) = {
        let st = inner.borrow();
        (
            st.title.clone(),
            st.tabs.clone(),
            st.store.clone(),
            st.scroller.clone(),
            st.empty.clone(),
            st.empty_label.clone(),
            st.global_button.clone(),
            st.bar.clone(),
            st.bar_label.clone(),
            st.header.clone(),
            st.sel_header.clone(),
            st.download_button.clone(),
        )
    };
    title_label.set_label(&view.title);
    let on_select: Rc<dyn Fn(usize)> = Rc::new(move |index| {
        if let Some(inner) = weak.upgrade() {
            select_tab(&inner, index);
        }
    });
    tabs.set(&view.labels, view.index, on_select);

    apply_columns(inner);
    show_mode_view(inner);
    let objects: Vec<glib::BoxedAnyObject> = view.grid_cells.iter().cloned().map(glib::BoxedAnyObject::new).collect();
    store.splice(0, store.n_items(), objects.as_slice());
    let has_items = !view.items.is_empty();
    scroller.set_visible(has_items);
    empty.set_visible(!has_items);
    empty_label.set_label(view.empty_text);
    global.set_visible(view.show_global);
    global.set_label(&view.global_label);
    bar_label.set_label(&view.bar_text);
    bar.set_reveal_child(view.bar_open);
    header.set_visible(!view.bar_open);
    sel_header.set_visible(view.bar_open);
    download.set_visible(view.download_visible);
    let on_filters = inner.borrow().on_filters.clone();
    if let Some(listener) = on_filters {
        listener(view.filters_active);
    }
    let changed = {
        let mut st = inner.borrow_mut();
        let changed = st.selecting_shown != view.bar_open;
        st.selecting_shown = view.bar_open;
        changed.then(|| st.nav.selecting.clone())
    };
    if let Some(notify) = changed {
        notify(view.bar_open);
    }
}

/// Puts the list or the grid in the scroller for the display mode.
fn show_mode_view(inner: &Rc<RefCell<Inner>>) {
    let (scroller, list, grid, list_mode) = {
        let st = inner.borrow();
        (st.scroller.clone(), st.list.clone(), st.grid.clone(), is_list_mode(&st))
    };
    let wanted: gtk::Widget = if list_mode { list.upcast() } else { grid.upcast() };
    if scroller.child().as_ref() != Some(&wanted) {
        scroller.set_child(Some(&wanted));
    }
}

fn is_list_mode(st: &Inner) -> bool {
    st.deps.prefs.display_mode.get() == kuro_prefs::library::LibraryDisplayMode::List
}

/// The row height of List mode for the current page height and list size (T-37a-021).
fn list_row_height(st: &Inner) -> f64 {
    let (portrait, landscape) = display_tab::column_prefs(&st.deps.prefs, st.deps.kind);
    let size = layout::list_size(window_width(&st.scroller), portrait.get(), landscape.get());
    layout::list_row_height(f64::from(st.scroller.height()), size)
}

/// Width of the window that holds `widget`, or of the widget itself before it has a window.
fn window_width(widget: &impl IsA<gtk::Widget>) -> i32 {
    widget.root().map_or_else(|| widget.width(), |root| root.width())
}

/// Sets the grid's column count from the window width and the portrait and landscape keys of this kind (T-08a-043):
/// below 900 px the portrait key, from 900 px the landscape key, 0 meaning `max(2, width / 160)`.
fn apply_columns(inner: &Rc<RefCell<Inner>>) {
    let (grid, columns) = {
        let st = inner.borrow();
        let (portrait, landscape) = display_tab::column_prefs(&st.deps.prefs, st.deps.kind);
        let width = window_width(&st.scroller);
        (st.grid.clone(), columns_for_width(width, portrait.get(), landscape.get()) as u32)
    };
    grid.set_min_columns(columns);
    grid.set_max_columns(columns);
}

/// Everything `render` draws, read in one borrow and applied without it.
struct Rendered {
    title: String,
    labels: Vec<String>,
    index: usize,
    items: Vec<LibraryItem>,
    grid_cells: Vec<Cell>,
    empty_text: &'static str,
    show_global: bool,
    global_label: String,
    bar_text: String,
    bar_open: bool,
    download_visible: bool,
    filters_active: bool,
}

/// Makes `index` the active tab and remembers its category.
fn select_tab(inner: &Rc<RefCell<Inner>>, index: usize) {
    let (id, prefs, kind) = {
        let mut st = inner.borrow_mut();
        st.page = index;
        let id = st.built.as_ref().and_then(|b| b.pages.get(index)).map(|p| p.category.id);
        st.active = id;
        (id, st.deps.prefs.clone(), st.deps.kind)
    };
    if let Some(id) = id {
        let saved = match kind {
            EntryKind::Manga => prefs.last_used_manga_category.set(id as i32),
            EntryKind::Anime => prefs.last_used_anime_category.set(id as i32),
        };
        if let Err(e) = saved {
            tracing::warn!(error = %e, "remembering the library tab failed");
        }
    }
    render(inner);
}

/// Runs a selection change against the active page, then redraws. The page slice is read under the same borrow.
fn select_with(inner: &Rc<RefCell<Inner>>, change: impl FnOnce(&mut Selection, &[LibraryItem])) {
    {
        let mut guard = inner.borrow_mut();
        let st = &mut *guard;
        let index = active_index(st);
        let page: &[LibraryItem] =
            st.built.as_ref().and_then(|b| b.pages.get(index)).map_or(&[], |p| p.items.as_slice());
        change(&mut st.selection, page);
    }
    render(inner);
}

/// Queues the downloads of the selected entries with one amount of the Download menu, then clears the selection.
fn selected_download(inner: &Rc<RefCell<Inner>>, amount: actions::DownloadAmount) {
    let (ids, run) = {
        let st = inner.borrow();
        (st.selection.ids().to_vec(), st.nav.download.clone())
    };
    run(ids, amount);
    inner.borrow_mut().selection.clear();
    render(inner);
}

/// Hands the selected entry ids to a navigation hook (delete), then clears the selection.
fn selected_action(inner: &Rc<RefCell<Inner>>, hook: impl Fn(&Nav) -> Rc<dyn Fn(Vec<i64>)>) {
    let (ids, run) = {
        let st = inner.borrow();
        (st.selection.ids().to_vec(), hook(&st.nav))
    };
    run(ids);
    inner.borrow_mut().selection.clear();
    render(inner);
}

/// Marks every selected entry, clears the selection once the writes are done (E-35) and reports a partial failure.
fn mark(inner: &Rc<RefCell<Inner>>, read: bool) {
    let (db, kind, ids) = {
        let st = inner.borrow();
        (st.deps.db.clone(), st.deps.kind, st.selection.ids().to_vec())
    };
    let ok = actions::mark_many(&db, kind, &ids, read);
    inner.borrow_mut().selection.clear();
    if let Some(text) = actions::partial_failure_toast(ok, ids.len()) {
        crate::library::toast::notify(text);
    }
    render(inner);
    submit(inner);
}

fn open_categories(inner: &Rc<RefCell<Inner>>) {
    let (svc, entries, edit) = {
        let st = inner.borrow();
        (st.deps.categories.clone(), st.selection.ids().to_vec(), st.nav.edit_categories.clone())
    };
    let categories = match svc.repo().visible() {
        Ok(categories) => categories,
        Err(e) => {
            tracing::error!(error = %e, "reading the categories failed");
            return;
        }
    };
    let memberships: Vec<Vec<i64>> = entries
        .iter()
        .map(|id| match svc.repo().by_entry(*id, true) {
            Ok(list) => list.iter().map(|c| c.id).collect(),
            Err(e) => {
                tracing::error!(error = %e, entry = id, "reading the entry categories failed");
                Vec::new()
            }
        })
        .collect();
    let dialog = CategoryDialog::new(entries.clone(), &categories, &memberships);
    let weak = Rc::downgrade(inner);
    let on_confirm = move |include: Vec<i64>, exclude: Vec<i64>| {
        let Some(inner) = weak.upgrade() else { return };
        inner.borrow_mut().selection.clear();
        render(&inner);
        let svc = svc.clone();
        let entries = entries.clone();
        let weak = Rc::downgrade(&inner);
        glib::spawn_future_local(async move {
            match actions::set_membership(&svc, &entries, &include, &exclude).await {
                Ok(0) => {}
                Ok(failed) => tracing::error!(failed, "some entries kept their categories"),
                Err(e) => tracing::error!(error = %e, "saving categories failed"),
            }
            if let Some(inner) = weak.upgrade() {
                submit(&inner);
            }
        });
    };
    dialog::show(dialog, on_confirm, move || edit());
}

/// Builds one cell of the grid: the global search row, or the card of an entry. On a card, a click opens, toggles
/// or selects a range (`keys::press`); a long press selects a range (Android parity).
fn bind_item(weak: &Weak<RefCell<Inner>>, list_item: &gtk::ListItem) {
    let Some(inner) = weak.upgrade() else { return };
    let Some(boxed) = list_item.item().and_then(|o| o.downcast::<glib::BoxedAnyObject>().ok()) else { return };
    let cell: Cell = boxed.borrow::<Cell>().clone();
    let item = match cell {
        Cell::Global(query) => {
            let button = gtk::Button::with_label(&cells::global_label(&query));
            button.add_css_class("library-global-search");
            let weak_global = weak.clone();
            button.connect_clicked(move |_| {
                let Some(inner) = weak_global.upgrade() else { return };
                let open = inner.borrow().nav.global_search.clone();
                open(&query);
            });
            list_item.set_child(Some(&button));
            return;
        }
        Cell::Item(item) => item,
    };
    let (mode, selected, show_continue_pref) = {
        let st = inner.borrow();
        (
            st.deps.prefs.display_mode.get(),
            st.selection.contains(item.row.id),
            st.deps.prefs.show_continue_viewing_button.get(),
        )
    };
    let show_continue = actions::continue_visible(show_continue_pref, &item.row);
    let thumbnail = local_thumbnail(&inner.borrow(), &item);
    let season_parent = is_season_parent(&inner.borrow(), &item);
    let card = if mode == kuro_prefs::library::LibraryDisplayMode::List {
        let row_height = list_row_height(&inner.borrow());
        card::build_list(&item, selected, show_continue, row_height, thumbnail.as_deref(), season_parent)
    } else {
        card::build(&item, mode, selected, show_continue, thumbnail.as_deref(), season_parent)
    };
    list_item.set_child(Some(&card.root));

    // the action runs on release, so a long press can claim the sequence first and the release then does nothing
    let long_pressed = Rc::new(FlagCell::new(false));
    let click = gtk::GestureClick::new();
    click.set_button(0);
    let row: LibraryRow = item.row.clone();
    let flag = long_pressed.clone();
    click.connect_pressed(move |_, _, _, _| flag.set(false));
    let (weak_click, flag, click_row) = (weak.clone(), long_pressed.clone(), row.clone());
    click.connect_released(move |gesture, _, _, _| {
        let Some(inner) = weak_click.upgrade() else { return };
        if flag.replace(false) {
            return;
        }
        let modifiers = gesture.current_event_state();
        let selecting = !inner.borrow().selection.is_empty();
        let action = keys::press(
            gesture.current_button(),
            modifiers.contains(gdk::ModifierType::SHIFT_MASK),
            modifiers.contains(gdk::ModifierType::CONTROL_MASK),
            selecting,
        );
        match action {
            keys::Press::Range => select_with(&inner, |sel, page| sel.toggle_range(&click_row, page)),
            keys::Press::Toggle => select_with(&inner, |sel, _| sel.toggle(&click_row)),
            keys::Press::Open => {
                let open = inner.borrow().nav.open.clone();
                open(click_row.id);
            }
        }
    });
    card.root.add_controller(click);

    let long = gtk::GestureLongPress::new();
    long.set_touch_only(true);
    let (weak_long, long_row) = (weak.clone(), row.clone());
    long.connect_pressed(move |_, _, _| {
        let Some(inner) = weak_long.upgrade() else { return };
        long_pressed.set(true);
        select_with(&inner, |sel, page| sel.toggle_range(&long_row, page));
    });
    card.root.add_controller(long);

    if let Some(button) = card.continue_button {
        let weak_resume = weak.clone();
        let resume_row = item.row.clone();
        let id = resume_row.id;
        button.connect_clicked(move |_| {
            let Some(inner) = weak_resume.upgrade() else { return };
            let allowed = {
                let mut st = inner.borrow_mut();
                let now = Instant::now();
                let allowed = actions::continue_click_allowed(st.last_resume.get(&id).copied(), now);
                if allowed {
                    st.last_resume.insert(id, now);
                }
                allowed
            };
            if allowed {
                resume_entry(&inner, &resume_row);
            }
        });
    }
}

/// Whether the item is an anime season parent: its unread badge counts seasons (T-37a-048).
fn is_season_parent(st: &Inner, item: &LibraryItem) -> bool {
    st.deps.kind == EntryKind::Anime
        && AnimeRepo::new(st.deps.db.clone()).by_id(item.row.id).is_ok_and(|a| a.fetch_type == FetchType::Seasons)
}

/// The stored cover of a local entry, which its card draws in place of the placeholder. Other entries have none here.
fn local_thumbnail(st: &Inner, item: &LibraryItem) -> Option<String> {
    if !item.row.is_local() {
        return None;
    }
    let db = st.deps.db.clone();
    match st.deps.kind {
        EntryKind::Manga => MangaRepo::new(db).by_id(item.row.id).ok()?.thumbnail_url,
        EntryKind::Anime => AnimeRepo::new(db).by_id(item.row.id).ok()?.thumbnail_url,
    }
}

/// The settings sheet over the active page. The sort tab writes through `CategoryService::set_sort_mode`, which
/// draws a new Random seed itself; the library reloads when the write is done.
fn open_settings(inner: &Rc<RefCell<Inner>>) {
    let (kind, prefs, categories, has_tracker, category, current_sort, downloaded_only, trackers) = {
        let st = inner.borrow();
        let page = st.built.as_ref().and_then(|b| b.pages.get(active_index(&st)));
        let categorized = st.deps.prefs.categorized_display_settings.get();
        let category = page.map(|p| p.category.id).filter(|_| categorized);
        let current_sort = match page {
            Some(p) if categorized => p.category.sort(st.deps.kind),
            _ => st.deps.categories.global_sort(),
        };
        (
            st.deps.kind,
            st.deps.prefs.clone(),
            st.deps.categories.clone(),
            !(st.deps.logged_in)().is_empty(),
            category,
            current_sort,
            (st.deps.downloaded_only)(),
            (st.deps.tracker_names)(),
        )
    };
    let weak = Rc::downgrade(inner);
    let set_sort: Rc<dyn Fn(LibrarySort)> = Rc::new(move |sort| {
        let (categories, weak) = (categories.clone(), weak.clone());
        glib::spawn_future_local(async move {
            if let Err(e) = categories.set_sort_mode(category, sort, kuro_domain::library::order::new_seed).await {
                tracing::error!(error = %e, "saving the library sort failed");
            }
            if let Some(inner) = weak.upgrade() {
                submit(&inner);
            }
        });
    });
    // a filter write reloads the page; the sheet may already be gone, so the reload goes through a weak handle
    let weak_filter = Rc::downgrade(inner);
    let on_change: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(inner) = weak_filter.upgrade() {
            submit(&inner);
        }
    });
    let filter_tab = filter_tab::build(kind, prefs.clone(), downloaded_only, &trackers, on_change);
    let window_width = window_width(&inner.borrow().scroller);
    sheet::show(sheet::Ctx { kind, prefs, has_tracker, current_sort, set_sort, filter_tab, window_width });
}

/// The continue button: opens the entry's next item, in the external player when the preference is on, or says
/// that nothing is left (E-25).
fn resume_entry(inner: &Rc<RefCell<Inner>>, row: &LibraryRow) {
    let (db, kind, downloaded_only, item_downloaded, always_external, resume) = {
        let st = inner.borrow();
        (
            st.deps.db.clone(),
            st.deps.kind,
            (st.deps.downloaded_only)(),
            st.deps.item_downloaded.clone(),
            st.deps.always_external.clone(),
            st.nav.resume.clone(),
        )
    };
    match actions::next_item(&db, kind, row, downloaded_only, |item| item_downloaded(item)) {
        Ok(Some(item)) => resume(Resume { entry: row.id, item, external: always_external() }),
        Ok(None) => crate::library::toast::notify(actions::NO_NEXT_ITEM),
        Err(e) => {
            tracing::error!(error = %e, entry = row.id, "finding the next item failed");
            crate::library::toast::notify(actions::NO_NEXT_ITEM);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::query::SourceInfo;
    use kuro_data::manga::{Manga, MangaRepo};
    use kuro_prefs::store::PrefStore;
    use std::time::Duration;

    /// Pumps the default main context until `done` holds or two seconds pass.
    fn pump_until(done: impl Fn() -> bool) -> bool {
        let ctx = glib::MainContext::default();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            while ctx.iteration(false) {}
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        done()
    }

    #[test]
    fn the_view_loads_the_worker_reply_and_renders_selection_and_search() {
        crate::testing::with_display(|| {
            let ctx = glib::MainContext::default();
            let _owner = ctx.acquire().expect("main context");
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("manga.db");
            let writer = kuro_db::schema_manga::open(&path).expect("open manga db");
            let db = Arc::new(Db::new(&path, writer).expect("manga db handle"));
            let store = PrefStore::in_memory();
            let prefs = Rc::new(LibraryPrefs::new(store.clone()));
            MangaRepo::new(db.clone())
                .insert(&Manga { favorite: true, source: 7, url: "u".into(), title: "Berserk".into(), ..Manga::create() })
                .expect("insert");
            let categories =
                Rc::new(CategoryService::new(db.clone(), EntryKind::Manga, Arc::new(LibraryPrefs::new(store))));
            let deps = Deps {
                kind: EntryKind::Manga,
                db,
                prefs,
                categories,
                facts: Facts {
                    downloads: Arc::new(|_| 0),
                    source: Arc::new(|_| SourceInfo { name: "Local".into(), language: "en".into() }),
                },
                downloaded_only: Rc::new(|| false),
                logged_in: Rc::new(BTreeSet::new),
                tracker_names: Rc::new(Vec::new),
                always_external: Rc::new(|| false),
                item_downloaded: Rc::new(|_| false),
            };
            let selecting_calls: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(Vec::new()));
            let nav = Nav {
                open: Rc::new(|_| {}),
                resume: Rc::new(|_| {}),
                download: Rc::new(|_, _| {}),
                delete: Rc::new(|_| {}),
                global_search: Rc::new(|_| {}),
                edit_categories: Rc::new(|| {}),
                selecting: Rc::new({
                    let seen = selecting_calls.clone();
                    move |on| seen.borrow_mut().push(on)
                }),
                update: Rc::new(|_| true),
                update_running: Rc::new(|| false),
            };
            let view = LibraryView::new(deps, nav).expect("view");
            let inner = view.inner.clone();
            assert!(pump_until(|| inner.borrow().built.is_some()), "the worker reply arrives");
            assert_eq!(inner.borrow().store.n_items(), 1, "one cover for the one entry");
            assert_eq!(inner.borrow().title.label().as_str(), "Manga", "B-7.3: tabs on shows the default title, even with one page");

            let row = inner.borrow().built.as_ref().expect("built").pages[0].items[0].row.clone();
            select_with(&inner, |sel, _| sel.toggle(&row));
            assert!(inner.borrow().bar.reveals_child(), "the selection bar opens");
            assert_eq!(inner.borrow().bar_label.label().as_str(), "1 selected");
            assert!(inner.borrow().sel_header.is_visible() && !inner.borrow().header.is_visible(), "the selection header replaces the toolbar");
            assert!(inner.borrow().download_button.is_visible(), "no local entry is selected");
            select_with(&inner, |sel, _| sel.clear());
            assert!(!inner.borrow().bar.reveals_child());
            assert!(inner.borrow().header.is_visible() && !inner.borrow().sel_header.is_visible());

            assert_eq!(*selecting_calls.borrow(), vec![true, false], "the shell hears selection start and end once each (B-1.4)");

            // B-1.3: a query from outside fills the box, and B-6.4: the global row is the first cell while it matches
            view.set_query("Berserk");
            assert_eq!(inner.borrow().search.text().as_str(), "Berserk");
            assert!(pump_until(|| inner.borrow().store.n_items() == 2), "the global row plus the match");
            let first = inner.borrow().store.item(0).and_downcast::<glib::BoxedAnyObject>().expect("first cell");
            assert!(matches!(&*first.borrow::<Cell>(), Cell::Global(q) if q == "Berserk"));

            view.set_query("zzz");
            let search_done = pump_until(|| inner.borrow().built.as_ref().is_some_and(|b| b.distinct_count == 0));
            assert!(search_done, "a search with no match empties the page");
            assert_eq!(inner.borrow().store.n_items(), 0);
            assert_eq!(inner.borrow().empty_label.label().as_str(), "No results found");
        });
    }
}
