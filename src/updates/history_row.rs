//! The history row (T-42a-014): a book-aspect cover that opens the entry, a bold title (two lines, ellipsised), the
//! subtitle, a favorite button only when the entry is not in the library, and a delete button. A click on the row
//! resumes (T-42a-016).

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, ContentFit, GestureClick, Label, ListBoxRow, Orientation as O, Picture};
use kuro_domain::dates::item_row_text;

/// Cover width of the row, at the book aspect for a 96 px row.
const COVER_WIDTH: i32 = 68;
const ROW_HEIGHT: i32 = 96;

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

/// One history row. `in_library` decides whether the favorite button is shown.
pub fn history_row(title: &str, subtitle: &str, cover: &Picture, in_library: bool, actions: RowActions) -> ListBoxRow {
    let row = ListBoxRow::new();
    let line = GBox::new(O::Horizontal, 12);
    line.set_margin_top(8);
    line.set_margin_bottom(8);
    line.set_margin_start(12);
    line.set_margin_end(12);
    line.set_size_request(-1, ROW_HEIGHT);

    cover.set_content_fit(ContentFit::Cover);
    cover.set_size_request(COVER_WIDTH, ROW_HEIGHT);
    let open = GestureClick::new();
    let open_entry = actions.open_entry.clone();
    open.connect_pressed(move |_, _, _, _| open_entry());
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
        favorite.set_tooltip_text(Some("Add to library"));
        let add = actions.add_to_library.clone();
        favorite.connect_clicked(move |_| add());
        line.append(&favorite);
    }
    let delete = Button::from_icon_name("user-trash-symbolic");
    delete.set_tooltip_text(Some("Remove"));
    let remove = actions.remove.clone();
    delete.connect_clicked(move |_| remove());
    line.append(&delete);

    row.set_child(Some(&line));
    let resume = actions.resume.clone();
    row.connect_activate(move |_| resume());
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_subtitle_rules() {
        assert_eq!(subtitle("Ch.", 12.0, "14:05"), "Ch. 12 - 14:05");
        assert_eq!(subtitle("Ep.", 3.5, "21:30"), "Ep. 3.5 - 21:30");
        assert_eq!(subtitle("Ch.", -1.0, "14:05"), "14:05");
    }
}
