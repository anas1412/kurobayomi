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

use crate::entry_core::chapter_actions::{IndicatorAction, Outcome, Press, menu_label, outcome, ring};
use crate::entry_core::list_model::{DOT_SIZE, Diff, ROW_PADDING, RowItem, diff, row_alpha};
use crate::entry_core::selection::Mods;
use crate::entry_core::page::RowMenuAction;

/// A click or long press on a row: (chapter id, modifiers, long press or right click).
pub type OnRow = Rc<dyn Fn(i64, Mods, bool)>;

/// What a row reports by chapter id and action: the download action a press or a menu item of the indicator chose.
pub type OnIndicator = Rc<dyn Fn(i64, IndicatorAction)>;

/// What a row reports by chapter id alone: "Mark previous as read" from the context menu.
pub type OnChapter = Rc<dyn Fn(i64)>;

/// The row callbacks besides a click: the download indicator and the context menu's mark-previous item.
#[derive(Clone)]
pub struct RowActions {
    pub on_indicator: OnIndicator,
    pub on_previous: OnChapter,
}

/// What to run for one row when an item of its context menu is chosen.
pub type OnMenu = Rc<dyn Fn(i64, RowMenuAction)>;

/// The items of a row's context menu after Select, with their labels.
const MENU: [(RowMenuAction, &str); 3] = [
    (RowMenuAction::ToggleRead, "Mark as read"),
    (RowMenuAction::ToggleBookmark, "Bookmark chapter"),
    (RowMenuAction::Download, "Download"),
];

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

/// The icon of the three states that draw one. A queued or downloading row draws a ring instead (`chapter_actions::ring`).
fn download_icon(state: DownloadState) -> Option<&'static str> {
    match state {
        DownloadState::NotDownloaded => Some("document-save-symbolic"),
        DownloadState::Queue | DownloadState::Downloading => None,
        DownloadState::Downloaded => Some("emblem-ok-symbolic"),
        DownloadState::Error => Some("dialog-error-symbolic"),
    }
}

/// The side of the indicator in px, the size of the ring and its arrow.
const INDICATOR_SIZE: i32 = 24;

/// The determinate ring: the filled wedge of `fraction` of a turn over a faint track, and the arrow on top. The arrow takes
/// the background colour once the fill is half way round (`arrow_on_fill`), so it stays visible on it.
fn draw_ring(area: &gtk::DrawingArea, cr: &gtk::cairo::Context, w: i32, h: i32, fraction: f64, arrow_on_fill: bool) {
    let fg = area.color();
    #[allow(deprecated)]
    let bg = area.style_context().lookup_color("theme_bg_color").unwrap_or_else(|| gtk::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0));
    let (cx, cy) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
    let radius = f64::from(w.min(h)) / 2.0;
    let rgba = |c: &gtk::gdk::RGBA, alpha: f64| (f64::from(c.red()), f64::from(c.green()), f64::from(c.blue()), alpha);
    // the track
    let (r, g, b, a) = rgba(&fg, 0.25);
    cr.set_source_rgba(r, g, b, a);
    cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
    let _ = cr.fill();
    // the fill: a stroke as wide as the radius, so it is a wedge
    let (r, g, b, a) = rgba(&fg, 1.0);
    cr.set_source_rgba(r, g, b, a);
    cr.set_line_width(radius);
    cr.new_path();
    cr.arc(cx, cy, radius / 2.0, -std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2 + fraction * std::f64::consts::TAU);
    let _ = cr.stroke();
    // the arrow
    let (r, g, b, a) = if arrow_on_fill { rgba(&bg, 1.0) } else { rgba(&fg, 1.0) };
    cr.set_source_rgba(r, g, b, a);
    cr.set_line_width(2.0);
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    let reach = radius * 0.4;
    cr.move_to(cx, cy - reach);
    cr.line_to(cx, cy + reach);
    cr.move_to(cx - reach * 0.7, cy + reach * 0.3);
    cr.line_to(cx, cy + reach);
    cr.line_to(cx + reach * 0.7, cy + reach * 0.3);
    let _ = cr.stroke();
}

/// The menu a press on the indicator opens: one flat button per item, each reporting its action.
fn indicator_menu(button: &gtk::Button, id: i64, items: &[IndicatorAction], on_indicator: &OnIndicator) {
    let popover = gtk::Popover::new();
    popover.set_parent(button);
    let column = GBox::new(O::Vertical, 0);
    for &action in items {
        let entry = gtk::Button::builder().label(tr(menu_label(action))).css_classes(["flat"]).build();
        let (popover, on_indicator) = (popover.clone(), Rc::clone(on_indicator));
        entry.connect_clicked(move |_| {
            popover.popdown();
            on_indicator(id, action);
        });
        column.append(&entry);
    }
    popover.set_child(Some(&column));
    // the popover is parented to the button, so it is removed once closed
    popover.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    popover.popup();
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
    /// The indicator's pages: "icon" (`download`), "busy" (a spinner under an arrow) and "ring" (`ring`).
    indicator: gtk::Stack,
    download: gtk::Image,
    ring: gtk::DrawingArea,
    spinner: gtk::Spinner,
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
        let indicator = download_button.clone().downcast::<gtk::Button>().ok()?.child()?.downcast::<gtk::Stack>().ok()?;
        let busy = indicator.child_by_name("busy")?.downcast::<gtk::Overlay>().ok()?;
        Some(Self {
            dot: dot.downcast().ok()?,
            check: check.downcast().ok()?,
            text: text.downcast().ok()?,
            title: title.downcast().ok()?,
            subtitle: subtitle.downcast().ok()?,
            download: indicator.child_by_name("icon")?.downcast().ok()?,
            ring: indicator.child_by_name("ring")?.downcast().ok()?,
            spinner: busy.child()?.downcast().ok()?,
            indicator,
            download_button: download_button.downcast().ok()?,
            bookmark: bookmark.downcast().ok()?,
            marker: marker.downcast().ok()?,
            root,
        })
    }
}

/// The right-click menu of a row: Select, the read, bookmark and download items, then Mark previous as read.
fn context_menu(root: &GBox, list_item: &gtk::ListItem, on_menu: &OnMenu, on_row: &OnRow, on_previous: &OnChapter, at: (f64, f64)) {
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
    for (action, label) in MENU {
        let run = Rc::clone(on_menu);
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
    let ring = gtk::DrawingArea::builder().content_width(INDICATOR_SIZE).content_height(INDICATOR_SIZE).build();
    let spinner = gtk::Spinner::builder().width_request(INDICATOR_SIZE).height_request(INDICATOR_SIZE).build();
    let arrow = gtk::Image::from_icon_name("go-down-symbolic");
    arrow.set_pixel_size(INDICATOR_SIZE / 2);
    arrow.set_halign(Align::Center);
    arrow.set_valign(Align::Center);
    let busy = gtk::Overlay::new();
    busy.set_child(Some(&spinner));
    busy.add_overlay(&arrow);
    let indicator = gtk::Stack::new();
    indicator.add_named(&download, Some("icon"));
    indicator.add_named(&busy, Some("busy"));
    indicator.add_named(&ring, Some("ring"));
    let download_button = gtk::Button::builder().child(&indicator).css_classes(["flat", "circular"]).focus_on_click(false).valign(Align::Center).build();
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
    RowWidgets { root, dot, check, text, title, subtitle, download_button, indicator, download, ring, spinner, bookmark, marker }
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
    w.download_button.set_visible(row.indicator);
    // the indicator answers presses only while nothing is selected
    w.download_button.set_sensitive(row.indicator_enabled);
    w.spinner.set_spinning(false);
    match (ring(row.download, row.progress), download_icon(row.download)) {
        (Some(ring), _) if ring.indeterminate => {
            w.spinner.set_spinning(true);
            w.indicator.set_visible_child_name("busy");
        }
        (Some(ring), _) => {
            let area = w.ring.clone();
            w.ring.set_draw_func(move |_, cr, width, height| draw_ring(&area, cr, width, height, ring.fraction, ring.arrow_on_fill));
            w.ring.queue_draw();
            w.indicator.set_visible_child_name("ring");
        }
        (None, Some(icon)) => {
            w.download.set_icon_name(Some(icon));
            w.indicator.set_visible_child_name("icon");
        }
        (None, None) => {}
    }
    w.download_button.set_tooltip_text((row.progress > 0 && row.indicator).then(|| format!("{}%", row.progress)).as_deref());
}

impl ChapterList {
    pub fn new(on_row: OnRow, on_menu: OnMenu, actions: RowActions) -> Rc<Self> {
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
                let (list_item, on_row, on_menu, root, actions) = (list_item.clone(), Rc::clone(&on_row), Rc::clone(&on_menu), widgets.root.clone(), actions.clone());
                move |_, _, x, y| context_menu(&root, &list_item, &on_menu, &on_row, &actions.on_previous, (x, y))
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
            // a press on the indicator follows `chapter_actions::outcome` by the state of its row: an action at once, or a menu
            let press_indicator = {
                let (list_item, on_indicator, button) = (list_item.clone(), Rc::clone(&actions.on_indicator), widgets.download_button.clone());
                Rc::new(move |press: Press| {
                    let Some(item) = item_of(&list_item) else { return };
                    let (id, state) = {
                        let row = item.borrow::<RowItem>();
                        (row.id, row.download)
                    };
                    match outcome(state, press) {
                        Outcome::Run(action) => on_indicator(id, action),
                        Outcome::Menu(items) => indicator_menu(&button, id, items, &on_indicator),
                    }
                })
            };
            let indicator_long_fired = Rc::new(Cell::new(false));
            widgets.download_button.connect_clicked({
                let (press_indicator, long_fired) = (Rc::clone(&press_indicator), Rc::clone(&indicator_long_fired));
                move |_| {
                    // the release that ends a long press is not a click
                    if !long_fired.replace(false) {
                        press_indicator(Press::Click);
                    }
                }
            });
            let indicator_long = gtk::GestureLongPress::new();
            indicator_long.connect_pressed({
                let (press_indicator, long_fired) = (Rc::clone(&press_indicator), Rc::clone(&indicator_long_fired));
                move |g, _, _| {
                    // a long press on the indicator is its own, not the row's selection toggle
                    g.set_state(gtk::EventSequenceState::Claimed);
                    long_fired.set(true);
                    press_indicator(Press::LongPress);
                }
            });
            widgets.download_button.add_controller(indicator_long);
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
