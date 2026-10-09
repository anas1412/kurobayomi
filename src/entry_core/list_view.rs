//! The chapter list widget: a `gio::ListStore` of `RowItem`s behind a `gtk::ListView`. `sync` diffs the new rows
//! against the shown ones (`list_model::diff`): a changed row gets one `items_changed`, a changed structure gets one
//! splice. Rows report clicks and long presses; keys are handled by the page.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gdk::ModifierType;
use gtk::prelude::*;
use gtk::{Align, Box as GBox, Label, Orientation as O, gio, glib, pango};
use kuro_downloads::DownloadState;

use kuro_i18n::{MsgKey, tr};
use kuro_prefs::library::{ChapterSwipeAction, LibraryPrefs};

use crate::entry_core::list_model::{DOT_SIZE, Diff, ROW_PADDING, RowItem, diff, row_alpha};
use crate::entry_core::selection::Mods;
use crate::entry_core::swipe::{action_for, menu, side_of_drag};

/// A click or long press on a row: (chapter id, modifiers, long press or right click).
pub type OnRow = Rc<dyn Fn(i64, Mods, bool)>;

/// What a row reports by chapter id and its download state: a click on the download indicator.
pub type OnIndicator = Rc<dyn Fn(i64, DownloadState)>;

/// What a row reports by chapter id alone: "Mark previous as read" from the context menu.
pub type OnChapter = Rc<dyn Fn(i64)>;

/// The row callbacks besides a click: the download indicator and the context menu's mark-previous item.
#[derive(Clone)]
pub struct RowActions {
    pub on_indicator: OnIndicator,
    pub on_previous: OnChapter,
}

/// The swipe actions of a row (T-08a-029): the stored action per side, and what to run for one row.
#[derive(Clone)]
pub struct Swipe {
    pub prefs: Rc<LibraryPrefs>,
    pub run: Rc<dyn Fn(i64, ChapterSwipeAction)>,
}

pub struct ChapterList {
    pub widget: gtk::ListView,
    store: gio::ListStore,
    shown: RefCell<Vec<RowItem>>,
}

fn item_of(list_item: &gtk::ListItem) -> Option<glib::BoxedAnyObject> {
    list_item.item().and_downcast::<glib::BoxedAnyObject>()
}

fn mods_of(state: ModifierType) -> Mods {
    Mods {
        shift: state.contains(ModifierType::SHIFT_MASK),
        ctrl: state.contains(ModifierType::CONTROL_MASK),
    }
}

fn download_icon(state: DownloadState) -> Option<&'static str> {
    match state {
        DownloadState::NotDownloaded => Some("document-save-symbolic"),
        DownloadState::Queue => Some("content-loading-symbolic"),
        DownloadState::Downloading => Some("folder-download-symbolic"),
        DownloadState::Downloaded => Some("emblem-ok-symbolic"),
        DownloadState::Error => Some("dialog-error-symbolic"),
    }
}

/// The unread dot and the bookmark icon take the theme's primary colour (`kuro_primary`, looked up across every
/// provider of the display).
const CSS: &str = ".chapter-primary { color: @kuro_primary; }";

pub fn ensure_css() {
    thread_local! {
        static DONE: Cell<bool> = const { Cell::new(false) };
    }
    if DONE.with(|d| d.replace(true)) {
        return;
    }
    if let Some(display) = gtk::gdk::Display::default() {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}

/// The row widget: unread dot, check mark, title and subtitle, download icon, bookmark icon, and the label a
/// missing-count marker row shows instead of all of them.
struct RowWidgets {
    root: GBox,
    dot: gtk::Image,
    check: gtk::Image,
    text: GBox,
    title: Label,
    subtitle: Label,
    download_button: gtk::Button,
    download: gtk::Image,
    bookmark: gtk::Image,
    marker: Label,
}

impl RowWidgets {
    /// Finds the inner widgets again from the root, in the order `row_widgets` appended them.
    fn of(root: &gtk::Widget) -> Option<Self> {
        let root = root.clone().downcast::<GBox>().ok()?;
        let dot = root.first_child()?;
        let check = dot.next_sibling()?;
        let text = check.next_sibling()?;
        let title = text.first_child()?;
        let subtitle = title.next_sibling()?;
        let download_button = text.next_sibling()?;
        let bookmark = download_button.next_sibling()?;
        let marker = bookmark.next_sibling()?;
        Some(Self {
            dot: dot.downcast().ok()?,
            check: check.downcast().ok()?,
            text: text.downcast().ok()?,
            title: title.downcast().ok()?,
            subtitle: subtitle.downcast().ok()?,
            download: download_button.clone().downcast::<gtk::Button>().ok()?.child()?.downcast().ok()?,
            download_button: download_button.downcast().ok()?,
            bookmark: bookmark.downcast().ok()?,
            marker: marker.downcast().ok()?,
            root,
        })
    }
}

/// The right-click menu of a row: Select, then each swipe action that runs on this page.
fn context_menu(root: &GBox, list_item: &gtk::ListItem, swipe: &Swipe, on_row: &OnRow, on_previous: &OnChapter, at: (f64, f64)) {
    let Some(item) = item_of(list_item) else { return };
    let id = item.borrow::<RowItem>().id;
    if item.borrow::<RowItem>().missing.is_some() {
        return;
    }
    let popover = gtk::Popover::new();
    popover.set_parent(root);
    popover.set_has_arrow(false);
    popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(at.0 as i32, at.1 as i32, 1, 1)));
    let column = GBox::new(O::Vertical, 0);
    let entry = |label: &str, run: Box<dyn Fn()>| {
        let button = gtk::Button::builder().label(label).css_classes(["flat"]).build();
        let popover = popover.clone();
        button.connect_clicked(move |_| {
            popover.popdown();
            run();
        });
        column.append(&button);
    };
    let on_row = Rc::clone(on_row);
    entry("Select", Box::new(move || on_row(id, Mods::default(), true)));
    for (action, label) in menu(&swipe.prefs) {
        let run = Rc::clone(&swipe.run);
        entry(label, Box::new(move || run(id, action)));
    }
    let on_previous = Rc::clone(on_previous);
    entry(&tr(MsgKey::action_mark_previous_as_read), Box::new(move || on_previous(id)));
    popover.set_child(Some(&column));
    // the popover is parented to the row, so it is removed once closed
    popover.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    popover.popup();
}

fn row_widgets() -> RowWidgets {
    ensure_css();
    let root = GBox::new(O::Horizontal, 10);
    let (start, top, end, bottom) = ROW_PADDING;
    root.set_margin_start(start);
    root.set_margin_top(top);
    root.set_margin_end(end);
    root.set_margin_bottom(bottom);
    let dot = gtk::Image::from_icon_name("media-record-symbolic");
    dot.set_pixel_size(DOT_SIZE);
    dot.add_css_class("chapter-primary");
    let check = gtk::Image::from_icon_name("object-select-symbolic");
    let text = GBox::new(O::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(Align::Center);
    let title = Label::builder().xalign(0.0).ellipsize(pango::EllipsizeMode::End).single_line_mode(true).build();
    let subtitle = Label::builder().xalign(0.0).ellipsize(pango::EllipsizeMode::End).css_classes(["dim-label", "caption"]).build();
    text.append(&title);
    text.append(&subtitle);
    let download = gtk::Image::new();
    let download_button = gtk::Button::builder().child(&download).css_classes(["flat", "circular"]).focus_on_click(false).valign(Align::Center).build();
    let bookmark = gtk::Image::from_icon_name("starred-symbolic");
    bookmark.add_css_class("chapter-primary");
    let marker = Label::builder().xalign(0.0).hexpand(true).visible(false).css_classes(["dim-label"]).build();
    for w in [dot.upcast_ref::<gtk::Widget>(), check.upcast_ref(), bookmark.upcast_ref()] {
        w.set_valign(Align::Center);
    }
    root.append(&dot);
    root.append(&check);
    root.append(&text);
    root.append(&download_button);
    root.append(&bookmark);
    root.append(&marker);
    RowWidgets { root, dot, check, text, title, subtitle, download_button, download, bookmark, marker }
}

fn bind(w: &RowWidgets, row: &RowItem) {
    w.root.set_widget_name(&if row.missing.is_some() { row.key() } else { format!("row:{}", row.id) });
    let chapter = row.missing.is_none();
    for widget in [w.dot.upcast_ref::<gtk::Widget>(), w.check.upcast_ref(), w.text.upcast_ref()] {
        widget.set_visible(chapter);
    }
    w.marker.set_visible(!chapter);
    if !chapter {
        // a marker is text only: no download or bookmark, nothing to select
        w.marker.set_label(&row.subtitle);
        w.download_button.set_visible(false);
        w.bookmark.set_visible(false);
        w.root.set_opacity(1.0);
        return;
    }
    w.title.set_label(&row.title);
    w.subtitle.set_label(&row.subtitle);
    w.subtitle.set_visible(!row.subtitle.is_empty());
    // the dot marks an unread chapter; it keeps its space when read, so the titles line up
    w.dot.set_opacity(if row.read { 0.0 } else { 1.0 });
    w.check.set_opacity(if row.selected { 1.0 } else { 0.0 });
    w.bookmark.set_visible(row.bookmark);
    w.root.set_opacity(row_alpha(row.read));
    match download_icon(row.download).filter(|_| row.indicator) {
        Some(icon) => {
            w.download.set_icon_name(Some(icon));
            w.download_button.set_visible(true);
            w.download_button.set_tooltip_text((row.progress > 0).then(|| format!("{}%", row.progress)).as_deref());
        }
        None => w.download_button.set_visible(false),
    }
}

impl ChapterList {
    pub fn new(on_row: OnRow, swipe: Option<Swipe>, actions: RowActions) -> Rc<Self> {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, object| {
            let Some(list_item) = object.downcast_ref::<gtk::ListItem>() else { return };
            let widgets = row_widgets();
            let long_fired = Rc::new(Cell::new(false));
            let report = |list_item: &gtk::ListItem, on_row: &OnRow, state: ModifierType, long: bool| {
                if let Some(item) = item_of(list_item) {
                    let row = item.borrow::<RowItem>();
                    // a missing-count marker is not a chapter
                    if row.missing.is_none() {
                        on_row(row.id, mods_of(state), long);
                    }
                }
            };
            let left = gtk::GestureClick::builder().button(1).build();
            left.connect_released({
                let (list_item, on_row, long_fired) = (list_item.clone(), Rc::clone(&on_row), Rc::clone(&long_fired));
                move |g, _, _, _| {
                    if !long_fired.replace(false) {
                        report(&list_item, &on_row, g.current_event_state(), false);
                    }
                }
            });
            let right = gtk::GestureClick::builder().button(3).build();
            right.connect_pressed({
                let (list_item, on_row, swipe, root, actions) = (list_item.clone(), Rc::clone(&on_row), swipe.clone(), widgets.root.clone(), actions.clone());
                move |g, _, x, y| match &swipe {
                    // the context menu holds Select and the swipe actions of the two sides
                    Some(swipe) => context_menu(&root, &list_item, swipe, &on_row, &actions.on_previous, (x, y)),
                    None => report(&list_item, &on_row, g.current_event_state(), true),
                }
            });
            let long = gtk::GestureLongPress::new();
            long.connect_pressed({
                let (list_item, on_row, long_fired) = (list_item.clone(), Rc::clone(&on_row), Rc::clone(&long_fired));
                move |g, _, _| {
                    long_fired.set(true);
                    report(&list_item, &on_row, g.current_event_state(), true);
                }
            });
            for c in [left.upcast_ref::<gtk::EventController>(), right.upcast_ref(), long.upcast_ref()] {
                widgets.root.add_controller(c.clone());
            }
            if let Some(swipe) = swipe.clone() {
                // a horizontal drag on a touch screen runs the action of the side it ends on
                let drag = gtk::GestureDrag::new();
                drag.set_touch_only(true);
                let list_item = list_item.clone();
                drag.connect_drag_end(move |_, dx, dy| {
                    let (Some(side), Some(item)) = (side_of_drag(dx, dy), item_of(&list_item)) else { return };
                    let row = item.borrow::<RowItem>();
                    if row.missing.is_none() {
                        (swipe.run)(row.id, action_for(&swipe.prefs, side));
                    }
                });
                widgets.root.add_controller(drag);
            }
            // a click on the indicator starts, cancels or deletes by the state of its row
            widgets.download_button.connect_clicked({
                let (list_item, on_indicator) = (list_item.clone(), Rc::clone(&actions.on_indicator));
                move |_| {
                    if let Some(item) = item_of(&list_item) {
                        let row = item.borrow::<RowItem>();
                        on_indicator(row.id, row.download);
                    }
                }
            });
            list_item.set_child(Some(&widgets.root));
        });
        factory.connect_bind(|_, object| {
            let Some(list_item) = object.downcast_ref::<gtk::ListItem>() else { return };
            let (Some(item), Some(child)) = (item_of(list_item), list_item.child()) else { return };
            if let Some(widgets) = RowWidgets::of(&child) {
                bind(&widgets, &item.borrow::<RowItem>());
            }
        });
        let widget = gtk::ListView::new(Some(gtk::NoSelection::new(Some(store.clone()))), Some(factory));
        widget.set_focusable(true);
        Rc::new(Self { widget, store, shown: RefCell::new(Vec::new()) })
    }

    /// Shows `rows`. A changed row is updated in place with one `items_changed`; a different set of rows replaces the
    /// list with one splice.
    pub fn sync(&self, rows: Vec<RowItem>) {
        let change = diff(&self.shown.borrow(), &rows);
        match change {
            Diff::Same => {}
            Diff::Rows(indices) => {
                for i in indices {
                    if let Some(item) = self.store.item(i as u32).and_downcast::<glib::BoxedAnyObject>() {
                        *item.borrow_mut::<RowItem>() = rows[i].clone();
                    }
                    self.store.items_changed(i as u32, 1, 1);
                }
            }
            Diff::Reset => {
                let objects: Vec<glib::BoxedAnyObject> = rows.iter().cloned().map(glib::BoxedAnyObject::new).collect();
                self.store.splice(0, self.store.n_items(), &objects);
            }
        }
        *self.shown.borrow_mut() = rows;
    }

    /// The chapter id of the row that holds keyboard focus.
    pub fn focused_id(&self) -> Option<i64> {
        let focus = self.widget.root()?.focus()?;
        let mut cur = Some(focus);
        while let Some(w) = cur {
            if let Some(id) = w.widget_name().strip_prefix("row:").and_then(|n| n.parse().ok()) {
                return Some(id);
            }
            if let Some(id) = w.first_child().and_then(|c| c.widget_name().strip_prefix("row:").and_then(|n| n.parse().ok())) {
                return Some(id);
            }
            if w == *self.widget.upcast_ref::<gtk::Widget>() {
                break;
            }
            cur = w.parent();
        }
        None
    }

    /// Moves keyboard focus to the row of chapter `id`.
    pub fn focus_row(&self, id: i64) {
        if let Some(index) = self.shown.borrow().iter().position(|r| r.id == id) {
            self.widget.scroll_to(index as u32, gtk::ListScrollFlags::FOCUS, None);
        }
    }
}
