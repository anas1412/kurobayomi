//! T2-UI-01 and T2-UI-02: grid columns and list row height.

use crate::library::layout::{CELL_MIN_WIDTH, grid_columns, list_row_height};

#[test]
fn t2_ui_01_zero_columns_fit_the_width_and_a_count_wins() {
    assert_eq!(CELL_MIN_WIDTH, 128);
    assert_eq!(grid_columns(1024, 0), 8);
    assert_eq!(grid_columns(127, 0), 1);
    assert_eq!(grid_columns(1024, 5), 5);
}

#[test]
fn t2_ui_02_list_height_uses_float_division_with_a_48_pixel_floor() {
    assert_eq!(list_row_height(480.0, 4), 480.0 / 4.0 - 3.0 / 4.0);
    assert_eq!(list_row_height(90.0, 3), 48.0);
}
