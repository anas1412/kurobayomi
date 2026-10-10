//! The Updates screen of one kind (T-42a-030, T-42a-034): the feed under date headers, the selection gestures
//! (long press, tap while selecting, Ctrl and Shift click, Escape) and the bulk bar (bookmark, fillermark, read
//! state). In selection mode the cover tap is off and the bar shows; there is no search field.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use chrono::{Local, Months};
use gtk::gdk::ModifierType;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Box as GBox, Button, CheckButton, GestureClick, GestureLongPress, Image, Label, ListBox, Orientation as O};
use kuro_domain::dates::ListRow;
use kuro_domain::kind::EntryKind;
use kuro_domain::updates::repo::UpdatesRepo;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::ui::UiPrefs;

use crate::data;
use crate::updates::bottom_bar::{self, BarAction, Selected};
use crate::updates::bulk::{self, BulkAction};
use crate::updates::coalesce::watch_coalesced;
use crate::updates::download_state::DownloadState;
use crate::updates::feed::{self, FeedRow, Gesture, Press};
use crate::updates::format;
use crate::updates::history::COALESCE_MS;
use crate::updates::screen_toast::ScreenToast;
use crate::updates::selection::{Selection, is_selection_mode};
use crate::updates::state_page::PageStates;
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
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&screen);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(screen) = weak.upgrade() else { return glib::Propagation::Proceed };
            if key == gtk::gdk::Key::Escape && screen.escape() {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        screen.root.add_controller(keys);
        screen.start();
        screen
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
                *load = UpdatesLoad::Loaded(feed::replace_rows(rows, &previous, &mut self.selection.borrow_mut()));
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

    fn row(self: &Rc<Self>, feed_row: &FeedRow, mode: bool) -> gtk::ListBoxRow {
        let row = &feed_row.row;
        let line = GBox::new(O::Horizontal, 12);
        crate::ui::margins(&line, 8);
        let check = CheckButton::builder().active(feed_row.selected).visible(mode).can_target(false).valign(gtk::Align::Center).build();
        line.append(&check);

        let cover = gtk::Picture::new();
        cover.set_size_request(48, 68);
        let cover_tap = GestureClick::new();
        let (weak, title) = (Rc::downgrade(self), row.entry_title.clone());
        cover_tap.connect_pressed(move |gesture, _, _, _| {
            let Some(screen) = weak.upgrade() else { return };
            if cover_click_enabled(screen.selection_mode()) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                crate::screens::open_entry(&title, screen.kind == EntryKind::Anime);
            }
        });
        cover.add_controller(cover_tap);
        line.append(&cover);

        let texts = GBox::new(O::Vertical, 2);
        texts.set_hexpand(true);
        texts.set_valign(gtk::Align::Center);
        let title = Label::builder().label(&row.entry_title).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["heading"]).build();
        let sub = Label::builder().label(&row.item_name).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
        if row.done {
            sub.add_css_class("dim-label");
        }
        texts.append(&title);
        texts.append(&sub);
        line.append(&texts);
        if row.bookmark {
            line.append(&gtk::Image::from_icon_name("starred-symbolic"));
        }
        if row.fillermark {
            line.append(&gtk::Image::from_icon_name("tag-symbolic"));
        }

        let list_row = gtk::ListBoxRow::builder().child(&line).activatable(false).build();
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
            Press::Open => self.open_unit(id),
            Press::Changed => self.render(),
            Press::Nothing => {}
        }
    }

    fn open_unit(&self, id: i64) {
        let target = match &*self.load.borrow() {
            UpdatesLoad::Loaded(rows) => rows.iter().find(|r| r.row.item_id == id).map(|r| (r.row.entry_title.clone(), r.row.item_name.clone())),
            _ => None,
        };
        if let Some((title, unit)) = target {
            crate::screens::open_media(self.kind == EntryKind::Anime, &title, &unit);
        }
    }

    /// One batch write of the selected rows whose flag differs from the target, then the selection rule of the action.
    fn bulk(self: &Rc<Self>, action: BulkAction) {
        let ids = match &*self.load.borrow() {
            UpdatesLoad::Loaded(rows) => bulk::targets(&feed::selected_flags(rows), action),
            _ => return,
        };
        let result = bulk::apply(&self.db(), self.kind, &ids, action);
        let message = match &mut *self.load.borrow_mut() {
            UpdatesLoad::Loaded(rows) => bulk::after_bulk(action, result, &mut self.selection.borrow_mut(), rows),
            _ => None,
        };
        self.render();
        if let Some(message) = message {
            self.toast.show(&message, TOAST_MS);
        }
    }
}

impl UpdatesScreen {
    /// Rebuilds the bulk bar from the selection (T-42a-036): the close button and the "N selected" count, then the
    /// actions the predicates allow, then select-all. Actions with no executor yet (download, delete, play) are not
    /// drawn; see `bottom_bar` for their predicates.
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
                // The download state is not in the feed row yet (the queue is not in the app state).
                state: DownloadState::NotDownloaded,
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
        for action in bottom_bar::visible_actions(anime, &selected, false) {
            if let Some(bulk_action) = bulk_for(action) {
                self.bar.append(&self.bar_button(action, anime, bulk_action));
            }
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
    }

    fn bar_button(self: &Rc<Self>, action: BarAction, anime: bool, bulk_action: BulkAction) -> Button {
        let content = GBox::new(O::Horizontal, 6);
        content.append(&Image::from_icon_name(bottom_bar::icon(action)));
        content.append(&Label::new(Some(&tr(bottom_bar::label(action, anime)))));
        let button = Button::new();
        button.set_child(Some(&content));
        let weak = Rc::downgrade(self);
        button.connect_clicked(move |_| {
            if let Some(screen) = weak.upgrade() {
                screen.bulk(bulk_action);
            }
        });
        button
    }
}

/// The bulk write of a bar action. Download, delete and play have no executor yet, so they map to nothing.
fn bulk_for(action: BarAction) -> Option<BulkAction> {
    match action {
        BarAction::Bookmark => Some(BulkAction::Bookmark(true)),
        BarAction::RemoveBookmark => Some(BulkAction::Bookmark(false)),
        BarAction::Fillermark => Some(BulkAction::Fillermark(true)),
        BarAction::RemoveFillermark => Some(BulkAction::Fillermark(false)),
        BarAction::MarkRead => Some(BulkAction::MarkRead(true)),
        BarAction::MarkUnread => Some(BulkAction::MarkRead(false)),
        BarAction::Download | BarAction::Delete | BarAction::PlayExternally | BarAction::PlayInternally => None,
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
