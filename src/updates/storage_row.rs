//! The storage entry row (T-42a-050): a cover tile, a bold title on one line, and a second line with a 12 px colour dot,
//! the size, a separator dot and the count. A trailing delete button asks the page's confirm dialog.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, DrawingArea, Label, ListBoxRow, Orientation as O};

use kuro_i18n::{MsgKey, tr};
use kuro_platform::kind::Kind;

use crate::browse::look;
use crate::updates::format::format_size;
use crate::updates::row_texts;
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

/// The row (`look::list_row`): the cover tile, the bold title, the size and count line under it, and the delete button.
/// `on_delete` runs the confirm dialog.
pub fn storage_row(row: &StorageRow, kind: Kind, on_delete: Rc<dyn Fn()>) -> ListBoxRow {
    let delete = Button::from_icon_name("user-trash-symbolic");
    delete.set_tooltip_text(Some(&tr(MsgKey::action_delete)));
    delete.connect_clicked(move |_| on_delete());

    let cover = look::row_cover(&row.title, None, look::SMALL.width, look::SMALL.height);
    let list_row = look::list_row(Some(cover), &row.title, None, Some(delete.upcast()));
    list_row.set_activatable(false);

    let meta = GBox::new(O::Horizontal, 6);
    meta.append(&colour_dot(row.color));
    let size = format_size(row.size_bytes);
    let count = count_text(kind, row.entries_count);
    for text in [size.as_str(), "·", count.as_str()] {
        meta.append(&Label::builder().label(text).xalign(0.0).css_classes(["dim-label", "caption"]).build());
    }
    row_texts(&list_row).append(&meta);
    list_row
}
