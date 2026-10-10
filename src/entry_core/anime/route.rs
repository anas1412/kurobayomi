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
use kuro_data::anime::AnimeRelationGroup;
use kuro_db::handle::Db;
use kuro_domain::download::prefs::DownloadPrefs;
use kuro_db::schema_anime::FetchType;
use kuro_domain::entries::anime::AnimeExt;
use kuro_domain::entries::defaults::{set_episode_settings_default, set_season_settings_default};
use kuro_domain::entries::season_display::SeasonDisplayMode;
use kuro_domain::kind::EntryKind;
use kuro_downloads::anime::layout::AnimeLayout;
use kuro_i18n::{MsgKey, tr, tr_plural};
use kuro_prefs::library::EpisodeSwipeAction;
use kuro_prefs::source::SourcePrefs;
use kuro_prefs::store::PrefStore;

use super::actions::{OpenPlan, continue_plan, tap_plan};
use super::airing::{Airing, format_airing, visible as airing_visible};
use super::bottom::{BottomAction, MenuContext, Selected, bottom_actions};
use super::download_backend::{AnimeDownloads, installed_queue};
use super::dispatch::{bottom_cmds, swipe_cmds};
use super::details_error::{DetailsError, required};
use super::downloads::{DownloadMenu, NEXT_COUNTS, menu_visible};
use super::duplicate::Choice;
use super::images::ImageKind;
use super::images_view;
use super::season_grid::{Layout, SeasonGrid};
use super::season_settings_view::SeasonSettingsWindow;
use super::season_tile::{Overlays, SeasonItem, continue_episode, season_facts, season_item};
use super::set_default::{self, Scope};
use super::swipe_look::{self, DownloadState, EpisodeState};
use super::selection::Click;
use super::info::header_model;
use super::layout::{bottom_bar_width, fab_expanded, two_pane};
use super::model::{AnimeEntryModel, AnimeLoadContext};
use super::refresh::refresh_snack;
use super::remote::{NoSources, RemoteAnimeBackend};
use super::row_view::{EpisodeList, EpisodeSwipe};
use super::runner::{self, AnimeBackend, AnimeCmd, AnimeEvent, AnimeRunner};
use super::settings_view::{Change, SettingsWindow};
use super::store::AnimeStore;
use super::{duplicate_view, info_view, related_view};
use crate::data;
use crate::entry_core::banner_view::{self, BannerView};
use crate::entry_core::category_dialog_view;
use crate::entry_core::description_view;
use crate::entry_core::dialog::CategoryDialog;
use crate::entry_core::handoff::{self, MigrateRequest, WebViewRequest};
use crate::entry_core::header::{HeaderModel, SearchRequest};
use crate::entry_core::action_row::{TrackingClick, interval_click_opens_dialog, next_update, tracking_click};
use crate::entry_core::action_row_view::{ActionRow, RowState};
use crate::entry_core::fetch_interval_dialog;
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

/// The page for the `anime_entry_details` screen id. Without a target there is no row to show, so it is the page of a
/// row that does not exist: the "not found" state with a Back button.
pub fn build() -> GBox {
    page_for(TARGET.with(Cell::take).unwrap_or(NO_ENTRY))
}

/// An id no row has (ids start at 1): `open(NO_ENTRY)` shows the "not found" page, for a caller that has no row to name.
pub const NO_ENTRY: i64 = 0;

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
    let root = crate::shell::host::Shell::get().map(|shell| shell.downloads().root().to_path_buf());
    let downloads = AnimeDownloads::new(AnimeLayout::new(root), installed_queue());
    let backend: Arc<dyn AnimeBackend> =
        Arc::new(RemoteAnimeBackend::new(Arc::clone(&db), &prefs, Arc::new(NoSources), handle.clone()).with_downloads(downloads));
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
    action_row: ActionRow,
    refresh_button: Button,
    download_menu: gtk::MenuButton,
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
    season_settings: RefCell<Option<Rc<SeasonSettingsWindow>>>,
    seasons: SeasonGrid,
    /// What the season grid shows: its cells, columns, whether the size is fixed, and the width bucket.
    seasons_shown: RefCell<Option<(Vec<SeasonItem>, Layout)>>,
    two_pane: Cell<Option<bool>>,
    width: Cell<i32>,
    built: RefCell<Option<(HeaderModel, Description, bool)>>,
    expansion: Rc<Cell<Expansion>>,
    /// The related groups the runner published, the box they are drawn in (under the description) and what it shows.
    related: RefCell<Vec<AnimeRelationGroup>>,
    related_host: GBox,
    related_shown: RefCell<Option<(Vec<AnimeRelationGroup>, kuro_prefs::library::LibraryDisplayMode)>>,
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
    let weak_view: Rc<RefCell<Weak<View>>> = Rc::default();
    // a `Seasons` entry lists its seasons in a grid under the head; the grid is hidden for an episode list. A tile's
    // continue button plays the next unseen episode of that season.
    let seasons = SeasonGrid::new(
        Rc::new(open),
        Some(Rc::new({
            let weak = Rc::clone(&weak_view);
            move |season_id: i64| {
                if let Some(view) = weak.borrow().upgrade() {
                    view.continue_season(season_id);
                }
            }
        })),
    );
    let list_head = GBox::new(O::Vertical, 4);
    list_head.append(&header_button);
    list_head.append(&spinner);
    list_head.append(&empty);
    list_head.append(&seasons.widget);

    // the action row (T-39a-026): library, interval (the dialog of T-38a-028), tracking and WebView. WebView shows
    // only for an HTTP source (`RowState::webview`).
    let action_row = ActionRow::new(
        Rc::new({
            let weak = Rc::clone(&weak_view);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.open_interval();
                }
            }
        }),
        Some(Rc::new({
            let weak = Rc::clone(&weak_view);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.tracking_pressed();
                }
            }
        })),
        Some(Rc::new({
            let weak = Rc::clone(&weak_view);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.webview_pressed();
                }
            }
        })),
    );
    let library = action_row.library.clone();
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
        {
            let weak = Rc::clone(&weak_view);
            Rc::new(move |id: i64| {
                if let Some(view) = weak.borrow().upgrade() {
                    view.summary_pressed(id);
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
            look: {
                let weak = Rc::clone(&weak_view);
                Rc::new(move |id: i64, action: EpisodeSwipeAction| {
                    let state = weak.borrow().upgrade().and_then(|view| view.episode_state(id));
                    state.and_then(|state| swipe_look::look(action, state))
                })
            },
            menu: {
                let weak = Rc::clone(&weak_view);
                Rc::new(move |id: i64| {
                    let view = weak.borrow().upgrade();
                    view.and_then(|view| view.episode_state(id).map(|state| swipe_look::menu(state, view.env.backend.has_download_handler()))).unwrap_or_default()
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
    // the Download menu: shown for an entry that is not local, when the app can queue downloads
    let download_menu = gtk::MenuButton::builder().icon_name("folder-download-symbolic").tooltip_text(tr(MsgKey::action_download)).visible(false).build();
    let toolbar: Vec<gtk::Widget> = vec![download_menu.clone().upcast(), refresh_button.clone().upcast(), filter.clone().upcast()];
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
        action_row: action_row.clone(),
        refresh_button: refresh_button.clone(),
        download_menu: download_menu.clone(),
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
        season_settings: RefCell::new(None),
        seasons,
        seasons_shown: RefCell::new(None),
        two_pane: Cell::new(None),
        width: Cell::new(FALLBACK_WIDTH),
        built: RefCell::new(None),
        expansion: Rc::new(Cell::new(Expansion::new(false, false))),
        related: RefCell::new(Vec::new()),
        related_host: GBox::new(O::Vertical, 0),
        related_shown: RefCell::new(None),
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
    library.connect(
        {
            let weak = weak.clone();
            move || {
                if let Some(view) = weak.upgrade() {
                    view.toggle_library();
                }
            }
        },
        {
            let weak = weak.clone();
            move || {
                if let Some(view) = weak.upgrade() {
                    view.open_categories();
                }
            }
        },
    );
    download_menu.set_create_popup_func({
        let weak = weak.clone();
        move |button| {
            if let Some(view) = weak.upgrade() {
                button.set_popover(Some(&view.download_popover()));
            }
        }
    });
    header_button.connect_clicked(on_view(View::open_settings));
    cancel.connect_clicked(on_view(|v| v.edit_selection(|m, _, _| m.selection.clear())));
    select_all.connect_clicked(on_view(|v| v.edit_selection(|m, _, full| m.selection.select_all(full))));
    invert.connect_clicked(on_view(|v| v.edit_selection(|m, _, full| m.selection.invert(full))));
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
            // F5 refreshes like the header button (T-39a-038); a selection disables it
            gtk::gdk::Key::F5 if !selecting => {
                view.refresh();
                glib::Propagation::Stop
            }
            gtk::gdk::Key::a | gtk::gdk::Key::A if ctrl && selecting => {
                view.edit_selection(|m, _, full| m.selection.select_all(full));
                glib::Propagation::Stop
            }
            gtk::gdk::Key::i | gtk::gdk::Key::I if ctrl && selecting => {
                view.edit_selection(|m, _, full| m.selection.invert(full));
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
                    if let Some(settings) = self.season_settings.borrow().as_ref() {
                        settings.update(m.anime.season_flags, m.downloaded_only);
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
            AnimeEvent::Downloads(on_disk) => {
                if let Some(m) = self.model.borrow_mut().as_mut() {
                    m.downloaded = on_disk;
                }
            }
            AnimeEvent::Categories { all, linked } => self.show_category_dialog(&all, &linked),
            AnimeEvent::Added => {}
            AnimeEvent::Removed(outcome) => {
                // downloads exist: ask whether they go too. Dismissing the prompt keeps them.
                let anime_id = self.model.borrow().as_ref().map(|m| m.anime.id);
                if let (true, Some(anime_id)) = (outcome.ask_delete_downloads, anime_id) {
                    self.present(UserMessage::DeleteDownloadsForAnime, Some(AnimeCmd::DeleteDownloads { anime_id }));
                }
            }
            AnimeEvent::Failed(message) => self.present(UserMessage::Plain(message), None),
            AnimeEvent::Related(groups) => *self.related.borrow_mut() = groups,
            AnimeEvent::SeasonFacts(extras) => {
                if let Some(m) = self.model.borrow_mut().as_mut() {
                    m.set_season_extras(extras);
                }
            }
            AnimeEvent::DefaultsApplied { scope, updated } => self.present(set_default::applied_message(scope, updated), None),
            AnimeEvent::Duplicate { existing } => self.show_duplicate(&existing),
        }
    }

    /// Switches between the two layouts. It moves the info block between the left pane and the first row of the list.
    fn apply_width(self: &Rc<Self>, width: i32) {
        let resized = self.width.replace(width) != width;
        let two = two_pane(width);
        if self.two_pane.get() == Some(two) {
            // the layout is the same, but the automatic columns of the season grid follow the width (T-39a-034)
            if resized && let Some(m) = self.model.borrow().as_ref() {
                self.show_seasons(m, m.anime.fetch_type == FetchType::Seasons);
            }
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
        let lines = m.lines();
        let seasons_entry = m.anime.fetch_type == FetchType::Seasons;
        let none_shown = if seasons_entry { m.seasons.is_empty() } else { lines.is_empty() };
        self.list.sync(lines);
        self.show_seasons(m, seasons_entry);
        let header = m.item_header();
        self.count.set_label(&header.count);
        self.missing.set_label(header.missing.as_deref().unwrap_or(""));
        self.missing.set_visible(header.missing.is_some());
        self.header_button.set_sensitive(header.enabled);
        self.spinner.set_spinning(m.flow.refreshing);
        self.spinner.set_visible(m.flow.refreshing);
        // one refresh runs at a time
        self.refresh_button.set_sensitive(!m.flow.refreshing && !m.selection.is_active());
        self.empty.set_visible(none_shown && !m.flow.refreshing);
        self.empty.set_label(&tr(if seasons_entry { MsgKey::no_seasons_error } else { MsgKey::no_episodes_error }));
        // the anime and manga statuses share the SAnime/SManga completed code (2)
        self.action_row.apply(&RowState {
            favorite: m.anime.favorite,
            next: next_update(
                Some(m.anime.next_update),
                chrono::Utc::now().timestamp_millis(),
                m.anime.status == kuro_domain::entries::manga::STATUS_COMPLETED,
            ),
            user_interval: m.anime.fetch_interval < 0,
            tracking_count: m.tracking_count as i64,
            has_tracking: true,
            webview: self.env.backend.is_http_source(m.anime.source),
        });
        self.download_menu.set_visible(menu_visible(m.is_local(), self.env.backend.has_download_handler()));
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
        self.render_related();
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
            .map(|e| Selected { bookmark: e.bookmark, fillermark: e.fillermark, seen: e.seen, last_second_seen: e.last_second_seen, downloaded: m.downloaded.contains(&e.id) })
            .collect();
        // Download and Delete show only when the app can queue downloads (`onDownloadEpisode != null`)
        let ctx = MenuContext { has_download_handler: self.env.backend.has_download_handler(), always_external: always_external(&self.env.prefs) };
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
        let remove = self.remove_after_seen();
        let dispatch = {
            let mut model = self.model.borrow_mut();
            let Some(m) = model.as_mut().filter(|m| m.accepts_dispatch()) else { return };
            let dispatch = bottom_cmds(m, action.effect(), remove);
            m.selection.clear();
            dispatch
        };
        for cmd in dispatch.cmds {
            self.runner.send(cmd);
        }
        if let Some((id, external)) = dispatch.open {
            self.open_episode(id, OpenPlan { external, torrent_server: self.torrent_server_needed() });
        }
        self.render();
    }

    /// `pref_remove_after_marked_as_read`: a mark seen deletes the downloads of the episodes it changed (T-39b-013).
    fn remove_after_seen(&self) -> bool {
        DownloadPrefs::new(&self.env.prefs).remove_after_marked_as_read.get()
    }

    /// A swipe on a row: the action of the side, written for that one episode. It works while selecting too (E-5).
    fn swipe_pressed(self: &Rc<Self>, id: i64, action: kuro_prefs::library::EpisodeSwipeAction) {
        let cmds = {
            let model = self.model.borrow();
            let Some(m) = model.as_ref().filter(|m| m.accepts_dispatch()) else { return };
            swipe_cmds(m, id, action, self.env.backend.has_download_handler(), self.remove_after_seen())
        };
        for cmd in cmds {
            self.runner.send(cmd);
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

    /// The interval button (T-39a-026): the dialog of a favorite. OK and Reset store the wheel through the runner.
    fn open_interval(self: &Rc<Self>) {
        let (anime_id, interval, expected, favorite) = {
            let model = self.model.borrow();
            let Some(m) = model.as_ref().filter(|m| m.accepts_dispatch()) else { return };
            let expected = (m.anime.status != kuro_domain::entries::manga::STATUS_COMPLETED).then_some(m.anime.next_update);
            (m.anime.id, m.anime.fetch_interval, expected, m.anime.favorite)
        };
        if !interval_click_opens_dialog(favorite) {
            return;
        }
        let runner = self.runner.clone();
        fetch_interval_dialog::open(interval, expected, move |wheel| {
            runner.send(AnimeCmd::SetFetchInterval { anime_id, wheel });
        });
    }

    /// The library button: adds the entry, or removes it from the library.
    fn toggle_library(self: &Rc<Self>) {
        let target = self.model.borrow().as_ref().filter(|m| m.accepts_dispatch()).map(|m| (m.anime.id, m.anime.favorite));
        match target {
            // a favorite leaves the library
            Some((anime_id, true)) => self.runner.send(AnimeCmd::ToggleFavorite { anime_id }),
            // an add checks for a same-named library entry first (T-39a-047)
            Some((anime_id, false)) => self.runner.send(AnimeCmd::AddToLibrary { anime_id, check_duplicates: true }),
            None => {}
        }
    }

    /// The duplicate window of an add that found a same-named library entry. A row closes it first and then acts: Show
    /// opens the entry that is there, Migrate starts the migrate flow from it, Add anyway adds without the check.
    fn show_duplicate(&self, existing: &[kuro_data::anime::Anime]) {
        let (Some(duplicate), Some(anime_id)) = (existing.first().cloned(), self.model.borrow().as_ref().map(|m| m.anime.id)) else { return };
        let runner = Rc::clone(&self.runner);
        duplicate_view::open(move |choice| match choice {
            Choice::ShowAnime => open(duplicate.id),
            Choice::Migrate => handoff::open_migrate(MigrateRequest { entry_id: duplicate.id, anime: true, title: duplicate.title.clone() }),
            Choice::AddAnyway => runner.send(AnimeCmd::AddToLibrary { anime_id, check_duplicates: false }),
        });
    }

    /// The tracking button: the tracking sheet when a tracker is logged in, the tracking settings when none is (E-25).
    fn tracking_pressed(&self) {
        match tracking_click(self.env.backend.has_logged_in_tracker()) {
            TrackingClick::Sheet => ui::open("tracking_sheet"),
            TrackingClick::Settings => ui::open("settings_tracking"),
        }
    }

    /// The WebView button (an HTTP source only): the entry's page in the WebView.
    fn webview_pressed(&self) {
        let request = self.model.borrow().as_ref().map(|m| WebViewRequest { url: m.anime.url.clone(), title: m.anime.title.clone() });
        if let Some(request) = request {
            handoff::open_webview(request);
        }
    }

    /// A long press on the library button of a favorite: reads the categories, and the answer opens the editor.
    fn open_categories(&self) {
        let anime_id = self.model.borrow().as_ref().filter(|m| m.accepts_dispatch()).map(|m| m.anime.id);
        if let Some(anime_id) = anime_id {
            self.runner.send(AnimeCmd::LoadCategories { anime_id });
        }
    }

    /// The category editor over the answer of `LoadCategories`. OK writes the checked ids; any other way out writes
    /// nothing.
    fn show_category_dialog(&self, all: &[(i64, String)], linked: &[i64]) {
        let Some(anime_id) = self.model.borrow().as_ref().map(|m| m.anime.id) else { return };
        let runner = Rc::clone(&self.runner);
        category_dialog_view::open(
            CategoryDialog::new(anime_id, false, all, linked),
            move |category_ids| runner.send(AnimeCmd::SetCategories { anime_id, category_ids }),
            || {},
        );
    }

    /// The width the season grid has: the page, or the right pane of the two-pane layout.
    fn grid_width(&self) -> i32 {
        let width = self.width.get();
        if self.two_pane.get() == Some(true) { (width - LEFT_WIDTH - 1).max(0) } else { width }
    }

    /// The season grid of a `Seasons` entry. Its minimum column count is the entry's size setting (`Layout::columns_for`); it
    /// is redrawn only when the cells, the columns or the width changed.
    fn show_seasons(&self, m: &AnimeEntryModel, seasons_entry: bool) {
        if !seasons_entry {
            self.seasons.widget.set_visible(false);
            *self.seasons_shown.borrow_mut() = None;
            return;
        }
        let size = m.anime.season_display_grid_size();
        let mode = m.anime.season_display_grid_mode();
        let width = self.grid_width();
        // the overlays are the parent entry's stored bits; the facts of a season are what the runner read for it
        let overlays = Overlays::of(&m.anime);
        let items: Vec<SeasonItem> = m
            .seasons
            .iter()
            .map(|s| {
                let local = s.anime.source == kuro_source::local::LOCAL_SOURCE_ID;
                season_item(s, overlays, &season_facts(s, m.season_extras.get(&s.id()), local))
            })
            .collect();
        let layout = Layout {
            mode,
            number_mode: m.anime.season_display_mode() == kuro_domain::entries::anime::season_flags::DISPLAY_MODE_NUMBER,
            // the page can continue watching: a tile shows the button when its season has an unseen episode
            has_continue: true,
            columns: Layout::columns_for(width, i64::from(size), mode),
            fixed: size > 0 || mode == SeasonDisplayMode::List,
            width,
        };
        let key = (items, layout);
        if self.seasons_shown.borrow().as_ref() == Some(&key) {
            return;
        }
        self.seasons.set(&key.0, &key.1);
        *self.seasons_shown.borrow_mut() = Some(key);
    }

    /// The Download menu: the next 1, 5, 10 or 25 unseen episodes, or every unseen one.
    fn download_popover(self: &Rc<Self>) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let col = GBox::new(O::Vertical, 2);
        ui::margins(&col, 6);
        let mut entries: Vec<(String, DownloadMenu)> =
            NEXT_COUNTS.iter().map(|&n| (tr_plural(MsgKey::download_amount_anime, n as i64, &[]), DownloadMenu::Next(n))).collect();
        entries.push((tr(MsgKey::download_unseen), DownloadMenu::Unseen));
        for (label, menu) in entries {
            let button = Button::builder().css_classes(["flat"]).build();
            button.set_child(Some(&Label::builder().label(&label).xalign(0.0).build()));
            let (weak, popover) = (Rc::downgrade(self), popover.clone());
            button.connect_clicked(move |_| {
                popover.popdown();
                if let Some(view) = weak.upgrade() {
                    view.run_download(menu);
                }
            });
            col.append(&button);
        }
        popover.set_child(Some(&col));
        popover
    }

    fn run_download(&self, menu: DownloadMenu) {
        let (anime_id, ids) = {
            let model = self.model.borrow();
            let Some(m) = model.as_ref().filter(|m| m.accepts_dispatch()) else { return };
            (m.anime.id, m.download_targets(menu))
        };
        if !ids.is_empty() {
            self.runner.send(AnimeCmd::Download { anime_id, ids, start_now: false });
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
        let handlers = self.handlers();
        self.top.append(&info_view::build(&key.0, two, column, &handlers));
        let text = description_view::build(&key.1, Rc::clone(&self.expansion), Some((&key.0.genres, &handlers)));
        text.set_margin_start(16);
        text.set_margin_end(16);
        self.top.append(&text);
        // the related rows come with the description (`ExpandableAnimeDescription`)
        detach(&self.related_host);
        self.top.append(&self.related_host);
        detach(&self.action_row.root);
        self.top.append(&self.action_row.root);
        *self.built.borrow_mut() = Some(key);
    }

    fn handlers(self: &Rc<Self>) -> Handlers {
        let weak = Rc::downgrade(self);
        Handlers {
            // the query goes to the opened screen through `handoff::take_search`
            on_search: Rc::new(|request: SearchRequest| handoff::open_search(request)),
            on_copy: Rc::new(|text: String| {
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&text);
                }
            }),
            // a click on the cover opens the images window on the cover page
            on_cover: Rc::new(move || {
                if let Some(view) = weak.upgrade() {
                    view.open_images();
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
            let view = m.processed_ids();
            if long {
                m.selection.long_press(&view, id);
                None
            } else {
                match m.selection.click(&view, id) {
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

    /// A tap on a summary toggles it, unless an episode is selected (T-39a-012). The row is drawn again with its new limit.
    fn summary_pressed(self: &Rc<Self>, id: i64) {
        let changed = {
            let mut model = self.model.borrow_mut();
            let Some(m) = model.as_mut() else { return };
            let selecting = m.selection.is_active();
            m.summaries.click(id, selecting)
        };
        if changed {
            self.render();
        }
    }

    fn is_torrent_source(&self) -> bool {
        let source = self.model.borrow().as_ref().map(|m| m.anime.source);
        source.is_some_and(|s| self.env.backend.is_torrent_source(s))
    }

    /// Opens an episode as the plan says. A missing episode is a typed error shown in the snackbar, not a panic.
    fn open_episode(&self, id: i64, plan: OpenPlan) {
        let found = {
            let model = self.model.borrow();
            required(model.as_ref().and_then(|m| m.episodes.iter().find(|e| e.id == id).map(|e| (m.anime.id, m.anime.title.clone(), e.name.clone()))), || {
                DetailsError::EpisodeNotFound(id)
            })
        };
        match found {
            Ok((anime_id, title, name)) => self.play(anime_id, id, &title, &name, plan),
            Err(error) => self.show_error(error),
        }
    }

    /// Starts the torrent server when the plan says so, then plays in the external player, or in the internal one when
    /// the external one cannot start.
    fn play(&self, anime_id: i64, episode_id: i64, title: &str, name: &str, plan: OpenPlan) {
        if plan.torrent_server {
            self.env.backend.start_torrent_server();
        }
        if plan.external && self.env.backend.play_external(anime_id, episode_id) {
            return;
        }
        crate::screens::open_media(true, title, name);
    }

    /// Logs a typed error and shows its text in the snackbar.
    fn show_error(&self, error: DetailsError) {
        self.present(UserMessage::Plain(error.record().user_message()), None);
        self.render_snack();
    }

    /// The continue button of a season tile: plays that season's next unseen episode, as the FAB does for an entry.
    fn continue_season(&self, season_id: i64) {
        let store = AnimeStore::new(Arc::clone(&self.env.db));
        let played = (|| -> Result<(), DetailsError> {
            let season = required(store.anime(season_id)?, || DetailsError::AnimeNotFound(season_id))?;
            let episodes = store.episodes(season_id)?;
            let episode = required(continue_episode(&episodes), || DetailsError::EpisodeNotFound(season_id))?;
            let torrent = self.env.backend.is_torrent_source(season.source);
            let plan = continue_plan(always_external(&self.env.prefs), torrent_enabled(&self.env.prefs), torrent);
            self.play(season_id, episode.id, &season.title, &episode.name, plan);
            Ok(())
        })();
        if let Err(error) = played {
            self.show_error(error);
        }
    }

    /// The state of one episode the swipe strip and the row menu read.
    fn episode_state(&self, id: i64) -> Option<EpisodeState> {
        let model = self.model.borrow();
        let m = model.as_ref()?;
        let e = m.episodes.iter().find(|e| e.id == id)?;
        let download = if m.downloaded.contains(&id) { DownloadState::Downloaded } else { DownloadState::NotDownloaded };
        Some(EpisodeState { seen: e.seen, bookmark: e.bookmark, fillermark: e.fillermark, download })
    }

    /// The images window, on the cover page. An edit or a delete there moves the image's last-modified through the runner.
    fn open_images(&self) {
        let found = {
            let model = self.model.borrow();
            required(model.as_ref().map(|m| m.anime.clone()), || DetailsError::AnimeNotFound(0))
        };
        match (found, self.env.handle.clone()) {
            (Ok(anime), Some(handle)) => {
                let (runner, anime_id) = (Rc::clone(&self.runner), anime.id);
                let bump = Rc::new(move |kind: ImageKind| runner.send(AnimeCmd::BumpImage { anime_id, kind }));
                images_view::open(anime, ImageKind::Cover, images_view::Env { handle, bump });
            }
            (Err(error), _) => self.show_error(error),
            (Ok(_), None) => {}
        }
    }

    /// Draws the related groups under the description when they or the display mode changed.
    fn render_related(&self) {
        let mode = SourcePrefs::new(self.env.prefs.clone()).source_display_mode.get();
        let key = (self.related.borrow().clone(), mode);
        if self.related_shown.borrow().as_ref() == Some(&key) {
            return;
        }
        while let Some(child) = self.related_host.first_child() {
            self.related_host.remove(&child);
        }
        if let Some(section) = related_view::section(&key.0, mode, Rc::new(open)) {
            self.related_host.append(&section);
        }
        *self.related_shown.borrow_mut() = Some(key);
    }

    /// The FAB: opens the first unseen episode, with the setting as it is and no inversion (17.2).
    fn open_next(self: &Rc<Self>) {
        let next = self.model.borrow().as_ref().and_then(AnimeEntryModel::next_unseen);
        if let Some(id) = next {
            let plan = continue_plan(always_external(&self.env.prefs), torrent_enabled(&self.env.prefs), self.is_torrent_source());
            self.open_episode(id, plan);
        }
    }

    /// Edits the selection, then redraws. The closure gets the processed ids (the view, for toggles) and every
    /// episode id (for select all and invert, which act on the full list).
    fn edit_selection(self: &Rc<Self>, edit: impl FnOnce(&mut AnimeEntryModel, &[i64], &[i64])) {
        {
            let mut model = self.model.borrow_mut();
            let Some(m) = model.as_mut().filter(|m| m.accepts_dispatch()) else { return };
            let view = m.processed_ids();
            let full: Vec<i64> = m.episodes.iter().map(|e| e.id).collect();
            edit(m, &view, &full);
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
        if self.model.borrow().as_ref().is_some_and(|m| m.anime.fetch_type == FetchType::Seasons) {
            return self.open_season_settings();
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
    fn apply_change(self: &Rc<Self>, change: Change) {
        let Some((anime_id, word)) = self.model.borrow().as_ref().map(|m| (m.anime.id, m.anime.episode_flags)) else { return };
        // an entry outside the library keeps its flags for the session: the model takes the change as well as the row
        if let Some(m) = self.model.borrow_mut().as_mut() {
            match change {
                Change::Flag((flag, mask)) => m.note_flag_write(flag, mask),
                Change::Sort(mode) => m.note_sort(mode),
                Change::SetDefault { .. } => {}
            }
        }
        self.runner.send(match change {
            Change::Flag((flag, mask)) => AnimeCmd::SetFlagBits { anime_id, flag, mask },
            Change::Sort(mode) => AnimeCmd::SortBy { anime_id, mode },
            Change::SetDefault { apply_all } => return self.save_default(Scope::Episode, word, apply_all),
        });
        self.render();
    }

    /// Opens the season settings window of a `Seasons` entry, from the header or the filter button.
    fn open_season_settings(self: &Rc<Self>) {
        if let Some(open) = self.season_settings.borrow().as_ref() {
            open.present();
            return;
        }
        let Some((word, downloaded_only, favorite)) = self
            .model
            .borrow()
            .as_ref()
            .filter(|m| m.accepts_dispatch())
            .map(|m| (m.anime.season_flags, m.downloaded_only, m.anime.favorite))
        else {
            return;
        };
        let weak = Rc::downgrade(self);
        let window = SeasonSettingsWindow::new(
            word,
            downloaded_only,
            favorite,
            Rc::new({
                let weak = weak.clone();
                move |change| {
                    if let Some(view) = weak.upgrade() {
                        view.apply_season_change(change);
                    }
                }
            }),
        );
        window.on_close(move || {
            if let Some(view) = weak.upgrade() {
                view.season_settings.borrow_mut().take();
            }
        });
        window.present();
        *self.season_settings.borrow_mut() = Some(window);
    }

    /// A click in the season settings window: a masked write of the season flag word, or the sort. Both go through the
    /// runner (the masked-write path); "Set as default" stores the word of this entry as the season default.
    fn apply_season_change(self: &Rc<Self>, change: Change) {
        let Some((anime_id, word)) = self.model.borrow().as_ref().map(|m| (m.anime.id, m.anime.season_flags)) else { return };
        if let Some(m) = self.model.borrow_mut().as_mut() {
            match change {
                Change::Flag((flag, mask)) => m.note_season_write(flag, mask),
                Change::Sort(code) => m.note_season_sort(code),
                Change::SetDefault { .. } => {}
            }
        }
        self.runner.send(match change {
            Change::Flag((flag, mask)) => AnimeCmd::SetSeasonFlagBits { anime_id, flag, mask },
            Change::Sort(code) => AnimeCmd::SeasonSortBy { anime_id, code },
            Change::SetDefault { apply_all } => return self.save_default(Scope::Season, word, apply_all),
        });
        self.render();
    }

    /// "Set as default": stores the flag word of this entry as the default of `scope`, and with the checkbox applies
    /// that default to every favorite through the runner. Only that scope is written: an episode save never touches the
    /// season flags of the library, and the reverse. With the checkbox the message waits for the run, to say how many
    /// entries took the settings (E-DEF-3); without it the message shows once per save (T-39b-010).
    fn save_default(self: &Rc<Self>, scope: Scope, word: i64, apply_all: bool) {
        match scope {
            Scope::Episode => set_episode_settings_default(&self.env.prefs, word),
            Scope::Season => set_season_settings_default(&self.env.prefs, word),
        };
        if apply_all {
            self.runner.send(AnimeCmd::ApplyDefaultsToLibrary { scope });
            return;
        }
        self.present(set_default::saved_message(scope), None);
        self.render_snack();
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
        assert!(labels.iter().any(|t| t == "Tracking"), "the tracking button has its callback");
        let webview = descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<Label>().ok()).find(|l| l.label() == "WebView").and_then(|l| l.ancestor(Button::static_type()));
        assert!(webview.is_some_and(|b| !b.is_visible()), "WebView shows for an HTTP source only");
        let download = descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<gtk::MenuButton>().ok()).find(|b| b.tooltip_text().as_deref() == Some("Download")).expect("the Download menu exists");
        assert!(!download.is_visible(), "no download handler, no Download menu");
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
