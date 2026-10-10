//! Grid geometry of the updates widget (T-50a-001, T-50a-002). Port of Aniyomi `calculateRowAndColumnCount`.
//! These numbers appear nowhere else in `src/`; the applet package holds a copy in `contents/code/grid.js`
//! that a test here keeps in step.

pub const CELL_W_DP: f64 = 64.0;
pub const CELL_H_DP: f64 = 95.0;
pub const MAX_ROWS: u32 = 10;
pub const MAX_COLS: u32 = 10;
/// Rows and columns used when the caller gives none (the 4 x 2 default size of the applet).
pub const DEFAULT_ROWS: u32 = 2;
pub const DEFAULT_COLS: u32 = 4;
/// Most rows read from the database before the entries are deduplicated.
pub const DB_LIMIT: usize = 500;
/// How many calendar months back the feed looks.
pub const WINDOW_MONTHS: u32 = 3;

/// Rows and columns that fit a size in dp. `pad_*` is the space the layout keeps at the top and bottom.
/// A negative or tiny height gives one row (no panic).
pub fn rows_cols(width: f64, height: f64, pad_top: f64, pad_bottom: f64) -> (u32, u32) {
    let fit = |len: f64, cell: f64, max: u32| ((len / cell).trunc().clamp(1.0, f64::from(max))) as u32;
    (fit(height - pad_top - pad_bottom, CELL_H_DP, MAX_ROWS), fit(width, CELL_W_DP, MAX_COLS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_from_the_plan() {
        let cases = [
            ((64.0, 95.0, 0.0, 0.0), (1, 1)),
            ((128.0, 190.0, 0.0, 0.0), (2, 2)),
            ((256.0, 190.0, 0.0, 0.0), (2, 4)),
            ((256.0, 190.0, 24.0, 24.0), (1, 4)),
            ((640.0, 950.0, 0.0, 0.0), (10, 10)),
            ((2000.0, 2000.0, 0.0, 0.0), (10, 10)),
            ((10.0, 10.0, 0.0, 0.0), (1, 1)),
            ((256.0, 100.0, 10.0, 0.0), (1, 4)),
        ];
        for ((w, h, t, b), want) in cases {
            assert_eq!(rows_cols(w, h, t, b), want, "{w}x{h} pad {t}/{b}");
        }
    }

    #[test]
    fn negative_height_gives_one_row() {
        assert_eq!(rows_cols(256.0, -50.0, 0.0, 0.0), (1, 4));
        assert_eq!(rows_cols(256.0, 10.0, 40.0, 40.0).0, 1);
    }

    #[test]
    fn size_rules_at_each_boundary() {
        // minimum one cell, default 256 x 190 (4 x 2 cells), maximum 640 x 950 (10 x 10 cells)
        assert_eq!(rows_cols(CELL_W_DP, CELL_H_DP, 0.0, 0.0), (1, 1));
        assert_eq!(rows_cols(CELL_W_DP - 1.0, CELL_H_DP - 1.0, 0.0, 0.0), (1, 1));
        assert_eq!(rows_cols(256.0, 190.0, 0.0, 0.0), (DEFAULT_ROWS, DEFAULT_COLS));
        assert_eq!(rows_cols(640.0, 950.0, 0.0, 0.0), (MAX_ROWS, MAX_COLS));
        assert_eq!(rows_cols(704.0, 1045.0, 0.0, 0.0), (MAX_ROWS, MAX_COLS));
    }

    /// The applet keeps its own copy of the pitch; both must name the same cell.
    #[test]
    fn applet_grid_helper_uses_the_same_numbers() {
        let js = include_str!("../../dist/plasmoid/org.kurobayomi.updates.manga/contents/code/grid.js");
        for needle in [
            format!("CELL_W = {}", CELL_W_DP),
            format!("CELL_H = {}", CELL_H_DP),
            format!("MAX_ROWS = {MAX_ROWS}"),
            format!("MAX_COLS = {MAX_COLS}"),
        ] {
            assert!(js.contains(&needle), "grid.js lacks {needle}");
        }
    }
}
