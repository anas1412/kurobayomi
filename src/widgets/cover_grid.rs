//! The one geometry of a cover grid. The Library and the Browse listing both draw 2:3 covers in cells of the same width, and
//! both count their columns here, so the two screens lay out alike on every GTK version: the app decides how many columns
//! there are and how wide a cell is, and never leaves that to the grid's own measuring (which differs between GTK versions).

use kuro_domain::library::columns::{COLUMN_WIDTH_PX, columns_for_width};

/// The margin of a cell on every side, so cells sit 4 px apart.
pub const CELL_MARGIN_PX: i32 = 2;

/// The width of a cover in a cell: the column width of the Library cards minus the cell margins.
pub const COVER_WIDTH_PX: i32 = COLUMN_WIDTH_PX - 2 * CELL_MARGIN_PX;

/// Sets the column count of `grid` for a viewport `viewport_px` wide: the automatic count, `max(2, width / 160)`. Never a
/// forced minimum: a forced count makes the grid demand width, so the window could only grow.
pub fn apply_columns(grid: &gtk::GridView, viewport_px: i32) {
    let columns = columns_for_width(viewport_px, crate::misc::diet_defaults::LIBRARY_COLUMNS, crate::misc::diet_defaults::LIBRARY_COLUMNS);
    grid.set_min_columns(1);
    grid.set_max_columns(columns as u32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cover_fills_its_column_less_the_cell_margins() {
        assert_eq!(COVER_WIDTH_PX + 2 * CELL_MARGIN_PX, COLUMN_WIDTH_PX);
    }

    #[test]
    fn the_count_follows_the_viewport_with_at_least_two_columns() {
        crate::testing::with_display(|| {
            let grid = gtk::GridView::new(None::<gtk::NoSelection>, None::<gtk::ListItemFactory>);
            apply_columns(&grid, 1000);
            assert_eq!((grid.min_columns(), grid.max_columns()), (1, 6));
            apply_columns(&grid, 100);
            assert_eq!(grid.max_columns(), 2);
        });
    }
}
