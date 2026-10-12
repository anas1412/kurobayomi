//! The storage entry row (T-42a-050): a 48 px square cover, a bold title on one line, and a second line with a 12 px
//! colour dot, the size, a separator dot and the count. A trailing delete button asks the page's confirm dialog.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, DrawingArea, Label, ListBoxRow, Orientation as O};

use kuro_i18n::{MsgKey, tr};
use kuro_platform::kind::Kind;

use crate::updates::format::format_size;
use crate::updates::storage::{Rgba, StorageRow, count_text};

/// Side of the colour dot, in pixels.
const DOT_SIZE: i32 = 12;

/// The dot as a filled circle in the entry's colour.
fn colour_dot(colour: Rgba) -> DrawingArea {
    let dot = DrawingArea::new();
    dot.set_content_width(DOT_SIZE);
    dot.set_content_height(DOT_SIZE);
    dot.set_valign(Align::Center);
    let [r, g, b, _] = colour;
    dot.set_draw_func(move |_, cr, width, height| {
        let radius = f64::from(width.min(height)) / 2.0;
        cr.set_source_rgb(f64::from(r) / 255.0, f64::from(g) / 255.0, f64::from(b) / 255.0);
        cr.arc(f64::from(width) / 2.0, f64::from(height) / 2.0, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
    });
    dot
}

/// The row. `cover` is the thumbnail widget, already loaded by the caller; `on_delete` runs the confirm dialog.
pub fn storage_row(row: &StorageRow, kind: Kind, cover: &impl IsA<gtk::Widget>, on_delete: Rc<dyn Fn()>) -> ListBoxRow {
    let line = GBox::new(O::Horizontal, 12);
    crate::ui::margins(&line, 8);

    cover.set_size_request(48, 48);
    cover.set_valign(Align::Center);
    line.append(cover);

    let texts = GBox::new(O::Vertical, 2);
    texts.set_hexpand(true);
    texts.set_valign(Align::Center);
    texts.append(
        &Label::builder()
            .label(&row.title)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["heading"])
            .build(),
    );

    let meta = GBox::new(O::Horizontal, 6);
    meta.append(&colour_dot(row.color));
    let size = format_size(row.size_bytes);
    let count = count_text(kind, row.entries_count);
    for text in [size.as_str(), "·", count.as_str()] {
        meta.append(&Label::builder().label(text).xalign(0.0).css_classes(["dim-label", "caption"]).build());
    }
    texts.append(&meta);
    line.append(&texts);

    let delete = Button::from_icon_name("user-trash-symbolic");
    delete.set_valign(Align::Center);
    delete.set_tooltip_text(Some(&tr(MsgKey::action_delete)));
    delete.connect_clicked(move |_| on_delete());
    line.append(&delete);

    ListBoxRow::builder().child(&line).activatable(false).build()
}
