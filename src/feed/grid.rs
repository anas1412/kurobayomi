//! Grid geometry of the updates widget (T-50a-001, T-50a-002). Port of Aniyomi `calculateRowAndColumnCount`.
//! These numbers appear nowhere else in `src/`; the applet package holds a copy in `contents/code/grid.js`
//! that a test here keeps in step.

pub const MAX_ROWS: u32 = 10;
pub const MAX_COLS: u32 = 10;
/// Rows and columns used when the caller gives none (the 4 x 2 default size of the applet).
pub const DEFAULT_ROWS: u32 = 2;
pub const DEFAULT_COLS: u32 = 4;
/// Most rows read from the database before the entries are deduplicated.
pub const DB_LIMIT: usize = 500;
/// How many calendar months back the feed looks.
pub const WINDOW_MONTHS: u32 = 3;
