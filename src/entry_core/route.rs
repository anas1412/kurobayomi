//! The manga details route: a thin GTK page over `Page` (the controller) and `Runner` (the store). It shows a
//! centered loading state until the first read arrives, the page when it is `Ready`, and a "not found" state with a
//! Back button when the row is missing. Toolbar actions stay disabled while `Loading`.
//! `ui::screen("manga_entry_details")` resolves here through `build`, and `open(id)` pushes a page for one manga.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O, glib};
use kuro_domain::download::prefs::DownloadPrefs;
use kuro_domain::entries::defaults::chapter_default_word;
use kuro_domain::kind::EntryKind;
use kuro_i18n::{Arg, MsgKey, tr, trf};
use kuro_prefs::reader::ReaderPrefs;
use kuro_prefs::tristate::TriState;

use crate::data;
use crate::entry_core::backend::StubBackend;
use crate::entry_core::banner_view::{self, BannerView};
use crate::entry_core::chapter::ChapterRow;
use crate::entry_core::description_view;
use crate::entry_core::detail::{LatestWins, LoadContext, LoadState, StoreCmd, StoreEvent};
use crate::entry_core::dialog::{CategoryDialog, Dialog};
use crate::entry_core::flags::ChapterFlags;
use crate::entry_core::header::{HeaderModel, SearchRequest, is_large};
use crate::entry_core::header_view::{self, Handlers};
use crate::entry_core::list_view::ChapterList;
use crate::entry_core::page::{Page, UiAction};
use crate::entry_core::runner::{self, Runner};
use crate::entry_core::selection::{Key, Mods, key_command};
use crate::entry_core::store::DbStore;
use crate::entry_core::text::{Description, Expansion, description};
use crate::ui;

/// The window width assumed before the page has a window: the app's default width.
const FALLBACK_WIDTH: i32 = 1180;

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
    description_slot: GBox,
    count: Label,
    empty: Label,
    spinner: gtk::Spinner,
    list: Rc<ChapterList>,
    selection_bar: gtk::Revealer,
    banner: BannerView,
    toolbar: Vec<gtk::Widget>,
    refresh: Button,
    title: Option<Label>,
    expansion: Rc<Cell<Expansion>>,
    ready_once: Cell<bool>,
    shown_header: RefCell<Option<HeaderModel>>,
    shown_description: RefCell<Option<Description>>,
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
    assemble(Page::new(manga_id, remove_after_read), runner)
}

fn assemble(page: Page, runner: Rc<Runner>) -> GBox {
    // the loading, ready and missing states are three children of one stack
    let stack = gtk::Stack::new();
    let spinner = gtk::Spinner::builder().spinning(true).width_request(32).height_request(32).build();
    let loading = GBox::new(O::Vertical, 12);
    loading.set_halign(Align::Center);
    loading.set_valign(Align::Center);
    loading.append(&spinner);
    loading.append(&Label::new(Some(&tr(MsgKey::loading))));
    stack.add_named(&loading, Some("loading"));

    let missing = ui::status("dialog-error-symbolic", &tr(MsgKey::no_results_found));
    let back = Button::with_label(&tr(MsgKey::action_webview_back));
    back.set_halign(Align::Center);
    back.connect_clicked(|_| ui::pop());
    missing.append(&back);
    stack.add_named(&missing, Some("missing"));

    let col = ui::column();
    let header_slot = GBox::new(O::Vertical, 0);
    let description_slot = GBox::new(O::Vertical, 0);
    let count = ui::heading("");
    // UI parity 1: a spinner while refreshing, and a line when there are no chapters
    let spinner = gtk::Spinner::builder().halign(Align::Center).build();
    let empty = ui::dim(&tr(MsgKey::no_chapters_error));
    empty.set_halign(Align::Center);
    let on_row_view: Rc<RefCell<Weak<View>>> = Rc::default();
    let list = ChapterList::new({
        let weak = Rc::clone(&on_row_view);
        Rc::new(move |id: i64, mods: Mods, long: bool| {
            if let Some(view) = weak.borrow().upgrade() {
                view.act(UiAction::Row { id, mods, long_press_or_right: long });
            }
        })
    });
    col.append(&header_slot);
    col.append(&description_slot);
    col.append(&count);
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

    let body = GBox::new(O::Vertical, 0);
    let overlay = gtk::Overlay::new();
    let scroller = ui::scrolled(&col);
    stack.add_named(&scroller, Some("ready"));
    overlay.set_child(Some(&stack));
    overlay.add_overlay(&banner.widget);
    overlay.set_vexpand(true);
    body.append(&overlay);
    body.append(&selection_bar);

    // toolbar: created disabled, enabled when the page is Ready
    let refresh = ui::icon_button("view-refresh-symbolic", &tr(MsgKey::action_webview_refresh));
    let filter = gtk::MenuButton::builder().icon_name("view-filter-symbolic").tooltip_text(tr(MsgKey::action_filter)).build();
    let categories = ui::icon_button("emblem-favorite-symbolic", &tr(MsgKey::action_edit_categories));
    let toolbar: Vec<gtk::Widget> = vec![refresh.clone().upcast(), filter.clone().upcast(), categories.clone().upcast()];
    let end = toolbar.clone();
    let page_widget = ui::page("", &body, end, true);
    ui::tag(&page_widget, "manga_entry_details");
    let title = page_widget.first_child().and_downcast::<gtk::HeaderBar>().and_then(|h| h.title_widget()).and_downcast::<Label>();

    let view = Rc::new(View {
        page: RefCell::new(page),
        runner: Rc::clone(&runner),
        clock: Instant::now(),
        stack,
        header_slot,
        description_slot,
        count,
        empty,
        spinner,
        list: Rc::clone(&list),
        selection_bar,
        banner,
        toolbar,
        refresh: refresh.clone(),
        title,
        expansion: Rc::new(Cell::new(Expansion::new(false, false))),
        ready_once: Cell::new(false),
        shown_header: RefCell::new(None),
        shown_description: RefCell::new(None),
    });
    let weak = Rc::downgrade(&view);
    *on_row_view.borrow_mut() = weak.clone();
    *banner_weak.borrow_mut() = weak.clone();
    view.build_selection_bar();
    view.render();

    refresh.connect_clicked(on_view(&weak, |v| v.act(UiAction::Refresh)));
    categories.connect_clicked(on_view(&weak, |v| v.open_category_dialog()));
    filter.set_create_popup_func({
        let weak = weak.clone();
        move |button| {
            if let Some(view) = weak.upgrade() {
                button.set_popover(Some(&view.settings_popover()));
            }
        }
    });
    list.widget.add_controller(key_controller(&weak));

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

/// Space, Escape, Ctrl+A, Ctrl+I and Enter on the list. A key the page has no command for goes on to GTK.
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
            gtk::gdk::Key::Escape => Key::Escape,
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

impl View {
    fn on_event(self: &Rc<Self>, event: StoreEvent) {
        let now = now_ms(self.clock);
        let moved = self.page.borrow_mut().apply(event, now);
        if let Some(id) = moved {
            self.list.focus_row(id);
        }
    }

    /// Runs one action, sends its commands to the runner and redraws.
    fn act(self: &Rc<Self>, action: UiAction) {
        let out = self.page.borrow_mut().act(action);
        for cmd in out.cmds {
            self.runner.send(cmd);
        }
        if let Some(id) = out.open_chapter {
            self.open_chapter(id);
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

    fn handlers(self: &Rc<Self>) -> Handlers {
        let weak = Rc::downgrade(self);
        Handlers {
            on_search: Rc::new(|request: SearchRequest| {
                // ponytail: the search screens take no query argument yet, so this opens them
                ui::open(if request.global { "global_search" } else { "browse_source" });
            }),
            on_copy: Rc::new({
                let weak = weak.clone();
                move |text: String| {
                    if let Some(display) = gtk::gdk::Display::default() {
                        display.clipboard().set_text(&text);
                    }
                    if let Some(view) = weak.upgrade() {
                        let message = trf(MsgKey::copied_to_clipboard, &[("arg1", Arg::Str(&text))]);
                        view.page.borrow_mut().toast(message, now_ms(view.clock));
                        view.render_banner();
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
        }
        self.render_banner();
    }

    fn render_ready(self: &Rc<Self>) {
        let (model, desc, rows, refreshing, selected, count_text, title, from_source, no_chapters) = {
            let page = self.page.borrow();
            let Some(ready) = page.ready() else { return };
            let rows = page.rows();
            let count = format!("{} {}", ready.chapters.len(), tr(MsgKey::chapters));
            (
                HeaderModel::new(ready),
                description(ready.manga.description.as_deref()),
                rows,
                ready.is_refreshing,
                ready.selection.len(),
                count,
                ready.manga.title.clone(),
                ready.from_source,
                ready.chapters.is_empty(),
            )
        };
        let width = self
            .stack
            .root()
            .and_downcast::<gtk::Window>()
            .map(|w| if w.width() > 0 { w.width() } else { w.default_width() })
            .filter(|w| *w > 0)
            .unwrap_or(FALLBACK_WIDTH);
        let large = is_large(width);
        if !self.ready_once.replace(true) {
            self.expansion.set(Expansion::new(large, from_source));
        }
        if let Some(label) = &self.title {
            label.set_label(&title);
        }
        if self.shown_header.borrow().as_ref() != Some(&model) {
            clear(&self.header_slot);
            let column = if large { width / 2 } else { width };
            self.header_slot.append(&header_view::build(&model, large, column, &self.handlers()));
            *self.shown_header.borrow_mut() = Some(model);
        }
        if self.shown_description.borrow().as_ref() != Some(&desc) {
            clear(&self.description_slot);
            self.description_slot.append(&description_view::build(&desc, Rc::clone(&self.expansion)));
            *self.shown_description.borrow_mut() = Some(desc);
        }
        self.count.set_label(&count_text);
        self.refresh.set_sensitive(!refreshing);
        self.spinner.set_spinning(refreshing);
        self.spinner.set_visible(refreshing);
        self.empty.set_visible(no_chapters && !refreshing);
        self.list.sync(rows);
        self.selection_bar.set_reveal_child(selected > 0);
    }

    /// The bar of actions on the selected chapters.
    fn build_selection_bar(self: &Rc<Self>) {
        let bar = GBox::new(O::Horizontal, 6);
        bar.add_css_class("toolbar");
        bar.set_halign(Align::Center);
        let weak = Rc::downgrade(self);
        let items: [(&str, MsgKey, UiAction); 7] = [
            ("object-select-symbolic", MsgKey::action_mark_as_read, UiAction::MarkRead(true)),
            ("edit-undo-symbolic", MsgKey::action_mark_as_unread, UiAction::MarkRead(false)),
            ("starred-symbolic", MsgKey::action_bookmark, UiAction::Bookmark(true)),
            ("non-starred-symbolic", MsgKey::action_remove_bookmark, UiAction::Bookmark(false)),
            ("edit-select-all-symbolic", MsgKey::action_select_all, UiAction::Key { key: Key::Letter('a'), mods: Mods { shift: false, ctrl: true }, focused: None }),
            ("view-reveal-symbolic", MsgKey::action_select_inverse, UiAction::Key { key: Key::Letter('i'), mods: Mods { shift: false, ctrl: true }, focused: None }),
            ("window-close-symbolic", MsgKey::action_close, UiAction::Key { key: Key::Escape, mods: Mods::default(), focused: None }),
        ];
        for (icon, label, action) in items {
            let button = ui::icon_button(icon, &tr(label));
            button.connect_clicked(on_view(&weak, move |v| v.act(action.clone())));
            bar.append(&button);
        }
        self.selection_bar.set_child(Some(&bar));
    }

    /// The filter, sort and display popover, built from the stored flags each time it opens.
    fn settings_popover(self: &Rc<Self>) -> gtk::Popover {
        let popover = gtk::Popover::new();
        let col = GBox::new(O::Vertical, 2);
        ui::margins(&col, 6);
        let (flags, downloaded_only) = {
            let page = self.page.borrow();
            match page.ready() {
                Some(r) => (r.manga.flags, r.downloaded_only),
                None => (ChapterFlags::default(), false),
            }
        };
        let weak = Rc::downgrade(self);
        let tri: [(String, TriState, fn(TriState) -> UiAction); 3] = [
            (tr(MsgKey::action_filter_unread), flags.unread(), UiAction::SetUnread),
            (tr(MsgKey::label_downloaded), flags.downloaded(downloaded_only), UiAction::SetDownloaded),
            (tr(MsgKey::action_filter_bookmarked), flags.bookmarked(), UiAction::SetBookmarked),
        ];
        for (label, state, make) in tri {
            let icon = match state {
                TriState::DISABLED => "",
                TriState::ENABLED_IS => "object-select-symbolic",
                TriState::ENABLED_NOT => "window-close-symbolic",
            };
            col.append(&menu_row(&label, icon, &popover, on_view(&weak, move |v| v.act(make(state.next())))));
        }
        col.append(&gtk::Separator::new(O::Horizontal));
        let sorts = [
            ("By source".to_owned(), kuro_db::flags::chapter::SORTING_SOURCE),
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
            col.append(&menu_row(&label, icon, &popover, on_view(&weak, move |v| v.act(UiAction::SortBy(mode)))));
        }
        col.append(&gtk::Separator::new(O::Horizontal));
        for (label, number) in [("Source title", false), ("Chapter number", true)] {
            let icon = if flags.display_number() == number { "object-select-symbolic" } else { "" };
            col.append(&menu_row(label, icon, &popover, on_view(&weak, move |v| v.act(UiAction::SetDisplayNumber(number)))));
        }
        popover.set_child(Some(&col));
        popover
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
            .map(|n| Chapter { manga_id: id, url: format!("/c{n}"), name: format!("Chapter {n}"), chapter_number: f64::from(n), source_order: i64::from(n), ..Chapter::create() })
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
            })
            .collect();
        let model_list = ChapterList::new(Rc::new(|_: i64, _: Mods, _: bool| {}));
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
