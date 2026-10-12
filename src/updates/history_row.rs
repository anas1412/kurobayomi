//! The history row (T-42a-014): a book-aspect cover (initials until an image draws) that opens the entry, a bold title (two lines, ellipsised), the
//! subtitle, a favorite button only when the entry is not in the library, and a delete button. A click on the row
//! resumes (T-42a-016); its right-click menu offers Resume, Open details and Remove from history.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, GestureClick, Label, ListBoxRow, Orientation as O};

use crate::library::layout::{SPACE_M, SPACE_S};
use crate::widgets::context_menu::Item;
use kuro_domain::dates::item_row_text;
use kuro_i18n::{MsgKey, tr};

/// The row is at least this high (T-42a-014).
pub const ROW_MIN_HEIGHT: i32 = 96;
/// Cover size of the row: the book aspect (2:3) of the library cards, filling the row's height inside its margins.
pub const COVER_HEIGHT: i32 = ROW_MIN_HEIGHT - 2 * SPACE_S;
pub const COVER_WIDTH: i32 = COVER_HEIGHT * 2 / 3;

/// The right-click item that opens the entry's page (no catalogue key yet, as in `updates::copy`).
pub const OPEN_DETAILS: &str = "Open details";
/// The right-click item that asks to remove the row, as the trash button does.
pub const REMOVE_FROM_HISTORY: &str = "Remove from history";

/// The subtitle: `Ch. {n} - {time}` (or `Ep.`) when the unit number is above -1, else the time only.
pub fn subtitle(prefix: &str, number: f64, time: &str) -> String {
    item_row_text(prefix, number, time)
}

/// The row's actions. Each closure is the screen's own; the row only calls it.
#[derive(Clone)]
pub struct RowActions {
    pub open_entry: Rc<dyn Fn()>,
    pub resume: Rc<dyn Fn()>,
    pub remove: Rc<dyn Fn()>,
    pub add_to_library: Rc<dyn Fn()>,
}

/// One history row. `in_library` decides whether the favorite button is shown. `cover` is the row's cover widget
/// (`browse::look::row_cover`); the row sizes it.
pub fn history_row(title: &str, subtitle: &str, cover: &impl IsA<gtk::Widget>, in_library: bool, actions: RowActions) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_size_request(-1, ROW_MIN_HEIGHT);
    let line = GBox::new(O::Horizontal, SPACE_M);
    line.set_margin_top(SPACE_S);
    line.set_margin_bottom(SPACE_S);
    line.set_margin_start(SPACE_M);
    line.set_margin_end(SPACE_S);

    let cover = cover.as_ref();
    cover.set_size_request(COVER_WIDTH, COVER_HEIGHT);
    cover.set_valign(Align::Center);
    let open = GestureClick::new();
    let open_entry = actions.open_entry.clone();
    open.connect_pressed(move |gesture, _, _, _| {
        // the cover opens the entry; the row's own click (resume) does not see it
        gesture.set_state(gtk::EventSequenceState::Claimed);
        open_entry();
    });
    cover.add_controller(open);
    line.append(cover);

    let texts = GBox::new(O::Vertical, 2);
    texts.set_hexpand(true);
    texts.set_valign(Align::Center);
    let title_label = Label::builder().label(title).xalign(0.0).wrap(true).lines(2).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["heading"]).build();
    let subtitle_label = Label::builder().label(subtitle).xalign(0.0).css_classes(["dim-label"]).build();
    texts.append(&title_label);
    texts.append(&subtitle_label);
    line.append(&texts);

    if !in_library {
        let favorite = Button::from_icon_name("starred-symbolic");
        favorite.set_tooltip_text(Some(&tr(MsgKey::add_to_library)));
        favorite.set_valign(Align::Center);
        favorite.add_css_class("flat");
        let add = actions.add_to_library.clone();
        favorite.connect_clicked(move |_| add());
        line.append(&favorite);
    }
    let delete = Button::from_icon_name("user-trash-symbolic");
    delete.set_tooltip_text(Some(&tr(MsgKey::action_remove)));
    delete.set_valign(Align::Center);
    delete.add_css_class("flat");
    let remove = actions.remove.clone();
    delete.connect_clicked(move |_| remove());
    line.append(&delete);

    row.set_child(Some(&line));
    let resume = actions.resume.clone();
    crate::widgets::row_activate::on_activate(&row, move |_| resume());
    crate::widgets::context_menu::attach(&row, move |_, _| menu(&actions));
    row
}

/// The row's right-click menu: Resume, Open details, then Remove from history (which asks first, as the trash button).
fn menu(actions: &RowActions) -> Vec<Item> {
    let (resume, open, remove) = (actions.resume.clone(), actions.open_entry.clone(), actions.remove.clone());
    vec![
        Item::new(tr(MsgKey::action_resume), 0, move || resume()),
        Item::new(OPEN_DETAILS, 0, move || open()),
        Item::new(REMOVE_FROM_HISTORY, 1, move || remove()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_menu_resumes_opens_the_details_and_asks_to_remove() {
        let log: Rc<std::cell::RefCell<Vec<&str>>> = Rc::default();
        let call = |name: &'static str| -> Rc<dyn Fn()> {
            let log = Rc::clone(&log);
            Rc::new(move || log.borrow_mut().push(name))
        };
        let actions = RowActions { open_entry: call("open"), resume: call("resume"), remove: call("remove"), add_to_library: call("add") };
        let items = menu(&actions);
        assert_eq!(items.iter().map(|i| i.label.clone()).collect::<Vec<_>>(), [tr(MsgKey::action_resume), OPEN_DETAILS.to_owned(), REMOVE_FROM_HISTORY.to_owned()]);
        for item in &items {
            (item.run)();
        }
        assert_eq!(*log.borrow(), ["resume", "open", "remove"]);
    }

    #[test]
    fn history_subtitle_rules() {
        assert_eq!(subtitle("Ch.", 12.0, "14:05"), "Ch. 12 - 14:05");
        assert_eq!(subtitle("Ep.", 3.5, "21:30"), "Ep. 3.5 - 21:30");
        assert_eq!(subtitle("Ch.", -1.0, "14:05"), "14:05");
    }

    #[test]
    fn t_42a_014_the_row_is_at_least_96_px_and_its_cover_fills_that_height_at_the_book_aspect() {
        crate::testing::with_display(|| {
            let none: Rc<dyn Fn()> = Rc::new(|| {});
            let actions = RowActions { open_entry: none.clone(), resume: none.clone(), remove: none.clone(), add_to_library: none };
            let cover = gtk::Picture::new();
            let row = history_row("Title", "Ch. 1", &cover, false, actions);
            assert_eq!(row.size_request().1, 96);
            assert_eq!(cover.size_request(), (COVER_WIDTH, COVER_HEIGHT));
            assert_eq!(COVER_HEIGHT + 2 * SPACE_S, 96, "the cover plus the row margins is the minimum height");
            assert_eq!(COVER_WIDTH * 3, COVER_HEIGHT * 2 - COVER_HEIGHT * 2 % 3, "2:3");
        });
    }

    /// The cover opens the entry and claims its press, so the row under it does not also resume.
    #[test]
    fn a_cover_press_opens_the_entry_and_does_not_resume() {
        crate::testing::with_display(|| {
            use std::cell::Cell;
            let (opened, resumed) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
            let (o, r) = (Rc::clone(&opened), Rc::clone(&resumed));
            let none: Rc<dyn Fn()> = Rc::new(|| {});
            let actions = RowActions { open_entry: Rc::new(move || o.set(o.get() + 1)), resume: Rc::new(move || r.set(r.get() + 1)), remove: none.clone(), add_to_library: none };
            let cover = gtk::Picture::new();
            let _row = history_row("Title", "Ch. 1", &cover, true, actions);
            let all = cover.observe_controllers();
            let press = (0..all.n_items()).filter_map(|i| all.item(i)).find_map(|c| c.downcast::<GestureClick>().ok()).expect("the cover's click");
            press.emit_by_name::<()>("pressed", &[&1_i32, &0.0_f64, &0.0_f64]);
            assert_eq!((opened.get(), resumed.get()), (1, 0), "the cover opens the entry; the resume is the row's");
        });
    }
}
