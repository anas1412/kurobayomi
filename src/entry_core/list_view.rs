//! The chapter list widget: a `gio::ListStore` of `RowItem`s behind a `gtk::ListView`. `sync` diffs the new rows
//! against the shown ones (`list_model::diff`): a changed row gets one `items_changed`, a changed structure gets one
//! splice. Rows report clicks and long presses; keys are handled by the page.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gdk::ModifierType;
use gtk::prelude::*;
use gtk::{Align, Box as GBox, Label, Orientation as O, gio, glib};
use kuro_downloads::DownloadState;

use kuro_prefs::library::{ChapterSwipeAction, LibraryPrefs};

use crate::entry_core::list_model::{Diff, RowItem, diff};
use crate::entry_core::selection::Mods;
use crate::entry_core::swipe::{action_for, menu, side_of_drag};

/// A click or long press on a row: (chapter id, modifiers, long press or right click).
pub type OnRow = Rc<dyn Fn(i64, Mods, bool)>;

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
        DownloadState::NotDownloaded => None,
        DownloadState::Queue => Some("content-loading-symbolic"),
        DownloadState::Downloading => Some("folder-download-symbolic"),
        DownloadState::Downloaded => Some("emblem-ok-symbolic"),
        DownloadState::Error => Some("dialog-error-symbolic"),
    }
}

/// The row widget: check mark, title and subtitle, download icon, bookmark icon.
struct RowWidgets {
    root: GBox,
    check: gtk::Image,
    title: Label,
    subtitle: Label,
    download: gtk::Image,
    bookmark: gtk::Image,
}

impl RowWidgets {
    /// Finds the inner widgets again from the root, in the order `row_widgets` appended them.
    fn of(root: &gtk::Widget) -> Option<Self> {
        let root = root.clone().downcast::<GBox>().ok()?;
        let check = root.first_child()?;
        let text = check.next_sibling()?;
        let title = text.first_child()?;
        let subtitle = title.next_sibling()?;
        let download = text.next_sibling()?;
        let bookmark = download.next_sibling()?;
        Some(Self {
            check: check.downcast().ok()?,
            title: title.downcast().ok()?,
            subtitle: subtitle.downcast().ok()?,
            download: download.downcast().ok()?,
            bookmark: bookmark.downcast().ok()?,
            root,
        })
    }
}

/// The right-click menu of a row: Select, then each swipe action that runs on this page.
fn context_menu(root: &GBox, list_item: &gtk::ListItem, swipe: &Swipe, on_row: &OnRow, at: (f64, f64)) {
    let Some(item) = item_of(list_item) else { return };
    let id = item.borrow::<RowItem>().id;
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
    popover.set_child(Some(&column));
    // the popover is parented to the row, so it is removed once closed
    popover.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    popover.popup();
}

fn row_widgets() -> RowWidgets {
    let root = GBox::new(O::Horizontal, 10);
    root.set_margin_top(8);
    root.set_margin_bottom(8);
    root.set_margin_start(12);
    root.set_margin_end(12);
    let check = gtk::Image::from_icon_name("object-select-symbolic");
    let text = GBox::new(O::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(Align::Center);
    let title = Label::builder().xalign(0.0).wrap(true).build();
    let subtitle = Label::builder().xalign(0.0).css_classes(["dim-label", "caption"]).build();
    text.append(&title);
    text.append(&subtitle);
    let download = gtk::Image::new();
    let bookmark = gtk::Image::from_icon_name("starred-symbolic");
    for w in [check.upcast_ref::<gtk::Widget>(), download.upcast_ref(), bookmark.upcast_ref()] {
        w.set_valign(Align::Center);
    }
    root.append(&check);
    root.append(&text);
    root.append(&download);
    root.append(&bookmark);
    RowWidgets { root, check, title, subtitle, download, bookmark }
}

fn bind(w: &RowWidgets, row: &RowItem) {
    w.root.set_widget_name(&format!("row:{}", row.id));
    w.title.set_label(&row.title);
    w.subtitle.set_label(&row.subtitle);
    w.subtitle.set_visible(!row.subtitle.is_empty());
    w.check.set_opacity(if row.selected { 1.0 } else { 0.0 });
    w.bookmark.set_visible(row.bookmark);
    w.root.set_opacity(if row.read { 0.6 } else { 1.0 });
    match download_icon(row.download) {
        Some(icon) => {
            w.download.set_icon_name(Some(icon));
            w.download.set_visible(true);
            w.download.set_tooltip_text((row.progress > 0).then(|| format!("{}%", row.progress)).as_deref());
        }
        None => w.download.set_visible(false),
    }
}

impl ChapterList {
    pub fn new(on_row: OnRow, swipe: Option<Swipe>) -> Rc<Self> {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, object| {
            let Some(list_item) = object.downcast_ref::<gtk::ListItem>() else { return };
            let widgets = row_widgets();
            let long_fired = Rc::new(Cell::new(false));
            let report = |list_item: &gtk::ListItem, on_row: &OnRow, state: ModifierType, long: bool| {
                if let Some(item) = item_of(list_item) {
                    on_row(item.borrow::<RowItem>().id, mods_of(state), long);
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
                let (list_item, on_row, swipe, root) = (list_item.clone(), Rc::clone(&on_row), swipe.clone(), widgets.root.clone());
                move |g, _, x, y| match &swipe {
                    // the context menu holds Select and the swipe actions of the two sides
                    Some(swipe) => context_menu(&root, &list_item, swipe, &on_row, (x, y)),
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
                    (swipe.run)(item.borrow::<RowItem>().id, action_for(&swipe.prefs, side));
                });
                widgets.root.add_controller(drag);
            }
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
