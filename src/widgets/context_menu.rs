//! The one right-click menu of the app. Every list row, cover and text that has actions gets them through `attach`: a
//! right-click (and, for touch screens, a long press) opens a native GTK menu at the pointer, built fresh each time from
//! the items the caller returns, so the labels always match the current state (Mark as read / Mark as unread). The Menu
//! key and Shift+F10 open the same menu from the keyboard. An item can carry a check mark or open a submenu. A widget
//! whose long press means something else (chapter and episode rows select on it) uses `attach_without_long_press`.

use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

/// One menu entry. Entries with the same `section` number are grouped between separators, in ascending order.
pub struct Item {
    pub label: String,
    pub section: u8,
    pub run: Rc<dyn Fn()>,
    /// `Some` draws a check mark slot, filled when true (the current choice of a list).
    pub checked: Option<bool>,
    /// Not empty: the entry opens this submenu instead of running `run`.
    pub children: Vec<Item>,
}

impl Item {
    pub fn new(label: impl Into<String>, section: u8, run: impl Fn() + 'static) -> Self {
        Self { label: label.into(), section, run: Rc::new(run), checked: None, children: Vec::new() }
    }

    /// An entry that opens `children` as a submenu.
    pub fn submenu(label: impl Into<String>, section: u8, children: Vec<Item>) -> Self {
        Self { children, ..Self::new(label, section, || {}) }
    }

    /// The same entry with a check mark that is on when `on` is true.
    pub fn checked(self, on: bool) -> Self {
        Self { checked: Some(on), ..self }
    }
}

/// Opens a menu of `items(x, y)` on `widget` at the click position: on a right click, a touch long press, or the
/// Menu key. An empty list opens nothing. The handler must not keep any borrow of the row's data while it runs an item:
/// items run after the menu has closed, on idle.
pub fn attach(widget: &impl IsA<gtk::Widget>, items: impl Fn(f64, f64) -> Vec<Item> + 'static) {
    install(widget.as_ref(), Rc::new(items), true);
}

/// [`attach`] without the touch long press, for a widget whose long press has its own meaning.
pub fn attach_without_long_press(widget: &impl IsA<gtk::Widget>, items: impl Fn(f64, f64) -> Vec<Item> + 'static) {
    install(widget.as_ref(), Rc::new(items), false);
}

fn install(widget: &gtk::Widget, items: Rc<dyn Fn(f64, f64) -> Vec<Item>>, long_press: bool) {
    let widget = widget.clone();
    let open: Rc<dyn Fn(f64, f64)> = {
        let widget = widget.downgrade();
        Rc::new(move |x, y| {
            if let Some(widget) = widget.upgrade() {
                show(&widget, items(x, y), x, y);
            }
        })
    };

    let right = gtk::GestureClick::builder().button(gtk::gdk::BUTTON_SECONDARY).build();
    let on_right = Rc::clone(&open);
    right.connect_pressed(move |gesture, _, x, y| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        on_right(x, y);
    });
    widget.add_controller(right);

    if long_press {
        let long = gtk::GestureLongPress::new();
        long.set_touch_only(true);
        let on_long = Rc::clone(&open);
        long.connect_pressed(move |gesture, x, y| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            on_long(x, y);
        });
        widget.add_controller(long);
    }

    // the keyboard's way: the Menu key or Shift+F10 opens the menu at the middle of the widget
    let keys = gtk::EventControllerKey::new();
    let target = widget.downgrade();
    keys.connect_key_pressed(move |_, key, _, state| {
        if !is_menu_key(key, state) {
            return gtk::glib::Propagation::Proceed;
        }
        if let Some(widget) = target.upgrade() {
            open(f64::from(widget.width()) / 2.0, f64::from(widget.height()) / 2.0);
        }
        gtk::glib::Propagation::Stop
    });
    widget.add_controller(keys);
}

/// The Menu key, or Shift+F10.
fn is_menu_key(key: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool {
    key == gtk::gdk::Key::Menu || (key == gtk::gdk::Key::F10 && state.contains(gtk::gdk::ModifierType::SHIFT_MASK))
}

fn show(widget: &gtk::Widget, items: Vec<Item>, x: f64, y: f64) {
    if items.is_empty() {
        return;
    }
    let actions = gio::SimpleActionGroup::new();
    let mut next = 0;
    let menu = build(items, &actions, &mut next);

    let popover = gtk::PopoverMenu::from_model(Some(&menu));
    popover.insert_action_group("ctx", Some(&actions));
    popover.set_parent(widget);
    popover.set_has_arrow(false);
    popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    popover.connect_closed(move |p| {
        let p = p.clone();
        // unparent once the close animation and the activated item are done
        gtk::glib::idle_add_local_once(move || p.unparent());
    });
    popover.popup();
}

/// The menu model of `items`, sections in ascending order, with one action per runnable entry in `actions`. `next`
/// numbers the actions across submenus.
fn build(mut items: Vec<Item>, actions: &gio::SimpleActionGroup, next: &mut usize) -> gio::Menu {
    items.sort_by_key(|item| item.section);
    let menu = gio::Menu::new();
    let mut section = gio::Menu::new();
    let mut current = items.first().map_or(0, |item| item.section);
    for item in items {
        if item.section != current {
            menu.append_section(None, &section);
            section = gio::Menu::new();
            current = item.section;
        }
        if !item.children.is_empty() {
            section.append_submenu(Some(&item.label), &build(item.children, actions, next));
            continue;
        }
        let name = format!("item{next}");
        *next += 1;
        section.append(Some(&item.label), Some(&format!("ctx.{name}")));
        // a boolean state is what makes the menu draw a check mark
        let action = match item.checked {
            Some(on) => gio::SimpleAction::new_stateful(&name, None, &on.to_variant()),
            None => gio::SimpleAction::new(&name, None),
        };
        let run = Rc::clone(&item.run);
        // run after the menu closed and every handler up the stack returned, so no caller borrow is still held
        action.connect_activate(move |_, _| {
            let run = Rc::clone(&run);
            gtk::glib::idle_add_local_once(move || run());
        });
        actions.add_action(&action);
    }
    menu.append_section(None, &section);
    menu
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn items_are_grouped_by_section_in_order() {
        let mut items = vec![Item::new("b", 2, || {}), Item::new("a", 1, || {}), Item::new("c", 2, || {})];
        items.sort_by_key(|i| i.section);
        let labels: Vec<_> = items.iter().map(|i| (i.label.as_str(), i.section)).collect();
        assert_eq!(labels, [("a", 1), ("b", 2), ("c", 2)]);
    }

    #[test]
    fn the_menu_key_and_shift_f10_open_the_menu_and_f10_alone_does_not() {
        let (none, shift) = (gtk::gdk::ModifierType::empty(), gtk::gdk::ModifierType::SHIFT_MASK);
        assert!(is_menu_key(gtk::gdk::Key::Menu, none));
        assert!(is_menu_key(gtk::gdk::Key::F10, shift));
        assert!(!is_menu_key(gtk::gdk::Key::F10, none));
        assert!(!is_menu_key(gtk::gdk::Key::a, shift));
    }

    #[test]
    fn a_submenu_and_a_check_are_kept_on_the_item() {
        let speed = Item::submenu("Speed", 1, vec![Item::new("1x", 0, || {}).checked(true), Item::new("2x", 0, || {}).checked(false)]);
        assert_eq!(speed.children.len(), 2);
        assert_eq!(speed.children[0].checked, Some(true));
        assert_eq!(Item::new("x", 0, || {}).checked, None);
    }

    #[test]
    fn an_item_runs_its_action() {
        let hit = Rc::new(Cell::new(false));
        let flag = Rc::clone(&hit);
        let item = Item::new("x", 0, move || flag.set(true));
        (item.run)();
        assert!(hit.get());
    }
}
