//! T2-UI-01 and T2-UI-02: grid columns and list row height.

use crate::library::layout::list_row_height;
use kuro_domain::library::columns::columns_for_width;

#[test]
fn t2_ui_01_zero_columns_fit_the_width_and_a_count_wins() {
    assert_eq!(columns_for_width(1024, 0, 0), 6);
    assert_eq!(columns_for_width(100, 0, 0), 2, "Auto keeps two columns");
    assert_eq!(columns_for_width(1024, 3, 5), 5, "from 900 px the landscape key wins");
    assert_eq!(columns_for_width(800, 3, 5), 3, "below 900 px the portrait key wins");
}

#[test]
fn t2_ui_02_list_height_uses_float_division_with_a_48_pixel_floor() {
    assert_eq!(list_row_height(480.0, 4), 480.0 / 4.0 - 3.0 / 4.0);
    assert_eq!(list_row_height(90.0, 3), 48.0);
}
