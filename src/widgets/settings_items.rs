//! Settings rows beyond the base family (`theme::components::settings_row`): tri-state (T-49a-040),
//! text entry (T-49a-043), chip row and icon grid (T-49a-044). Each row is a `gtk::ListBoxRow`
//! built on the shared `row` shell (24 px sides, 10 px vertical, Enter and Space activate).

use std::rc::Rc;

use gtk::prelude::*;

use super::layout::column_count;
use crate::theme::components::settings_row::row;
use crate::theme::tokens::DISABLED_ALPHA;

pub const GRID_MIN_CELL: i32 = 128;
pub const GRID_COLUMN_GAP: i32 = 8;
pub const GRID_ROW_GAP: i32 = 4;
/// The icon grid's tallest height before it scrolls (E-49-33: a bounded scroller).
pub const GRID_MAX_HEIGHT: i32 = 320;
pub const CHIP_GAP: u32 = 8;

/// The three-state preference: disabled, enabled-is, enabled-not (`TriStateCheckbox`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState {
    Disabled,
    EnabledIs,
    EnabledNot,
}

impl TriState {
    /// The click cycle: Disabled, then EnabledIs, then EnabledNot, then Disabled.
    pub fn next(self) -> Self {
        match self {
            TriState::Disabled => TriState::EnabledIs,
            TriState::EnabledIs => TriState::EnabledNot,
            TriState::EnabledNot => TriState::Disabled,
        }
    }

    /// The icon of the state (`p1-rust-design.md` 9.4).
    pub fn icon(self) -> &'static str {
        match self {
            TriState::Disabled => "checkbox-symbolic",
            TriState::EnabledIs => "checkbox-checked-symbolic",
            TriState::EnabledNot => "action-unavailable-symbolic",
        }
    }
}

/// A tri-state row. Without a callback the row is inert and its icon is dimmed (E-49-26).
pub fn tri_state_row(
    label: &str,
    state: TriState,
    on_click: Option<impl Fn(TriState) + 'static>,
) -> gtk::ListBoxRow {
    let icon = gtk::Image::from_icon_name(state.icon());
    icon.add_css_class(crate::theme::components::classes::SETTINGS_ICON);
    let activatable = on_click.is_some();
    if !activatable {
        icon.set_opacity(f64::from(DISABLED_ALPHA));
    }
    let r = row(label, Some(icon.upcast_ref()), activatable);
    if let Some(on_click) = on_click {
        let current = std::cell::Cell::new(state);
        r.connect_activate(move |_| {
            let next = current.get().next();
            current.set(next);
            icon.set_icon_name(Some(next.icon()));
            on_click(next);
        });
    }
    r
}

/// A text entry with the label above it, full width, 24 px sides and 4 px vertical margins.
pub fn text_row(label: &str, value: &str, on_change: impl Fn(String) + 'static) -> gtk::ListBoxRow {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
    column.set_margin_start(24);
    column.set_margin_end(24);
    column.set_margin_top(4);
    column.set_margin_bottom(4);
    let caption = gtk::Label::new(Some(label));
    caption.set_xalign(0.0);
    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry.set_text(value);
    entry.connect_changed(move |e| on_change(e.text().to_string()));
    column.append(&caption);
    column.append(&entry);
    let r = gtk::ListBoxRow::new();
    r.set_activatable(false);
    r.set_child(Some(&column));
    r
}

/// A wrapping row of toggle chips, 8 px apart, with padding 24/0/24/10 px. `on_toggle` gets the
/// chip index and its new state.
pub fn chip_row(chips: &[(&str, bool)], on_toggle: impl Fn(usize, bool) + 'static) -> gtk::ListBoxRow {
    let flow = gtk::FlowBox::new();
    flow.set_column_spacing(CHIP_GAP);
    flow.set_row_spacing(CHIP_GAP);
    flow.set_selection_mode(gtk::SelectionMode::None);
    let on_toggle = Rc::new(on_toggle);
    for (index, (label, selected)) in chips.iter().enumerate() {
        let chip = gtk::ToggleButton::with_label(label);
        chip.set_active(*selected);
        let on_toggle = on_toggle.clone();
        chip.connect_toggled(move |c| on_toggle(index, c.is_active()));
        flow.append(&chip);
    }
    let r = gtk::ListBoxRow::new();
    r.set_activatable(false);
    flow.set_margin_start(24);
    flow.set_margin_end(24);
    flow.set_margin_top(0);
    flow.set_margin_bottom(10);
    r.set_child(Some(&flow));
    r
}

/// Columns of the icon grid for a width: 128 px cells with 8 px gaps.
pub fn grid_columns(width: i32) -> usize {
    column_count(width, GRID_MIN_CELL, GRID_COLUMN_GAP)
}

/// An icon grid of `(icon, label, selected)` cells in a bounded scroller. `on_select` gets the index.
pub fn icon_grid(items: &[(&str, &str, bool)], on_select: impl Fn(usize) + 'static) -> gtk::ListBoxRow {
    let flow = gtk::FlowBox::new();
    flow.set_column_spacing(GRID_COLUMN_GAP as u32);
    flow.set_row_spacing(GRID_ROW_GAP as u32);
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_homogeneous(true);
    let on_select = Rc::new(on_select);
    for (index, (icon, label, selected)) in items.iter().enumerate() {
        let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
        cell.append(&gtk::Image::from_icon_name(icon));
        cell.append(&gtk::Label::new(Some(label)));
        let button = gtk::ToggleButton::new();
        button.add_css_class("flat");
        button.set_child(Some(&cell));
        button.set_size_request(GRID_MIN_CELL, -1);
        button.set_active(*selected);
        let on_select = on_select.clone();
        button.connect_toggled(move |b| {
            if b.is_active() {
                on_select(index);
            }
        });
        flow.append(&button);
    }
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_propagate_natural_height(true);
    scroller.set_max_content_height(GRID_MAX_HEIGHT);
    scroller.set_child(Some(&flow));
    let r = gtk::ListBoxRow::new();
    r.set_activatable(false);
    r.set_child(Some(&scroller));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tri_state_cycles_in_order() {
        assert_eq!(TriState::Disabled.next(), TriState::EnabledIs);
        assert_eq!(TriState::EnabledIs.next(), TriState::EnabledNot);
        assert_eq!(TriState::EnabledNot.next(), TriState::Disabled);
    }

    #[test]
    fn tri_state_icons_per_state() {
        assert_eq!(TriState::Disabled.icon(), "checkbox-symbolic");
        assert_eq!(TriState::EnabledIs.icon(), "checkbox-checked-symbolic");
        assert_eq!(TriState::EnabledNot.icon(), "action-unavailable-symbolic");
    }

    #[test]
    fn grid_fits_128_px_cells() {
        assert_eq!(grid_columns(400), 3);
        assert_eq!(grid_columns(100), 1);
        assert_eq!(GRID_ROW_GAP, 4);
    }
}
