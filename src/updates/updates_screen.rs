//! The Updates screen of one kind (T-42a-030, T-42a-034, T-42a-036): the feed under date headers, the selection gestures
//! (long press, tap while selecting, Ctrl and Shift click, Escape) and the bulk bar (bookmark, fillermark, read
//! state, download, delete, play). Each row shows its download state and offers the indicator's actions. In selection
//! mode the cover tap is off and the bar shows; there is no search field.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use std::collections::HashMap;
use std::time::Duration;

use chrono::{Local, Months};
use gtk::gdk::ModifierType;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Box as GBox, Button, CheckButton, GestureClick, GestureLongPress, Image, Label, ListBox, Orientation as O};
use kuro_downloads::download_prefs::DownloadPrefs;
use kuro_domain::dates::ListRow;
use kuro_domain::kind::EntryKind;
use kuro_domain::updates::repo::UpdatesRepo;
use kuro_images::art::{ArtKind, ArtRef, Owner};
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::ui::UiPrefs;

use crate::data;
use crate::entry_core::cover_view::CoverView;
use crate::updates::bottom_bar::{self, BarAction, BarCommand, Selected};
use crate::updates::bulk::{self, BulkAction};
use crate::updates::coalesce::watch_coalesced;
use crate::updates::download_actions::{self, DownloadAction, EntryFacts, UnitTarget};
use crate::updates::download_exec;
use crate::updates::download_host::{self, UnitEvent};
use crate::updates::download_state::{Change, DiskCache, DownloadState};
use crate::updates::feed::{self, FeedRow, Gesture, Press};
use crate::updates::format;
use crate::updates::history::COALESCE_MS;
use crate::updates::{copy, open_route, qualities};
use crate::updates::screen_toast::ScreenToast;
use crate::updates::selection::{Selection, is_selection_mode};
use crate::updates::state_page::PageStates;
use crate::updates::updates_dialog::confirm_bulk_delete;
use crate::updates::updates_page::{self, BackAction, UpdatesLoad, back_action, cover_click_enabled};

/// How long the screen's toasts stay (the Updates failure and the bar's errors).
const TOAST_MS: u64 = 4_000;

/// The gesture of a click: Ctrl and Shift change what a tap means.
fn click_gesture(state: ModifierType) -> Gesture {
    if state.contains(ModifierType::CONTROL_MASK) {
        Gesture::CtrlClick
    } else if state.contains(ModifierType::SHIFT_MASK) {
        Gesture::ShiftClick
    } else {
        Gesture::Tap
    }
}

pub struct UpdatesScreen {
    kind: EntryKind,
    root: gtk::Overlay,
    bar: GBox,
    col: GBox,
    pages: PageStates,
    toast: ScreenToast,
    load: RefCell<UpdatesLoad<FeedRow>>,
    selection: RefCell<Selection>,
    watch: RefCell<Option<glib::JoinHandle<()>>>,
    /// The watches on the download hub (unit events and cache or queue changes), alive as long as the screen.
    download_watch: RefCell<Vec<glib::JoinHandle<()>>>,
    /// Disk checks per unit, dropped on each download-cache signal (T-42b-003).
    disk: RefCell<DiskCache>,
    /// Row position by unit id, rebuilt with each new list, so a download event finds its row (T-42b-040).
    index: RefCell<HashMap<i64, usize>>,
    /// The anime "use external downloader" preference, read once at construction (T-42b-004).
    external_downloader: bool,
    refresh_pending: std::cell::Cell<bool>,
    invalidate_pending: std::cell::Cell<bool>,
}

impl UpdatesScreen {
    /// Builds the screen and starts its query.
    pub fn new(kind: EntryKind) -> Rc<Self> {
        let screen = Rc::new_cyclic(|weak: &Weak<Self>| {
            let col = GBox::new(O::Vertical, 6);
            crate::ui::margins(&col, 12);
            let list = crate::ui::scrolled(&col);
            let retry = weak.clone();
            let pages = PageStates::new(&list, move || {
                if let Some(screen) = retry.upgrade() {
                    screen.load.borrow_mut().retry();
                    screen.start();
                }
            });
            let bar = GBox::new(O::Horizontal, 6);
            crate::ui::margins(&bar, 6);
            bar.set_visible(false);
            let body = GBox::new(O::Vertical, 0);
            body.append(&bar);
            body.append(pages.widget());
            let root = gtk::Overlay::new();
            root.set_child(Some(&body));
            let toast = ScreenToast::new();
            root.add_overlay(toast.widget());
            Self {
                kind,
                root,
                bar,
                col,
                pages,
                toast,
                load: RefCell::new(UpdatesLoad::Loading),
                selection: RefCell::default(),
                watch: RefCell::default(),
                download_watch: RefCell::default(),
                disk: RefCell::default(),
                index: RefCell::default(),
                external_downloader: kind == EntryKind::Anime
                    && DownloadPrefs::new(data::app().base.store().clone()).use_external_downloader().get(),
                refresh_pending: std::cell::Cell::new(false),
                invalidate_pending: std::cell::Cell::new(false),
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&screen);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(screen) = weak.upgrade() else { return glib::Propagation::Proceed };
            let ctrl = state.contains(ModifierType::CONTROL_MASK);
            if key == gtk::gdk::Key::Escape && screen.escape() {
                glib::Propagation::Stop
            } else if ctrl && key.to_lower() == gtk::gdk::Key::a {
                screen.select_all();
                glib::Propagation::Stop
            } else if ctrl && key.to_lower() == gtk::gdk::Key::i {
                screen.select_inverse();
                glib::Propagation::Stop
            } else if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) && screen.enter() {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        screen.root.add_controller(keys);
        screen.start();
        screen.watch_downloads();
        screen.watch_last_updated();
        screen
    }

    /// Redraws the "Library last updated" line every 30 s while the screen is shown and not selecting (T-42a-026).
    fn watch_last_updated(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_secs(30), move || {
            let Some(screen) = weak.upgrade() else { return glib::ControlFlow::Break };
            if screen.root.is_mapped() && !screen.selection_mode() {
                screen.render();
            }
            glib::ControlFlow::Continue
        });
    }

    pub fn kind(&self) -> EntryKind {
        self.kind
    }

    /// Shows `text` at the bottom of this screen for the usual toast time.
    pub fn show_toast(&self, text: &str) {
        self.toast.show(text, TOAST_MS);
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    fn db(&self) -> std::sync::Arc<kuro_db::handle::Db> {
        data::app().db(self.kind)
    }

    fn selection_mode(&self) -> bool {
        matches!(&*self.load.borrow(), UpdatesLoad::Loaded(rows) if is_selection_mode(rows))
    }

    /// Escape in selection mode clears the selection and is consumed; otherwise it navigates (not consumed).
    fn escape(self: &Rc<Self>) -> bool {
        match back_action(self.selection_mode()) {
            BackAction::ClearSelection => {
                self.clear_selection();
                true
            }
            BackAction::Navigate => false,
        }
    }

    /// Enter on the focused row does what a tap on it does: opens the unit, or toggles it while selecting (T-42b-038).
    /// `false` when the focus is not on a row itself (a button inside it keeps its own Enter), so the key goes on.
    fn enter(self: &Rc<Self>) -> bool {
        let focus = self.root.root().and_then(|root| gtk::prelude::RootExt::focus(&root));
        let Some(id) = focus.and_then(|w| feed::unit_of_row_name(&w.widget_name())) else { return false };
        self.press_later(Gesture::Tap, id);
        true
    }

    fn clear_selection(self: &Rc<Self>) {
        if let UpdatesLoad::Loaded(rows) = &mut *self.load.borrow_mut() {
            self.selection.borrow_mut().toggle_all(rows, false);
        }
        self.render();
    }

    fn select_all(self: &Rc<Self>) {
        if let UpdatesLoad::Loaded(rows) = &mut *self.load.borrow_mut() {
            self.selection.borrow_mut().toggle_all(rows, true);
        }
        self.render();
    }

    fn select_inverse(self: &Rc<Self>) {
        if let UpdatesLoad::Loaded(rows) = &mut *self.load.borrow_mut() {
            self.selection.borrow_mut().invert(rows);
        }
        self.render();
    }

    fn start(self: &Rc<Self>) {
        if let Some(previous) = self.watch.borrow_mut().take() {
            previous.abort();
        }
        let months = u32::try_from(kuro_domain::UPDATES_WINDOW_MONTHS).unwrap_or(3);
        let since = Local::now().checked_sub_months(Months::new(months)).map_or(0, |t| t.timestamp_millis());
        let sub = UpdatesRepo::new(self.db()).watch_recent(self.kind, since);
        let weak = Rc::downgrade(self);
        let handle = watch_coalesced(sub, COALESCE_MS, move |published| {
            let Some(screen) = weak.upgrade() else { return };
            let toast = screen.publish(published.map(|rows| (*rows).clone()));
            screen.render();
            if let Some(message) = toast {
                screen.toast.show(&message, TOAST_MS);
            }
        });
        *self.watch.borrow_mut() = Some(handle);
        self.render();
    }

    /// Applies one query result. A failure keeps a feed on screen and returns the toast text (T-42b-003).
    fn publish(&self, result: Result<Vec<kuro_domain::updates::model::UpdateRow>, String>) -> Option<String> {
        let mut load = self.load.borrow_mut();
        match result {
            Ok(rows) => {
                let previous = match &*load {
                    UpdatesLoad::Loaded(previous) => previous.clone(),
                    _ => Vec::new(),
                };
                let mut next = feed::replace_rows(rows, &previous, &mut self.selection.borrow_mut());
                self.rederive(&mut next, false);
                *self.index.borrow_mut() = feed::index_rows(&next);
                *load = UpdatesLoad::Loaded(next);
                None
            }
            Err(message) => load.apply_failure(&message),
        }
    }

    fn render(self: &Rc<Self>) {
        let (page, rows) = {
            let load = self.load.borrow();
            let rows = match &*load {
                UpdatesLoad::Loaded(rows) => rows.clone(),
                _ => Vec::new(),
            };
            (load.page(), rows)
        };
        let mode = is_selection_mode(&rows);
        self.fill_bar(&rows, mode);
        self.fill(&rows, mode);
        self.pages.show(&page);
    }

    fn fill(self: &Rc<Self>, rows: &[FeedRow], mode: bool) {
        while let Some(child) = self.col.first_child() {
            child.unparent();
        }
        self.col.append(&last_updated_label());
        let ui = UiPrefs::new(data::app().base.store().clone());
        let items = feed::layout(
            rows,
            &Local,
            Local::now().date_naive(),
            ui.relative_time.get(),
            &ui.date_format.get(),
            kuro_i18n::current_locale().tag,
        );
        let mut current: Option<ListBox> = None;
        for item in items {
            match item {
                ListRow::Header(text) => {
                    self.col.append(&crate::ui::heading(&text));
                    let list = crate::ui::boxed_list();
                    self.col.append(&list);
                    current = Some(list);
                }
                ListRow::Item(i) => {
                    let list = current.get_or_insert_with(|| {
                        let list = crate::ui::boxed_list();
                        self.col.append(&list);
                        list
                    });
                    list.append(&self.row(&rows[i], mode));
                }
            }
        }
        if let Some(footer) = updates_page::limit_footer(rows.len()) {
            self.col.append(&crate::ui::dim(footer));
        }
    }

    /// One feed row at the spec height of 56 px (T-42b-022): the selection check, the unread dot, the cover (40 px
    /// wide, 6 px inset), the title over a line of bookmark, fillermark, unit name and progress, and the download
    /// indicator. A read or seen row dims the title and the unit name to 0.38.
    fn row(self: &Rc<Self>, feed_row: &FeedRow, mode: bool) -> gtk::ListBoxRow {
        let row = &feed_row.row;
        let anime = self.kind == EntryKind::Anime;
        let line = GBox::new(O::Horizontal, 12);
        line.set_size_request(-1, updates_page::ROW_HEIGHT);
        line.set_margin_start(12);
        line.set_margin_end(12);
        let check = CheckButton::builder().active(feed_row.selected).visible(mode).can_target(false).valign(gtk::Align::Center).build();
        line.append(&check);
        let dot = Image::from_icon_name("media-record-symbolic");
        dot.set_pixel_size(8);
        dot.add_css_class("accent");
        dot.set_valign(gtk::Align::Center);
        dot.set_visible(!row.done);
        line.append(&dot);

        let cover = CoverView::new(updates_page::COVER_WIDTH, updates_page::ROW_HEIGHT - 2 * updates_page::COVER_INSET);
        cover.frame.set_valign(gtk::Align::Center);
        cover.frame.set_halign(gtk::Align::Center);
        cover.load(ArtRef {
            owner: if anime { Owner::Anime } else { Owner::Manga },
            entry_id: row.entry_id,
            source_id: row.source_id,
            kind: ArtKind::Cover,
            url: row.thumbnail_url.clone(),
            favorite: true,
            last_modified: row.cover_last_modified,
        });
        let cover_tap = GestureClick::new();
        let (weak, title) = (Rc::downgrade(self), row.entry_title.clone());
        cover_tap.connect_pressed(move |gesture, _, _, _| {
            let Some(screen) = weak.upgrade() else { return };
            if cover_click_enabled(screen.selection_mode()) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                crate::screens::open_entry(&title, screen.kind == EntryKind::Anime);
            }
        });
        cover.frame.add_controller(cover_tap);
        line.append(&cover.frame);

        let alpha = updates_page::row_alpha(row.done);
        let texts = GBox::new(O::Vertical, 2);
        texts.set_hexpand(true);
        texts.set_valign(gtk::Align::Center);
        let title = Label::builder().label(&row.entry_title).xalign(0.0).single_line_mode(true).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["heading"]).build();
        title.set_opacity(alpha);
        texts.append(&title);
        let detail = GBox::new(O::Horizontal, 4);
        if row.bookmark {
            let star = Image::from_icon_name("starred-symbolic");
            star.set_pixel_size(14);
            star.add_css_class("accent");
            detail.append(&star);
        }
        if row.fillermark && anime {
            // the stock theme has no tertiary colour; the warning colour keeps it apart from the bookmark
            let tag = Image::from_icon_name("tag-symbolic");
            tag.set_pixel_size(14);
            tag.add_css_class("warning");
            detail.append(&tag);
        }
        let name = Label::builder().label(&row.item_name).xalign(0.0).single_line_mode(true).ellipsize(gtk::pango::EllipsizeMode::End).build();
        name.set_opacity(alpha);
        detail.append(&name);
        if let Some(progress) = updates_page::progress_text(anime, row.done, row.progress, row.total_seconds) {
            detail.append(&Label::builder().label(progress).css_classes(["dim-label"]).build());
        }
        texts.append(&detail);
        line.append(&texts);
        line.append(&self.download_indicator(feed_row));

        let list_row = gtk::ListBoxRow::builder().child(&line).activatable(false).name(feed::row_name(row.item_id)).build();
        self.attach_gestures(&list_row, row.item_id);
        list_row
    }

    /// Long press, tap, Ctrl+click and Shift+click on a row. A long press that fired suppresses the release's tap.
    fn attach_gestures(self: &Rc<Self>, row: &gtk::ListBoxRow, id: i64) {
        let long_fired = Rc::new(std::cell::Cell::new(false));
        let long = GestureLongPress::new();
        let (weak, fired) = (Rc::downgrade(self), Rc::clone(&long_fired));
        long.connect_pressed(move |_, _, _| {
            fired.set(true);
            if let Some(screen) = weak.upgrade() {
                screen.press_later(Gesture::LongPress, id);
            }
        });
        let fired = Rc::clone(&long_fired);
        long.connect_cancelled(move |_| fired.set(false));
        row.add_controller(long);

        let click = GestureClick::new();
        let (weak, fired) = (Rc::downgrade(self), long_fired);
        click.connect_released(move |gesture, _, _, _| {
            if fired.replace(false) {
                return;
            }
            if let Some(screen) = weak.upgrade() {
                screen.press_later(click_gesture(gesture.current_event_state()), id);
            }
        });
        row.add_controller(click);
    }

    /// Handles the press after the gesture callback returns, because a change rebuilds the row that owns the gesture.
    fn press_later(self: &Rc<Self>, gesture: Gesture, id: i64) {
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(screen) = weak.upgrade() {
                screen.press(gesture, id);
            }
        });
    }

    fn press(self: &Rc<Self>, gesture: Gesture, id: i64) {
        let outcome = match &mut *self.load.borrow_mut() {
            UpdatesLoad::Loaded(rows) => feed::press(gesture, &mut self.selection.borrow_mut(), rows, id),
            _ => Press::Nothing,
        };
        match outcome {
            Press::Open => self.open_unit(id, false),
            Press::Changed => self.render(),
            Press::Nothing => {}
        }
    }

    /// Opens the unit. Anime follows the "always use external player" preference inverted by `alt` (E-U35): the row
    /// tap passes `false`, the bar's play actions `true`. Manga opens in the reader.
    fn open_unit(&self, id: i64, alt: bool) {
        let target = match &*self.load.borrow() {
            UpdatesLoad::Loaded(rows) => rows.iter().find(|r| r.row.item_id == id).map(|r| (r.row.entry_title.clone(), r.row.item_name.clone())),
            _ => None,
        };
        let anime = self.kind == EntryKind::Anime;
        if let Some((title, unit)) = target {
            let always_external = anime && kuro_prefs::player::PlayerPrefs::new(data::app().base.store().clone()).always_use_external_player.get();
            open_route::open(anime, &title, &unit, open_route::route(anime, always_external, alt));
        }
    }

    /// One batch write of the selected rows whose flag differs from the target, then the selection rule of the action.
    /// With "remove after marked as read" on, the downloads of the rows the write marked read are removed (E-U33).
    fn bulk(self: &Rc<Self>, action: BulkAction) {
        let (ids, newly_read) = match &*self.load.borrow() {
            UpdatesLoad::Loaded(rows) => {
                let ids = bulk::targets(&feed::selected_flags(rows), action);
                let newly_read: Vec<UnitTarget> = rows
                    .iter()
                    .filter(|r| ids.contains(&r.row.item_id))
                    .map(|r| UnitTarget { entry_id: r.row.entry_id, unit_id: r.row.item_id, state: r.download.state })
                    .collect();
                (ids, newly_read)
            }
            _ => return,
        };
        let result = bulk::apply(&self.db(), self.kind, &ids, action);
        if result.is_ok() && action == BulkAction::MarkRead(true) {
            self.remove_downloads_of_read(newly_read);
        }
        let message = match &mut *self.load.borrow_mut() {
            UpdatesLoad::Loaded(rows) => bulk::after_bulk(action, result, &mut self.selection.borrow_mut(), rows),
            _ => None,
        };
        self.render();
        if let Some(message) = message {
            self.toast.show(&message, TOAST_MS);
        }
    }

    /// Removes the downloads of the units just marked read, grouped per entry, when the preference is on (E-U33).
    fn remove_downloads_of_read(&self, newly_read: Vec<UnitTarget>) {
        let enabled = DownloadPrefs::new(data::app().base.store().clone()).remove_after_marked_as_read().get();
        if !enabled || newly_read.is_empty() {
            return;
        }
        let (backend, kind) = (download_host::current(), self.kind);
        download_host::spawn_detached(move || {
            download_exec::remove_read_downloads(&*backend, kind, true, &newly_read);
        });
    }

    /// Runs a download action over `targets` detached from the screen (T-42a-031), so leaving the screen does not
    /// cancel it. The backend is taken here because the shell's index is only reachable on the GTK thread.
    fn run_download(&self, action: DownloadAction, targets: Vec<UnitTarget>) {
        if action == DownloadAction::ShowQualities {
            return self.show_qualities(&targets);
        }
        let sources: HashMap<i64, i64> = match &*self.load.borrow() {
            UpdatesLoad::Loaded(rows) => rows.iter().map(|r| (r.row.entry_id, r.row.source_id)).collect(),
            _ => return,
        };
        let (backend, kind, external) = (download_host::current(), self.kind, self.external_downloader);
        // every row of the feed is a library entry
        let facts = move |backend: &dyn download_host::DownloadBackend, entry: i64| {
            sources.get(&entry).map(|source| EntryFacts { in_library: true, source_installed: backend.source_installed(kind, *source) })
        };
        if action == DownloadAction::Delete {
            // the source lookups are in-memory reads, so the skipped entries are known here to warn and toast (T-42b-010)
            let skipped = download_actions::delete_skipped(&targets, |entry| facts(&*backend, entry));
            if !skipped.is_empty() {
                tracing::warn!(?skipped, "delete skipped entries whose source is not installed");
                self.show_toast(copy::DELETE_SOURCE_MISSING);
            }
        }
        download_host::spawn_detached(move || {
            download_exec::dispatch(&*backend, kind, action, &targets, |entry| facts(&*backend, entry), external);
        });
    }

    /// SHOW_QUALITIES: opens the episode options sheet when exactly one row is the target; any other count leaves
    /// everything as it is (E-U49).
    fn show_qualities(&self, targets: &[UnitTarget]) {
        let request = match &*self.load.borrow() {
            UpdatesLoad::Loaded(rows) => qualities::request_of(targets, rows, self.external_downloader),
            _ => None,
        };
        if let Some(request) = request {
            qualities::open(request);
        }
    }

    /// A bar download action over the selection; the selection clears afterwards (as `toggleAllSelection(false)`).
    fn bulk_download(self: &Rc<Self>, action: DownloadAction) {
        let targets = match &*self.load.borrow() {
            UpdatesLoad::Loaded(rows) => feed::selected_targets(rows),
            _ => return,
        };
        self.run_download(action, targets);
        self.clear_selection();
    }

    /// An indicator action on one row.
    fn row_download(self: &Rc<Self>, action: DownloadAction, id: i64) {
        let target = match &*self.load.borrow() {
            UpdatesLoad::Loaded(rows) => rows
                .iter()
                .find(|r| r.row.item_id == id)
                .map(|r| UnitTarget { entry_id: r.row.entry_id, unit_id: r.row.item_id, state: r.download.state }),
            _ => None,
        };
        if let Some(target) = target {
            self.run_download(action, vec![target]);
        }
    }

    /// The bar's play actions: the single selected unit opens in the player that is not the preferred one (E-U35).
    fn play_selected(self: &Rc<Self>, alt: bool) {
        let id = match &*self.load.borrow() {
            UpdatesLoad::Loaded(rows) => match rows.iter().filter(|r| r.selected).collect::<Vec<_>>().as_slice() {
                [only] => Some(only.row.item_id),
                _ => None,
            },
            _ => None,
        };
        let Some(id) = id else { return };
        self.open_unit(id, alt);
        self.clear_selection();
    }

    /// Re-derives the download state of `rows` from the backend's queue and the cached disk check.
    fn rederive(&self, rows: &mut [FeedRow], invalidate: bool) {
        let (backend, kind) = (download_host::current(), self.kind);
        let mut cache = self.disk.borrow_mut();
        if invalidate {
            cache.invalidate();
        }
        feed::rederive(rows, |id| backend.queue_entry(kind, id), &mut cache, |key| backend.on_disk(kind, key));
    }

    /// Redraws after the quiet period of a burst of download events. `invalidate` also drops the disk checks.
    fn refresh_later(self: &Rc<Self>, invalidate: bool) {
        if invalidate {
            self.invalidate_pending.set(true);
        }
        if self.refresh_pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(COALESCE_MS), move || {
            let Some(screen) = weak.upgrade() else { return };
            screen.refresh_pending.set(false);
            if screen.invalidate_pending.replace(false) {
                if let UpdatesLoad::Loaded(rows) = &mut *screen.load.borrow_mut() {
                    screen.rederive(rows, true);
                }
            }
            screen.render();
        });
    }

    /// Follows the download hub: a unit event patches the row it names, a cache or queue change re-derives every
    /// row (T-42b-002, T-42b-003). The feed query itself is not re-run for these.
    fn watch_downloads(self: &Rc<Self>) {
        let hub = download_host::hub();
        let (mut changes, mut units) = (hub.cache.subscribe(), hub.subscribe_units());
        let weak = Rc::downgrade(self);
        let on_changes = glib::spawn_future_local(async move {
            use tokio::sync::broadcast::error::RecvError;
            loop {
                match changes.recv().await {
                    Ok(()) | Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return,
                }
                match weak.upgrade() {
                    Some(screen) => screen.refresh_later(true),
                    None => return,
                }
            }
        });
        let weak = Rc::downgrade(self);
        let on_units = glib::spawn_future_local(async move {
            use tokio::sync::broadcast::error::RecvError;
            loop {
                let event: Option<UnitEvent> = match units.recv().await {
                    Ok(event) => Some(event),
                    Err(RecvError::Lagged(_)) => None,
                    Err(RecvError::Closed) => return,
                };
                let Some(screen) = weak.upgrade() else { return };
                match event {
                    Some(UnitEvent { unit_id, change }) => screen.patch_unit(unit_id, change),
                    None => screen.refresh_later(true),
                }
            }
        });
        *self.download_watch.borrow_mut() = vec![on_changes, on_units];
    }

    /// One download event: patches the matching row, then redraws after the quiet period.
    fn patch_unit(self: &Rc<Self>, unit_id: i64, change: Change) {
        let found = match &mut *self.load.borrow_mut() {
            UpdatesLoad::Loaded(rows) => feed::apply_download(rows, &self.index.borrow(), unit_id, change),
            _ => false,
        };
        if found {
            self.refresh_later(false);
        }
    }

    /// The download indicator of a row: the state's icon (with the percent while downloading) over a menu of the
    /// state's actions (T-42a-031).
    fn download_indicator(self: &Rc<Self>, feed_row: &FeedRow) -> gtk::MenuButton {
        let state = feed_row.download.state;
        let content = GBox::new(O::Horizontal, 4);
        content.append(&Image::from_icon_name(download_actions::indicator_icon(state)));
        if state == DownloadState::Downloading {
            content.append(&Label::new(Some(&format!("{}%", feed_row.download.progress.clamp(0, 100)))));
        }
        let button = gtk::MenuButton::builder().child(&content).has_frame(false).valign(gtk::Align::Center).build();
        let popover = gtk::Popover::new();
        let menu = GBox::new(O::Vertical, 2);
        for action in download_actions::indicator_actions(self.kind == EntryKind::Anime, state, qualities::available()) {
            let item = Button::with_label(&tr(download_actions::row_action_label(action, state)));
            item.add_css_class("flat");
            let (weak, popover, id) = (Rc::downgrade(self), popover.clone(), feed_row.row.item_id);
            item.connect_clicked(move |_| {
                popover.popdown();
                if let Some(screen) = weak.upgrade() {
                    screen.row_download(action, id);
                }
            });
            menu.append(&item);
        }
        popover.set_child(Some(&menu));
        button.set_popover(Some(&popover));
        // the long press of a not-downloaded anime indicator shows the qualities instead of the menu, and the row's own
        // long press (selection) does not see it
        if let Some(action) = download_actions::indicator_long_press(self.kind == EntryKind::Anime, state, qualities::available()) {
            let long = GestureLongPress::new();
            let (weak, id) = (Rc::downgrade(self), feed_row.row.item_id);
            long.connect_pressed(move |gesture, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                if let Some(screen) = weak.upgrade() {
                    screen.row_download(action, id);
                }
            });
            button.add_controller(long);
        }
        button
    }
}

impl UpdatesScreen {
    /// Rebuilds the bulk bar from the selection (T-42a-036): the close button and the "N selected" count, then the
    /// actions the predicates allow (up to the ten of `bottom_bar`), then select-all.
    fn fill_bar(self: &Rc<Self>, rows: &[FeedRow], mode: bool) {
        while let Some(child) = self.bar.first_child() {
            self.bar.remove(&child);
        }
        self.bar.set_visible(mode);
        if !mode {
            return;
        }
        let anime = self.kind == EntryKind::Anime;
        let selected: Vec<Selected> = rows
            .iter()
            .filter(|r| r.selected)
            .map(|r| Selected {
                read: r.row.done,
                bookmark: r.row.bookmark,
                fillermark: r.row.fillermark,
                progress: r.row.progress,
                state: r.download.state,
            })
            .collect();
        let close = Button::from_icon_name("window-close-symbolic");
        close.set_tooltip_text(Some(&tr(MsgKey::action_cancel)));
        let weak = Rc::downgrade(self);
        close.connect_clicked(move |_| {
            if let Some(screen) = weak.upgrade() {
                screen.clear_selection();
            }
        });
        self.bar.append(&close);
        self.bar.append(&Label::new(Some(&bottom_bar::selected_text(selected.len()))));
        let always_external = kuro_prefs::player::PlayerPrefs::new(data::app().base.store().clone()).always_use_external_player.get();
        for action in bottom_bar::visible_actions(anime, &selected, always_external) {
            self.bar.append(&self.bar_button(action, anime));
        }
        let spacer = GBox::new(O::Horizontal, 0);
        spacer.set_hexpand(true);
        self.bar.append(&spacer);
        let select_all = Button::from_icon_name("edit-select-all-symbolic");
        select_all.set_tooltip_text(Some(&tr(MsgKey::action_select_all)));
        let weak = Rc::downgrade(self);
        select_all.connect_clicked(move |_| {
            if let Some(screen) = weak.upgrade() {
                screen.select_all();
            }
        });
        self.bar.append(&select_all);
        let select_inverse = Button::from_icon_name("edit-select-invert-symbolic");
        select_inverse.set_tooltip_text(Some(&tr(MsgKey::action_select_inverse)));
        let weak = Rc::downgrade(self);
        select_inverse.connect_clicked(move |_| {
            if let Some(screen) = weak.upgrade() {
                screen.select_inverse();
            }
        });
        self.bar.append(&select_inverse);
    }

    fn bar_button(self: &Rc<Self>, action: BarAction, anime: bool) -> Button {
        let content = GBox::new(O::Horizontal, 6);
        content.append(&Image::from_icon_name(bottom_bar::icon(action)));
        content.append(&Label::new(Some(&tr(bottom_bar::label(action, anime)))));
        let button = Button::new();
        button.set_child(Some(&content));
        let weak = Rc::downgrade(self);
        button.connect_clicked(move |_| {
            let Some(screen) = weak.upgrade() else { return };
            match bottom_bar::command(action) {
                BarCommand::Bulk(bulk_action) => screen.bulk(bulk_action),
                BarCommand::Download(DownloadAction::Delete) => {
                    // the bar's delete always asks; OK runs the delete plan (T-42a-032)
                    let confirm = Rc::downgrade(&screen);
                    confirm_bulk_delete(screen.kind == EntryKind::Anime, move || {
                        if let Some(screen) = confirm.upgrade() {
                            screen.bulk_download(DownloadAction::Delete);
                        }
                    });
                }
                BarCommand::Download(download) => screen.bulk_download(download),
                BarCommand::Play { alt } => screen.play_selected(alt),
            }
        });
        button
    }
}

/// The "Library last updated: ..." row at the top of the feed (T-42b-016), in italics.
fn last_updated_label() -> Label {
    let last = data::app().library.last_updated_timestamp.get();
    let text = format::last_updated_line(last, Local::now().timestamp_millis());
    let label = Label::builder().xalign(0.0).css_classes(["dim-label"]).build();
    label.set_markup(&format!("<i>{}</i>", gtk::glib::markup_escape_text(&text)));
    label
}
