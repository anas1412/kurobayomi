//! The info header widget over `header.rs`: cover, title, author, artist, status and source. A click on a line runs
//! its search, a right click or a long press copies it, and a stub source shows a warning icon in the error color.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Label, Orientation as O};

use crate::entry_core::header::{Field, HeaderModel, SearchRequest, cover_size};

/// What the header reports back to the page.
#[derive(Clone)]
pub struct Handlers {
    pub on_search: Rc<dyn Fn(SearchRequest)>,
    pub on_copy: Rc<dyn Fn(String)>,
    pub on_cover: Rc<dyn Fn()>,
}

/// A label that searches on click and copies on right click or long press, for the fields that have text.
fn field_label(model: &HeaderModel, field: Field, text: &str, css: &[&str], xalign: f32, h: &Handlers) -> Label {
    let label = Label::builder().label(text).xalign(xalign).wrap(true).css_classes(css.to_vec()).build();
    if let Some(request) = model.search(field) {
        label.set_cursor_from_name(Some("pointer"));
        let click = gtk::GestureClick::builder().button(1).build();
        let (on_search, long_fired) = (Rc::clone(&h.on_search), Rc::new(Cell::new(false)));
        let skip = Rc::clone(&long_fired);
        click.connect_released(move |_, _, _, _| {
            if !skip.replace(false) {
                on_search(request.clone());
            }
        });
        label.add_controller(click);
        if let Some(copy) = model.copy(field) {
            let long = gtk::GestureLongPress::new();
            let on_copy = Rc::clone(&h.on_copy);
            let text = copy.clone();
            long.connect_pressed(move |_, _, _| {
                long_fired.set(true);
                on_copy(text.clone());
            });
            label.add_controller(long);
        }
    }
    if let Some(copy) = model.copy(field) {
        let right = gtk::GestureClick::builder().button(3).build();
        let on_copy = Rc::clone(&h.on_copy);
        right.connect_pressed(move |_, _, _, _| on_copy(copy.clone()));
        label.add_controller(right);
    }
    label
}

fn icon_row(icon: &str, content: &impl IsA<gtk::Widget>, centered: bool) -> GBox {
    let row = GBox::new(O::Horizontal, 6);
    row.set_halign(if centered { Align::Center } else { Align::Start });
    row.append(&gtk::Image::from_icon_name(icon));
    row.append(content);
    row
}

/// Builds the header. The small layout puts the cover beside the text, the large layout puts a wider cover above
/// centered text. `column_width` is the width of the column that holds the header.
pub fn build(model: &HeaderModel, large: bool, column_width: i32, h: &Handlers) -> GBox {
    let (w, ht) = cover_size(large, column_width);
    let cover = crate::ui::cover(w, ht);
    cover.set_valign(Align::Start);
    let on_cover = Rc::clone(&h.on_cover);
    let tap = gtk::GestureClick::new();
    tap.connect_released(move |_, _, _, _| on_cover());
    cover.add_controller(tap);

    let xalign = if large { 0.5 } else { 0.0 };
    let info = GBox::new(O::Vertical, 4);
    info.set_hexpand(true);
    info.set_valign(Align::End);
    info.append(&field_label(model, Field::Title, &model.title, &["title-2"], xalign, h));
    let author = field_label(model, Field::Author, &model.author, &[], xalign, h);
    info.append(&icon_row("avatar-default-symbolic", &author, large));
    if let Some(artist) = &model.artist {
        let artist = field_label(model, Field::Artist, artist, &[], xalign, h);
        info.append(&icon_row("document-edit-symbolic", &artist, large));
    }
    info.append(&icon_row(model.status_icon, &Label::builder().label(&model.status).css_classes(["dim-label"]).build(), large));
    let source_row = GBox::new(O::Horizontal, 6);
    source_row.set_halign(if large { Align::Center } else { Align::Start });
    source_row.append(&gtk::Image::from_icon_name("network-server-symbolic"));
    source_row.append(&field_label(model, Field::Source, &model.source, &["dim-label"], xalign, h));
    if model.stub {
        // the stock `error` class colors the icon like an error label
        source_row.append(&gtk::Image::builder().icon_name("dialog-warning-symbolic").css_classes(["error"]).build());
    }
    info.append(&source_row);

    let root = GBox::new(if large { O::Vertical } else { O::Horizontal }, 16);
    if large {
        cover.set_halign(Align::Center);
    }
    root.append(&cover);
    root.append(&info);
    root
}
