//! The episode list widget: a `gio::ListStore` behind a `gtk::ListView`, so only the rows on screen exist (3000 rows
//! cost the same as 30). The store holds leading slots (the info block and the item header, which scroll with the
//! list in the single-column layout) followed by one item per episode row. `sync` diffs the new rows against the
//! shown ones: one changed row gets one `items_changed`, several changed rows (select all, mark many) and a changed
//! structure get one `splice` each, so a bulk update is one signal. Rows report clicks, long presses and right clicks;
//! the page decides what they mean (`selection::Selection`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Label, Orientation as O, gio, glib, pango};
use kuro_i18n::{MsgKey, tr_plural};

use super::actions::swipe_action;
use super::episode_row::{Diff, EpisodeRow, Line, PADDING, RowLayout, changed_span, diff_lines, info_alpha, title_alpha, title_gap};
use super::swipe_look::{MenuItem, SwipeLook, reveal};
use crate::entry_core::selection::Mods;
use crate::entry_core::swipe::{Side, side_of_drag};

/// The swipe actions of a row (T-39b-015): the stored actions, and what to run for one row on a touch drag.
#[derive(Clone)]
pub struct EpisodeSwipe {
    pub prefs: Rc<kuro_prefs::library::LibraryPrefs>,
    pub run: Rc<dyn Fn(i64, kuro_prefs::library::EpisodeSwipeAction)>,
    /// The icon and undo state of an action on one episode, for the strip a drag reveals (`swipe_look::look`).
    pub look: Rc<dyn Fn(i64, kuro_prefs::library::EpisodeSwipeAction) -> Option<SwipeLook>>,
    /// The items of the row's context menu for one episode (`swipe_look::menu`): the actions of the swipe, by click.
    pub menu: Rc<dyn Fn(i64) -> Vec<MenuItem>>,
}

/// A press on a row: (episode id, long press or right click, Alt held, Shift and Ctrl held). Alt opens in the other player;
/// Shift selects a range and Ctrl toggles one row (T-39a-055).
pub type OnRow = Rc<dyn Fn(i64, bool, bool, Mods)>;

/// Summary lines a collapsed summary shows (T-39a-012).
const COLLAPSED_LINES: i32 = super::summary::COLLAPSED_LINES;

/// The class of the filler label and the bookmark icon colours. They resolve the theme's `kuro_tertiary` and
/// `kuro_primary` named colours, which GTK looks up across every provider of the display.
const CSS: &str = ".episode-primary { color: @kuro_primary; } .episode-tertiary { color: @kuro_tertiary; } \
    .episode-swipe { background: @kuro_primary_container; color: @kuro_on_primary_container; } \
    .episode-swipe-idle { background: @kuro_surface_container_lowest; color: @kuro_on_surface; }";

fn ensure_css() {
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

/// A thumbnail is 40% of the window wide, at most 250 px (behaviour 3.7), and 16:9.
pub fn thumbnail_size(window_width: i32) -> (i32, i32) {
    let width = (window_width * 2 / 5).clamp(1, 250);
    (width, width * 9 / 16)
}

/// One item of the store.
#[derive(Clone)]
enum Item {
    /// The n-th leading widget.
    Slot(usize),
    Episode(EpisodeRow),
    /// The missing-count separator with the number of episodes it stands for (T-39a-015).
    Missing(i32),
}

/// The store item of one line of the list.
fn item_for(line: &Line) -> Item {
    match line {
        Line::Episode(row) => Item::Episode(row.clone()),
        Line::Missing { count, .. } => Item::Missing(*count),
    }
}

fn item_of(list_item: &gtk::ListItem) -> Option<Item> {
    list_item.item().and_downcast::<glib::BoxedAnyObject>().map(|o| o.borrow::<Item>().clone())
}

/// The widgets of one recycled list item: the episode row, or a host for a leading slot.
struct RowWidgets {
    stack: gtk::Stack,
    row: GBox,
    check: gtk::Image,
    thumb: gtk::Frame,
    text: GBox,
    title: Label,
    filler_icon: gtk::Image,
    filler: Label,
    info: Label,
    info_line: GBox,
    summary: Label,
    bookmark: gtk::Image,
    slot: GBox,
    /// The strip a drag reveals on the side it heads for (T-39a-017), over the row.
    reveal: GBox,
    reveal_icon: gtk::Image,
}

fn row_widgets() -> RowWidgets {
    ensure_css();
    let [start, top, end, bottom] = PADDING;
    let row = GBox::new(O::Horizontal, 8);
    row.set_margin_start(start);
    row.set_margin_top(top);
    row.set_margin_end(end);
    row.set_margin_bottom(bottom);
    let check = gtk::Image::from_icon_name("object-select-symbolic");
    let thumb = gtk::Frame::new(None);
    thumb.set_valign(Align::Start);
    let text = GBox::new(O::Vertical, 0);
    text.set_hexpand(true);
    text.set_valign(Align::Center);
    let title = Label::builder().xalign(0.0).ellipsize(pango::EllipsizeMode::End).build();
    let filler_icon = gtk::Image::from_icon_name("tag-symbolic");
    filler_icon.add_css_class("episode-tertiary");
    let filler = Label::builder().label(kuro_i18n::tr(kuro_i18n::MsgKey::filler)).css_classes(["caption", "episode-tertiary"]).build();
    let info = Label::builder().xalign(0.0).ellipsize(pango::EllipsizeMode::End).css_classes(["caption"]).build();
    let info_line = GBox::new(O::Horizontal, 4);
    info_line.append(&filler_icon);
    info_line.append(&filler);
    info_line.append(&info);
    let summary = Label::builder().xalign(0.0).wrap(true).lines(3).ellipsize(pango::EllipsizeMode::End).css_classes(["caption"]).build();
    let bookmark = gtk::Image::from_icon_name("user-bookmarks-symbolic");
    bookmark.add_css_class("episode-primary");
    for w in [check.upcast_ref::<gtk::Widget>(), bookmark.upcast_ref()] {
        w.set_valign(Align::Center);
    }
    text.append(&title);
    text.append(&info_line);
    text.append(&summary);
    row.append(&check);
    row.append(&thumb);
    row.append(&text);
    row.append(&bookmark);
    let slot = GBox::new(O::Vertical, 0);
    let reveal_icon = gtk::Image::new();
    reveal_icon.set_pixel_size(24);
    let reveal = GBox::new(O::Horizontal, 0);
    reveal.set_halign(Align::Start);
    reveal.set_can_target(false);
    reveal.set_visible(false);
    reveal.set_overflow(gtk::Overflow::Hidden);
    reveal_icon.set_margin_start(16);
    reveal_icon.set_margin_end(16);
    reveal.append(&reveal_icon);
    let host = gtk::Overlay::new();
    host.set_child(Some(&row));
    host.add_overlay(&reveal);
    let stack = gtk::Stack::new();
    stack.add_named(&host, Some("row"));
    stack.add_named(&slot, Some("slot"));
    RowWidgets { stack, row, check, thumb, text, title, filler_icon, filler, info, info_line, summary, bookmark, slot, reveal, reveal_icon }
}

impl RowWidgets {
    fn show_row(&self, row: &EpisodeRow, window_width: i32) {
        self.empty_slot();
        self.stack.set_visible_child_name("row");
        self.row.set_widget_name(&format!("row:{}", row.id));
        self.check.set_opacity(if row.selected { 1.0 } else { 0.0 });
        self.title.set_label(&row.title);
        self.title.set_opacity(title_alpha(row.seen));
        let alpha = info_alpha(row.seen);
        for w in [self.info_line.upcast_ref::<gtk::Widget>(), self.summary.upcast_ref()] {
            w.set_opacity(alpha);
        }
        self.filler_icon.set_visible(row.indicators.filler);
        self.filler.set_visible(row.indicators.filler);
        self.info.set_label(&row.info);
        self.info.set_visible(!row.info.is_empty());
        self.info_line.set_visible(row.indicators.filler || !row.info.is_empty());
        self.bookmark.set_visible(row.indicators.bookmark);
        match row.layout {
            RowLayout::Simple => {
                // one title line; the gap under it is 6 px, or 0 when the filler label sits there
                self.text.set_spacing(0);
                self.title.set_wrap(false);
                self.title.set_lines(1);
                self.info_line.set_margin_top(title_gap(row.indicators.filler));
            }
            RowLayout::Rich { title_lines } => {
                self.text.set_spacing(2);
                self.title.set_wrap(true);
                self.title.set_lines(i32::from(title_lines));
                self.info_line.set_margin_top(0);
            }
        }
        let rich = matches!(row.layout, RowLayout::Rich { .. });
        match (&row.preview_url, rich) {
            (Some(_), true) => {
                let (w, h) = thumbnail_size(window_width);
                self.thumb.set_size_request(w, h);
                self.thumb.set_visible(true);
            }
            _ => self.thumb.set_visible(false),
        }
        self.summary.set_visible(row.summary.is_some());
        self.summary.set_label(row.summary.as_deref().unwrap_or(""));
        self.summary.set_lines(if row.summary_expanded { -1 } else { COLLAPSED_LINES });
    }

    /// A separator row: "Missing N items", dimmed, in the slot.
    fn show_missing(&self, count: i32) {
        let label = Label::builder().label(tr_plural(MsgKey::missing_items, i64::from(count), &[])).xalign(0.0).css_classes(["dim-label", "caption"]).build();
        label.set_margin_start(16);
        label.set_margin_top(8);
        label.set_margin_bottom(8);
        self.show_slot(label.upcast_ref());
    }

    fn show_slot(&self, widget: &gtk::Widget) {
        self.empty_slot();
        self.row.set_widget_name("");
        if let Some(parent) = widget.parent().and_downcast::<GBox>() {
            parent.remove(widget);
        }
        self.slot.append(widget);
        self.stack.set_visible_child_name("slot");
    }

    fn empty_slot(&self) {
        while let Some(child) = self.slot.first_child() {
            self.slot.remove(&child);
        }
    }

    /// Draws the strip of a drag in progress: the icon of the action of the side it heads for, on `primaryContainer`
    /// once past the threshold and on the dimmed background before. No action (Disabled) draws nothing.
    fn show_reveal(&self, dx: f64, dy: f64, looks: (Option<SwipeLook>, Option<SwipeLook>)) {
        let Some(strip) = reveal(dx, dy) else { return self.hide_reveal() };
        let look = match strip.side {
            Side::Left => looks.0,
            Side::Right => looks.1,
        };
        let Some(look) = look else { return self.hide_reveal() };
        self.reveal_icon.set_icon_name(Some(look.icon));
        self.reveal.set_halign(if strip.side == Side::Right { Align::Start } else { Align::End });
        self.reveal.set_width_request(strip.width);
        self.reveal.remove_css_class(if strip.armed { "episode-swipe-idle" } else { "episode-swipe" });
        self.reveal.add_css_class(if strip.armed { "episode-swipe" } else { "episode-swipe-idle" });
        self.reveal.set_visible(true);
    }

    fn hide_reveal(&self) {
        self.reveal.set_visible(false);
    }
}

/// The context menu of a row: the swipe actions as buttons, shown at the pointer. A press runs the action and closes it.
fn show_menu(parent: &GBox, x: f64, y: f64, id: i64, swipe: &EpisodeSwipe) {
    let popover = gtk::Popover::new();
    let col = GBox::new(O::Vertical, 2);
    for item in (swipe.menu)(id) {
        let content = GBox::new(O::Horizontal, 8);
        if !item.icon.is_empty() {
            content.append(&gtk::Image::from_icon_name(item.icon));
        }
        content.append(&Label::new(Some(&kuro_i18n::tr(item.label))));
        let button = gtk::Button::builder().child(&content).css_classes(["flat"]).build();
        let (run, popover) = (Rc::clone(&swipe.run), popover.clone());
        button.connect_clicked(move |_| {
            popover.popdown();
            run(id, item.action);
        });
        col.append(&button);
    }
    popover.set_child(Some(&col));
    popover.set_parent(parent);
    popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    // a popover parented to a recycled row must be unparented when it closes
    popover.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    popover.popup();
}

/// Brings the episode part of `store` (everything from `offset`) from `shown` to `rows`. One changed row is updated in
/// place with one `items_changed`, so its widget is kept. Several changed rows are replaced by one `splice` over their
/// span, and a different set of rows by one `splice` over the whole episode part.
fn sync_store(store: &gio::ListStore, offset: u32, shown: &[Line], rows: &[Line]) {
    let item_of = |line: &Line| glib::BoxedAnyObject::new(item_for(line));
    match diff_lines(shown, rows) {
        Diff::Same => {}
        Diff::Rows(indices) if indices.len() == 1 => {
            let at = offset + indices[0] as u32;
            if let Some(item) = store.item(at).and_downcast::<glib::BoxedAnyObject>() {
                *item.borrow_mut::<Item>() = item_for(&rows[indices[0]]);
            }
            store.items_changed(at, 1, 1);
        }
        Diff::Rows(indices) => {
            if let Some((start, len)) = changed_span(&indices) {
                let objects: Vec<glib::BoxedAnyObject> = rows[start..start + len].iter().map(item_of).collect();
                store.splice(offset + start as u32, len as u32, &objects);
            }
        }
        Diff::Reset => {
            let objects: Vec<glib::BoxedAnyObject> = rows.iter().map(item_of).collect();
            store.splice(offset, store.n_items() - offset, &objects);
        }
    }
}

pub struct EpisodeList {
    pub widget: gtk::ListView,
    store: gio::ListStore,
    leading: RefCell<Vec<gtk::Widget>>,
    shown: RefCell<Vec<Line>>,
}

impl EpisodeList {
    /// `on_row` gets row presses. `on_activate` gets the id of the row that Enter or a double click activated.
    /// `on_summary` gets the id of an episode whose summary was tapped (T-39a-012).
    pub fn new(on_row: OnRow, on_activate: Rc<dyn Fn(i64)>, on_summary: Rc<dyn Fn(i64)>, swipe: Option<EpisodeSwipe>) -> Rc<Self> {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let registry: Rc<RefCell<HashMap<usize, Rc<RowWidgets>>>> = Rc::default();
        let list: Rc<RefCell<Option<std::rc::Weak<Self>>>> = Rc::default();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup({
            let registry = Rc::clone(&registry);
            let swipe = swipe.clone();
            move |_, object| {
                let Some(list_item) = object.downcast_ref::<gtk::ListItem>() else { return };
                let widgets = Rc::new(row_widgets());
                let long_fired = Rc::new(Cell::new(false));
                let press = |list_item: &gtk::ListItem, on_row: &OnRow, long: bool, state: gtk::gdk::ModifierType| {
                    if let Some(Item::Episode(row)) = item_of(list_item) {
                        let mods = Mods { shift: state.contains(gtk::gdk::ModifierType::SHIFT_MASK), ctrl: state.contains(gtk::gdk::ModifierType::CONTROL_MASK) };
                        on_row(row.id, long, state.contains(gtk::gdk::ModifierType::ALT_MASK), mods);
                    }
                };
                let left = gtk::GestureClick::builder().button(1).build();
                left.connect_released({
                    let (list_item, on_row, long_fired) = (list_item.clone(), Rc::clone(&on_row), Rc::clone(&long_fired));
                    move |g, _, _, _| {
                        if !long_fired.replace(false) {
                            press(&list_item, &on_row, false, g.current_event_state());
                        }
                    }
                });
                // a right click opens the menu of the swipe actions; without them (no installed app) it selects the row
                let right = gtk::GestureClick::builder().button(3).build();
                right.connect_pressed({
                    let (list_item, on_row, swipe, widgets) = (list_item.clone(), Rc::clone(&on_row), swipe.clone(), Rc::clone(&widgets));
                    move |_, _, x, y| match (&swipe, item_of(&list_item)) {
                        (Some(swipe), Some(Item::Episode(row))) => show_menu(&widgets.row, x, y, row.id, swipe),
                        _ => press(&list_item, &on_row, true, gtk::gdk::ModifierType::empty()),
                    }
                });
                let long = gtk::GestureLongPress::new();
                long.connect_pressed({
                    let (list_item, on_row, long_fired) = (list_item.clone(), Rc::clone(&on_row), Rc::clone(&long_fired));
                    move |_, _, _| {
                        long_fired.set(true);
                        press(&list_item, &on_row, true, gtk::gdk::ModifierType::empty());
                    }
                });
                for c in [left.upcast_ref::<gtk::EventController>(), right.upcast_ref(), long.upcast_ref()] {
                    widgets.row.add_controller(c.clone());
                }
                // a tap on the summary toggles it and claims the press, so the row does not open (T-39a-012)
                let summary_tap = gtk::GestureClick::builder().button(1).build();
                summary_tap.connect_released({
                    let (list_item, on_summary) = (list_item.clone(), Rc::clone(&on_summary));
                    move |gesture, _, _, _| {
                        gesture.set_state(gtk::EventSequenceState::Claimed);
                        if let Some(Item::Episode(row)) = item_of(&list_item) {
                            on_summary(row.id);
                        }
                    }
                });
                widgets.summary.add_controller(summary_tap);
                if let Some(swipe) = swipe.clone() {
                    // a horizontal drag on a touch screen reveals the action of the side it heads for, and the release
                    // past the threshold runs it, selected or not
                    let drag = gtk::GestureDrag::new();
                    drag.set_touch_only(true);
                    let looks: Rc<Cell<(Option<SwipeLook>, Option<SwipeLook>)>> = Rc::default();
                    drag.connect_drag_begin({
                        let (list_item, swipe, looks) = (list_item.clone(), swipe.clone(), Rc::clone(&looks));
                        move |_, _, _| {
                            if let Some(Item::Episode(row)) = item_of(&list_item) {
                                let of = |side| (swipe.look)(row.id, swipe_action(&swipe.prefs, side));
                                looks.set((of(Side::Left), of(Side::Right)));
                            }
                        }
                    });
                    drag.connect_drag_update({
                        let (widgets, looks) = (Rc::clone(&widgets), Rc::clone(&looks));
                        move |_, dx, dy| widgets.show_reveal(dx, dy, looks.get())
                    });
                    drag.connect_cancel({
                        let widgets = Rc::clone(&widgets);
                        move |_, _| widgets.hide_reveal()
                    });
                    let ended = Rc::clone(&widgets);
                    let list_item = list_item.clone();
                    drag.connect_drag_end(move |_, dx, dy| {
                        ended.hide_reveal();
                        let (Some(side), Some(Item::Episode(row))) = (side_of_drag(dx, dy), item_of(&list_item)) else { return };
                        (swipe.run)(row.id, swipe_action(&swipe.prefs, side));
                    });
                    widgets.row.add_controller(drag);
                }
                list_item.set_child(Some(&widgets.stack));
                registry.borrow_mut().insert(list_item.as_ptr() as usize, widgets);
            }
        });
        factory.connect_bind({
            let (registry, list) = (Rc::clone(&registry), Rc::clone(&list));
            move |_, object| {
                let Some(list_item) = object.downcast_ref::<gtk::ListItem>() else { return };
                let (Some(item), Some(widgets)) = (item_of(list_item), registry.borrow().get(&(list_item.as_ptr() as usize)).cloned()) else { return };
                let Some(this) = list.borrow().as_ref().and_then(std::rc::Weak::upgrade) else { return };
                match item {
                    Item::Episode(row) => {
                        let width = this.widget.root().map_or(1180, |r| r.width());
                        widgets.show_row(&row, width);
                    }
                    Item::Missing(count) => widgets.show_missing(count),
                    Item::Slot(i) => {
                        if let Some(w) = this.leading.borrow().get(i) {
                            widgets.show_slot(w);
                        }
                    }
                }
            }
        });
        factory.connect_unbind({
            let registry = Rc::clone(&registry);
            move |_, object| {
                if let Some(list_item) = object.downcast_ref::<gtk::ListItem>()
                    && let Some(widgets) = registry.borrow().get(&(list_item.as_ptr() as usize))
                {
                    widgets.empty_slot();
                }
            }
        });
        factory.connect_teardown({
            let registry = Rc::clone(&registry);
            move |_, object| {
                if let Some(list_item) = object.downcast_ref::<gtk::ListItem>() {
                    registry.borrow_mut().remove(&(list_item.as_ptr() as usize));
                }
            }
        });
        let widget = gtk::ListView::new(Some(gtk::NoSelection::new(Some(store.clone()))), Some(factory));
        widget.set_focusable(true);
        let this = Rc::new(Self { widget, store, leading: RefCell::new(Vec::new()), shown: RefCell::new(Vec::new()) });
        *list.borrow_mut() = Some(Rc::downgrade(&this));
        this.widget.connect_activate({
            let weak = Rc::downgrade(&this);
            move |_, position| {
                if let Some(this) = weak.upgrade()
                    && let Some(item) = this.store.item(position).and_downcast::<glib::BoxedAnyObject>()
                    && let Item::Episode(row) = item.borrow::<Item>().clone()
                {
                    on_activate(row.id);
                }
            }
        });
        this
    }

    /// Replaces the leading slots (widgets that scroll with the rows). One splice of the head of the store.
    pub fn set_leading(&self, widgets: Vec<gtk::Widget>) {
        let old = self.leading.borrow().len() as u32;
        let objects: Vec<glib::BoxedAnyObject> = (0..widgets.len()).map(|i| glib::BoxedAnyObject::new(Item::Slot(i))).collect();
        *self.leading.borrow_mut() = widgets;
        self.store.splice(0, old, &objects);
    }

    /// Shows `rows`: see `sync_store`.
    pub fn sync(&self, rows: Vec<Line>) {
        let offset = self.leading.borrow().len() as u32;
        sync_store(&self.store, offset, &self.shown.borrow(), &rows);
        *self.shown.borrow_mut() = rows;
    }

    /// The model, for tests and for the page's scroll tracking.
    pub fn n_items(&self) -> u32 {
        self.store.n_items()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn rows(n: i64) -> Vec<EpisodeRow> {
        (1..=n)
            .map(|id| EpisodeRow {
                id,
                title: format!("Episode {id}"),
                info: String::new(),
                seen: false,
                layout: RowLayout::Simple,
                indicators: super::super::episode_row::Indicators { bookmark: false, filler: false },
                preview_url: None,
                summary: None,
                summary_expanded: false,
                selected: false,
            })
            .collect()
    }

    fn lines(rows: &[EpisodeRow]) -> Vec<Line> {
        rows.iter().cloned().map(Line::Episode).collect()
    }

    /// A store with two leading slots and `shown`, and a counter of its `items_changed` signals.
    fn store_with(shown: &[EpisodeRow]) -> (gio::ListStore, Rc<Cell<u32>>) {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let items: Vec<glib::BoxedAnyObject> = (0..2).map(|i| glib::BoxedAnyObject::new(Item::Slot(i))).collect();
        store.splice(0, 0, &items);
        let items: Vec<glib::BoxedAnyObject> = shown.iter().cloned().map(|r| glib::BoxedAnyObject::new(Item::Episode(r))).collect();
        store.splice(2, 0, &items);
        let signals = Rc::new(Cell::new(0));
        store.connect_items_changed({
            let signals = Rc::clone(&signals);
            move |_, _, _, _| signals.set(signals.get() + 1)
        });
        (store, signals)
    }

    #[test]
    fn one_changed_row_is_one_signal_and_an_identical_list_is_none() {
        let shown = rows(5);
        let (store, signals) = store_with(&shown);
        let mut next = shown.clone();
        next[3].seen = true;
        sync_store(&store, 2, &lines(&shown), &lines(&next));
        assert_eq!(signals.get(), 1);
        sync_store(&store, 2, &lines(&next), &lines(&next));
        assert_eq!(signals.get(), 1, "nothing changed");
    }

    #[test]
    fn a_bulk_change_is_one_splice_that_keeps_the_list_the_same_length() {
        let shown = rows(300);
        let (store, signals) = store_with(&shown);
        let all_selected: Vec<EpisodeRow> = shown.iter().cloned().map(|r| EpisodeRow { selected: true, ..r }).collect();
        sync_store(&store, 2, &lines(&shown), &lines(&all_selected));
        assert_eq!(signals.get(), 1, "select all is one signal, not 300");
        assert_eq!(store.n_items(), 302);
        let last = store.item(301).and_downcast::<glib::BoxedAnyObject>().unwrap();
        assert!(matches!(&*last.borrow::<Item>(), Item::Episode(r) if r.selected));
        let slot = store.item(1).and_downcast::<glib::BoxedAnyObject>().unwrap();
        assert!(matches!(&*slot.borrow::<Item>(), Item::Slot(1)), "the leading slots are untouched");
    }

    #[test]
    fn a_different_set_of_rows_is_one_splice_over_the_episode_part() {
        let shown = rows(4);
        let (store, signals) = store_with(&shown);
        sync_store(&store, 2, &lines(&shown), &lines(&rows(2)));
        assert_eq!((signals.get(), store.n_items()), (1, 4));
    }

    #[test]
    fn thumbnails_take_40_percent_of_the_window_up_to_250_px() {
        assert_eq!(thumbnail_size(360), (144, 81));
        assert_eq!(thumbnail_size(1180).0, 250);
        assert_eq!(thumbnail_size(625).0, 250);
        assert_eq!(thumbnail_size(0).0, 1);
    }
}
