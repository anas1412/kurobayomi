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
use kuro_data::anime_relation::AnimeRelationGroup;
use kuro_data::entry::SeasonEntry;
use kuro_db::handle::Db;
use kuro_db::codec::FetchType;
use kuro_domain::entries::defaults::{set_item_settings_default, set_season_settings_default};
use kuro_domain::entries::flags::EntryFlags;
use kuro_domain::entries::season_display::SeasonDisplayMode;
use kuro_platform::kind::Kind;
use kuro_downloads::DownloadState;
use kuro_i18n::{MsgKey, tr, tr_plural};
use kuro_prefs::store::PrefStore;

use crate::entry_core::constants::{FALLBACK_WIDTH, LEFT_PANE_WIDTH};

use super::actions::{OpenPlan, continue_plan, tap_plan};
use super::airing::{Airing, format_airing, visible as airing_visible};
use super::bottom::{BottomAction, MenuContext, Selected, bottom_actions};
use super::download_backend::AnimeDownloads;
use super::dispatch::{bottom_cmds, menu_cmds};
use super::details_error::{DetailsError, required};
use super::downloads::{DownloadMenu, NEXT_COUNTS, menu_visible};
use super::intro_dialog;
use super::torrent_server;
use super::overflow::{self, Entry as OverflowEntry, Item as OverflowItem, LINK_COPIED, can_launch};
use super::viewer;
use super::dialog_state::{AnimeDialog, DialogSlot};
use super::duplicate::Choice;
use super::season_grid::{Layout, SeasonGrid};
use super::season_pipeline::process_seasons;
use super::season_tile::{SeasonFacts, SeasonItem, continue_episode, season_facts};
use super::set_default::{self, Scope};
use super::selection::Click;
use super::info::header_model;
use super::layout::{bottom_bar_width, two_pane};
use super::model::{AnimeEntryModel, AnimeLoadContext};
use super::refresh::refresh_snack;
use super::remote::{LiveSources, RemoteAnimeBackend};
use super::runner::{self, AnimeBackend, AnimeCmd, AnimeEvent, AnimeRunner};
use super::store::AnimeStore;
use super::{duplicate_view, related_view};
use crate::data;
use crate::entry_core::dialog::CategoryDialog;
use crate::entry_core::handoff::{self, MigrateRequest, WebViewRequest};
use crate::entry_core::header::SearchRequest;
use crate::entry_core::action_row::interval_click_opens_dialog;
use crate::entry_core::action_row_view::{ActionRow, RowState};
use crate::entry_core::fetch_interval_dialog;
use crate::entry_core::header_view::{Handlers, LibraryButton};
use crate::entry_core::message::{Notice, UserMessage, present};
use crate::entry_core::info_block::InfoBlock;
use crate::entry_core::info_pane::TwoPane;
use crate::entry_core::list_model::RowAction;
use crate::entry_core::list_options::{self, Spec};
use crate::entry_core::list_view::{self, Callbacks, ItemList, Press};
use crate::entry_core::text::{Expansion, description};
use crate::entry_core::page_shell::{self, detach, plain_row};
use crate::ui;
use crate::widgets::download_button::Request;
use crate::widgets::view_options::{Pick, ViewOptions};

/// The skip-intro length a manual skip starts from (the Default intro length preference is cut, so this is fixed).
const DEFAULT_INTRO_LENGTH: i32 = 85;


thread_local! {
    static TARGET: Cell<Option<i64>> = const { Cell::new(None) };
}

/// Names the anime the next `build()` shows. `ui::open("anime_entry_details")` carries no argument, so a caller sets
/// the target first.
pub fn target(anime_id: i64) {
    TARGET.with(|t| t.set(Some(anime_id)));
}

/// The anime sources this build runs: the builtin ones (none) and the recipe sources (`docs/source-recipes.md`).
pub(crate) fn live_anime_sources() -> Vec<Arc<dyn kuro_source::anime::AnimeSource>> {
    let mut sources = kuro_source::anime::registry::builtin_anime_sources();
    sources.extend(crate::recipe_sources::anime_sources());
    sources
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
    crate::entry_core::tracking::session::refresh_from_host();
    let app = data::app();
    let db = app.db.clone();
    let prefs = app.base.store().clone();
    let show_airing = crate::misc::diet_defaults::ENTRY_SHOW_AIRING_TIME;
    let ctx = AnimeLoadContext {
        from_source: false,
        downloaded_only: app.base.downloaded_only.get(),
        ..Default::default()
    };
    let handle = tokio::runtime::Handle::current();
    // the live sources of this build; an id it does not know reports that the source is not installed
    let downloads = AnimeDownloads::new();
    let backend: Arc<dyn AnimeBackend> =
        Arc::new(RemoteAnimeBackend::new(Arc::clone(&db), &prefs, Arc::new(LiveSources::new(live_anime_sources())), handle.clone()).with_downloads(downloads));
    let runner = Rc::new(runner::spawn(&handle, AnimeStore::new(Arc::clone(&db)), prefs.clone(), Arc::clone(&backend), ctx));
    runner.send(AnimeCmd::Load { anime_id });
    assemble(runner, show_airing, Env { prefs, backend, db })
}

/// What the page needs from the app besides its runner: the preference store, the backend the runner uses (for the
/// player and torrent choices that happen on the GTK thread) and the database.
struct Env {
    prefs: PrefStore,
    backend: Arc<dyn AnimeBackend>,
    db: Arc<Db>,
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
    library: LibraryButton,
    refresh_button: Button,
    download_menu: gtk::MenuButton,
    bottom: gtk::Revealer,
    bottom_row: GBox,
    stack: gtk::Stack,
    title: Option<Label>,
    header_bar: Option<gtk::HeaderBar>,
    toolbar: Vec<gtk::Widget>,
    list: Rc<ItemList>,
    scroller: gtk::ScrolledWindow,
    /// The info block (header, action row, description, related rows): the left pane, or the first row of the list.
    info: InfoBlock,
    /// The two-pane body: the left pane with the info block, and the list.
    panes: TwoPane,
    /// The item header, the refresh spinner and the empty line: the head of the list.
    list_head: GBox,
    count: Label,
    missing: Label,
    spinner: gtk::Spinner,
    empty: GBox,
    empty_label: Label,
    airing_row: GBox,
    airing_title: Label,
    airing_time: Label,
    fab: Button,
    fab_label: Label,
    scroll: Cell<(f64, bool)>,
    bar: gtk::Revealer,
    bar_count: Label,
    /// The list options menu of the top bar, the two lists it can describe, and which one it shows (`Some(true)`: the
    /// season grid of a `Seasons` entry).
    options: ViewOptions,
    episode_spec: Spec,
    season_spec: Spec,
    options_seasons: Cell<Option<bool>>,
    /// The one dialog slot (T-39b-030) and the window that shows it. `dialog_gen` names the request on screen, so the
    /// close of a window that a newer request replaced is ignored.
    dialogs: RefCell<DialogSlot>,
    dialog_window: RefCell<Option<gtk::Window>>,
    dialog_gen: Cell<u64>,
    seasons: SeasonGrid,
    /// What the season grid shows: its cells, columns, whether the size is fixed, and the width bucket.
    seasons_shown: RefCell<Option<(Vec<SeasonItem>, Layout)>>,
    two_pane: Cell<Option<bool>>,
    width: Cell<i32>,
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

/// The preference of the player the taps use (`pref_always_use_external_player`).
fn always_external(prefs: &PrefStore) -> bool {
    prefs.pref::<bool>("pref_always_use_external_player", false).get()
}

/// The torrent server preference (`pref_torrserver_enable`).
fn torrent_enabled(prefs: &PrefStore) -> bool {
    prefs.pref::<bool>("pref_torrserver_enable", false).get()
}

fn assemble(runner: Rc<AnimeRunner>, show_airing: bool, env: Env) -> GBox {
    let stack = page_shell::state_stack();

    // the list head: the item header row, the refresh spinner and the empty line
    let page_shell::ItemHeaderRow { row: header_row, count, missing } = page_shell::item_header_row();
    let spinner = gtk::Spinner::builder().halign(Align::Center).build();
    let weak_view: Rc<RefCell<Weak<View>>> = Rc::default();
    // nothing listed: the list says so, with Retry (a refresh that finds nothing raises no notification)
    let (empty, empty_label) = list_view::empty_list(&tr(MsgKey::no_episodes_error), {
        let weak = Rc::clone(&weak_view);
        move || {
            if let Some(view) = weak.borrow().upgrade() {
                view.refresh();
            }
        }
    });
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
    list_head.append(&header_row);
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
    let list = ItemList::new(Kind::Anime, Callbacks {
        on_row: {
            let weak = Rc::clone(&weak_view);
            Rc::new(move |id: i64, press: Press| {
                let view = weak.borrow().upgrade();
                if let Some(view) = view {
                    view.row_pressed(id, false, press.alt, press.mods);
                }
            })
        },
        on_menu: {
            let weak = Rc::clone(&weak_view);
            Rc::new(move |id: i64, action: RowAction| {
                let view = weak.borrow().upgrade();
                match (view, action) {
                    (Some(view), RowAction::Select) => view.row_pressed(id, true, false, crate::entry_core::selection::Mods::default()),
                    (Some(view), action) => view.menu_pressed(id, action),
                    (None, _) => {}
                }
            })
        },
        on_download: {
            let weak = Rc::clone(&weak_view);
            Rc::new(move |id: i64, request: Request| {
                if let Some(view) = weak.borrow().upgrade() {
                    view.download_pressed(id, request);
                }
            })
        },
        on_summary: {
            let weak = Rc::clone(&weak_view);
            Rc::new(move |id: i64| {
                if let Some(view) = weak.borrow().upgrade() {
                    view.summary_pressed(id);
                }
            })
        },
    });
    let scroller = gtk::ScrolledWindow::builder()
        .child(&list.widget)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .hexpand(true)
        .vexpand(true)
        .build();
    let panes = TwoPane::new(scroller.upcast_ref());
    let info = InfoBlock::new(action_row);
    // the related rows come with the description (`ExpandableAnimeDescription`); there are none until the runner has some
    let related_host = GBox::new(O::Vertical, 0);
    related_host.set_visible(false);
    info.root.append(&related_host);

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

    let (fab, fab_label) = page_shell::fab_button();
    fab.set_halign(Align::End);
    fab.set_valign(Align::End);
    fab.set_margin_end(16);
    fab.set_margin_bottom(16);
    fab.set_visible(false);

    let column = GBox::new(O::Vertical, 0);
    column.append(&airing_row);
    column.append(&panes.root);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&column));
    overlay.add_overlay(&fab);
    overlay.set_vexpand(true);

    // the action bar: cancel, the count, select all and invert
    let cancel = icon_button("window-close-symbolic", &tr(MsgKey::action_cancel));
    let bar_count = Label::builder().xalign(0.0).hexpand(true).build();
    let select_all = icon_button("edit-select-all-symbolic", &tr(MsgKey::action_select_all));
    let invert = icon_button("object-flip-horizontal-symbolic", &tr(MsgKey::action_select_inverse));
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

    let (episode_spec, season_spec) = (list_options::item_spec(Kind::Anime), list_options::season_spec());
    let options = ViewOptions::new(list_options::TOOLTIP, &episode_spec.sections());
    let refresh_button = icon_button("view-refresh-symbolic", &tr(MsgKey::action_webview_refresh));
    // the Download menu: shown for an entry that is not local, when the app can queue downloads
    let download_menu = crate::ui::quiet_menu(gtk::MenuButton::builder().icon_name("folder-download-symbolic").tooltip_text(tr(MsgKey::action_download)).visible(false).build());
    // the overflow menu: the items `overflow::items` gives for this entry and its source (T-39a-075)
    let overflow_menu = crate::ui::quiet_menu(gtk::MenuButton::builder().icon_name("view-more-symbolic").tooltip_text(tr(MsgKey::action_menu_overflow_description)).build());
    let toolbar: Vec<gtk::Widget> =
        vec![download_menu.clone().upcast(), refresh_button.clone().upcast(), options.widget.clone().upcast(), overflow_menu.clone().upcast()];
    let page = ui::page("", &stack, toolbar.clone(), false);
    ui::tag(&page, "anime_entry_details");
    let (header, title, back) = page_shell::header_with_back(&page);

    let view = Rc::new(View {
        model: RefCell::new(None),
        loaded: Cell::new(false),
        runner: Rc::clone(&runner),
        env,
        show_airing,
        clock: std::time::Instant::now(),
        library: library.clone(),
        refresh_button: refresh_button.clone(),
        download_menu: download_menu.clone(),
        bottom,
        bottom_row,
        stack,
        title,
        header_bar: header.clone(),
        toolbar,
        list: Rc::clone(&list),
        scroller: scroller.clone(),
        info,
        panes,
        list_head,
        count,
        missing,
        spinner,
        empty,
        empty_label,
        airing_row,
        airing_title,
        airing_time,
        fab: fab.clone(),
        fab_label,
        scroll: Cell::new((0.0, false)),
        bar,
        bar_count,
        options: options.clone(),
        episode_spec,
        season_spec,
        options_seasons: Cell::new(None),
        dialogs: RefCell::default(),
        dialog_window: RefCell::new(None),
        dialog_gen: Cell::new(0),
        seasons,
        seasons_shown: RefCell::new(None),
        two_pane: Cell::new(None),
        width: Cell::new(FALLBACK_WIDTH),
        expansion: Rc::new(Cell::new(Expansion::new(false, false))),
        related: RefCell::new(Vec::new()),
        related_host,
        related_shown: RefCell::new(None),
    });
    let weak = Rc::downgrade(&view);
    *weak_view.borrow_mut() = weak.clone();
    view.apply_width(FALLBACK_WIDTH);

    let on_view = |f: fn(&Rc<View>)| {
        let weak = weak.clone();
        move |_: &Button| {
            if let Some(view) = weak.upgrade() {
                f(&view);
            }
        }
    };
    options.connect({
        let weak = weak.clone();
        move |pick| {
            if let Some(view) = weak.upgrade() {
                view.list_option(pick);
            }
        }
    });
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
    overflow_menu.set_create_popup_func({
        let weak = weak.clone();
        move |button| {
            if let Some(view) = weak.upgrade() {
                button.set_popover(Some(&view.overflow_popover()));
            }
        }
    });
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

    // the queue state of the episodes follows the download queue live
    crate::downloads::live::follow(&page, {
        let weak = weak.clone();
        move |live| {
            if live.kind == Kind::Anime
                && let Some(view) = weak.upgrade()
            {
                view.live_changed(live.id, live.state, live.percent);
            }
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
            _ if super::super::shortcut_help_view::is_help_key(key) => {
                super::super::shortcut_help_view::open();
                glib::Propagation::Stop
            }
            _ if view.header_key_pressed(key, state) => glib::Propagation::Stop,
            _ => glib::Propagation::Proceed,
        }
    });
    keys
}

impl View {
    /// F (the list options menu), R (refresh), Ctrl+M (migrate, a favorite only) and W (WebView, an HTTP source only), the
    /// header keys of `shortcuts::header_key` (T-38a-046). `true` when the key was one of them and ran.
    fn header_key_pressed(self: &Rc<Self>, key: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool {
        use crate::entry_core::selection::Mods;
        use crate::entry_core::shortcuts::{HeaderKey, header_key};
        if state.intersects(gtk::gdk::ModifierType::ALT_MASK | gtk::gdk::ModifierType::SUPER_MASK) {
            return false;
        }
        let Some(letter) = key.to_lower().to_unicode() else { return false };
        let (favorite, http_source, selecting) = {
            let model = self.model.borrow();
            let Some(m) = model.as_ref().filter(|m| m.accepts_dispatch()) else { return false };
            (m.anime.favorite, self.env.backend.is_http_source(m.anime.source), m.selection.is_active())
        };
        let mods = Mods { shift: state.contains(gtk::gdk::ModifierType::SHIFT_MASK), ctrl: state.contains(gtk::gdk::ModifierType::CONTROL_MASK) };
        match header_key(letter, mods, http_source) {
            Some(HeaderKey::Filter) => self.options.popup(),
            // a selection disables Refresh, as it does the header button
            Some(HeaderKey::Refresh) if !selecting => self.refresh(),
            Some(HeaderKey::Migrate) if favorite => self.migrate(),
            Some(HeaderKey::WebView) => self.webview_pressed(),
            _ => return false,
        }
        true
    }

    fn on_event(self: &Rc<Self>, event: AnimeEvent) {
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
                }
            }
            AnimeEvent::RefreshEnded(result) => {
                // a failed refresh says why in a notification (behaviour 19)
                if let Some(message) = result.as_ref().err().and_then(refresh_snack) {
                    // Retry runs the refresh again; an uninstalled source has nothing to retry
                    let anime_id = self.model.borrow().as_ref().map(|m| m.anime.id);
                    match anime_id.filter(|_| !matches!(result, Err(crate::entry_core::errors::DetailError::SourceNotInstalled))) {
                        Some(anime_id) => {
                            self.show_notice(Notice::of(&crate::entry_core::message::with_retry(message), Some(AnimeCmd::Refresh { anime_id })));
                        }
                        None => self.present(message, None),
                    }
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
            AnimeEvent::AddCategories { all, linked } => self.show_add_category_dialog(&all, &linked),
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
            AnimeEvent::Tracking(count) => {
                if let Some(m) = self.model.borrow_mut().as_mut() {
                    m.tracking_count = count;
                }
            }
        }
    }

    /// Switches between the two layouts. It moves the info block between the left pane and the first row of the list.
    fn apply_width(self: &Rc<Self>, width: i32) {
        let resized = self.width.replace(width) != width;
        let two = two_pane(width);
        if self.two_pane.get() == Some(two) {
            // the layout is the same, but the automatic columns of the season grid follow the width (T-39a-034)
            if resized && let Some(m) = self.model.borrow().as_ref() {
                self.show_seasons(m, m.anime.fetch_type() == FetchType::Seasons);
            }
            return;
        }
        self.two_pane.set(Some(two));
        self.panes.set_large(two);
        detach(&self.info.root);
        if two {
            self.panes.left.append(&self.info.root);
            self.list.set_leading(vec![self.list_head.clone().upcast()]);
        } else {
            self.list.set_leading(vec![self.info.root.clone().upcast(), self.list_head.clone().upcast()]);
        }
        self.update_fade();
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
        let lines = self.list_rows(m);
        let seasons_entry = m.anime.fetch_type() == FetchType::Seasons;
        let none_shown = if seasons_entry { m.seasons.is_empty() } else { lines.is_empty() };
        self.list.sync(lines);
        self.show_seasons(m, seasons_entry);
        let header = m.item_header();
        self.count.set_label(&header.count);
        self.missing.set_label(header.missing.as_deref().unwrap_or(""));
        self.missing.set_visible(header.missing.is_some());
        // the list options follow the list on screen: the episode list, or the season grid of a `Seasons` entry
        if self.options_seasons.replace(Some(seasons_entry)) != Some(seasons_entry) {
            self.options.set_sections(&self.spec_of(seasons_entry).sections());
        }
        let word = if seasons_entry { m.anime.season_flags() } else { m.anime.item_flags };
        self.spec_of(seasons_entry).sync(&self.options, word, m.downloaded_only);
        // a selection disables the menu, as it did the old header button
        self.options.widget.set_sensitive(header.enabled);
        self.spinner.set_spinning(m.flow.refreshing);
        self.spinner.set_visible(m.flow.refreshing);
        // one refresh runs at a time
        self.refresh_button.set_sensitive(!m.flow.refreshing && !m.selection.is_active());
        self.empty.set_visible(none_shown && !m.flow.refreshing);
        self.empty_label.set_label(&tr(if seasons_entry { MsgKey::no_seasons_error } else { MsgKey::no_episodes_error }));
        // the anime and manga statuses share the SAnime/SManga completed code (2)
        self.info.action_row.apply(&RowState::of_entry(
            m.anime.favorite,
            m.anime.next_update,
            m.anime.status,
            m.anime.fetch_interval,
            m.tracking_count as i64,
            self.env.backend.is_http_source(m.anime.source),
        ));
        // a local or stub source has no downloads to queue (`isLocalOrStub`, T-39b-059)
        let local_or_stub = m.is_local() || self.env.backend.source_info(m.anime.source).stub;
        self.download_menu.set_visible(menu_visible(local_or_stub, self.env.backend.has_download_handler()));
        // the button stays while nothing is selected, and is disabled when no episode is unseen (T-EPS-NEXT-1)
        let fab = m.fab();
        self.fab.set_visible(!m.selection.is_active());
        self.fab.set_sensitive(m.next_unseen().is_some());
        self.fab_label.set_label(&tr(fab.label()));
        self.update_fab();
        self.bar.set_reveal_child(m.selection.is_active());
        let selected = m.selection.count_in(&m.processed_ids());
        self.bar_count.set_label(&format!("{selected} {}", tr(MsgKey::selected).to_lowercase()));
        self.render_info(m);
        self.render_related();
        self.render_bottom(m);
        drop(model);
        self.tick_airing();
    }

    /// The bottom action bar: the rows `bottom_actions` gives for the selected episodes, shown only while something
    /// is selected, as wide as the window narrow and half of it wide.
    fn render_bottom(self: &Rc<Self>, m: &AnimeEntryModel) {
        let selected: Vec<Selected> = m
            .episodes
            .iter()
            .filter(|e| m.selection.contains(e.id))
            .map(|e| Selected { bookmark: e.bookmark, fillermark: e.fillermark, seen: e.done, last_second_seen: e.progress, downloaded: m.downloaded.contains(&e.id) })
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
            self.track_marked(&cmd);
            self.runner.send(cmd);
        }
        if let Some((id, external)) = dispatch.open {
            self.open_episode(id, OpenPlan { external, torrent_server: self.torrent_server_needed() });
        }
        self.render();
    }

    /// `pref_remove_after_marked_as_read`: a mark seen deletes the downloads of the episodes it changed (T-39b-013).
    fn remove_after_seen(&self) -> bool {
        crate::misc::diet_defaults::DOWNLOAD_REMOVE_AFTER_MARKED_READ
    }

    /// A context menu item on a row, written for that one episode. It works while selecting too (E-5).
    fn menu_pressed(self: &Rc<Self>, id: i64, action: RowAction) {
        let cmds = {
            let model = self.model.borrow();
            let Some(m) = model.as_ref().filter(|m| m.accepts_dispatch()) else { return };
            menu_cmds(m, id, action, self.env.backend.has_download_handler(), self.remove_after_seen())
        };
        for cmd in cmds {
            self.track_marked(&cmd);
            self.runner.send(cmd);
        }
    }

    /// A mark seen goes to the trackers too, with the numbers of the episodes it changed.
    fn track_marked(&self, cmd: &AnimeCmd) {
        let AnimeCmd::SetSeen { ids, seen: true } = cmd else { return };
        let model = self.model.borrow();
        let Some(m) = model.as_ref() else { return };
        let numbers: Vec<f64> = m.episodes.iter().filter(|e| ids.contains(&e.id)).map(|e| e.number).collect();
        crate::entry_core::tracking::progress::marked(Kind::Anime, m.anime.id, true, &numbers, crate::data::app().base.incognito_mode.get());
    }

    /// Whether the entry's source is a torrent source and the torrent server is on (17.4, 17.5).
    fn torrent_server_needed(&self) -> bool {
        let source = self.model.borrow().as_ref().map(|m| m.anime.source);
        torrent_enabled(&self.env.prefs) && source.is_some_and(|s| self.env.backend.is_torrent_source(s))
    }

    /// Shows a message on the app's notification layer, replacing the one on screen. `action` is what its button runs.
    fn present(&self, message: UserMessage, action: Option<AnimeCmd>) {
        self.show_notice(Notice::of(&present(message), action));
    }

    /// Shows `notice`; its button sends its command to this page's runner while the page is alive (E-UI-1).
    fn show_notice(&self, notice: Notice<AnimeCmd>) {
        let runner = Rc::downgrade(&self.runner);
        notice.show(move |cmd| {
            if let Some(runner) = runner.upgrade() {
                runner.send(cmd);
            }
        });
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

    /// Puts `dialog` in the slot: the one on screen closes first, so the last request wins. Returns the name of the
    /// request, for `adopt`.
    fn claim_slot(&self, dialog: AnimeDialog) -> u64 {
        let id = self.dialog_gen.get() + 1;
        self.dialog_gen.set(id);
        self.dialogs.borrow_mut().open(dialog);
        let old = self.dialog_window.borrow_mut().take();
        if let Some(old) = old {
            old.close();
        }
        id
    }

    /// Keeps the window of request `id`. Its close, however it happens, dismisses the slot unless the window said
    /// otherwise (`dismiss_on_close` false: the category dialog reports its own OK and dismiss).
    fn adopt(self: &Rc<Self>, id: u64, window: &gtk::Window, dismiss_on_close: bool) {
        *self.dialog_window.borrow_mut() = Some(window.clone());
        if dismiss_on_close {
            let weak = Rc::downgrade(self);
            window.connect_close_request(move |_| {
                if let Some(view) = weak.upgrade() {
                    view.dismiss_dialog(id);
                }
                glib::Propagation::Proceed
            });
        }
    }

    /// `autoOpenTrack`: a tracker is logged in and "open the track menu on adding" is on.
    fn auto_open_track(&self) -> bool {
        crate::entry_core::tracking::session::refresh_from_host();
        let pref_on = crate::misc::diet_defaults::ENTRY_TRACK_ON_ADDING;
        crate::entry_core::tracking::summary::open_track_sheet_on_add(pref_on, self.env.backend.has_logged_in_tracker())
    }

    /// The dismiss of request `id` (`onDismissRequest`). When the add flow chose a category and auto-open is on, the
    /// track sheet opens now.
    fn dismiss_dialog(&self, id: u64) {
        if self.dialog_gen.get() != id {
            return;
        }
        self.dialog_window.borrow_mut().take();
        let auto = self.auto_open_track();
        if self.dialogs.borrow_mut().dismiss(auto) {
            self.open_tracking();
        }
    }

    /// The interval button (T-39a-026): the dialog of a favorite. OK and Reset store the wheel through the runner.
    fn open_interval(self: &Rc<Self>) {
        let (anime_id, interval, expected, favorite) = {
            let model = self.model.borrow();
            let Some(m) = model.as_ref().filter(|m| m.accepts_dispatch()) else { return };
            let expected = (m.anime.status != kuro_domain::entries::flags::STATUS_COMPLETED).then_some(m.anime.next_update);
            (m.anime.id, m.anime.fetch_interval, expected, m.anime.favorite)
        };
        if !interval_click_opens_dialog(favorite) {
            return;
        }
        let runner = self.runner.clone();
        let id = self.claim_slot(AnimeDialog::SetAnimeFetchInterval { anime_id });
        let window = fetch_interval_dialog::open(interval, expected, move |wheel| {
            runner.send(AnimeCmd::SetFetchInterval { anime_id, wheel });
        });
        self.adopt(id, &window, true);
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
    fn show_duplicate(self: &Rc<Self>, existing: &[kuro_data::entry::Entry]) {
        let (Some(duplicate), Some(anime_id)) = (existing.first().cloned(), self.model.borrow().as_ref().map(|m| m.anime.id)) else { return };
        let runner = Rc::clone(&self.runner);
        let id = self.claim_slot(AnimeDialog::DuplicateAnime { anime_id, duplicate_id: duplicate.id });
        let window = duplicate_view::open(move |choice| match choice {
            Choice::ShowAnime => open(duplicate.id),
            Choice::Migrate => handoff::open_migrate(MigrateRequest { entry_id: duplicate.id, kind: Kind::Anime, title: duplicate.title.clone() }),
            Choice::AddAnyway => runner.send(AnimeCmd::AddToLibrary { anime_id, check_duplicates: false }),
        });
        self.adopt(id, &window, true);
    }

    /// The tracking page of this anime. It explains the AniList login itself when AniList is not logged in.
    fn open_tracking(&self) {
        crate::entry_core::tracking::session::refresh_from_host();
        let loaded = self.model.borrow().as_ref().map(|m| (m.anime.id, m.anime.source, m.anime.title.clone()));
        if let Some((id, source_id, title)) = loaded {
            // a bind, an edit or a stop on the tracking page reads the count again, so the button follows at once
            let runner = Rc::downgrade(&self.runner);
            let changed = move || {
                if let Some(runner) = runner.upgrade() {
                    runner.send(AnimeCmd::ReadTracking { anime_id: id, source_id });
                }
            };
            crate::entry_core::tracking::page::open_sheet(crate::entry_core::tracking::page::Entry { kind: Kind::Anime, id, title }, changed);
        }
    }

    /// The tracking button.
    fn tracking_pressed(&self) {
        self.claim_slot(AnimeDialog::TrackSheet);
        self.open_tracking();
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
    fn show_category_dialog(self: &Rc<Self>, all: &[(i64, String)], linked: &[i64]) {
        let Some(anime_id) = self.model.borrow().as_ref().map(|m| m.anime.id) else { return };
        let runner = Rc::clone(&self.runner);
        let id = self.claim_slot(AnimeDialog::ChangeCategory { anime_id });
        let (ok, dismissed) = (Rc::downgrade(self), Rc::downgrade(self));
        let window = crate::category_picker::open(
            CategoryDialog::new(anime_id, false, all, linked),
            move |m| {
                runner.send(AnimeCmd::SetCategories { anime_id, category_ids: m.checked_ids() });
                if let Some(view) = ok.upgrade() {
                    view.confirm_category(id);
                }
            },
            || crate::ui::open("categories"),
            move || {
                if let Some(view) = dismissed.upgrade() {
                    view.dismiss_dialog(id);
                }
            },
        );
        self.adopt(id, &window, false);
    }

    /// OK in the category dialog: the slot empties without the dismiss, so the track sheet does not follow (5.3).
    fn confirm_category(&self, id: u64) {
        if self.dialog_gen.get() == id {
            self.dialog_window.borrow_mut().take();
            self.dialogs.borrow_mut().confirm_category();
        }
    }

    /// The category dialog of an add with no default category (T-39b-026). OK adds the entry in the ticked categories;
    /// a dismiss adds nothing, since favorite is written only with the answer.
    fn show_add_category_dialog(self: &Rc<Self>, all: &[(i64, String)], linked: &[i64]) {
        let Some(anime_id) = self.model.borrow().as_ref().map(|m| m.anime.id) else { return };
        let runner = Rc::clone(&self.runner);
        // the choose branch sets the pending flag: a dismiss may open the track sheet after it
        let id = self.claim_slot(AnimeDialog::ChangeCategory { anime_id });
        self.dialogs.borrow_mut().open_change_category_from_add(anime_id);
        let (ok, dismissed) = (Rc::downgrade(self), Rc::downgrade(self));
        let window = crate::category_picker::open(
            CategoryDialog::new(anime_id, true, all, linked),
            move |m| {
                runner.send(AnimeCmd::AddWithCategories { anime_id, category_ids: m.checked_ids() });
                if let Some(view) = ok.upgrade() {
                    view.confirm_category(id);
                }
            },
            || crate::ui::open("categories"),
            move || {
                if let Some(view) = dismissed.upgrade() {
                    view.dismiss_dialog(id);
                }
            },
        );
        self.adopt(id, &window, false);
    }

    /// The width the season grid has: the page, or the right pane of the two-pane layout.
    fn grid_width(&self) -> i32 {
        let width = self.width.get();
        if self.two_pane.get() == Some(true) { (width - LEFT_PANE_WIDTH - 1).max(0) } else { width }
    }

    /// The season grid of a `Seasons` entry. Its minimum column count is the entry's size setting (`Layout::columns_for`); it
    /// is redrawn only when the cells, the columns or the width changed.
    fn show_seasons(&self, m: &AnimeEntryModel, seasons_entry: bool) {
        if !seasons_entry {
            self.seasons.widget.set_visible(false);
            *self.seasons_shown.borrow_mut() = None;
            return;
        }
        let size = m.anime.season().grid_size();
        let mode = m.anime.season().grid_mode();
        let width = self.grid_width();
        // the facts of a season are what the runner read for it; the filters, the sort and the overlays are the pipeline's
        let seasons: Vec<(SeasonEntry, SeasonFacts)> = m
            .seasons
            .iter()
            .map(|s| {
                let local = s.entry.source == kuro_source::local::LOCAL_SOURCE_ID;
                (s.clone(), season_facts(s, m.season_extras.get(&s.entry.id), local))
            })
            .collect();
        let items: Vec<SeasonItem> = process_seasons(&m.anime, &seasons, m.downloaded_only);
        let layout = Layout {
            mode,
            number_mode: m.anime.season().display_mode() == kuro_db::flags::season::DISPLAY_MODE_NUMBER,
            // the page can continue watching: a tile shows the button when its season has an unseen episode
            has_continue: true,
            columns: Layout::columns_for(width, i64::from(size), mode),
            size: i64::from(size),
            fixed: size > 0 || mode == SeasonDisplayMode::List,
            width,
            height: self.scroller.height(),
        };
        let key = (items, layout);
        if self.seasons_shown.borrow().as_ref() == Some(&key) {
            return;
        }
        self.seasons.set(&key.0, &key.1);
        *self.seasons_shown.borrow_mut() = Some(key);
    }

    /// The overflow menu (T-39a-075, T-39a-057). Built each time it opens, so the items follow the entry.
    fn overflow_popover(self: &Rc<Self>) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let col = GBox::new(O::Vertical, 2);
        ui::margins(&col, 6);
        let entry = {
            let model = self.model.borrow();
            model.as_ref().filter(|m| m.accepts_dispatch()).map(|m| {
                let source = m.anime.source;
                (
                    OverflowEntry {
                        favorite: m.anime.favorite,
                        episodes: m.anime.fetch_type() == FetchType::Episodes,
                        http_source: self.env.backend.is_http_source(source),
                        configurable_source: self.env.backend.is_configurable_source(source),
                    },
                    m.flow.refreshing,
                )
            })
        };
        let Some((entry, refreshing)) = entry else { return popover };
        for item in overflow::items(entry) {
            let (label, action): (String, fn(&Rc<View>)) = match item {
                OverflowItem::ChangeIntro => (tr(MsgKey::action_change_intro_length), |v: &Rc<View>| v.open_intro_length()),
                OverflowItem::Refresh => (tr(MsgKey::action_webview_refresh), |v: &Rc<View>| v.refresh()),
                OverflowItem::EditCategories => (tr(MsgKey::action_edit_categories), |v: &Rc<View>| v.open_categories()),
                OverflowItem::Migrate => (tr(MsgKey::action_migrate), |v: &Rc<View>| v.migrate()),
                OverflowItem::Share => (tr(MsgKey::action_share), |v: &Rc<View>| v.share()),
                OverflowItem::OpenInBrowser => (tr(MsgKey::action_open_in_browser), |v: &Rc<View>| v.open_in_browser()),
                OverflowItem::Settings => (tr(MsgKey::action_settings), |v: &Rc<View>| v.open_source_preferences()),
            };
            let weak = Rc::downgrade(self);
            let button = plain_row(&label, &popover, move |_| {
                if let Some(view) = weak.upgrade() {
                    action(&view);
                }
            });
            // one refresh runs at a time
            button.set_sensitive(item != OverflowItem::Refresh || !refreshing);
            col.append(&button);
        }
        popover.set_child(Some(&col));
        popover
    }

    /// Change intro length (T-39a-075): the wheel of the entry's skip-intro length. OK stores it through the runner.
    fn open_intro_length(self: &Rc<Self>) {
        let (anime_id, stored, disabled) = {
            let model = self.model.borrow();
            let Some(m) = model.as_ref().filter(|m| m.accepts_dispatch()) else { return };
            (m.anime.id, m.anime.viewer().skip_intro_length(), m.anime.viewer().skip_intro_disable())
        };
        let default = DEFAULT_INTRO_LENGTH;
        let runner = self.runner.clone();
        let id = self.claim_slot(AnimeDialog::ChangeAnimeSkipIntro);
        let window = intro_dialog::open(viewer::dialog_start(stored, disabled, default), move |length| {
            runner.send(AnimeCmd::SetSkipIntroLength { anime_id, length });
        });
        self.adopt(id, &window, true);
    }

    /// Settings (T-39a-057): the preferences screen of this entry's source, which the request names.
    fn open_source_preferences(self: &Rc<Self>) {
        let source_id = self.model.borrow().as_ref().filter(|m| m.accepts_dispatch()).map(|m| m.anime.source);
        if let Some(source_id) = source_id {
            handoff::open_source_preferences(handoff::SourcePrefsRequest { source_id, kind: Kind::Anime });
        }
    }

    /// Migrate (T-39a-075): starts the migrate flow from this entry, as the duplicate choice does.
    fn migrate(self: &Rc<Self>) {
        let request = self
            .model
            .borrow()
            .as_ref()
            .filter(|m| m.accepts_dispatch())
            .map(|m| MigrateRequest { entry_id: m.anime.id, kind: Kind::Anime, title: m.anime.title.clone() });
        if let Some(request) = request {
            handoff::open_migrate(request);
        }
    }

    /// Share (T-39a-075): copies the entry's address to the clipboard and says so, as the manga page does.
    fn share(self: &Rc<Self>) {
        let url = self.model.borrow().as_ref().filter(|m| m.accepts_dispatch()).map(|m| m.anime.url.clone());
        if let (Some(url), Some(display)) = (url, gtk::gdk::Display::default()) {
            display.clipboard().set_text(&url);
            self.present(UserMessage::Plain(LINK_COPIED.to_owned()), None);
        }
    }

    /// Open in browser (T-39b-051): the entry's address in the desktop's browser through `gtk::UriLauncher`. A launch
    /// that fails is a toast, never a panic. Offered for an HTTP source only (`overflow::items`).
    fn open_in_browser(self: &Rc<Self>) {
        let url = self.model.borrow().as_ref().filter(|m| m.accepts_dispatch()).map(|m| m.anime.url.clone());
        let Some(url) = url.filter(|u| can_launch(u)) else { return };
        let weak = Rc::downgrade(self);
        gtk::UriLauncher::new(&url).launch(Some(&ui::window()), None::<&gtk::gio::Cancellable>, move |result| {
            if let (Err(error), Some(view)) = (result, weak.upgrade()) {
                view.present(UserMessage::Plain(error.to_string()), None);
            }
        });
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
            let weak = Rc::downgrade(self);
            col.append(&plain_row(&label, &popover, move |_| {
                if let Some(view) = weak.upgrade() {
                    view.run_download(menu);
                }
            }));
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

    /// Draws the info block: the header in the layout and column of the page, then the description with its genre chips.
    fn render_info(self: &Rc<Self>, m: &AnimeEntryModel) {
        let two = self.two_pane.get().unwrap_or(false);
        let model = header_model(&m.anime, &m.source);
        let column = if two { LEFT_PANE_WIDTH } else { self.width.get() };
        let handlers = self.handlers();
        self.info.set_header(&model, two, column, &handlers);
        self.info.set_description(&description(m.anime.description.as_deref()), &model.genres, &self.expansion, &handlers);
    }

    fn handlers(self: &Rc<Self>) -> Handlers {
        let weak = Rc::downgrade(self);
        Handlers {
            // the query goes to the opened screen through `handoff::take_search`
            on_search: Rc::new({
                move |request: SearchRequest| {
                    let http_source = weak.upgrade().is_some_and(|view| {
                        let source = view.model.borrow().as_ref().map(|m| m.anime.source);
                        source.is_some_and(|source| view.env.backend.is_http_source(source))
                    });
                    handoff::open_search(request, Kind::Anime, http_source);
                }
            }),
            on_copy: Rc::new(|text: String| {
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&text);
                }
            }),
        }
    }

    /// The FAB label shows at the top, after a backward scroll and at the end of the list (`shouldExpandFAB`).
    fn update_fab(&self) {
        self.fab_label.set_visible(page_shell::fab_label_shown(&self.scroll, &self.scroller.vadjustment()));
        self.update_fade();
    }

    /// The scroll position fades the title and the header bar background in the narrow layout (T-39a-036).
    fn update_fade(&self) {
        let narrow = !self.two_pane.get().unwrap_or(false);
        let value = self.scroller.vadjustment().value();
        page_shell::fade_header(self.title.as_ref(), self.header_bar.as_ref(), narrow, value, self.info.root.height(), false);
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

    fn row_pressed(self: &Rc<Self>, id: i64, long: bool, alt: bool, mods: crate::entry_core::selection::Mods) {
        let open = {
            let mut model = self.model.borrow_mut();
            let Some(m) = model.as_mut().filter(|m| m.accepts_dispatch()) else { return };
            let view = m.processed_ids();
            if long {
                m.selection.long_press(&view, id);
                None
            } else {
                match m.selection.click_with(&view, id, mods.shift, mods.ctrl) {
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

    /// The rows of the list. The download button shows only when the app can queue this entry's episodes: not for a
    /// local or stub source, and not without a download handler (`isLocalOrStub`, T-39b-059).
    fn list_rows(&self, m: &AnimeEntryModel) -> Vec<crate::entry_core::list_model::RowItem> {
        let mut rows = m.lines();
        let can_download = !m.is_local() && !self.env.backend.source_info(m.anime.source).stub && self.env.backend.has_download_handler();
        if !can_download {
            for row in &mut rows {
                row.indicator = false;
            }
        }
        rows
    }

    /// A change of one episode in the download queue: only the list is drawn again.
    fn live_changed(&self, id: i64, state: DownloadState, percent: u8) {
        let rows = {
            let mut model = self.model.borrow_mut();
            let Some(m) = model.as_mut() else { return };
            m.apply_live(id, state, percent);
            self.list_rows(m)
        };
        self.list.sync(rows);
    }

    /// A press on an episode's download button: Start queues it (an error is queued again), Cancel takes it out of the
    /// queue at once, and Delete (confirmed in the button's popover) removes its files.
    fn download_pressed(&self, id: i64, request: Request) {
        let Some(anime_id) = self.model.borrow().as_ref().filter(|m| m.accepts_dispatch()).map(|m| m.anime.id) else { return };
        match request {
            Request::Start => self.runner.send(AnimeCmd::Download { anime_id, ids: vec![id], start_now: false }),
            Request::Cancel => {
                crate::downloads::live::cancel(Kind::Anime, id);
                self.live_changed(id, DownloadState::NotDownloaded, 0);
            }
            Request::Delete => self.runner.send(AnimeCmd::DeleteEpisodeDownloads { anime_id, ids: vec![id] }),
        }
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
        // repeated clicks, or clicks queued while the window was busy, open the player once
        if !crate::ui::first_within("player", 1500) {
            return;
        }
        // a local episode plays in the internal player at its saved position, whatever the plan says
        if crate::local_flow::open::open_local_episode(episode_id) {
            return;
        }
        if plan.torrent_server {
            // the server start waits up to eleven seconds, so it runs off the GTK thread; the episode opens when it
            // answers, times out or fails (the failure is a toast and playback goes on), T-39b-052
            self.env.backend.start_torrent_server();
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let _ = tx.send_blocking(torrent_server::start_blocking(true, true));
            });
            let (backend, title, name) = (Arc::clone(&self.env.backend), title.to_owned(), name.to_owned());
            glib::spawn_future_local(async move {
                if let Ok(torrent_server::Outcome::Toast(text)) = rx.recv().await {
                    crate::toast::notify(text);
                }
                Self::open_player(&backend, plan, anime_id, episode_id, &title, &name);
            });
            return;
        }
        Self::open_player(&self.env.backend, plan, anime_id, episode_id, title, name);
    }

    /// Hands the episode to the external player the plan names, or to the internal one.
    fn open_player(backend: &Arc<dyn AnimeBackend>, plan: OpenPlan, anime_id: i64, episode_id: i64, title: &str, name: &str) {
        if plan.external && backend.play_external(anime_id, episode_id) {
            return;
        }
        // an episode of an online source resolves its video and plays in the internal player
        if crate::player_view::open_source_episode(&live_anime_sources(), episode_id) {
            return;
        }
        crate::screens::open_media(Kind::Anime, title, name);
    }

    /// Logs a typed error and shows its text in the snackbar.
    fn show_error(&self, error: DetailsError) {
        self.present(UserMessage::Plain(error.record().user_message()), None);
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

    /// Draws the related groups under the description when they or the display mode changed.
    fn render_related(&self) {
        let mode = kuro_prefs::library::LibraryDisplayMode::CompactGrid;
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
        // an empty box would still take the info block's spacing
        self.related_host.set_visible(self.related_host.first_child().is_some());
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

    fn spec_of(&self, seasons: bool) -> &Spec {
        if seasons { &self.season_spec } else { &self.episode_spec }
    }

    /// A pick in the list options menu: a masked write of the episode or season flag word, or "Use as default for all
    /// titles" (asked first). The menu shows the stored word again on the next render.
    fn list_option(self: &Rc<Self>, pick: Pick) {
        if pick == Pick::Action(list_options::DEFAULT) {
            let weak = Rc::downgrade(self);
            list_options::confirm_default(move || {
                if let Some(view) = weak.upgrade() {
                    view.use_as_default();
                }
            });
            return;
        }
        let Some((anime_id, seasons)) =
            self.model.borrow().as_ref().filter(|m| m.accepts_dispatch()).map(|m| (m.anime.id, m.anime.fetch_type() == FetchType::Seasons))
        else {
            return;
        };
        let Some((flag, mask)) = self.spec_of(seasons).write(&pick) else { return };
        // an entry outside the library keeps its flags for the session: the model takes the change as well as the row
        if let Some(m) = self.model.borrow_mut().as_mut() {
            if seasons {
                m.note_season_write(flag, mask);
            } else {
                m.note_flag_write(flag, mask);
            }
        }
        self.runner.send(if seasons { AnimeCmd::SetSeasonFlagBits { anime_id, flag, mask } } else { AnimeCmd::SetFlagBits { anime_id, flag, mask } });
        self.render();
    }

    /// "Use as default for all titles": stores the flag word of the list on screen as the default of its scope, and the
    /// runner applies that default to every favorite. Only that scope is written: an episode save never touches the
    /// season flags of the library, and the reverse. The message waits for the run, to say how many entries took the
    /// settings (E-DEF-3, `AnimeEvent::DefaultsApplied`).
    fn use_as_default(self: &Rc<Self>) {
        let Some((seasons, word)) = self.model.borrow().as_ref().map(|m| {
            let seasons = m.anime.fetch_type() == FetchType::Seasons;
            (seasons, if seasons { m.anime.season_flags() } else { m.anime.item_flags })
        }) else {
            return;
        };
        let scope = if seasons {
            set_season_settings_default(&self.env.prefs, word);
            Scope::Season
        } else {
            set_item_settings_default(&self.env.prefs, Kind::Anime, word);
            Scope::Episode
        };
        self.runner.send(AnimeCmd::ApplyDefaultsToLibrary { scope });
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
        Env { prefs: PrefStore::in_memory(), backend: Arc::new(Stub), db: store.db() }
    }

    use crate::entry_core::page_shell::test_util::{descendants, pump};

    /// A backend whose every fetch fails with `error`; it counts the fetches.
    struct Failing(crate::entry_core::errors::DetailError, std::sync::atomic::AtomicUsize);
    impl runner::AnimeBackend for Failing {
        fn source_info(&self, _: i64) -> crate::entry_core::detail::SourceInfo {
            crate::entry_core::detail::SourceInfo { name: "Src".into(), stub: false }
        }
        fn fetch(&self, _: i64, _: super::super::entry_state::RefreshPlan, _: super::super::refresh::Trigger) -> runner::AnimeFetch {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let error = self.0.clone();
            Box::pin(async move { Err(error) })
        }
    }

    /// A failed refresh (HTTP 503) shows its notification with Retry, and Retry refreshes again; an uninstalled source
    /// shows its notification with no Retry. The notification is the app's one layer (`toast::attach`).
    #[test]
    fn a_failed_refresh_offers_retry_and_an_uninstalled_source_does_not() {
        use crate::entry_core::errors::DetailError;
        use std::sync::atomic::Ordering;
        crate::testing::with_display(|| {
            let ctx = glib::MainContext::default();
            let _owner = ctx.acquire().expect("main context");
            let rt = kuro_core::runtime::build().expect("runtime");
            let (_dir, store, id) = super::super::store::fixture::with_episodes(true, true, 3, 1);
            for (error, retry) in [(DetailError::Http(503), true), (DetailError::SourceNotInstalled, false)] {
                let backend = Arc::new(Failing(error.clone(), Default::default()));
                let runner = Rc::new(runner::spawn(rt.handle(), store.clone(), PrefStore::in_memory(), backend.clone(), AnimeLoadContext::default()));
                let host = gtk::Overlay::new();
                crate::toast::attach(&host);
                let page = assemble(Rc::clone(&runner), true, Env { backend: backend.clone(), ..env_of(&store) });
                runner.send(AnimeCmd::Load { anime_id: id });
                let stack = descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<gtk::Stack>().ok()).find(|s| s.child_by_name("ready").is_some()).expect("the page has a stack");
                assert!(pump(&ctx, || stack.visible_child_name().as_deref() == Some("ready")), "the page becomes ready");
                assert!(
                    !descendants(page.upcast_ref()).iter().any(|w| w.has_css_class("app-notification")),
                    "the page has no notification bar of its own"
                );
                runner.send(AnimeCmd::Refresh { anime_id: id });
                let snack = descendants(host.upcast_ref())
                    .into_iter()
                    .filter_map(|w| w.downcast::<gtk::Revealer>().ok())
                    .find(|r| r.child().is_some_and(|c| c.has_css_class("app-notification")))
                    .expect("the notification layer");
                assert!(pump(&ctx, || snack.reveals_child()), "{error:?}: the failed refresh shows a notification");
                let action = snack.child().and_then(|bar| bar.first_child()).and_then(|label| label.next_sibling()).and_downcast::<Button>().expect("the action button");
                assert_eq!(action.is_visible(), retry, "{error:?}: Retry only when there is something to retry");
                if retry {
                    assert_eq!(action.label().as_deref(), Some(tr(MsgKey::action_retry).as_str()));
                    let before = backend.1.load(Ordering::SeqCst);
                    action.emit_clicked();
                    assert!(pump(&ctx, || backend.1.load(Ordering::SeqCst) > before), "Retry sends the refresh to the runner");
                }
                runner.send(AnimeCmd::Shutdown);
            }
        });
    }

    /// A refresh that finds no episodes raises no notification: the list's empty state says so, with a Retry button that
    /// refreshes again.
    #[test]
    fn no_episodes_is_the_empty_state_with_retry_not_a_notification() {
        use crate::entry_core::errors::DetailError;
        use std::sync::atomic::Ordering;
        crate::testing::with_display(|| {
            let ctx = glib::MainContext::default();
            let _owner = ctx.acquire().expect("main context");
            let rt = kuro_core::runtime::build().expect("runtime");
            let (_dir, store, id) = super::super::store::fixture::with_episodes(true, true, 0, 0);
            let backend = Arc::new(Failing(DetailError::NoEpisodes, Default::default()));
            let runner = Rc::new(runner::spawn(rt.handle(), store.clone(), PrefStore::in_memory(), backend.clone(), AnimeLoadContext::default()));
            let host = gtk::Overlay::new();
            crate::toast::attach(&host);
            let page = assemble(Rc::clone(&runner), true, Env { backend: backend.clone(), ..env_of(&store) });
            runner.send(AnimeCmd::Load { anime_id: id });
            let stack = descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<gtk::Stack>().ok()).find(|s| s.child_by_name("ready").is_some()).expect("the page has a stack");
            assert!(pump(&ctx, || stack.visible_child_name().as_deref() == Some("ready")), "the page becomes ready");
            let before = backend.1.load(Ordering::SeqCst);
            runner.send(AnimeCmd::Refresh { anime_id: id });
            assert!(pump(&ctx, || backend.1.load(Ordering::SeqCst) > before), "the refresh ran and found nothing");
            let until = std::time::Instant::now() + Duration::from_millis(300);
            while std::time::Instant::now() < until {
                ctx.iteration(false);
            }
            let layer = descendants(host.upcast_ref()).into_iter().filter_map(|w| w.downcast::<gtk::Revealer>().ok()).find(|r| r.child().is_some_and(|c| c.has_css_class("app-notification"))).expect("the notification layer");
            assert!(!layer.reveals_child(), "nothing pops up for an empty list");
            // the list head is a row of the list view, so it exists once the page has a size
            let window = gtk::Window::builder().default_width(900).default_height(700).child(&page).build();
            window.present();
            let find_retry = || {
                descendants(page.upcast_ref())
                    .into_iter()
                    .filter_map(|w| w.downcast::<Button>().ok())
                    .find(|b| b.label().as_deref() == Some(tr(MsgKey::action_retry).as_str()))
            };
            assert!(pump(&ctx, || find_retry().is_some()), "the empty state has Retry");
            let retry = find_retry().expect("Retry");
            assert!(retry.parent().is_some_and(|state| state.is_visible()), "and it shows, since the list is empty");
            let before = backend.1.load(Ordering::SeqCst);
            retry.emit_clicked();
            assert!(pump(&ctx, || backend.1.load(Ordering::SeqCst) > before), "Retry refreshes again");
            runner.send(AnimeCmd::Shutdown);
            window.set_child(None::<&gtk::Widget>);
            window.close();
        });
    }

    /// Add to library on an entry opened from Browse (not a favorite, never refreshed): the click writes favorite=1 in
    /// the default category and the button turns to "In library".
    #[test]
    fn add_to_library_from_the_details_page_writes_the_row_and_flips_the_button() {
        crate::testing::with_display(|| {
            let ctx = glib::MainContext::default();
            let _owner = ctx.acquire().expect("main context");
            let rt = kuro_core::runtime::build().expect("runtime");
            let (_dir, store, id) = super::super::store::fixture::with_episodes(false, false, 2, 0);
            let category = kuro_testkit::insert_category(&store.db(), Kind::Anime, "Watching");
            let prefs = PrefStore::in_memory();
            kuro_prefs::library::LibraryPrefs::new(prefs.clone()).default_anime_category.set(category as i32).expect("default category");
            let runner = Rc::new(runner::spawn(rt.handle(), store.clone(), prefs, Arc::new(Stub), AnimeLoadContext::default()));
            let page = assemble(Rc::clone(&runner), true, env_of(&store));
            runner.send(AnimeCmd::Load { anime_id: id });
            let label = |text: &str| descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<Label>().ok()).find(|l| l.label() == text);
            assert!(pump(&ctx, || label("Add to library").is_some()), "the page shows Add to library");
            let button = label("Add to library").and_then(|l| l.ancestor(Button::static_type())).and_downcast::<Button>().expect("the library button");
            button.emit_clicked();
            assert!(pump(&ctx, || store.anime(id).unwrap().is_some_and(|a| a.favorite)), "the click writes favorite=1");
            assert_eq!(store.category_ids(id).unwrap(), vec![category], "the default category is linked");
            assert!(pump(&ctx, || label("In library").is_some()), "the button turns to In library");
            runner.send(AnimeCmd::Shutdown);
        });
    }

    /// One display test (GTK is bound to the thread that starts it): Loading, then the page with its rows and a
    /// stub warning, the single column at 360 px with no horizontal minimum above 360, the two panes from 900 px with
    /// the info block moved to the left, and Missing with a Back button.
    #[test]
    fn anime_route_smoke() {
        use kuro_db::flags::item as ep;
        crate::testing::with_display(|| {
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
            let stack_of = |page: &GBox| descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<gtk::Stack>().ok()).find(|s| s.child_by_name("ready").is_some()).expect("the page has a stack");

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
            let left = descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<gtk::ScrolledWindow>().ok()).find(|w| w.width_request() == LEFT_PANE_WIDTH).expect("the left pane");
            assert!(!left.is_visible(), "no left pane in one column");
            // one list options menu in the top bar, with the episode options; the count row is no button
            let options: Vec<gtk::MenuButton> = descendants(page.upcast_ref())
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::MenuButton>().ok())
                .filter(|b| b.tooltip_text().as_deref() == Some(list_options::TOOLTIP))
                .collect();
            assert_eq!(options.len(), 1, "one list options button");
            let labels = crate::widgets::view_options::menu_labels(&options[0].menu_model().expect("the menu"));
            for want in ["Show only", "Downloaded", "Unwatched", "Bookmarked", "Filler", "Sort by", "Source order", "Number", "Upload date", "Reverse order", "Display", "Episode number", "Use as default for all titles"] {
                assert!(labels.iter().any(|t| t == want), "the menu has {want}: {labels:?}");
            }
            assert!(!labels.iter().any(|t| t == "Items per row"), "an episode list shows no season options");
            let count_row = descendants(page.upcast_ref()).into_iter().filter_map(|w| w.downcast::<Label>().ok()).find(|l| l.label() == "3 episodes");
            assert!(count_row.is_none_or(|l| l.ancestor(Button::static_type()).is_none()), "no second sort button above the list");
            // Unwatched writes the flag and leaves the two unwatched episodes; turning it off brings the third back
            let rows = &store;
            let unseen = |on: bool| move || rows.anime(id).unwrap().is_some_and(|a| (a.item_flags & ep::UNDONE_MASK == ep::SHOW_UNDONE) == on);
            options[0].activate_action("viewopt.unseen", None).expect("the Unwatched check");
            assert!(pump(&ctx, unseen(true)), "the check writes the flag");
            assert!(pump(&ctx, || list.model().is_some_and(|m| m.n_items() == 4)), "two leading rows and two unwatched episodes, got {:?}", list.model().map(|m| m.n_items()));
            options[0].activate_action("viewopt.unseen", None).expect("the Unwatched check");
            assert!(pump(&ctx, unseen(false)), "off clears the group");
            assert!(pump(&ctx, || list.model().is_some_and(|m| m.n_items() == 5)), "every episode again");
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
        });
    }
}
