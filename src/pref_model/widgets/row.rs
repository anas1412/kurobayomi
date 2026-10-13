//! Base preference row: icon column, title and subtitle, trailing widget. Every settings row is built here.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{pango, Align, Box as GBox, Image, Label, ListBoxRow, Orientation};

use super::{
    ICON_END_MARGIN, PREF_HORIZONTAL_PADDING, PREF_MIN_HEIGHT, PREF_VERTICAL_PADDING, SUBTITLE_MAX_LINES,
    TITLE_MAX_LINES, TRAILING_WIDGET_BUFFER,
};

#[allow(dead_code)] // kept for the shape of the type; not built or read yet
/// Handles to the parts of a built row. The caller updates labels and attaches the trailing widget.
#[derive(Clone)]
pub struct PrefRow {
    pub row: ListBoxRow,
    pub title: Label,
    pub subtitle: Label,
    pub trailing: GBox,
}

/// What a row shows. `subtitle` is already formatted; blank text hides the subtitle (T-E22).
pub struct RowSpec<'a> {
    pub title: &'a str,
    pub subtitle: Option<&'a str>,
    pub icon: Option<&'a str>,
    pub trailing: Option<&'a gtk::Widget>,
    /// The row is activatable only when a click handler is given (T-E21).
    pub on_click: Option<Rc<dyn Fn()>>,
}

pub fn build(spec: RowSpec) -> PrefRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(spec.on_click.is_some());
    row.set_size_request(-1, PREF_MIN_HEIGHT);

    let inner = GBox::new(Orientation::Horizontal, 0);
    inner.set_margin_start(PREF_HORIZONTAL_PADDING);
    inner.set_margin_end(PREF_HORIZONTAL_PADDING);
    inner.set_margin_top(PREF_VERTICAL_PADDING);
    inner.set_margin_bottom(PREF_VERTICAL_PADDING);
    inner.set_valign(Align::Center);

    if let Some(icon) = spec.icon {
        let image = Image::from_icon_name(icon);
        image.set_margin_end(ICON_END_MARGIN);
        image.set_valign(Align::Center);
        image.set_accessible_role(gtk::AccessibleRole::Presentation);
        inner.append(&image);
    }

    let text = GBox::new(Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(Align::Center);
    let title = Label::new(Some(spec.title));
    title.add_css_class("pref-title");
    title.set_xalign(0.0);
    title.set_halign(Align::Start);
    title.set_wrap(true);
    title.set_lines(TITLE_MAX_LINES);
    title.set_ellipsize(pango::EllipsizeMode::End);
    text.append(&title);

    // the same subtitle style as `crate::ui::dim`: the theme's dim colour at caption size
    let subtitle = Label::new(None);
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("caption");
    subtitle.set_xalign(0.0);
    subtitle.set_halign(Align::Start);
    subtitle.set_wrap(true);
    subtitle.set_lines(SUBTITLE_MAX_LINES);
    subtitle.set_ellipsize(pango::EllipsizeMode::End);
    set_subtitle(&subtitle, spec.subtitle);
    text.append(&subtitle);
    inner.append(&text);

    let trailing = GBox::new(Orientation::Horizontal, 0);
    trailing.set_valign(Align::Center);
    trailing.set_margin_start(TRAILING_WIDGET_BUFFER);
    if let Some(widget) = spec.trailing {
        trailing.append(widget);
    }
    inner.append(&trailing);

    row.set_child(Some(&inner));
    if let Some(on_click) = spec.on_click {
        crate::widgets::row_activate::on_activate(&row, move |_| on_click());
    }
    PrefRow { row, title, subtitle, trailing }
}

/// Shows `text` in the subtitle label, or hides the label when the text is missing or blank.
pub fn set_subtitle(label: &Label, text: Option<&str>) {
    match text.filter(|t| !t.trim().is_empty()) {
        Some(t) => {
            label.set_text(t);
            label.set_visible(true);
        }
        None => label.set_visible(false),
    }
}
