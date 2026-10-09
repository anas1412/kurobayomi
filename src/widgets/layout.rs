//! Layout arithmetic shared by the widgets: the two-panel split (T-49a-046) and grid columns
//! (T-49a-029, T-49a-044). Pure functions, unit-tested.

use gtk::prelude::*;

use crate::theme::tokens::TWO_PANEL_FIRST_MAX;

/// The narrowest the start panel may be dragged (desktop resize, `p1-ui-parity.md` 13).
pub const TWO_PANEL_MIN_START: i32 = 280;

/// Initial start-panel width: `min(total / 2, 450)` (T-49a-046).
pub fn two_panel_start(total: i32) -> i32 {
    (total / 2).min(TWO_PANEL_FIRST_MAX as i32).max(0)
}

/// A dragged start width, kept between the minimum and the full width.
pub fn clamp_two_panel_start(width: i32, total: i32) -> i32 {
    let total = total.max(0);
    let min = TWO_PANEL_MIN_START.min(total);
    width.clamp(min, total.max(min))
}

/// Columns that fit `width` when each cell is at least `min_cell` wide with `spacing` between cells.
/// Always at least one. Computed on every call, so a changed width or spacing is picked up (E-49-15).
pub fn column_count(width: i32, min_cell: i32, spacing: i32) -> usize {
    if width <= 0 || min_cell <= 0 {
        return 1;
    }
    ((width + spacing) / (min_cell + spacing)).max(1) as usize
}

/// A horizontal split with a resizable divider. The start panel starts at `two_panel_start` of
/// `total_width`. Call `set_position` again once the real width is known.
pub fn two_panel(start: &impl IsA<gtk::Widget>, end: &impl IsA<gtk::Widget>, total_width: i32) -> gtk::Paned {
    let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
    paned.set_start_child(Some(start));
    paned.set_end_child(Some(end));
    paned.set_resize_start_child(true);
    paned.set_resize_end_child(true);
    paned.set_shrink_start_child(false);
    paned.set_shrink_end_child(false);
    paned.set_position(two_panel_start(total_width));
    paned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_panel_is_half_the_width_up_to_450() {
        assert_eq!(two_panel_start(600), 300);
        assert_eq!(two_panel_start(1000), 450);
        assert_eq!(two_panel_start(-5), 0);
    }

    #[test]
    fn dragged_start_stays_between_280_and_the_total() {
        assert_eq!(clamp_two_panel_start(100, 1000), 280);
        assert_eq!(clamp_two_panel_start(2000, 1000), 1000);
        assert_eq!(clamp_two_panel_start(400, 1000), 400);
        assert_eq!(clamp_two_panel_start(100, 200), 200);
    }

    #[test]
    fn columns_fit_whole_cells_with_spacing() {
        assert_eq!(column_count(400, 128, 8), 3);
        assert_eq!(column_count(127, 128, 8), 1);
        assert_eq!(column_count(0, 128, 8), 1);
        assert_eq!(column_count(1000, 0, 8), 1);
    }
}
