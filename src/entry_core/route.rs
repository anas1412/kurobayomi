//! The manga details route: a thin GTK page over `Page` (the controller) and `Runner` (the store). It shows a
//! centered loading state until the first read arrives, the page when it is `Ready`, and a "not found" state with a
//! Back button when the row is missing. Toolbar actions stay disabled while `Loading`.
//! `ui::screen("manga_entry_details")` resolves here through `build`, and `open(id)` pushes a page for one manga.
//!
//! The header bar is built from `toolbar.rs`: the Download menu (not for a local manga), the Filter menu (tinted while a
//! filter is on) and the overflow menu; with a selection it shows the count and only Select all and Select inverse.
//! The count row over the list opens the filter sheet. Back and Escape clear a selection before they pop the page.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O, glib, pango};
use kuro_domain::download::prefs::DownloadPrefs;
use kuro_domain::entries::defaults::chapter_default_word;
use kuro_domain::kind::EntryKind;
use kuro_i18n::{Arg, MsgKey, tr, tr_plural, trf};
use kuro_prefs::reader::ReaderPrefs;
use kuro_prefs::tristate::TriState;

use crate::data;
use crate::entry_core::backend::StubBackend;
use crate::entry_core::action_row::{TrackingClick, interval_click_opens_dialog, next_update, tracking_click};
use crate::entry_core::action_row_view::{ActionRow, RowState};
use crate::entry_core::banner_view::{self, BannerView};
use crate::entry_core::chapter::ChapterRow;
use crate::entry_core::chapter_actions::{DownloadAction, fab_state, indicator_action};
use crate::entry_core::description_view;
use crate::entry_core::detail::{LatestWins, LoadContext, LoadState, StoreCmd, StoreEvent};
use crate::entry_core::dialog::{CategoryDialog, Dialog};
use crate::entry_core::fetch_interval_dialog;
use crate::entry_core::handoff::{self, MigrateRequest, WebViewRequest};
use crate::entry_core::flags::{ChapterFlags, save_as_default, scanlator_filter_active};
use crate::entry_core::header::{GenreChip, HeaderModel, SearchRequest, is_large};
use crate::entry_core::header_view::{self, Handlers, LibraryButton};
use crate::entry_core::list_model::list_header;
use crate::entry_core::list_view::{ChapterList, RowActions, Swipe, ensure_css};
use crate::entry_core::page::{Page, UiAction};
use crate::entry_core::runner::{self, Runner};
use crate::entry_core::scanlator_sheet;
use crate::entry_core::selection::{Key, Mods, key_command};
use crate::entry_core::store::DbStore;
use crate::entry_core::text::{Description, Expansion, description};
use crate::entry_core::toolbar::{ActionMenu, Overflow, bar_state, background_opaque, title_opaque};
use crate::ui;

/// The window width assumed before the page has a window: the app's default width.
const FALLBACK_WIDTH: i32 = 1180;
const FALLBACK_HEIGHT: i32 = 780;
/// The Resume or Start button fades in and out over this long.
const FAB_FADE_MS: u32 = 150;

thread_local! {
    static TARGET: Cell<Option<i64>> = const { Cell::new(None) };
}

/// Names the manga the next `build()` shows. `ui::open("manga_entry_details")` carries no argument, so a caller sets
/// the target first.
pub fn target(manga_id: i64) {
    TARGET.with(|t| t.set(Some(manga_id)));
}

/// Pushes the details page of one manga.
pub fn open(manga_id: i64) {
    ui::push(&page_for(manga_id));
}

/// The page for the `manga_entry_details` screen id. Without a target it is the mockup page, so a caller that has no
/// row to name yet keeps working.
pub fn build() -> GBox {
    match TARGET.with(Cell::take) {
        Some(id) => page_for(id),
        None => crate::entry::entry("Title 1", false),
    }
}

/// Everything the widgets and handlers share. Handlers hold `Weak<View>`; the event pump holds the only strong
/// reference, so the page frees itself when the pump ends.
struct View {
    page: RefCell<Page>,
    runner: Rc<Runner>,
    clock: Instant,
    stack: gtk::Stack,
    header_slot: GBox,
    library: LibraryButton,
    action_row: ActionRow,
    description_slot: GBox,
    header_button: Button,
    count: Label,
    missing: Label,
    empty: Label,
    spinner: gtk::Spinner,
    list: Rc<ChapterList>,
    scroller: gtk::ScrolledWindow,
    selection_bar: gtk::Revealer,
    banner: BannerView,
    fab: gtk::Revealer,
    fab_label: Label,
    /// The header bar: every widget that waits for the page to be `Ready`.
    toolbar: Vec<gtk::Widget>,
    download_menu: gtk::MenuButton,
    filter: gtk::MenuButton,
    overflow: gtk::MenuButton,
    select_all: Button,
    select_inverse: Button,
    header_bar: Option<gtk::HeaderBar>,
    title: Option<Label>,
    large: Cell<bool>,
    /// The scroll value and whether the last scroll went backward, for the expanded label of the button.
    scroll: Cell<(f64, bool)>,
    handle: Option<tokio::runtime::Handle>,
    expansion: Rc<Cell<Expansion>>,
    ready_once: Cell<bool>,
    duplicate_open: Cell<bool>,
    shown_header: RefCell<Option<HeaderModel>>,
    /// What the description slot shows: the text and the genre chips under it.
    shown_description: RefCell<Option<(Description, Vec<GenreChip>)>>,
}

fn now_ms(clock: Instant) -> u64 {
    clock.elapsed().as_millis() as u64
}

fn page_for(manga_id: i64) -> GBox {
    let app = data::app();
    let db = app.db(EntryKind::Manga);
    let store = app.base.store().clone();
    let ctx = LoadContext {
        library_defaults: ChapterFlags(chapter_default_word(&store)),
        from_source: false,
        downloaded_only: app.base.downloaded_only.get(),
        skip_filtered: ReaderPrefs::new(store.clone()).skip_filtered.get(),
    };
    let remove_after_read = DownloadPrefs::new(&store).remove_after_marked_as_read.get();
    let handle = tokio::runtime::Handle::current();
    let runner = Rc::new(runner::spawn(&handle, DbStore::new(Arc::clone(&db)), Arc::new(StubBackend::new(db)), ctx));
    runner.send(StoreCmd::Load { manga_id });
    let mut page = Page::new(manga_id, remove_after_read);
    page.default_category = i64::from(app.library.default_manga_category.get());
    assemble(page, runner)
}

fn assemble(page: Page, runner: Rc<Runner>) -> GBox {
    ensure_css();
    // the loading, ready and missing states are three children of one stack
    let stack = gtk::Stack::new();
    let spinner = gtk::Spinner::builder().spinning(true).width_request(32).height_request(32).build();
    let loading = GBox::new(O::Vertical, 12);
    loading.set_halign(Align::Center);
    loading.set_valign(Align::Center);
    loading.append(&spinner);
    loading.append(&Label::new(Some(&tr(MsgKey::loading))));
    stack.add_named(&loading, Some("loading"));

    let missing_page = ui::status("dialog-error-symbolic", &tr(MsgKey::no_results_found));
    let back_missing = Button::with_label(&tr(MsgKey::action_webview_back));
    back_missing.set_halign(Align::Center);
    back_missing.connect_clicked(|_| ui::pop());
    missing_page.append(&back_missing);
    stack.add_named(&missing_page, Some("missing"));

    let col = ui::column();
    let header_slot = GBox::new(O::Vertical, 0);
    let description_slot = GBox::new(O::Vertical, 0);
    // the item header (ItemHeader): "N chapters" over the processed list, the missing warning, and the filter icon. A
    // click opens the filter sheet, and it is disabled while a row is selected.
    let count = Label::builder().xalign(0.0).hexpand(true).ellipsize(pango::EllipsizeMode::End).css_classes(["heading"]).build();
    let missing = Label::builder().xalign(1.0).ellipsize(pango::EllipsizeMode::End).css_classes(["dim-label", "caption"]).build();
    let header_row = GBox::new(O::Horizontal, 8);
    header_row.append(&count);
    header_row.append(&missing);
    header_row.append(&gtk::Image::from_icon_name("view-filter-symbolic"));
    let header_button = Button::builder().child(&header_row).css_classes(["flat"]).build();
    // UI parity 1: a spinner while refreshing, and a line when there are no chapters
    let spinner = gtk::Spinner::builder().halign(Align::Center).build();
    let empty = ui::dim(&tr(MsgKey::no_chapters_error));
    empty.set_halign(Align::Center);
    let on_row_view: Rc<RefCell<Weak<View>>> = Rc::default();
    // the action row (T-38a-017): library, interval (the dialog of T-38a-028), tracking and WebView. WebView shows
    // only for an HTTP source (`RowState::webview`).
    let action_row = ActionRow::new(
        Rc::new({
            let weak = Rc::clone(&on_row_view);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.open_interval();
                }
            }
        }),
        Some(Rc::new({
            let weak = Rc::clone(&on_row_view);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.tracking_pressed();
                }
            }
        })),
        Some(Rc::new({
            let weak = Rc::clone(&on_row_view);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.webview_pressed();
                }
            }
        })),
    );
    let library = action_row.library.clone();
    let swipe = crate::data::is_installed().then(|| Swipe {
        prefs: Rc::new((*data::app().library).clone()),
        run: {
            let weak = Rc::clone(&on_row_view);
            Rc::new(move |id: i64, action| {
                if let Some(view) = weak.borrow().upgrade() {
                    view.act(UiAction::Swipe { id, action });
                }
            })
        },
    });
    let list = ChapterList::new(
        {
            let weak = Rc::clone(&on_row_view);
            Rc::new(move |id: i64, mods: Mods, long: bool| {
                if let Some(view) = weak.borrow().upgrade() {
                    view.act(UiAction::Row { id, mods, long_press_or_right: long });
                }
            })
        },
        swipe,
        RowActions {
            on_indicator: {
                let weak = Rc::clone(&on_row_view);
                Rc::new(move |id, state| {
                    if let Some(view) = weak.borrow().upgrade() {
                        view.act(UiAction::Download { ids: vec![id], action: indicator_action(state) });
                    }
                })
            },
            on_previous: {
                let weak = Rc::clone(&on_row_view);
                Rc::new(move |id| {
                    if let Some(view) = weak.borrow().upgrade() {
                        view.act(UiAction::MarkPrevious(id));
                    }
                })
            },
        },
    );
    col.append(&header_slot);
    col.append(&action_row.root);
    col.append(&description_slot);
    col.append(&header_button);
    col.append(&spinner);
    col.append(&empty);
    col.append(&list.widget);
    let selection_bar = gtk::Revealer::new();

    let banner_weak: Rc<RefCell<Weak<View>>> = Rc::default();
    let banner = banner_view::build(
        {
            let weak = Rc::clone(&banner_weak);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.run_banner_action();
                }
            }
        },
        {
            let weak = Rc::clone(&banner_weak);
            move || {
                if let Some(view) = weak.borrow().upgrade() {
                    view.dismiss_banner();
                }
            }
        },
    );

    // the Resume or Start button floats over the list and fades over 150 ms
    let fab_label = Label::new(None);
    let fab_box = GBox::new(O::Horizontal, 8);
    fab_box.append(&gtk::Image::from_icon_name("media-playback-start-symbolic"));
    fab_box.append(&fab_label);
    let fab_button = Button::builder().child(&fab_box).css_classes(["suggested-action", "pill"]).build();
    fab_button.set_focus_on_click(false);
    let fab = gtk::Revealer::builder()
        .child(&fab_button)
        .transition_type(gtk::RevealerTransitionType::Crossfade)
        .transition_duration(FAB_FADE_MS)
        .halign(Align::End)
        .valign(Align::End)
        .margin_end(16)
        .margin_bottom(16)
        .build();

    let body = GBox::new(O::Vertical, 0);
    let overlay = gtk::Overlay::new();
    let scroller = ui::scrolled(&col);
    stack.add_named(&scroller, Some("ready"));
    overlay.set_child(Some(&stack));
    overlay.add_overlay(&banner.widget);
    overlay.add_overlay(&fab);
    overlay.set_vexpand(true);
    body.append(&overlay);
    body.append(&selection_bar);

    // the header bar: created disabled, enabled when the page is Ready
    let download_menu = gtk::MenuButton::builder().icon_name("folder-download-symbolic").tooltip_text(tr(MsgKey::action_download)).build();
    let filter = gtk::MenuButton::builder().icon_name("view-filter-symbolic").tooltip_text(tr(MsgKey::action_filter)).build();
    let overflow = gtk::MenuButton::builder().icon_name("view-more-symbolic").build();
    let select_all = ui::icon_button("edit-select-all-symbolic", &tr(MsgKey::action_select_all));
    let select_inverse = ui::icon_button("edit-select-invert-symbolic", &tr(MsgKey::action_select_inverse));
    select_all.set_visible(false);
    select_inverse.set_visible(false);
    let toolbar: Vec<gtk::Widget> = vec![
        download_menu.clone().upcast(),
        filter.clone().upcast(),
        overflow.clone().upcast(),
        select_all.clone().upcast(),
        select_inverse.clone().upcast(),
    ];
    let page_widget = ui::page("", &body, toolbar.clone(), false);
    ui::tag(&page_widget, "manga_entry_details");
    let header_bar = page_widget.first_child().and_downcast::<gtk::HeaderBar>();
    let title = header_bar.as_ref().and_then(|h| h.title_widget()).and_downcast::<Label>();
    // Back clears a selection before it pops, so the button is not the one `ui::page` wires to `ui::pop`
    let back = Button::from_icon_name("go-previous-symbolic");
    back.set_tooltip_text(Some(&tr(MsgKey::action_webview_back)));
    back.set_focus_on_click(false);
    if let Some(header) = &header_bar {
        header.pack_start(&back);
    }

    let view = Rc::new(View {
        page: RefCell::new(page),
        runner: Rc::clone(&runner),
        clock: Instant::now(),
        stack,
        header_slot,
        library: library.clone(),
        action_row: action_row.clone(),
        description_slot,
        header_button: header_button.clone(),
        count,
        missing,
        empty,
        spinner,
        list: Rc::clone(&list),
        scroller: scroller.clone(),
        selection_bar,
        banner,
        fab,
        fab_label,
        toolbar,
        download_menu: download_menu.clone(),
        filter: filter.clone(),
        overflow: overflow.clone(),
        select_all: select_all.clone(),
        select_inverse: select_inverse.clone(),
        header_bar,
        title,
        large: Cell::new(false),
        scroll: Cell::new((0.0, false)),
        handle: tokio::runtime::Handle::try_current().ok(),
        expansion: Rc::new(Cell::new(Expansion::new(false, false))),
        ready_once: Cell::new(false),
        duplicate_open: Cell::new(false),
        shown_header: RefCell::new(None),
        shown_description: RefCell::new(None),
    });
    let weak = Rc::downgrade(&view);
    *on_row_view.borrow_mut() = weak.clone();
    *banner_weak.borrow_mut() = weak.clone();
    view.build_selection_bar();
    view.render();

    library.connect(
        {
            let weak = weak.clone();
            move || {
                if let Some(view) = weak.upgrade() {
                    view.act(UiAction::ToggleFavorite { skip_duplicate: false });
                }
            }
        },
        {
            let weak = weak.clone();
            move || {
                if let Some(view) = weak.upgrade() {
                    view.open_category_dialog();
                }
            }
        },
    );
    header_button.connect_clicked({
        let weak = weak.clone();
        move |button| {
            if let Some(view) = weak.upgrade() {
                view.open_settings_at(button);
            }
        }
    });
    select_all.connect_clicked(on_view(&weak, |v| v.act(select_all_action())));
    select_inverse.connect_clicked(on_view(&weak, |v| v.act(select_inverse_action())));
    back.connect_clicked(on_view(&weak, |v| v.act(UiAction::Back)));
    fab_button.connect_clicked(on_view(&weak, |v| v.open_next_unread()));
    download_menu.set_create_popup_func({
        let weak = weak.clone();
        move |button| {
            if let Some(view) = weak.upgrade() {
                button.set_popover(Some(&view.download_popover()));
            }
        }
    });
    filter.set_create_popup_func({
        let weak = weak.clone();
        move |button| {
            if let Some(view) = weak.upgrade() {
                button.set_popover(Some(&view.settings_popover()));
            }
        }
    });
    overflow.set_create_popup_func({
        let weak = weak.clone();
        move |button| {
            if let Some(view) = weak.upgrade() {
                button.set_popover(Some(&view.overflow_popover()));
            }
        }
    });
    scroller.vadjustment().connect_value_changed({
        let weak = weak.clone();
        move |_| {
            if let Some(view) = weak.upgrade() {
                view.update_scroll();
            }
        }
    });
    list.widget.add_controller(key_controller(&weak));
    page_widget.add_controller(escape_controller(&weak));
    // a resize across the tablet threshold rebuilds the header for the other layout
    crate::window_size::on_tablet_change({
        let weak = weak.clone();
        move |_| {
            let Some(view) = weak.upgrade() else { return false };
            *view.shown_header.borrow_mut() = None;
            view.render();
            true
        }
    });

    // the pump owns the only strong reference to the view
    let events = runner.events();
    glib::spawn_future_local({
        let latest = LatestWins::new();
        async move {
            while let Ok(first) = events.recv().await {
                for event in runner::collect(first, &events, &latest) {
                    view.on_event(event);
                }
                view.render();
                view.take_requests();
            }
        }
    });
    // a page that leaves the stack stops its runner: subscriptions are dropped, detached writes finish
    glib::timeout_add_local(Duration::from_millis(250), {
        let weak = weak.clone();
        move || match weak.upgrade() {
            Some(view) => {
                let now = now_ms(view.clock);
                view.page.borrow_mut().banners.tick(now);
                view.render_banner();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        }
    });
    page_widget.connect_destroy(move |_| runner.send(StoreCmd::Shutdown));
    page_widget
}

fn select_all_action() -> UiAction {
    UiAction::Key { key: Key::Letter('a'), mods: Mods { shift: false, ctrl: true }, focused: None }
}

fn select_inverse_action() -> UiAction {
    UiAction::Key { key: Key::Letter('i'), mods: Mods { shift: false, ctrl: true }, focused: None }
}

/// A click handler that runs `f` while the view is alive.
fn on_view(weak: &Weak<View>, f: impl Fn(&Rc<View>) + 'static) -> impl Fn(&Button) + 'static {
    let weak = weak.clone();
    move |_| {
        if let Some(view) = weak.upgrade() {
            f(&view);
        }
    }
}

fn mods_of(state: gtk::gdk::ModifierType) -> Mods {
    Mods {
        shift: state.contains(gtk::gdk::ModifierType::SHIFT_MASK),
        ctrl: state.contains(gtk::gdk::ModifierType::CONTROL_MASK),
    }
}

/// Space, Ctrl+A, Ctrl+I and Enter on the list. A key the page has no command for goes on to GTK. Escape is the
/// page's: it clears the selection or goes back (`escape_controller`).
fn key_controller(weak: &Weak<View>) -> gtk::EventControllerKey {
    let keys = gtk::EventControllerKey::new();
    let weak = weak.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
        let mods = mods_of(state);
        let focused = view.list.focused_id();
        if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) {
            return match focused {
                Some(id) => {
                    view.act(UiAction::Row { id, mods, long_press_or_right: false });
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            };
        }
        let mapped = match key {
            gtk::gdk::Key::space => Key::Space,
            gtk::gdk::Key::a | gtk::gdk::Key::A => Key::Letter('a'),
            gtk::gdk::Key::i | gtk::gdk::Key::I => Key::Letter('i'),
            _ => return glib::Propagation::Proceed,
        };
        let (in_selection, dialog_open) = {
            let page = view.page.borrow();
            page.ready().map_or((false, false), |p| (!p.selection.is_empty(), p.dialog.current().is_some()))
        };
        if key_command(mapped, mods, in_selection, dialog_open).is_none() {
            return glib::Propagation::Proceed;
        }
        view.act(UiAction::Key { key: mapped, mods, focused });
        glib::Propagation::Stop
    });
    keys
}

/// Escape on the page: the same as the Back button. A selection is cleared first, and the page pops only when
/// nothing is selected, wherever the focus is.
fn escape_controller(weak: &Weak<View>) -> gtk::EventControllerKey {
    let keys = gtk::EventControllerKey::new();
    let weak = weak.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key != gtk::gdk::Key::Escape {
            return glib::Propagation::Proceed;
        }
        let Some(view) = weak.upgrade() else { return glib::Propagation::Proceed };
        view.act(UiAction::Back);
        glib::Propagation::Stop
    });
    keys
}

impl View {
    fn on_event(self: &Rc<Self>, event: StoreEvent) {
        let now = now_ms(self.clock);
        let moved = self.page.borrow_mut().apply(event, now);
        if let Some(id) = moved {
            self.list.focus_row(id);
        }
    }

    /// What a store event asked the widgets for: the category dialog of the add flow (E-FAV-3).
    fn take_requests(self: &Rc<Self>) {
        let ask = self.page.borrow_mut().take_category_request();
        if ask {
            self.open_category_dialog();
        }
    }

    /// Runs one action, sends its commands to the runner and redraws.
    /// The interval button (T-38a-028): the dialog of a favorite. OK and Reset store the wheel through the runner.
    fn open_interval(self: &Rc<Self>) {
        let (interval, expected, favorite) = {
            let page = self.page.borrow();
            let Some(r) = page.ready() else { return };
            let expected = (r.manga.status != kuro_domain::entries::manga::STATUS_COMPLETED).then_some(r.manga.next_update);
            (r.manga.fetch_interval, expected, r.manga.favorite)
        };
        if !interval_click_opens_dialog(favorite) {
            return;
        }
        let weak = Rc::downgrade(self);
        fetch_interval_dialog::open(interval, expected, move |wheel| {
            if let Some(view) = weak.upgrade() {
                view.act(UiAction::SetFetchInterval(wheel));
            }
        });
    }

    fn act(self: &Rc<Self>, action: UiAction) {
        let out = self.page.borrow_mut().act(action);
        for cmd in out.cmds {
            self.runner.send(cmd);
        }
        if let Some(id) = out.open_chapter {
            self.open_chapter(id);
        }
        if out.pop {
            ui::pop();
            return;
        }
        self.render();
        if let Some(id) = out.focus {
            self.list.focus_row(id);
        }
    }

    fn open_chapter(&self, id: i64) {
        let page = self.page.borrow();
        let Some(ready) = page.ready() else { return };
        let name = ready.chapters.iter().find(|c| c.id == id).map(|c: &ChapterRow| c.name.clone()).unwrap_or_default();
        crate::screens::open_media(false, &ready.manga.title, &name);
    }

    /// The Resume or Start button: opens the next unread chapter, and does nothing when there is none.
    fn open_next_unread(self: &Rc<Self>) {
        let next = self.page.borrow().ready().and_then(|p| p.next_unread());
        if let Some(id) = next {
            self.open_chapter(id);
        }
    }

    fn run_banner_action(self: &Rc<Self>) {
        let now = now_ms(self.clock);
        // the page is alive: this handler only runs while its widgets exist
        let cmd = self.page.borrow_mut().banners.activate_action(true, now);
        if let Some(cmd) = cmd {
            self.runner.send(cmd);
        }
        self.render_banner();
    }

    fn dismiss_banner(self: &Rc<Self>) {
        let now = now_ms(self.clock);
        self.page.borrow_mut().banners.dismiss(now);
        self.render_banner();
    }

    fn render_banner(&self) {
        self.banner.show(self.page.borrow().banners.current());
    }

    fn toast(&self, text: String) {
        let now = now_ms(self.clock);
        self.page.borrow_mut().toast(text, now);
        self.render_banner();
    }

    fn handlers(self: &Rc<Self>) -> Handlers {
        let weak = Rc::downgrade(self);
        Handlers {
            // the query goes to the opened screen through `handoff::take_search`
            on_search: Rc::new(|request: SearchRequest| handoff::open_search(request)),
            on_copy: Rc::new({
                let weak = weak.clone();
                move |text: String| {
                    if let Some(display) = gtk::gdk::Display::default() {
                        display.clipboard().set_text(&text);
                    }
                    if let Some(view) = weak.upgrade() {
                        view.toast(trf(MsgKey::copied_to_clipboard, &[("arg1", Arg::Str(&text))]));
                    }
                }
            }),
            on_cover: Rc::new(|| {
                let screen = ui::screen("manga_entry_details");
                if let Some(dialog) = screen.dialogs.iter().find(|d| d.id == "full_cover") {
                    ui::show_dialog(dialog);
                }
            }),
        }
    }

    /// Draws the state: which of the three children shows, the enabled toolbar, the header, the description, the rows
    /// and the banner. Each part redraws only when its input changed.
    fn render(self: &Rc<Self>) {
        let (name, enabled) = {
            let page = self.page.borrow();
            let name = match &page.state {
                LoadState::Loading => "loading",
                LoadState::Ready(_) => "ready",
                LoadState::NotFound => "missing",
            };
            (name, page.actions_enabled())
        };
        self.stack.set_visible_child_name(name);
        for w in &self.toolbar {
            w.set_sensitive(enabled);
        }
        if enabled {
            self.render_ready();
            self.sync_duplicate_dialog();
        }
        self.render_banner();
    }

    fn render_ready(self: &Rc<Self>) {
        let (model, desc, rows, refreshing, count_text, missing_text, header_enabled, title, from_source, no_chapters, bar, fab, favorite) = {
            let page = self.page.borrow();
            let Some(ready) = page.ready() else { return };
            let header = list_header(ready);
            (
                HeaderModel::new(ready),
                description(ready.manga.description.as_deref()),
                page.rows(),
                ready.is_refreshing,
                header.count,
                header.missing,
                header.enabled,
                ready.manga.title.clone(),
                ready.from_source,
                ready.chapters.is_empty(),
                // nothing tells the page that a source has settings yet, so Settings never shows
                bar_state(ready, ready.http_source, false),
                fab_state(ready),
                ready.manga.favorite,
            )
        };
        let width = self
            .stack
            .root()
            .and_downcast::<gtk::Window>()
            .map(|w| if w.width() > 0 { w.width() } else { w.default_width() })
            .filter(|w| *w > 0)
            .unwrap_or(FALLBACK_WIDTH);
        let height = self
            .stack
            .root()
            .and_downcast::<gtk::Window>()
            .map(|w| if w.height() > 0 { w.height() } else { w.default_height() })
            .filter(|h| *h > 0)
            .unwrap_or(FALLBACK_HEIGHT);
        let genres = model.genres.clone();
        let large = is_large(width, height);
        self.large.set(large);
        if !self.ready_once.replace(true) {
            self.expansion.set(Expansion::new(large, from_source));
        }
        // the action mode shows the count and only Select all and Select inverse
        let selecting = bar.menu == ActionMenu::Selection;
        if let Some(label) = &self.title {
            if selecting {
                label.set_label(&format!("{} {}", bar.selected_visible, tr(MsgKey::selected).to_lowercase()));
            } else {
                label.set_label(&title);
            }
        }
        self.download_menu.set_visible(bar.download_menu && !selecting);
        self.filter.set_visible(!selecting);
        self.overflow.set_visible(!selecting);
        self.select_all.set_visible(selecting);
        self.select_inverse.set_visible(selecting);
        if bar.filter_active {
            self.filter.add_css_class("chapter-primary");
        } else {
            self.filter.remove_css_class("chapter-primary");
        }
        if self.shown_header.borrow().as_ref() != Some(&model) {
            clear(&self.header_slot);
            let column = if large { width / 2 } else { width };
            self.header_slot.append(&header_view::build(&model, large, column, &self.handlers()));
            *self.shown_header.borrow_mut() = Some(model);
        }
        let row = self.page.borrow().ready().map(|r| RowState {
            favorite: r.manga.favorite,
            next: next_update(
                Some(r.manga.next_update),
                chrono::Utc::now().timestamp_millis(),
                r.manga.status == kuro_domain::entries::manga::STATUS_COMPLETED,
            ),
            user_interval: r.manga.fetch_interval < 0,
            tracking_count: r.tracking_count,
            has_tracking: true,
            webview: r.http_source,
        });
        match row {
            Some(row) => self.action_row.apply(&row),
            None => self.library.set(favorite),
        }
        let desc_key = (desc, genres);
        if self.shown_description.borrow().as_ref() != Some(&desc_key) {
            clear(&self.description_slot);
            let handlers = self.handlers();
            self.description_slot.append(&description_view::build(&desc_key.0, Rc::clone(&self.expansion), Some((&desc_key.1, &handlers))));
            *self.shown_description.borrow_mut() = Some(desc_key);
        }
        self.count.set_label(&count_text);
        self.missing.set_label(missing_text.as_deref().unwrap_or(""));
        self.missing.set_visible(missing_text.is_some());
        self.header_button.set_sensitive(header_enabled);
        self.spinner.set_spinning(refreshing);
        self.spinner.set_visible(refreshing);
        self.empty.set_visible(no_chapters && !refreshing);
        self.list.sync(rows);
        self.selection_bar.set_reveal_child(bar.selected_total > 0);
        self.fab_label.set_label(&tr(fab.label()));
        self.fab.set_reveal_child(fab.visible);
        self.update_scroll();
    }

    /// The scroll position drives the label of the button (`shouldExpandFAB`) and the fade of the title and the
    /// header bar background in the small layout (`MangaScreen.kt:295-308`).
    fn update_scroll(&self) {
        let adj = self.scroller.vadjustment();
        let value = adj.value();
        let (last, was_backward) = self.scroll.get();
        let backward = if (value - last).abs() > f64::EPSILON { value < last } else { was_backward };
        self.scroll.set((value, backward));
        let can_forward = value + adj.page_size() < adj.upper() - 1.0;
        self.fab_label.set_visible(crate::entry_core::anime::layout::fab_expanded(usize::from(value > 0.0), value as i32, backward, can_forward));
        let small = !self.large.get();
        let header_height = f64::from(self.header_slot.height());
        let first_visible = value < header_height;
        if let Some(label) = &self.title {
            let selecting = self.select_all.is_visible();
            label.set_opacity(if selecting || title_opaque(small, first_visible) { 1.0 } else { 0.0 });
        }
        if let Some(bar) = &self.header_bar {
            if background_opaque(small, first_visible, value > 0.0) {
                bar.remove_css_class("flat");
            } else {
                bar.add_css_class("flat");
            }
        }
    }

    /// The bar of actions on the selected chapters.
    fn build_selection_bar(self: &Rc<Self>) {
        let bar = GBox::new(O::Horizontal, 6);
        bar.add_css_class("toolbar");
        bar.set_halign(Align::Center);
        let weak = Rc::downgrade(self);
        let items: [(&str, MsgKey, UiAction); 4] = [
            ("object-select-symbolic", MsgKey::action_mark_as_read, UiAction::MarkRead(true)),
            ("edit-undo-symbolic", MsgKey::action_mark_as_unread, UiAction::MarkRead(false)),
            ("starred-symbolic", MsgKey::action_bookmark, UiAction::Bookmark(true)),
            ("non-starred-symbolic", MsgKey::action_remove_bookmark, UiAction::Bookmark(false)),
        ];
        for (icon, label, action) in items {
            let button = ui::icon_button(icon, &tr(label));
            button.connect_clicked(on_view(&weak, move |v| v.act(action.clone())));
            bar.append(&button);
        }
        self.selection_bar.set_child(Some(&bar));
    }

    /// The Download menu: the next 1, 5, 10 or 25 unread chapters, or every unread one.
    fn download_popover(self: &Rc<Self>) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let col = GBox::new(O::Vertical, 2);
        ui::margins(&col, 6);
        let weak = Rc::downgrade(self);
        for n in [1usize, 5, 10, 25] {
            let label = tr_plural(MsgKey::download_amount, n as i64, &[]);
            col.append(&plain_row(&label, &popover, on_view(&weak, move |v| v.act(UiAction::DownloadMenu(DownloadAction::Next(n))))));
        }
        col.append(&plain_row(&tr(MsgKey::download_unread), &popover, on_view(&weak, |v| v.act(UiAction::DownloadMenu(DownloadAction::Unread)))));
        popover.set_child(Some(&col));
        popover
    }

    /// The overflow menu: Refresh, Edit categories, Migrate, Share and Settings, as `toolbar::overflow_items` says.
    fn overflow_popover(self: &Rc<Self>) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let col = GBox::new(O::Vertical, 2);
        ui::margins(&col, 6);
        let weak = Rc::downgrade(self);
        let (items, refreshing) = {
            let page = self.page.borrow();
            match page.ready() {
                Some(ready) => (bar_state(ready, ready.http_source, false).overflow, ready.is_refreshing),
                None => (Vec::new(), false),
            }
        };
        for item in items {
            let (label, action): (String, fn(&Rc<View>)) = match item {
                Overflow::Refresh => (tr(MsgKey::action_webview_refresh), |v| v.act(UiAction::Refresh)),
                Overflow::EditCategories => (tr(MsgKey::action_edit_categories), View::open_category_dialog),
                Overflow::Migrate => (tr(MsgKey::action_migrate), View::migrate),
                Overflow::Share => (tr(MsgKey::action_share), View::share),
                Overflow::Settings => (tr(MsgKey::action_settings), |_| ui::open("source_preferences")),
            };
            let row = plain_row(&label, &popover, on_view(&weak, action));
            // one refresh runs at a time
            row.set_sensitive(item != Overflow::Refresh || !refreshing);
            col.append(&row);
        }
        popover.set_child(Some(&col));
        popover
    }

    /// The tracking button: the tracking sheet when a tracker is logged in, the tracking settings when none is (E-25).
    fn tracking_pressed(&self) {
        let logged_in = self.page.borrow().ready().is_some_and(|p| p.has_logged_in_trackers);
        match tracking_click(logged_in) {
            TrackingClick::Sheet => ui::open("tracking_sheet"),
            TrackingClick::Settings => ui::open("settings_tracking"),
        }
    }

    /// The exclusion sheet of the Scanlator filter row. OK writes the set it hands back.
    fn open_scanlator_sheet(self: &Rc<Self>) {
        let (available, excluded) = {
            let page = self.page.borrow();
            let Some(r) = page.ready() else { return };
            (r.available_scanlators.clone(), r.excluded_scanlators.clone())
        };
        let weak = Rc::downgrade(self);
        scanlator_sheet::open(&available, &excluded, move |set| {
            if let Some(view) = weak.upgrade() {
                view.act(UiAction::SetExcludedScanlators(set));
            }
        });
    }

    /// The WebView button (an HTTP source only): the entry's page in the WebView.
    fn webview_pressed(&self) {
        let request = self.page.borrow().ready().map(|p| WebViewRequest { url: p.manga.url.clone(), title: p.manga.title.clone() });
        if let Some(request) = request {
            handoff::open_webview(request);
        }
    }

    /// The overflow menu's Migrate: the migrate flow of this manga (a favorite only, `toolbar::overflow_items`).
    fn migrate(self: &Rc<Self>) {
        let target = self.page.borrow().ready().map(|p| (p.manga.id, p.manga.title.clone()));
        if let Some((id, title)) = target {
            self.migrate_entry(id, title);
        }
    }

    /// Opens the migrate flow for the entry `id`: the request carries its id and title, so the flow can name it, link
    /// to it and replace its page when it finishes.
    fn migrate_entry(&self, id: i64, title: String) {
        handoff::open_migrate(MigrateRequest { entry_id: id, anime: false, title });
    }

    /// Copies the link of the entry.
    fn share(self: &Rc<Self>) {
        let url = self.page.borrow().ready().map(|p| p.manga.url.clone());
        if let (Some(url), Some(display)) = (url, gtk::gdk::Display::default()) {
            display.clipboard().set_text(&url);
            self.toast(trf(MsgKey::copied_to_clipboard, &[("arg1", Arg::Str(&url))]));
        }
    }

    /// The filter sheet opened from the count row.
    fn open_settings_at(self: &Rc<Self>, anchor: &impl IsA<gtk::Widget>) {
        let popover = self.settings_popover();
        popover.set_parent(anchor);
        // the popover is parented to the anchor, so it is removed once closed
        popover.connect_closed(|p| {
            let p = p.clone();
            glib::idle_add_local_once(move || p.unparent());
        });
        popover.popup();
    }

    /// The filter, sort and display sheet: three tabs and an overflow menu with Set as default and Reset. It is built
    /// from the stored flags each time it opens.
    fn settings_popover(self: &Rc<Self>) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let (flags, downloaded_only, scanlator_active) = {
            let page = self.page.borrow();
            match page.ready() {
                Some(r) => (r.manga.flags, r.downloaded_only, scanlator_filter_active(&r.available_scanlators, &r.excluded_scanlators)),
                None => (ChapterFlags::default(), false, false),
            }
        };
        let weak = Rc::downgrade(self);
        let page_box = || {
            let b = GBox::new(O::Vertical, 2);
            ui::margins(&b, 6);
            b
        };
        let (filter_tab, sort_tab, display_tab) = (page_box(), page_box(), page_box());

        // Filter: Downloaded, Unread and Bookmarked are tri-states, and the Scanlator row opens the exclusion list. The
        // Downloaded row is disabled under downloaded-only (E-15), where the filter is forced on.
        let tri: [(String, TriState, fn(TriState) -> UiAction, bool); 3] = [
            (tr(MsgKey::label_downloaded), flags.downloaded(downloaded_only), UiAction::SetDownloaded, !downloaded_only),
            (tr(MsgKey::action_filter_unread), flags.unread(), UiAction::SetUnread, true),
            (tr(MsgKey::action_filter_bookmarked), flags.bookmarked(), UiAction::SetBookmarked, true),
        ];
        for (label, state, make, enabled) in tri {
            let icon = match state {
                TriState::DISABLED => "",
                TriState::ENABLED_IS => "object-select-symbolic",
                TriState::ENABLED_NOT => "window-close-symbolic",
            };
            let row = menu_row(&label, icon, &popover, on_view(&weak, move |v| v.act(make(state.next()))));
            row.set_sensitive(enabled);
            filter_tab.append(&row);
        }
        let scanlator = plain_row(&tr(MsgKey::scanlator), &popover, on_view(&weak, View::open_scanlator_sheet));
        if scanlator_active {
            scanlator.add_css_class("chapter-primary");
        }
        filter_tab.append(&scanlator);

        // Sort: four rows, the arrow on the active one
        let sorts = [
            (tr(MsgKey::sort_by_source), kuro_db::flags::chapter::SORTING_SOURCE),
            (tr(MsgKey::action_order_by_chapter_number), kuro_db::flags::chapter::SORTING_NUMBER),
            (tr(MsgKey::action_order_by_upload_date), kuro_db::flags::chapter::SORTING_UPLOAD_DATE),
            (tr(MsgKey::action_sort_alpha), kuro_db::flags::chapter::SORTING_ALPHABET),
        ];
        for (label, mode) in sorts {
            let active = flags.0 & kuro_db::flags::chapter::SORTING_MASK == mode;
            let icon = match (active, flags.sort_descending()) {
                (false, _) => "",
                (true, true) => "pan-down-symbolic",
                (true, false) => "pan-up-symbolic",
            };
            sort_tab.append(&menu_row(&label, icon, &popover, on_view(&weak, move |v| v.act(UiAction::SortBy(mode)))));
        }

        // Display: two radio rows, writing the display bits only
        let by_name = gtk::CheckButton::builder().label(tr(MsgKey::show_title)).build();
        let by_number = gtk::CheckButton::builder().label(tr(MsgKey::show_chapter_number)).build();
        by_number.set_group(Some(&by_name));
        if flags.display_number() {
            by_number.set_active(true);
        } else {
            by_name.set_active(true);
        }
        // a radio reports only when it becomes active, so the one it replaces sends nothing
        for (radio, number) in [(&by_name, false), (&by_number, true)] {
            let weak = weak.clone();
            let popover = popover.clone();
            radio.connect_toggled(move |r| {
                if r.is_active() {
                    popover.popdown();
                    if let Some(view) = weak.upgrade() {
                        view.act(UiAction::SetDisplayNumber(number));
                    }
                }
            });
            display_tab.append(radio);
        }

        let stack = gtk::Stack::new();
        stack.add_titled(&filter_tab, Some("filter"), &tr(MsgKey::action_filter));
        stack.add_titled(&sort_tab, Some("sort"), &tr(MsgKey::action_sort));
        stack.add_titled(&display_tab, Some("display"), &tr(MsgKey::action_display));
        let switcher = gtk::StackSwitcher::builder().stack(&stack).hexpand(true).build();
        let menu = gtk::MenuButton::builder().icon_name("view-more-symbolic").css_classes(["flat"]).build();
        menu.set_popover(Some(&self.defaults_popover(&popover)));
        let top = GBox::new(O::Horizontal, 6);
        top.append(&switcher);
        top.append(&menu);
        let col = GBox::new(O::Vertical, 4);
        ui::margins(&col, 6);
        col.append(&top);
        col.append(&stack);
        popover.set_child(Some(&col));
        popover
    }

    /// The overflow menu of the sheet: Set as default and Reset.
    fn defaults_popover(self: &Rc<Self>, sheet: &gtk::Popover) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let col = GBox::new(O::Vertical, 2);
        ui::margins(&col, 6);
        let weak = Rc::downgrade(self);
        let close = {
            let (popover, sheet) = (popover.clone(), sheet.clone());
            move || {
                popover.popdown();
                sheet.popdown();
            }
        };
        let set_default = Button::builder().label(tr(MsgKey::set_chapter_settings_as_default)).css_classes(["flat"]).build();
        set_default.connect_clicked({
            let (weak, close) = (weak.clone(), close.clone());
            move |_| {
                close();
                if let Some(view) = weak.upgrade() {
                    view.confirm_set_default();
                }
            }
        });
        let reset = Button::builder().label(tr(MsgKey::action_reset)).css_classes(["flat"]).build();
        reset.connect_clicked(move |_| {
            close();
            if let Some(view) = weak.upgrade() {
                view.act(UiAction::ResetFlags);
            }
        });
        col.append(&set_default);
        col.append(&reset);
        popover.set_child(Some(&col));
        popover
    }

    /// "Set as default": a confirm dialog with the library checkbox. OK stores the flags of this manga as the library
    /// default, and the checkbox also applies them to every favorite on a detached task.
    fn confirm_set_default(self: &Rc<Self>) {
        let Some(flags) = self.page.borrow().ready().map(|p| p.manga.flags) else { return };
        let also = gtk::CheckButton::builder().label(tr(MsgKey::also_set_chapter_settings_for_library)).build();
        let ok = Button::builder().label(tr(MsgKey::action_ok)).css_classes(["suggested-action"]).build();
        let cancel = Button::with_label(&tr(MsgKey::action_cancel));
        let buttons = GBox::new(O::Horizontal, 8);
        buttons.set_halign(Align::End);
        buttons.append(&cancel);
        buttons.append(&ok);
        let col = ui::column();
        col.append(&Label::builder().label(tr(MsgKey::confirm_set_chapter_settings)).wrap(true).xalign(0.0).build());
        col.append(&also);
        col.append(&buttons);
        let window = ui::modal(Label::new(Some(&tr(MsgKey::chapter_settings))).upcast_ref(), &col, 360, 200);
        cancel.connect_clicked({
            let window = window.clone();
            move |_| window.close()
        });
        let weak = Rc::downgrade(self);
        ok.connect_clicked({
            let window = window.clone();
            move |_| {
                window.close();
                if let Some(view) = weak.upgrade() {
                    view.save_default(flags, also.is_active());
                }
            }
        });
        window.present();
    }

    fn save_default(self: &Rc<Self>, flags: ChapterFlags, apply_all: bool) {
        if !data::is_installed() {
            return;
        }
        let app = data::app();
        let store = app.base.store().clone();
        let message = save_as_default(&store, flags);
        if apply_all && let Some(handle) = &self.handle {
            let repo = kuro_data::manga::MangaRepo::new(app.db(EntryKind::Manga));
            let (done, count) = async_channel::bounded::<usize>(1);
            // detached: closing the dialog or the page does not stop the run (`withNonCancellableContext`)
            handle.spawn(async move {
                match kuro_domain::entries::defaults::chapter_defaults_to_library_detached(repo, store).await {
                    Ok(updated) => {
                        let _ = done.send(updated).await;
                    }
                    Err(e) => tracing::warn!("could not apply the default chapter settings to the library: {e}"),
                }
            });
            // the message says how many favorites took the settings, so it waits for the run (E-DEF-3)
            let weak = Rc::downgrade(self);
            glib::spawn_future_local(async move {
                if let (Ok(updated), Some(view)) = (count.recv().await, weak.upgrade()) {
                    view.toast(applied_to_library_text(&message, updated));
                }
            });
            return;
        }
        self.toast(message);
    }

    /// The duplicate dialog of the library button: Add anyway, or Migrate existing entry. Cancel keeps nothing written.
    fn sync_duplicate_dialog(self: &Rc<Self>) {
        let open = matches!(self.page.borrow().ready().and_then(|p| p.dialog.current()), Some(Dialog::DuplicateManga { .. }));
        if !open || self.duplicate_open.replace(true) {
            return;
        }
        let duplicate_id = match self.page.borrow().ready().and_then(|p| p.dialog.current()) {
            Some(Dialog::DuplicateManga { duplicate_id, .. }) => *duplicate_id,
            _ => return,
        };
        let show = Button::with_label(&tr(MsgKey::action_show_manga));
        let add = Button::builder().label(tr(MsgKey::action_add_anyway)).css_classes(["suggested-action"]).build();
        let migrate = Button::with_label(&tr(MsgKey::action_migrate_duplicate));
        let cancel = Button::with_label(&tr(MsgKey::action_cancel));
        let buttons = GBox::new(O::Horizontal, 8);
        buttons.set_halign(Align::End);
        for b in [&cancel, &show, &migrate, &add] {
            buttons.append(b);
        }
        let col = ui::column();
        col.append(&Label::builder().label(tr(MsgKey::confirm_add_duplicate_manga)).wrap(true).xalign(0.0).build());
        col.append(&buttons);
        let window = ui::modal(Label::new(Some(&tr(MsgKey::are_you_sure))).upcast_ref(), &col, 400, 220);
        // Show entry dismisses the sheet first, then pushes the duplicate page on top (E-24)
        show.connect_clicked({
            let window = window.clone();
            move |_| {
                window.close();
                crate::entry_core::route::open(duplicate_id);
            }
        });
        cancel.connect_clicked({
            let window = window.clone();
            move |_| window.close()
        });
        migrate.connect_clicked({
            let (window, weak) = (window.clone(), Rc::downgrade(self));
            move |_| {
                window.close();
                if let Some(view) = weak.upgrade() {
                    // one primary-key read names the old entry for the migrate dialog
                    let title = kuro_data::manga::MangaRepo::new(data::app().db(EntryKind::Manga)).by_id(duplicate_id).map(|m| m.title).unwrap_or_default();
                    view.migrate_entry(duplicate_id, title);
                }
            }
        });
        let weak = Rc::downgrade(self);
        add.connect_clicked({
            let (window, weak) = (window.clone(), weak.clone());
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.act(UiAction::ToggleFavorite { skip_duplicate: true });
                }
                window.close();
            }
        });
        // every way out lets the page show the dialog again next time
        window.connect_close_request(move |_| {
            if let Some(view) = weak.upgrade() {
                view.duplicate_open.set(false);
                view.act(UiAction::DismissDialog);
            }
            glib::Propagation::Proceed
        });
        window.present();
    }

    /// The change-categories dialog (add flow for a manga outside the library).
    fn open_category_dialog(self: &Rc<Self>) {
        let model = {
            let page = self.page.borrow();
            let Some(ready) = page.ready() else { return };
            CategoryDialog::new(ready.manga.id, !ready.manga.favorite, &ready.categories, &ready.category_membership)
        };
        self.act(UiAction::OpenDialog(Dialog::ChangeCategory(model.clone())));
        let model = Rc::new(RefCell::new(model));
        let list = ui::boxed_list();
        for (i, item) in model.borrow().items.iter().enumerate() {
            let check = gtk::CheckButton::builder().label(&item.name).active(item.checked).margin_start(8).margin_end(8).build();
            let model = Rc::clone(&model);
            check.connect_toggled(move |_| model.borrow_mut().toggle(i));
            list.append(&check);
        }
        let ok = Button::builder().label(tr(MsgKey::action_ok)).css_classes(["suggested-action"]).build();
        let col = ui::column();
        col.append(&list);
        col.append(&ok);
        let title = Label::new(Some(&tr(MsgKey::action_edit_categories)));
        let window = ui::modal(title.upcast_ref(), &col, 360, 420);
        let weak = Rc::downgrade(self);
        ok.connect_clicked({
            let (window, weak, model) = (window.clone(), weak.clone(), Rc::clone(&model));
            move |_| {
                if let Some(view) = weak.upgrade() {
                    view.act(UiAction::ConfirmCategory(model.borrow().confirm()));
                }
                window.close();
            }
        });
        // any other way out (Escape, the close button) leaves without writing
        window.connect_close_request(move |_| {
            if let Some(view) = weak.upgrade() {
                view.act(UiAction::DismissDialog);
            }
            glib::Propagation::Proceed
        });
        window.present();
    }
}

/// The message after "Set as default" with the library checkbox: the saved text and how many favorites took it.
fn applied_to_library_text(saved: &str, count: usize) -> String {
    format!("{saved} ({count})")
}

/// A row of a menu with no mark: it closes the popover and runs.
fn plain_row(label: &str, popover: &gtk::Popover, on_click: impl Fn(&Button) + 'static) -> Button {
    let button = Button::builder().css_classes(["flat"]).build();
    button.set_child(Some(&Label::builder().label(label).xalign(0.0).build()));
    let popover = popover.clone();
    button.connect_clicked(move |b| {
        popover.popdown();
        on_click(b);
    });
    button
}

fn menu_row(label: &str, icon: &str, popover: &gtk::Popover, on_click: impl Fn(&Button) + 'static) -> Button {
    let row = GBox::new(O::Horizontal, 12);
    row.append(&Label::builder().label(label).xalign(0.0).hexpand(true).build());
    let mark = gtk::Image::new();
    mark.set_pixel_size(16);
    if icon.is_empty() {
        mark.set_opacity(0.0);
        mark.set_icon_name(Some("object-select-symbolic"));
    } else {
        mark.set_icon_name(Some(icon));
    }
    row.append(&mark);
    let button = Button::builder().child(&row).css_classes(["flat"]).build();
    let popover = popover.clone();
    button.connect_clicked(move |b| {
        popover.popdown();
        on_click(b);
    });
    button
}

fn clear(slot: &GBox) {
    while let Some(child) = slot.first_child() {
        slot.remove(&child);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    #[test]
    fn applying_to_the_library_reports_the_count_after_the_saved_text() {
        assert_eq!(super::applied_to_library_text("Updated default chapter settings", 3), "Updated default chapter settings (3)");
    }

    use kuro_data::chapter::{Chapter, ChapterRepo};
    use kuro_data::manga::{Manga, MangaRepo, MangaUpdate};
    use kuro_db::handle::Db;

    use super::*;
    use crate::entry_core::backend::StubBackend;

    fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut out = vec![root.clone()];
        let mut child = root.first_child();
        while let Some(c) = child {
            out.extend(descendants(&c));
            child = c.next_sibling();
        }
        out
    }

    fn pump(ctx: &glib::MainContext, done: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            while ctx.iteration(false) {}
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        done()
    }

    /// One display test (GTK is bound to the thread that starts it): Loading, then Ready with rows, a stub warning in
    /// the header, a non-focusable banner, and Missing with a Back button.
    #[test]
    #[ignore = "needs a display; run alone: cargo test -p kurobayomi entry_route_smoke -- --ignored"]
    fn entry_route_smoke() {
        if gtk::init().is_err() {
            eprintln!("skipped: no display for the entry route smoke test");
            return;
        }
        let ctx = glib::MainContext::default();
        let _owner = ctx.acquire().expect("main context");
        let rt = kuro_core::runtime::build().expect("runtime");
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("manga schema");
        let db = Arc::new(Db::new(&path, writer).expect("manga handle"));
        let mangas = MangaRepo::new(Arc::clone(&db));
        let id = mangas.insert(&Manga { source: 7, url: "/m".into(), title: "Berserk".into(), favorite: true, ..Manga::create() }).unwrap();
        mangas.update(&MangaUpdate { initialized: Some(true), ..MangaUpdate::new(id) }).unwrap();
        let chapters: Vec<Chapter> = (1..=3)
            .map(|n| Chapter { manga_id: id, url: format!("/c{n}"), name: format!("Chapter {n}"), chapter_number: f64::from(n), source_order: i64::from(3 - n), ..Chapter::create() })
            .collect();
        ChapterRepo::new(Arc::clone(&db)).add_all(&chapters).unwrap();

        let spawn = |manga_id| {
            let runner = Rc::new(runner::spawn(rt.handle(), DbStore::new(Arc::clone(&db)), Arc::new(StubBackend::new(Arc::clone(&db))), LoadContext::default()));
            let page = assemble(Page::new(manga_id, false), Rc::clone(&runner));
            runner.send(StoreCmd::Load { manga_id });
            page
        };
        let stack_of = |page: &GBox| descendants(page.upcast_ref()).into_iter().find_map(|w| w.downcast::<gtk::Stack>().ok()).expect("the page has a stack");
        let enabled = |page: &GBox| {
            let header = page.first_child().and_downcast::<gtk::HeaderBar>().expect("header bar");
            descendants(header.upcast_ref()).into_iter().filter_map(|w| w.downcast::<Button>().ok()).filter(|b| b.is_sensitive()).count()
        };

        let page = spawn(id);
        let stack = stack_of(&page);
        assert_eq!(stack.visible_child_name().as_deref(), Some("loading"), "loading until the first read");
        let loading_buttons = enabled(&page);
        assert_eq!(loading_buttons, 1, "only Back works while Loading");
        assert!(pump(&ctx, || stack.visible_child_name().as_deref() == Some("ready")), "the page becomes ready");
        assert!(enabled(&page) > loading_buttons, "toolbar actions work once Ready");
        let all = descendants(page.upcast_ref());
        let list = all.iter().find_map(|w| w.clone().downcast::<gtk::ListView>().ok()).expect("the list view");
        assert!(pump(&ctx, || list.model().is_some_and(|m| m.n_items() == 3)), "three rows in the model");
        let stub = all.iter().filter_map(|w| w.clone().downcast::<gtk::Image>().ok()).any(|i| i.icon_name().as_deref() == Some("dialog-warning-symbolic") && i.has_css_class("error"));
        assert!(stub, "a stub source shows the warning icon in the error color");
        // the count row, the Resume button and the filter sheet
        let texts = |root: &gtk::Widget| -> Vec<String> { descendants(root).into_iter().filter_map(|w| w.downcast::<Label>().ok()).map(|l| l.label().to_string()).collect() };
        assert!(texts(page.upcast_ref()).iter().any(|t| t == "3 chapters"), "the count row reads the processed list");
        assert!(texts(page.upcast_ref()).iter().any(|t| t == "In library"), "the library button of a favorite");
        let fab = all.iter().filter_map(|w| w.clone().downcast::<gtk::Revealer>().ok()).find(|r| r.transition_duration() == FAB_FADE_MS).expect("the Resume button");
        assert!(fab.reveals_child(), "unread chapters and nothing selected: the button shows");
        assert!(texts(fab.upcast_ref()).iter().any(|t| t == "Start"), "no chapter is read, so Start");
        let window = gtk::Window::builder().default_width(900).default_height(700).child(&page).build();
        window.present();
        let filter = all.iter().filter_map(|w| w.clone().downcast::<gtk::MenuButton>().ok()).find(|b| b.tooltip_text().as_deref() == Some("Filter")).expect("the Filter button");
        filter.popup();
        let sheet = filter.popover().expect("the filter sheet");
        let labels = texts(sheet.upcast_ref());
        for want in ["Filter", "Sort", "Display", "Downloaded", "Unread", "Bookmarked", "Scanlator", "By source", "Source title", "Chapter number"] {
            assert!(labels.iter().any(|t| t == want), "the sheet has {want}: {labels:?}");
        }
        filter.popdown();
        window.set_child(None::<&gtk::Widget>);
        window.close();
        let banner = banner_view::build(|| {}, || {});
        assert!(!banner.widget.is_focusable() && !banner.widget.can_focus());

        // the list model: one items_changed for a reset, one per changed row
        let rows: Vec<crate::entry_core::list_model::RowItem> = (1..=3)
            .map(|id| crate::entry_core::list_model::RowItem {
                id,
                title: format!("c{id}"),
                subtitle: String::new(),
                read: false,
                bookmark: false,
                selected: false,
                download: kuro_downloads::DownloadState::NotDownloaded,
                progress: 0,
                indicator: true,
                missing: None,
            })
            .collect();
        let model_list = ChapterList::new(
            Rc::new(|_: i64, _: Mods, _: bool| {}),
            None,
            RowActions { on_indicator: Rc::new(|_, _| {}), on_previous: Rc::new(|_| {}) },
        );
        let signals = Rc::new(Cell::new(0));
        model_list.widget.model().expect("model").connect_items_changed({
            let signals = Rc::clone(&signals);
            move |_, _, _, _| signals.set(signals.get() + 1)
        });
        model_list.sync(rows.clone());
        assert_eq!(signals.get(), 1, "a new list is one items_changed");
        let mut changed = rows.clone();
        changed[0].selected = true;
        changed[2].progress = 40;
        model_list.sync(changed.clone());
        assert_eq!(signals.get(), 3, "one items_changed per changed row");
        model_list.sync(changed.clone());
        assert_eq!(signals.get(), 3, "an identical list changes nothing");
        model_list.sync(changed[..2].to_vec());
        assert_eq!(signals.get(), 4, "a removed row is one more");

        let missing = spawn(4242);
        let stack = stack_of(&missing);
        assert!(pump(&ctx, || stack.visible_child_name().as_deref() == Some("missing")), "a missing row ends in the missing state");
        let has_back = descendants(missing.upcast_ref()).iter().filter_map(|w| w.clone().downcast::<Button>().ok()).any(|b| b.label().as_deref() == Some("Back"));
        assert!(has_back, "the missing state offers Back");
    }
}
