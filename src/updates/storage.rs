//! Storage page logic (T-42a-046, T-42a-047, T-42a-048, T-42a-050, T-42a-051, T-42a-052, T-42a-054). Pure: the sizes
//! come from the download cache and the rows come from the library; this file builds the snapshot the page shows, the
//! category options, the donut arcs and the delete outcome.

use std::collections::HashSet;

use crate::updates::copy;
use crate::updates::format::format_size;

/// An RGBA colour. The alpha is always 255 (EC-42-T-06).
pub type Rgba = [u8; 4];

/// The id the category dropdown gives to "All" (the first option).
pub const ALL_ID: i64 = -1;

/// One entry of the storage list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageRow {
    pub entry_id: i64,
    pub title: String,
    pub size_bytes: i64,
    pub thumbnail_url: Option<String>,
    pub entries_count: i64,
    pub color: Rgba,
    pub category_id: i64,
}

impl StorageRow {
    /// A row whose colour is derived from its entry id.
    pub fn new(
        entry_id: i64,
        title: impl Into<String>,
        size_bytes: i64,
        thumbnail_url: Option<String>,
        entries_count: i64,
        category_id: i64,
    ) -> Self {
        Self { entry_id, title: title.into(), size_bytes, thumbnail_url, entries_count, color: entry_color(entry_id), category_id }
    }
}

/// The colour of an entry: a fixed mix of its id (splitmix64 finalizer), three bytes, alpha 255. The same id always
/// gives the same colour (EC-42-T-06); the Kotlin random sequence is not reproduced.
pub fn entry_color(entry_id: i64) -> Rgba {
    let mut z = (entry_id as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    [(z >> 16) as u8, (z >> 8) as u8, z as u8, 255]
}

/// The dropdown selection: all categories, or one category by id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    All,
    Category(i64),
}

impl Selection {
    /// The dropdown id of the selection (`ALL_ID` for All).
    pub fn id(self) -> i64 {
        match self {
            Selection::All => ALL_ID,
            Selection::Category(id) => id,
        }
    }

    pub fn from_id(id: i64) -> Self {
        if id == ALL_ID { Selection::All } else { Selection::Category(id) }
    }
}

/// One option of the category dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryOption {
    pub id: i64,
    pub label: String,
}

/// The dropdown options: "All" first, then the categories in the given order. Id 0 is shown as "Default" whatever its
/// stored name is (EC-42-T-04).
pub fn category_options(categories: &[(i64, String)]) -> Vec<CategoryOption> {
    std::iter::once(CategoryOption { id: ALL_ID, label: copy::storage_all() })
        .chain(categories.iter().map(|(id, name)| CategoryOption {
            id: *id,
            label: if *id == 0 { copy::storage_default() } else { name.clone() },
        }))
        .collect()
}

/// The dropdown index of a selected id. An id that is not in the list falls back to "All" (index 0).
pub fn selected_index(options: &[CategoryOption], selected_id: i64) -> u32 {
    options.iter().position(|o| o.id == selected_id).unwrap_or(0) as u32
}

/// The id of the option at a dropdown index, when there is one.
pub fn id_at(options: &[CategoryOption], index: u32) -> Option<i64> {
    options.get(index as usize).map(|o| o.id)
}

/// The list the page shows: the rows of one selection, sorted by size, largest first, and the total of their sizes.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct StorageSnapshot {
    pub rows: Vec<StorageRow>,
    pub total_bytes: i64,
}

impl StorageSnapshot {
    /// The total as the donut centre shows it ("0 B" when empty).
    pub fn total_text(&self) -> String {
        format_size(self.total_bytes)
    }
}

/// Builds the snapshot of one emission (EC-42-T-05, EC-42-T-09, EC-42-T-12). Rows are kept once per entry id (the first
/// wins). `All` keeps the rows whose category is in `visible` (a category 0 row only when 0 is visible). A category
/// keeps its own rows, hidden or not. The sort is stable, so equal sizes keep the input order.
pub fn build_snapshot(rows: Vec<StorageRow>, selection: Selection, visible: &[i64]) -> StorageSnapshot {
    let mut seen = HashSet::new();
    let mut kept: Vec<StorageRow> = rows
        .into_iter()
        .filter(|row| seen.insert(row.entry_id))
        .filter(|row| match selection {
            Selection::All => visible.contains(&row.category_id),
            Selection::Category(id) => row.category_id == id,
        })
        .collect();
    kept.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    let total_bytes = kept.iter().map(|row| row.size_bytes).sum();
    StorageSnapshot { rows: kept, total_bytes }
}

/// The page state while the download cache starts up (T-42a-054). The page never waits for a value that will not come:
/// once the cache is not initializing, the snapshot is built from the current state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageState {
    Scanning,
    Ready,
}

pub fn page_state(initializing: bool) -> PageState {
    if initializing { PageState::Scanning } else { PageState::Ready }
}

/// One arc of the half donut: where it starts and how far it sweeps, as fractions of the whole half.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    pub start: f64,
    pub sweep: f64,
    pub color: Rgba,
}

/// From this content width the page sits in two columns (T-42a-053); narrower it is one column with the chart first.
pub const TWO_COLUMN_MIN_WIDTH: i32 = 720;
/// Extra padding after the left column (dropdown and donut), in pixels.
pub const LEFT_COLUMN_GAP: i32 = 24;

pub fn two_column(width: i32) -> bool {
    width >= TWO_COLUMN_MIN_WIDTH
}

/// The widths of the two columns of a content width: the left column (dropdown and donut) takes 2 of 5 and the right
/// one (rows) takes 3 of 5.
pub fn column_widths(width: i32) -> (i32, i32) {
    let left = width * 2 / 5;
    (left, width - left)
}

/// The half-donut arcs of a snapshot, largest first. Zero-size entries get no arc (EC-42-T-08); a zero total has none.
pub fn donut_arcs(snapshot: &StorageSnapshot) -> Vec<Arc> {
    if snapshot.total_bytes <= 0 {
        return Vec::new();
    }
    let total = snapshot.total_bytes as f64;
    let mut start = 0.0;
    let mut arcs = Vec::new();
    for row in snapshot.rows.iter().filter(|row| row.size_bytes > 0) {
        let sweep = row.size_bytes as f64 / total;
        arcs.push(Arc { start, sweep, color: row.color });
        start += sweep;
    }
    arcs
}

/// The count text of an entry row: "12 chapters", "1 episode" (`manga_num_chapters` / `anime_num_episodes`).
pub fn count_text(anime: bool, count: i64) -> String {
    let noun = match (anime, count) {
        (false, 1) => "chapter",
        (false, _) => "chapters",
        (true, 1) => "episode",
        (true, _) => "episodes",
    };
    format!("{count} {noun}")
}

/// What a storage delete does (T-42a-051). A missing source deletes nothing and says so (EC-42-T-15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    SourceMissing,
}

pub fn delete_outcome(source_installed: bool) -> DeleteOutcome {
    if source_installed { DeleteOutcome::Deleted } else { DeleteOutcome::SourceMissing }
}

/// The confirm text of a storage delete: `Are you sure you wish to delete "{title}"?`.
pub fn delete_text(title: &str) -> String {
    format!("Are you sure you wish to delete \"{title}\"?")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entry_id: i64, size: i64, category_id: i64) -> StorageRow {
        StorageRow::new(entry_id, format!("title {entry_id}"), size, None, 1, category_id)
    }

    #[test]
    fn storage_colour_is_deterministic_with_full_alpha() {
        assert_eq!(entry_color(7), entry_color(7));
        assert_eq!(entry_color(7)[3], 255);
        assert_ne!(entry_color(1), entry_color(2));
        assert_eq!(row(9, 1, 0).color, entry_color(9));
    }

    #[test]
    fn storage_category_options_start_with_all_and_show_default() {
        let options = category_options(&[(0, "System".into()), (3, "Reading".into())]);
        let labels: Vec<&str> = options.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(labels, ["All", "Default", "Reading"]);
        assert_eq!(selected_index(&options, ALL_ID), 0);
        assert_eq!(selected_index(&options, 3), 2);
        assert_eq!(selected_index(&options, 99), 0, "an unknown id falls back to All");
        assert_eq!(id_at(&options, 2), Some(3));
        assert_eq!(id_at(&options, 9), None);
        assert_eq!(Selection::from_id(ALL_ID), Selection::All);
        assert_eq!(Selection::Category(0).id(), 0);
    }

    #[test]
    fn build_snapshot_dedupes_filters_and_sorts_stable() {
        let rows = vec![row(1, 10, 1), row(2, 50, 1), row(2, 50, 1), row(3, 10, 1), row(4, 99, 2)];
        let snap = build_snapshot(rows, Selection::Category(1), &[1, 2]);
        let ids: Vec<i64> = snap.rows.iter().map(|r| r.entry_id).collect();
        assert_eq!(ids, [2, 1, 3], "equal sizes keep input order and duplicates are dropped");
        assert_eq!(snap.total_bytes, 70);
    }

    #[test]
    fn build_snapshot_all_keeps_only_visible_categories() {
        let rows = vec![row(1, 10, 1), row(2, 20, 5), row(3, 30, 0)];
        let snap = build_snapshot(rows.clone(), Selection::All, &[1, 0]);
        let ids: Vec<i64> = snap.rows.iter().map(|r| r.entry_id).collect();
        assert_eq!(ids, [3, 1], "category 5 is hidden; category 0 is kept because 0 is visible");
        let without_default = build_snapshot(rows, Selection::All, &[1]);
        assert_eq!(without_default.rows.len(), 1);
    }

    #[test]
    fn build_snapshot_empty_shows_zero_bytes() {
        let snap = build_snapshot(Vec::new(), Selection::All, &[0]);
        assert_eq!(snap.total_text(), "0 B");
        assert!(donut_arcs(&snap).is_empty());
    }

    #[test]
    fn storage_donut_arcs_cover_the_half_and_skip_zero_sizes() {
        let snap = build_snapshot(vec![row(1, 300, 0), row(2, 0, 0), row(3, 100, 0)], Selection::All, &[0]);
        let arcs = donut_arcs(&snap);
        assert_eq!(arcs.len(), 2);
        assert!((arcs[0].sweep - 0.75).abs() < 1e-9);
        assert!((arcs.iter().map(|a| a.sweep).sum::<f64>() - 1.0).abs() < 1e-9);
        assert!((arcs[1].start - 0.75).abs() < 1e-9);
    }

    #[test]
    fn storage_total_text_uses_si_units() {
        let snap = build_snapshot(vec![row(1, 1_250_000_000, 0)], Selection::All, &[0]);
        assert_eq!(snap.total_text(), "1.25 GB");
    }

    #[test]
    fn storage_page_never_waits_after_initialization() {
        assert_eq!(page_state(true), PageState::Scanning);
        assert_eq!(page_state(false), PageState::Ready);
    }

    #[test]
    fn storage_count_text_is_pluralised() {
        assert_eq!(count_text(false, 1), "1 chapter");
        assert_eq!(count_text(false, 12), "12 chapters");
        assert_eq!(count_text(true, 0), "0 episodes");
    }

    #[test]
    fn storage_delete_outcome_and_text() {
        assert_eq!(delete_outcome(true), DeleteOutcome::Deleted);
        assert_eq!(delete_outcome(false), DeleteOutcome::SourceMissing);
        assert_eq!(delete_text("Berserk"), "Are you sure you wish to delete \"Berserk\"?");
    }

    #[test]
    fn storage_two_columns_from_720_px_at_two_to_three() {
        assert!(!two_column(719) && two_column(720));
        assert_eq!(column_widths(800), (320, 480));
        let (left, right) = column_widths(1001);
        assert_eq!(left + right, 1001, "the columns fill the width");
    }
}
