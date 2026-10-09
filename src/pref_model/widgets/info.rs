//! Info block (Kotlin `InfoWidget`): an icon and body text, not activatable. Blank text is skipped (T-E43).

use gtk::prelude::*;
use gtk::{pango, Align, Box as GBox, Image, Label, ListBoxRow, Orientation};

use super::{PREF_HORIZONTAL_PADDING, PREF_VERTICAL_PADDING, SECONDARY_ALPHA};

pub fn build(text: &str) -> Option<ListBoxRow> {
    if text.trim().is_empty() {
        return None;
    }
    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    let column = GBox::new(Orientation::Vertical, 16);
    column.set_margin_start(PREF_HORIZONTAL_PADDING);
    column.set_margin_end(PREF_HORIZONTAL_PADDING);
    column.set_margin_top(PREF_VERTICAL_PADDING);
    column.set_margin_bottom(PREF_VERTICAL_PADDING);
    column.set_opacity(SECONDARY_ALPHA);
    let icon = Image::from_icon_name("dialog-information-symbolic");
    icon.set_halign(Align::Start);
    column.append(&icon);
    let label = Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_halign(Align::Start);
    label.set_ellipsize(pango::EllipsizeMode::None);
    label.add_css_class("caption");
    column.append(&label);
    row.set_child(Some(&column));
    Some(row)
}
