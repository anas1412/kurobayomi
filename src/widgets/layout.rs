//! Layout arithmetic shared by the widgets: the two-panel split (T-49a-046) and grid columns
//! (T-49a-029, T-49a-044). Pure functions, unit-tested.

/// Columns that fit `width` when each cell is at least `min_cell` wide with `spacing` between cells.
/// Always at least one. Computed on every call, so a changed width or spacing is picked up (E-49-15).
pub fn column_count(width: i32, min_cell: i32, spacing: i32) -> usize {
    if width <= 0 || min_cell <= 0 {
        return 1;
    }
    ((width + spacing) / (min_cell + spacing)).max(1) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_fit_whole_cells_with_spacing() {
        assert_eq!(column_count(400, 128, 8), 3);
        assert_eq!(column_count(127, 128, 8), 1);
        assert_eq!(column_count(0, 128, 8), 1);
        assert_eq!(column_count(1000, 0, 8), 1);
    }
}
