//! The anime details route: a thin GTK page over `AnimeEntryModel` (state) and `AnimeRunner` (the store). It shows a
//! loading state until the first read arrives, the page when the model is `Success`, and a "not found" state with a
//! Back button when the row is missing.
//!
//! Layout: at or above `layout::TWO_PANE_MIN_WIDTH` the info box, description and actions sit in a left pane and the
//! item header and episode list in the right one; below it everything is one column and the info block scrolls with
//! the list (it is the first row of the list view, so 3000 episodes still cost 30 widgets). The airing row is pinned
//! above both layouts, full width. The FAB floats over the list and collapses to its icon on a forward scroll.
//! `ui::screen("anime_entry_details")` resolves here through `build`, and `open(id)` pushes the page of one anime.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O, glib};
use kuro_data::anime::AnimeRepo;
use kuro_db::handle::Db;
use kuro_domain::entries::defaults::{anime_defaults_to_library_detached, set_episode_settings_default};
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::store::PrefStore;

use super::actions::{OpenPlan, SwipeEffect, bulk_mark_targets, continue_plan, swipe_effect, tap_plan};
use super::airing::{Airing, format_airing, visible as airing_visible};
use super::bottom::{BottomAction, Effect, MenuContext, Selected, bottom_actions};
use super::gesture::Click;
use super::info::header_model;
use super::layout::{bottom_bar_width, fab_expanded, two_pane};
use super::model::{AnimeEntryModel, AnimeLoadContext};
use super::refresh::refresh_snack;
use super::remote::{NoSources, RemoteAnimeBackend};
use super::row_view::{EpisodeList, EpisodeSwipe};
use super::runner::{self, AnimeBackend, AnimeCmd, AnimeEvent, AnimeRunner};
use super::settings::default_saved_message;
use super::settings_view::{Change, SettingsWindow};
use super::store::AnimeStore;
use super::info_view;
use crate::data;
use crate::entry_core::banner_view::{self, BannerView};
use crate::entry_core::chapter_actions::previous_ids;
use crate::entry_core::description_view;
use crate::entry_core::header::{HeaderModel, SearchRequest};
use crate::entry_core::header_view::{Handlers, LibraryButton};
use crate::entry_core::message::{Notices, UserMessage, present};
use crate::entry_core::text::{Description, Expansion, description};
use crate::ui;

/// The width of the left pane in the two-pane layout.
const LEFT_WIDTH: i32 = 380;
/// The window width assumed before the page has one: the app's default width.
const FALLBACK_WIDTH: i32 = 1180;

thread_local! {
    static TARGET: Cell<Option<i64>> = const { Cell::new(None) };
}

/// Names the anime the next `build()` shows. `ui::open("anime_entry_details")` carries no argument, so a caller sets
/// the target first.
pub fn target(anime_id: i64) {
    TARGET.with(|t| t.set(Some(anime_id)));
}

/// Pushes the details page of one anime.
pub fn open(anime_id: i64) {
    ui::push(&page_for(anime_id));
}

/// The page for the `anime_entry_details` screen id. Without a target it is the mockup page, so a caller that has no
/// row to name yet keeps working.
pub fn build() -> GBox {
    match TARGET.with(Cell::take) {
        Some(id) => page_for(id),
        None => crate::entry::entry("Title 1", true),
    }
}

fn page_for(anime_id: i64) -> GBox {
    let app = data::app();
    let db = app.db(EntryKind::Anime);
    let prefs = app.base.store().clone();
    let show_airing = prefs.pref::<bool>("show_next_episode_airing_time", true).get();
    let ctx = AnimeLoadContext {
        from_source: false,
        downloaded_only: app.base.downloaded_only.get(),
        ..Default::default()
    };
    let handle = tokio::runtime::Handle::current();
    // no registry of live anime sources yet: every fetch reports that the source is not installed
    let backend: Arc<dyn AnimeBackend> = Arc::new(RemoteAnimeBackend::new(Arc::clone(&db), &prefs, Arc::new(NoSources), handle.clone()));
    let runner = Rc::new(runner::spawn(&handle, AnimeStore::new(Arc::clone(&db)), prefs.clone(), Arc::clone(&backend), ctx));
    runner.send(AnimeCmd::Load { anime_id });
    assemble(runner, show_airing, Env { prefs, backend, db, handle: Some(handle) })
}

/// What the page needs from the app besides its runner: the preference store, the backend the runner uses (for the
/// player and torrent choices that happen on the GTK thread), the database and the runtime.
struct Env {
    prefs: PrefStore,
    backend: Arc<dyn AnimeBackend>,
    db: Arc<Db>,
    handle: Option<tokio::runtime::Handle>,
}

/// Everything the widgets and handlers share. Handlers hold `Weak<View>`; the event pump holds the only strong
/// reference, so the page frees itself when the pump ends.
struct View {
    model: RefCell<Option<AnimeEntryModel>>,
    /// The first read arrived (`Loaded`), with or without a row.
    loaded: Cell<bool>,
    runner: Rc<AnimeRunner>,
    env: Env,
    show_airing: bool,
    clock: std::time::Instant,
    /// The snackbar layer: one message at a time, a new one replaces it (E-UI-3).
    notices: RefCell<Notices<AnimeCmd>>,
    snack: BannerView,
    library: LibraryButton,
    refresh_button: Button,
    bottom: gtk::Revealer,
    bottom_row: GBox,
    stack: gtk::Stack,
    title: Option<Label>,
    toolbar: Vec<gtk::Widget>,
    list: Rc<EpisodeList>,
    scroller: gtk::ScrolledWindow,
    left_scroll: gtk::ScrolledWindow,
    left_host: GBox,
    separator: gtk::Separator,
    /// The info box and description: the left pane, or the first row of the list.
    top: GBox,
    /// The item header, the refresh spinner and the empty line: the head of the list.
    list_head: GBox,
    count: Label,
    missing: Label,
    header_button: Button,
    spinner: gtk::Spinner,
    empty: Label,
    airing_row: GBox,
    airing_title: Label,
    airing_time: Label,
    fab: Button,
    fab_label: Label,
    scroll: Cell<(f64, bool)>,
    bar: gtk::Revealer,
    bar_count: Label,
    settings: RefCell<Option<Rc<SettingsWindow>>>,
    two_pane: Cell<Option<bool>>,
    width: Cell<i32>,
    built: RefCell<Option<(HeaderModel, Description, bool)>>,
    expansion: Rc<Cell<Expansion>>,
}

fn icon_button(icon: &str, tip: &str) -> Button {
    let b = ui::icon_button(icon, tip);
    b.set_focus_on_click(false);
    b
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Milliseconds since the page opened, the clock of the snackbar layer.
fn now_ms_u64(clock: std::time::Instant) -> u64 {
    clock.elapsed().as_millis() as u64
}

/// The preference of the player the taps use (`pref_always_use_external_player`).
fn always_external(prefs: &PrefStore) -> bool {
    prefs.pref::<bool>("pref_always_use_external_player", false).get()
}

/// The torrent server preference (`pref_torrserver_enable`).
fn torrent_enabled(prefs: &PrefStore) -> bool {
    prefs.pref::<bool>("pref_torrserver_enable", false).get()
}

/// Takes a widget out of whatever box holds it, so it can be added to another.
fn detach(widget: &impl IsA<gtk::Widget>) {
    if let Some(parent) = widget.parent().and_downcast::<GBox>() {
        parent.remove(widget);
    }
}

fn assemble(runner: Rc<AnimeRunner>, show_airing: bool, env: Env) -> GBox {
    let stack = gtk::Stack::new();
    let spinner_big = gtk::Spinner::builder().spinning(true).width_request(32).height_request(32).build();
    let loading = GBox::new(O::Vertical, 12);
    loading.set_halign(Align::Center);
    loading.set_valign(Align::Center);
    loading.append(&spinner_big);
    loading.append(&Label::new(Some(&tr(MsgKey::loading))));
    stack.add_named(&loading, Some("loading"));
    let missing_page = ui::status("dialog-error-symbolic", &tr(MsgKey::no_results_found));
    let back_missing = Button::with_label(&tr(MsgKey::action_webview_back));
    back_missing.set_halign(Align::Center);
    back_missing.connect_clicked(|_| ui::pop());
    missing_page.append(&back_missing);
    stack.add_named(&missing_page, Some("missing"));

    // the list head: the item header button, the refresh spinner and the empty line
    let count = Label::builder().xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["heading"]).build();
    let missing = Label::builder().xalign(1.0).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["dim-label", "caption"]).build();
    let header_row = GBox::new(O::Horizontal, 8);
    header_row.append(&count);
    header_row.append(&missing);
    header_row.append(&gtk::Image::from_icon_name("view-filter-symbolic"));
    let header_button = Button::builder().child(&header_row).css_classes(["flat"]).build();
    header_button.set_margin_start(8);
    header_button.set_margin_end(8);
    let spinner = gtk::Spinner::builder().halign(Align::Center).build();
    let empty = ui::dim(&tr(MsgKey::no_episodes_error));
    empty.set_halign(Align::Center);
    empty.set_wrap(true);
    let list_head = GBox::new(O::Vertical, 4);
    list_head.append(&header_button);
    list_head.append(&spinner);
    list_head.append(&empty);

    let weak_view: Rc<RefCell<Weak<View>>> = Rc::default();
    let list = EpisodeList::new(
        {
            let weak = Rc::clone(&weak_view);
            Rc::new(move |id: i64, long: bool, alt: bool| {
                if let Some(view) = weak.borrow().upgrade() {
                    view.row_pressed(id, long, alt);
                }
            })
        },
        {
            let weak = Rc::clone(&weak_view);
            Rc::new(move |id: i64| {
                if let Some(view) = weak.borrow().upgrade() {
                    view.row_pressed(id, false, false);
                }
            })
        },
        crate::data::is_installed().then(|| EpisodeSwipe {
            prefs: Rc::new((*data::app().library).clone()),
            run: {
                let weak = Rc::clone(&weak_view);
                Rc::new(move |id: i64, action| {
                    if let Some(view) = weak.borrow().upgrade() {
                        view.swipe_pressed(id, action);
                    }
                })
            },
        }),
    );
    let scroller = gtk::ScrolledWindow::builder()
        .child(&list.widget)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .hexpand(true)
        .vexpand(true)
        .build();
    let top = GBox::new(O::Vertical, 0);
    let left_host = GBox::new(O::Vertical, 0);
    let left_scroll = gtk::ScrolledWindow::builder()
        .child(&left_host)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .width_request(LEFT_WIDTH)
        .vexpand(true)
        .visible(false)
        .build();
    let separator = gtk::Separator::new(O::Vertical);
    separator.set_visible(false);
    let body = GBox::new(O::Horizontal, 0);
    body.append(&left_scroll);
    body.append(&separator);
    body.append(&scroller);

    // the airing row spans both panes
    let airing_title = Label::builder().xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["heading"]).build();
    let airing_time = Label::builder().xalign(1.0).wrap(true).css_classes(["dim-label"]).build();
    let airing_row = GBox::new(O::Horizontal, 12);
    airing_row.set_margin_start(16);
    airing_row.set_margin_end(16);
    airing_row.set_margin_top(8);
    airing_row.set_margin_bottom(8);
    airing_row.set_visible(false);
    airing_row.append(&airing_title);
    airing_row.append(&airing_time);

    let fab_label = Label::new(None);
    let fab_box = GBox::new(O::Horizontal, 8);
    fab_box.append(&gtk::Image::from_icon_name("media-playback-start-symbolic"));
    fab_box.append(&fab_label);
    let fab = Button::builder()
        .child(&fab_box)
        .css_classes(["suggested-action", "pill"])
        .halign(Align::End)
        .valign(Align::End)
        .margin_end(16)
        .margin_bottom(16)
        .visible(false)
        .build();
    fab.set_focus_on_click(false);

    let column = GBox::new(O::Vertical, 0);
    column.append(&airing_row);
    body.set_vexpand(true);
    column.append(&body);
    let snack_weak: Rc<RefCell<Weak<View>>> = Rc::default();
    let snack = banner_view::build(
        {
            let weak = Rc::clone(&snack_weak);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.snack_action();
                }
            }
        },
        {
            let weak = Rc::clone(&snack_weak);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.snack_dismiss();
                }
            }
        },
    );
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&column));
    overlay.add_overlay(&fab);
    overlay.add_overlay(&snack.widget);
    overlay.set_vexpand(true);

    // the action bar: cancel, the count, select all and invert
    let cancel = icon_button("window-close-symbolic", &tr(MsgKey::action_cancel));
    let bar_count = Label::builder().xalign(0.0).hexpand(true).build();
    let select_all = icon_button("edit-select-all-symbolic", &tr(MsgKey::action_select_all));
    let invert = icon_button("edit-select-invert-symbolic", &tr(MsgKey::action_select_inverse));
    let bar_row = GBox::new(O::Horizontal, 6);
    bar_row.add_css_class("toolbar");
    for w in [cancel.upcast_ref::<gtk::Widget>(), bar_count.upcast_ref(), select_all.upcast_ref(), invert.upcast_ref()] {
        bar_row.append(w);
    }
    let bar = gtk::Revealer::builder().child(&bar_row).transition_type(gtk::RevealerTransitionType::SlideUp).build();

    // the bottom action bar of the selection (behaviour 1.4): its rows follow the selected episodes, it is hidden
    // when nothing is selected, and it is as wide as the window narrow and half of it wide
    let bottom_row = GBox::new(O::Horizontal, 6);
    bottom_row.add_css_class("toolbar");
    bottom_row.set_halign(Align::Center);
    let bottom = gtk::Revealer::builder().child(&bottom_row).transition_type(gtk::RevealerTransitionType::SlideUp).build();

    let ready = GBox::new(O::Vertical, 0);
    ready.append(&overlay);
    ready.append(&bar);
    ready.append(&bottom);
    stack.add_named(&ready, Some("ready"));

    let filter = icon_button("view-filter-symbolic", &tr(MsgKey::action_filter));
    filter.set_sensitive(false);
    let refresh_button = icon_button("view-refresh-symbolic", &tr(MsgKey::action_webview_refresh));
    let library = LibraryButton::new();
    let toolbar: Vec<gtk::Widget> = vec![refresh_button.clone().upcast(), filter.clone().upcast()];
    let page = ui::page("", &stack, toolbar.clone(), false);
    ui::tag(&page, "anime_entry_details");
    let header = page.first_child().and_downcast::<gtk::HeaderBar>();
    let title = header.as_ref().and_then(|h| h.title_widget()).and_downcast::<Label>();
    let back = Button::from_icon_name("go-previous-symbolic");
    back.set_tooltip_text(Some(&tr(MsgKey::action_webview_back)));
    back.set_focus_on_click(false);
    if let Some(header) = &header {
        header.pack_start(&back);
    }

    let view = Rc::new(View {
        model: RefCell::new(None),
        loaded: Cell::new(false),
        runner: Rc::clone(&runner),
        env,
        show_airing,
        clock: std::time::Instant::now(),
        notices: RefCell::new(Notices::default()),
        snack,
        library: library.clone(),
        refresh_button: refresh_button.clone(),
        bottom,
        bottom_row,
        stack,
        title,
        toolbar,
        list: Rc::clone(&list),
        scroller: scroller.clone(),
        left_scroll,
        left_host,
        separator,
        top,
        list_head,
        count,
        missing,
        header_button: header_button.clone(),
        spinner,
        empty,
        airing_row,
        airing_title,
        airing_time,
        fab: fab.clone(),
        fab_label,
        scroll: Cell::new((0.0, false)),
        bar,
        bar_count,
        settings: RefCell::new(None),
        two_pane: Cell::new(None),
        width: Cell::new(FALLBACK_WIDTH),
        built: RefCell::new(None),
        expansion: Rc::new(Cell::new(Expansion::new(false, false))),
    });
    let weak = Rc::downgrade(&view);
    *weak_view.borrow_mut() = weak.clone();
    *snack_weak.borrow_mut() = weak.clone();
    view.apply_width(FALLBACK_WIDTH);

    let on_view = |f: fn(&Rc<View>)| {
        let weak = weak.clone();
        move |_: &Button| {
            if let Some(view) = weak.upgrade() {
                f(&view);
            }
        }
    };
    filter.connect_clicked(on_view(View::open_settings));
    refresh_button.connect_clicked(on_view(View::refresh));
    library.button.connect_clicked(on_view(View::toggle_library));
    header_button.connect_clicked(on_view(View::open_settings));
    cancel.connect_clicked(on_view(|v| v.edit_selection(|m, _| m.selection.clear())));
    select_all.connect_clicked(on_view(|v| v.edit_selection(|m, ids| m.selection.select_all(ids))));
    invert.connect_clicked(on_view(|v| v.edit_selection(|m, ids| m.selection.invert(ids))));
    fab.connect_clicked(on_view(View::open_next));
    back.connect_clicked(on_view(|v| {
        if !v.back_pressed() {
            ui::pop();
        }
    }));
    scroller.vadjustment().connect_value_changed({
        let weak = weak.clone();
        move |_| {
            if let Some(view) = weak.upgrade() {
                view.update_fab();
            }
        }
    });
    page.add_controller(key_controller(&weak));
    // the layout follows the page width; the tick callback goes away with the widget
    page.add_tick_callback({
        let weak = weak.clone();
        move |page, _| {
            if let Some(view) = weak.upgrade()
                && page.width() > 0
            {
                view.apply_width(page.width());
            }
            glib::ControlFlow::Continue
        }
    });
    // the countdown re-derives from the absolute target each second and stops with the page
    glib::timeout_add_local(Duration::from_secs(1), {
        let weak = weak.clone();
        move || match weak.upgrade() {
            Some(view) => {
                view.tick_airing();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        }
    });

    // the snackbar times out on its own clock
    glib::timeout_add_local(Duration::from_millis(250), {
        let weak = weak.clone();
        move || match weak.upgrade() {
            Some(view) => {
                view.notices.borrow_mut().tick(now_ms_u64(view.clock));
                view.render_snack();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        }
    });

    // the pump owns the only strong reference to the view
    let events = runner.events();
    glib::spawn_future_local(async move {
        while let Ok(first) = events.recv().await {
            view.on_event(first);
            while let Ok(next) = events.try_recv() {
                view.on_event(next);
            }
            view.render();
        }
    });
    // a page that leaves the stack stops its runner: subscriptions are dropped, a fetch is cancelled, writes finish
    page.connect_destroy(move |_| runner.send(AnimeCmd::Shutdown));
    page
}

/// Escape clears the selection first and then goes back; Ctrl+A and Ctrl+I select all and invert while selecting.
fn key_controller(weak: &Weak<View>) -> gtk::EventControllerKey {
    let keys = gtk::EventControllerKey::new();
    let weak = weak.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
        let ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
        let selecting = view.model.borrow().as_ref().is_some_and(|m| m.selection.is_active());
        match key {
            gtk::gdk::Key::Escape => {
                if !view.back_pressed() {
                    ui::pop();
                }
                glib::Propagation::Stop
            }
            gtk::gdk::Key::a | gtk::gdk::Key::A if ctrl && selecting => {
                view.edit_selection(|m, ids| m.selection.select_all(ids));
                glib::Propagation::Stop
            }
            gtk::gdk::Key::i | gtk::gdk::Key::I if ctrl && selecting => {
                view.edit_selection(|m, ids| m.selection.invert(ids));
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    keys
}

impl View {
    fn on_event(&self, event: AnimeEvent) {
        match event {
            AnimeEvent::Loaded(loaded) => {
                self.loaded.set(true);
                if let Some(m) = &loaded {
                    let large = self.two_pane.get().unwrap_or(false);
                    self.expansion.set(Expansion::new(large, m.from_source));
                }
                *self.model.borrow_mut() = loaded.map(|b| *b);
            }
            AnimeEvent::Snapshot { anime, episodes } => {
                if let Some(m) = self.model.borrow_mut().as_mut() {
                    m.apply_snapshot(*anime, episodes);
                    if let Some(settings) = self.settings.borrow().as_ref() {
                        settings.update(m.anime.episode_flags, m.downloaded_only);
                    }
                }
            }
            AnimeEvent::RefreshEnded(result) => {
                // a failed refresh says why in a snackbar (behaviour 19)
                if let Some(message) = result.as_ref().err().and_then(refresh_snack) {
                    self.present(message, None);
                }
                if let Some(m) = self.model.borrow_mut().as_mut() {
                    m.finish_refresh(result);
                }
            }
            AnimeEvent::Added => {}
            AnimeEvent::Removed(outcome) => {
                // downloads exist: ask whether they go too. Dismissing the prompt keeps them.
                let anime_id = self.model.borrow().as_ref().map(|m| m.anime.id);
                if let (true, Some(anime_id)) = (outcome.ask_delete_downloads, anime_id) {
                    self.present(UserMessage::DeleteDownloadsForAnime, Some(AnimeCmd::DeleteDownloads { anime_id }));
                }
            }
            AnimeEvent::Failed(message) => tracing::warn!("anime entry: {message}"),
        }
    }

    /// Switches between the two layouts. It moves the info block between the left pane and the first row of the list.
    fn apply_width(self: &Rc<Self>, width: i32) {
        self.width.set(width);
        let two = two_pane(width);
        if self.two_pane.get() == Some(two) {
            return;
        }
        self.two_pane.set(Some(two));
        self.left_scroll.set_visible(two);
        self.separator.set_visible(two);
        detach(&self.top);
        if two {
            self.left_host.append(&self.top);
            self.list.set_leading(vec![self.list_head.clone().upcast()]);
        } else {
            self.list.set_leading(vec![self.top.clone().upcast(), self.list_head.clone().upcast()]);
        }
        // the cover size depends on the layout
        *self.built.borrow_mut() = None;
        self.render();
    }

    fn render(self: &Rc<Self>) {
        let model = self.model.borrow();
        let name = match (self.loaded.get(), model.as_ref()) {
            (false, _) => "loading",
            (true, None) => "missing",
            (true, Some(_)) => "ready",
        };
        self.stack.set_visible_child_name(name);
        for w in &self.toolbar {
            w.set_sensitive(model.is_some());
        }
        let Some(m) = model.as_ref() else { return };
        if let Some(title) = &self.title {
            title.set_label(&m.anime.title);
        }
        let rows = m.rows();
        let none_shown = rows.is_empty();
        self.list.sync(rows);
        let header = m.item_header();
        self.count.set_label(&header.count);
        self.missing.set_label(header.missing.as_deref().unwrap_or(""));
        self.missing.set_visible(header.missing.is_some());
        self.header_button.set_sensitive(header.enabled);
        self.spinner.set_spinning(m.flow.refreshing);
        self.spinner.set_visible(m.flow.refreshing);
        // one refresh runs at a time
        self.refresh_button.set_sensitive(!m.flow.refreshing);
        self.empty.set_visible(none_shown && !m.flow.refreshing);
        self.empty.set_label(&tr(MsgKey::no_episodes_error));
        self.library.set(m.anime.favorite);
        // the button stays while nothing is selected, and is disabled when no episode is unseen (T-EPS-NEXT-1)
        let fab = m.fab();
        self.fab.set_visible(!m.selection.is_active());
        self.fab.set_sensitive(m.next_unseen().is_some());
        self.fab_label.set_label(&tr(fab.label()));
        self.update_fab();
        self.bar.set_reveal_child(m.selection.is_active());
        let selected = m.selection.count_in(&m.processed_ids());
        self.bar_count.set_label(&format!("{selected} {}", tr(MsgKey::selected).to_lowercase()));
        self.rebuild_top(m);
        self.render_bottom(m);
        drop(model);
        self.render_snack();
        self.tick_airing();
    }

    /// The bottom action bar: the rows `bottom_actions` gives for the selected episodes, shown only while something
    /// is selected, as wide as the window narrow and half of it wide.
    fn render_bottom(self: &Rc<Self>, m: &AnimeEntryModel) {
        let selected: Vec<Selected> = m
            .episodes
            .iter()
            .filter(|e| m.selection.contains(e.id))
            .map(|e| Selected { bookmark: e.bookmark, fillermark: e.fillermark, seen: e.seen, last_second_seen: e.last_second_seen, downloaded: false })
            .collect();
        // no anime download queue is wired, so the bar has no download handler and offers no Download or Delete
        let ctx = MenuContext { has_download_handler: false, always_external: always_external(&self.env.prefs) };
        let rows = bottom_actions(&selected, ctx);
        while let Some(child) = self.bottom_row.first_child() {
            self.bottom_row.remove(&child);
        }
        for action in &rows {
            let button = Button::builder().css_classes(["flat"]).build();
            let content = GBox::new(O::Horizontal, 6);
            content.append(&gtk::Image::from_icon_name(action.icon()));
            content.append(&Label::new(Some(&tr(action.label()))));
            button.set_child(Some(&content));
            let weak = Rc::downgrade(self);
            let action = *action;
            button.connect_clicked(move |_| {
                if let Some(view) = weak.upgrade() {
                    view.bottom_pressed(action);
                }
            });
            self.bottom_row.append(&button);
        }
        match bottom_bar_width(self.width.get()) {
            Some(width) => {
                self.bottom_row.set_width_request(width);
                self.bottom_row.set_halign(Align::Center);
            }
            None => {
                self.bottom_row.set_width_request(-1);
                self.bottom_row.set_halign(Align::Fill);
            }
        }
        self.bottom.set_reveal_child(!rows.is_empty());
    }

    /// A press on a row of the bottom bar: the writes use the selected episodes that differ, and the selection is
    /// cleared after the call (behaviour 11.2).
    fn bottom_pressed(self: &Rc<Self>, action: BottomAction) {
        let anime_id;
        let (cmds, open) = {
            let mut model = self.model.borrow_mut();
            let Some(m) = model.as_mut().filter(|m| m.accepts_dispatch()) else { return };
            anime_id = m.anime.id;
            let selected: Vec<_> = m.episodes.iter().filter(|e| m.selection.contains(e.id)).cloned().collect();
            let mut open = None;
            let cmds = match action.effect() {
                Effect::Mark(mark, on) => vec![AnimeCmd::SetMark { ids: bulk_mark_targets(&selected, mark, on), mark, on }],
                Effect::Seen(seen) => {
                    let ids = selected.iter().filter(|e| e.seen != seen || (!seen && e.last_second_seen > 0)).map(|e| e.id).collect();
                    vec![AnimeCmd::SetSeen { ids, seen: seen }]
                }
                Effect::PreviousSeen => {
                    let pointer = selected.first().map(|e| e.id);
                    let ids = pointer
                        .map(|p| previous_ids(&m.processed_ids(), p, m.filters().is_some_and(|f| f.descending)))
                        .unwrap_or_default();
                    // only the unseen ones change
                    let ids = ids.into_iter().filter(|id| m.episodes.iter().any(|e| e.id == *id && !e.seen)).collect();
                    vec![AnimeCmd::SetSeen { ids, seen: true }]
                }
                Effect::Open { external } => {
                    open = selected.first().map(|e| (e.id, external));
                    Vec::new()
                }
                // no download queue: the bar does not offer them
                Effect::Download | Effect::Delete => Vec::new(),
            };
            m.selection.clear();
            (cmds, open)
        };
        let _ = anime_id;
        for cmd in cmds {
            self.runner.send(cmd);
        }
        if let Some((id, external)) = open {
            self.open_episode(id, OpenPlan { external, torrent_server: self.torrent_server_needed() });
        }
        self.render();
    }

    /// A swipe on a row: the action of the side, written for that one episode. It works while selecting too (E-5).
    fn swipe_pressed(self: &Rc<Self>, id: i64, action: kuro_prefs::library::EpisodeSwipeAction) {
        let effect = {
            let model = self.model.borrow();
            let Some(m) = model.as_ref().filter(|m| m.accepts_dispatch()) else { return };
            m.episodes.iter().find(|e| e.id == id).and_then(|e| swipe_effect(action, e))
        };
        match effect {
            Some(SwipeEffect::Seen { id, seen }) => self.runner.send(AnimeCmd::SetSeen { ids: vec![id], seen }),
            Some(SwipeEffect::Mark { ids, mark, on }) => self.runner.send(AnimeCmd::SetMark { ids, mark, on }),
            None => {}
        }
    }

    /// Whether the entry's source is a torrent source and the torrent server is on (17.4, 17.5).
    fn torrent_server_needed(&self) -> bool {
        let source = self.model.borrow().as_ref().map(|m| m.anime.source);
        torrent_enabled(&self.env.prefs) && source.is_some_and(|s| self.env.backend.is_torrent_source(s))
    }

    /// Shows a message in the snackbar layer, replacing the one on screen. `action` is what its button runs.
    fn present(&self, message: UserMessage, action: Option<AnimeCmd>) {
        self.notices.borrow_mut().show(present(message), action, now_ms_u64(self.clock));
    }

    fn render_snack(&self) {
        let notices = self.notices.borrow();
        match notices.current() {
            Some((banner, dismissible)) => self.snack.show_with(Some(banner), dismissible),
            None => self.snack.show_with::<AnimeCmd>(None, true),
        }
    }

    fn snack_action(self: &Rc<Self>) {
        let cmd = self.notices.borrow_mut().activate(now_ms_u64(self.clock));
        if let Some(cmd) = cmd {
            self.runner.send(cmd);
        }
        self.render_snack();
    }

    fn snack_dismiss(self: &Rc<Self>) {
        self.notices.borrow_mut().dismiss(now_ms_u64(self.clock));
        self.render_snack();
    }

    /// The refresh button: a manual refresh of everything. One runs at a time; a second request is not sent.
    fn refresh(self: &Rc<Self>) {
        let anime_id = {
            let mut model = self.model.borrow_mut();
            let Some(m) = model.as_mut().filter(|m| m.accepts_dispatch() && !m.flow.refreshing) else { return };
            m.flow.begin_refresh();
            m.anime.id
        };
        self.runner.send(AnimeCmd::Refresh { anime_id });
        self.render();
    }

    /// The library button: adds the entry, or removes it from the library.
    fn toggle_library(self: &Rc<Self>) {
        let anime_id = self.model.borrow().as_ref().filter(|m| m.accepts_dispatch()).map(|m| m.anime.id);
        if let Some(anime_id) = anime_id {
            self.runner.send(AnimeCmd::ToggleFavorite { anime_id });
        }
    }

    /// Rebuilds the info box and description when what they show changed.
    fn rebuild_top(self: &Rc<Self>, m: &AnimeEntryModel) {
        let two = self.two_pane.get().unwrap_or(false);
        let key = (header_model(&m.anime, &m.source), description(m.anime.description.as_deref()), two);
        if self.built.borrow().as_ref() == Some(&key) {
            return;
        }
        while let Some(child) = self.top.first_child() {
            self.top.remove(&child);
        }
        let column = if two { LEFT_WIDTH } else { self.width.get() };
        self.top.append(&info_view::build(&key.0, two, column, &self.handlers()));
        let text = description_view::build(&key.1, Rc::clone(&self.expansion));
        text.set_margin_start(16);
        text.set_margin_end(16);
        self.top.append(&text);
        detach(&self.library.button);
        self.top.append(&self.library.button);
        *self.built.borrow_mut() = Some(key);
    }

    fn handlers(self: &Rc<Self>) -> Handlers {
        Handlers {
            on_search: Rc::new(|request: SearchRequest| {
                // ponytail: the search screens take no query argument yet, so this opens them
                ui::open(if request.global { "global_search" } else { "browse_source" });
            }),
            on_copy: Rc::new(|text: String| {
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&text);
                }
            }),
            on_cover: Rc::new(|| {
                let screen = ui::screen("anime_entry_details");
                if let Some(dialog) = screen.dialogs.iter().find(|d| d.id == "full_cover") {
                    ui::show_dialog(dialog);
                }
            }),
        }
    }

    /// The FAB label shows at the top, after a backward scroll and at the end of the list (`shouldExpandFAB`).
    fn update_fab(&self) {
        let adj = self.scroller.vadjustment();
        let value = adj.value();
        let (last, was_backward) = self.scroll.get();
        let backward = if (value - last).abs() > f64::EPSILON { value < last } else { was_backward };
        self.scroll.set((value, backward));
        let can_forward = value + adj.page_size() < adj.upper() - 1.0;
        self.fab_label.set_visible(fab_expanded(usize::from(value > 0.0), value as i32, backward, can_forward));
    }

    /// One tick of the airing row: the remaining time comes from the absolute target.
    fn tick_airing(&self) {
        let model = self.model.borrow();
        let Some(m) = model.as_ref() else { return };
        let airing = Airing::of(&m.anime);
        let remaining = airing.map_or(0, |a| a.remaining(now_ms()));
        let show = airing_visible(remaining, self.show_airing, m.anime.status);
        self.airing_row.set_visible(show);
        if let (true, Some(airing)) = (show, airing) {
            self.airing_title.set_label(&airing.title());
            self.airing_time.set_label(&format_airing(remaining));
        }
    }

    fn row_pressed(self: &Rc<Self>, id: i64, long: bool, alt: bool) {
        let open = {
            let mut model = self.model.borrow_mut();
            let Some(m) = model.as_mut().filter(|m| m.accepts_dispatch()) else { return };
            if long {
                m.selection.long_press(id);
                None
            } else {
                match m.selection.click(id) {
                    Click::Open(id) => Some(id),
                    Click::Toggled { .. } => None,
                }
            }
        };
        if let Some(id) = open {
            // a tap opens in the player the setting says, and Alt picks the other one (17.4)
            let plan = tap_plan(always_external(&self.env.prefs), alt, torrent_enabled(&self.env.prefs), self.is_torrent_source());
            self.open_episode(id, plan);
        }
        self.render();
    }

    fn is_torrent_source(&self) -> bool {
        let source = self.model.borrow().as_ref().map(|m| m.anime.source);
        source.is_some_and(|s| self.env.backend.is_torrent_source(s))
    }

    /// Opens an episode as the plan says. The torrent server starts before the player, and only for a torrent source.
    /// An external player the app cannot start falls back to the internal one.
    fn open_episode(&self, id: i64, plan: OpenPlan) {
        if plan.torrent_server {
            self.env.backend.start_torrent_server();
        }
        let (anime_id, title, name) = {
            let model = self.model.borrow();
            let Some(m) = model.as_ref() else { return };
            let name = m.episodes.iter().find(|e| e.id == id).map(|e| e.name.clone()).unwrap_or_default();
            (m.anime.id, m.anime.title.clone(), name)
        };
        if plan.external && self.env.backend.play_external(anime_id, id) {
            return;
        }
        crate::screens::open_media(true, &title, &name);
    }

    /// The FAB: opens the first unseen episode, with the setting as it is and no inversion (17.2).
    fn open_next(self: &Rc<Self>) {
        let next = self.model.borrow().as_ref().and_then(AnimeEntryModel::next_unseen);
        if let Some(id) = next {
            let plan = continue_plan(always_external(&self.env.prefs), torrent_enabled(&self.env.prefs), self.is_torrent_source());
            self.open_episode(id, plan);
        }
    }

    /// Edits the selection over the ids of the processed list, then redraws.
    fn edit_selection(self: &Rc<Self>, edit: impl FnOnce(&mut AnimeEntryModel, &[i64])) {
        {
            let mut model = self.model.borrow_mut();
            let Some(m) = model.as_mut().filter(|m| m.accepts_dispatch()) else { return };
            let ids = m.processed_ids();
            edit(m, &ids);
        }
        self.render();
    }

    /// Back or Escape: the selection is cleared first and the key is consumed. `false` means navigate up.
    fn back_pressed(self: &Rc<Self>) -> bool {
        let consumed = self.model.borrow_mut().as_mut().is_some_and(|m| m.selection.back());
        if consumed {
            self.render();
        }
        consumed
    }

    /// Opens the episode settings window from the header or the filter button.
    fn open_settings(self: &Rc<Self>) {
        if let Some(open) = self.settings.borrow().as_ref() {
            open.present();
            return;
        }
        let Some((word, downloaded_only, favorite)) = self
            .model
            .borrow()
            .as_ref()
            .filter(|m| m.accepts_dispatch())
            .map(|m| (m.anime.episode_flags, m.downloaded_only, m.anime.favorite))
        else {
            return;
        };
        let weak = Rc::downgrade(self);
        let window = SettingsWindow::new(
            word,
            downloaded_only,
            favorite,
            Rc::new({
                let weak = weak.clone();
                move |change| {
                    if let Some(view) = weak.upgrade() {
                        view.apply_change(change);
                    }
                }
            }),
        );
        window.on_close(move || {
            if let Some(view) = weak.upgrade() {
                view.settings.borrow_mut().take();
            }
        });
        window.present();
        *self.settings.borrow_mut() = Some(window);
    }

    /// A click in the settings window becomes a store command; the window redraws when the snapshot arrives.
    fn apply_change(&self, change: Change) {
        let Some((anime_id, word)) = self.model.borrow().as_ref().map(|m| (m.anime.id, m.anime.episode_flags)) else { return };
        self.runner.send(match change {
            Change::Flag((flag, mask)) => AnimeCmd::SetFlagBits { anime_id, flag, mask },
            Change::Sort(mode) => AnimeCmd::SortBy { anime_id, mode },
            Change::SetDefault { apply_all } => return self.save_default(word, apply_all),
        });
    }

    /// "Set as default": stores the flags of this entry as the library default, and with the checkbox writes them to
    /// every favorite on a detached task. The message shows once per save (T-39b-010).
    fn save_default(&self, word: i64, apply_all: bool) {
        set_episode_settings_default(&self.env.prefs, word);
        if apply_all && let Some(handle) = &self.env.handle {
            let (repo, prefs) = (AnimeRepo::new(Arc::clone(&self.env.db)), self.env.prefs.clone());
            handle.spawn(async move {
                if let Err(e) = anime_defaults_to_library_detached(repo, prefs).await {
                    tracing::warn!("could not apply the default episode settings to the library: {e}");
                }
            });
        }
        if let Some(message) = default_saved_message(true) {
            self.present(message, None);
            self.render_snack();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub;
    impl runner::AnimeBackend for Stub {
        fn source_info(&self, _: i64) -> crate::entry_core::detail::SourceInfo {
            crate::entry_core::detail::SourceInfo { name: "Stub".into(), stub: true }
        }
        fn fetch(&self, _: i64, _: super::super::entry_state::RefreshPlan, _: super::super::refresh::Trigger) -> runner::AnimeFetch {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    fn env_of(store: &AnimeStore) -> Env {
        Env { prefs: PrefStore::in_memory(), backend: Arc::new(Stub), db: store.db(), handle: None }
    }

    fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut out = vec![root.clone()];
        let mut child = root.first_child();
        while let Some(w) = child {
            out.extend(descendants(&w));
            child = w.next_sibling();
        }
        out
    }

    fn pump(ctx: &glib::MainContext, done: impl Fn() -> bool) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            while ctx.iteration(false) {}
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        done()
    }

    /// One display test (GTK is bound to the thread that starts it): Loading, then the page with its rows and a
    /// stub warning, the single column at 360 px with no horizontal minimum above 360, the two panes from 900 px with
    /// the info block moved to the left, and Missing with a Back button.
    #[test]
    #[ignore = "needs a display; run alone: cargo test -p kurobayomi anime_route_smoke -- --ignored"]
    fn anime_route_smoke() {
        if gtk::init().is_err() {
            eprintln!("skipped: no display for the anime route smoke test");
            return;
        }
        let ctx = glib::MainContext::default();
        let _owner = ctx.acquire().expect("main context");
        let rt = kuro_core::runtime::build().expect("runtime");
        let (_dir, store, id) = super::super::store::fixture::with_episodes(true, true, 3, 1);
        let spawn = |anime_id| {
            let runner = Rc::new(runner::spawn(rt.handle(), store.clone(), kuro_prefs::store::PrefStore::in_memory(), Arc::new(Stub), AnimeLoadContext::default()));
            let page = assemble(Rc::clone(&runner), true, env_of(&store));
            runner.send(AnimeCmd::Load { anime_id });
            (page, runner)
        };
        let stack_of = |page: &GBox| descendants(page.upcast_ref()).into_iter().find_map(|w| w.downcast::<gtk::Stack>().ok()).expect("the page has a stack");

        let (page, _runner) = spawn(id);
        let stack = stack_of(&page);
        assert_eq!(stack.visible_child_name().as_deref(), Some("loading"));
        assert!(pump(&ctx, || stack.visible_child_name().as_deref() == Some("ready")), "the page becomes ready");
        let all = descendants(page.upcast_ref());
        let list = all.iter().find_map(|w| w.clone().downcast::<gtk::ListView>().ok()).expect("the list view");
        // at 360 px the page is one column: the info block and the list head are the first two rows of the list
        let window = gtk::Window::builder().default_width(360).default_height(700).child(&page).build();
        window.present();
        assert!(pump(&ctx, || list.model().is_some_and(|m| m.n_items() == 5)), "two leading rows and three episodes");
        for _ in 0..20 {
            while ctx.iteration(false) {}
            std::thread::sleep(Duration::from_millis(10));
        }
        let buttons: Vec<Button> = descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<Button>().ok()).collect();
        let labels: Vec<String> = descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<Label>().ok()).map(|l| l.label().to_string()).collect();
        assert!(labels.iter().any(|t| t == "In library"), "the library button of a favorite");
        assert!(buttons.iter().any(|b| b.tooltip_text().as_deref() == Some("Refresh") && b.is_sensitive()), "the refresh button");
        let fab = buttons.iter().find(|b| b.has_css_class("pill")).expect("the FAB");
        assert!(fab.is_visible() && fab.is_sensitive(), "an unseen episode exists");
        let (min, _, _, _) = page.measure(gtk::Orientation::Horizontal, -1);
        assert!(min <= 360, "no horizontal minimum above 360 px, got {min}");
        let left = descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<gtk::ScrolledWindow>().ok()).find(|w| w.width_request() == LEFT_WIDTH).expect("the left pane");
        assert!(!left.is_visible(), "no left pane in one column");
        // from 900 px the info block moves to the left pane and the list keeps only its head
        window.set_default_size(1000, 700);
        assert!(pump(&ctx, || list.model().is_some_and(|m| m.n_items() == 4)), "one leading row and three episodes");
        assert!(left.is_visible() && left.first_child().is_some(), "the left pane shows the info block");
        window.set_child(None::<&gtk::Widget>);
        window.close();

        // 3000 episodes: the list view builds only the rows on screen, so the page opens without a stall
        let (_big_dir, big_store, big_id) = super::super::store::fixture::with_episodes(true, true, 3000, 10);
        let big_runner = Rc::new(runner::spawn(rt.handle(), big_store.clone(), kuro_prefs::store::PrefStore::in_memory(), Arc::new(Stub), AnimeLoadContext::default()));
        let big = assemble(Rc::clone(&big_runner), true, env_of(&big_store));
        let big_window = gtk::Window::builder().default_width(1000).default_height(700).child(&big).build();
        big_window.present();
        big_runner.send(AnimeCmd::Load { anime_id: big_id });
        let started = std::time::Instant::now();
        let big_list = descendants(big.upcast_ref()).into_iter().find_map(|w| w.downcast::<gtk::ListView>().ok()).expect("the list view");
        assert!(pump(&ctx, || big_list.model().is_some_and(|m| m.n_items() == 3001)), "3000 rows and the list head");
        assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
        for _ in 0..50 {
            while ctx.iteration(false) {}
            std::thread::sleep(Duration::from_millis(10));
        }
        // GTK prepares a few off-screen rows ahead; what matters is that 3000 rows do not make 3000 widgets
        let built = descendants(big_list.upcast_ref()).into_iter().filter(|w| w.type_().name() == "GtkListItemWidget").collect::<Vec<_>>();
        assert!(built.len() < 400, "only a window of rows is built, got {}", built.len());
        assert!(built.iter().filter(|w| w.is_mapped()).count() < 40, "only the visible rows are on screen");
        let newest = descendants(big_list.upcast_ref()).into_iter().filter_map(|w| w.downcast::<Label>().ok()).any(|l| l.label() == "Episode 3000" && l.is_mapped());
        assert!(newest, "the first row shows the newest episode");
        big_window.set_child(None::<&gtk::Widget>);
        big_window.close();

        let missing_id = 4242;
        let (missing, _r) = spawn(missing_id);
        let stack = stack_of(&missing);
        assert!(pump(&ctx, || stack.visible_child_name().as_deref() == Some("missing")), "a missing row ends in the missing state");
        let has_back = descendants(missing.upcast_ref()).iter().filter_map(|w| w.clone().downcast::<Button>().ok()).any(|b| b.label().as_deref() == Some("Back"));
        assert!(has_back, "the missing state offers Back");
    }
}
