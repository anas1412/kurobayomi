//! Grid geometry and badge text (T-37a-018 to T-37a-020, T-37a-024). Pure numbers and strings, so the card widgets
//! only place them.

use kuro_prefs::library::LibraryDisplayMode;

/// Minimum cell width of the adaptive grid, in pixels (`LazyLibraryGrid.kt`, 128 dp). Cards request this width, so
/// `GridView` fits as many columns as the page allows when the column count is 0.
pub const CELL_MIN_WIDTH: i32 = 128;

/// Cover height for a cover of `width` pixels: the 2:3 book ratio.
pub fn cover_height(width: i32) -> i32 {
    width * 3 / 2
}

/// Whether the category tabs and the toolbar's tab-aware title show: the setting, or any active search (E-19, E-44).
/// The tab strip and the toolbar read this one answer.
pub fn tabs_visible(category_tabs: bool, search_active: bool) -> bool {
    category_tabs || search_active
}

/// Grid column count: a configured count wins, 0 fits as many [`CELL_MIN_WIDTH`] columns as `width` allows (T2-UI-01).
pub fn grid_columns(width: i32, configured: i32) -> i32 {
    if configured > 0 { configured } else { (width / CELL_MIN_WIDTH).max(1) }
}

/// Height of one list row when `entries` rows share `page_height` pixels: `height / n - 3 / n` in floating point,
/// at least 48 (E-23, an intended difference from the upstream integer division).
pub fn list_row_height(page_height: f64, entries: usize) -> f64 {
    if entries == 0 {
        return 48.0;
    }
    let n = entries as f64;
    (page_height / n - 3.0 / n).max(48.0)
}

/// The tab label: the category name, with its count after it when counts are shown.
pub fn tab_label(name: &str, count: Option<usize>) -> String {
    match count {
        Some(n) => format!("{name} ({n})"),
        None => name.to_owned(),
    }
}

/// Edge of the continue button in pixels. The cover-only and comfortable grids use the large size (B-8.3, B-8.5).
pub fn continue_size(mode: LibraryDisplayMode) -> i32 {
    match mode {
        LibraryDisplayMode::CoverOnlyGrid | LibraryDisplayMode::ComfortableGrid => 32,
        LibraryDisplayMode::CompactGrid | LibraryDisplayMode::List => 28,
    }
}

/// Whether the cover shows its title overlay. Only the compact grid does (B-8.3, B-8.4).
pub fn shows_title_overlay(mode: LibraryDisplayMode) -> bool {
    mode == LibraryDisplayMode::CompactGrid
}

/// The text of a count badge, or None when the badge is hidden (zero, or not computed because its switch is off).
pub fn count_badge(count: i64) -> Option<String> {
    (count > 0).then(|| count.to_string())
}

/// The language badge: the source language in upper case, or None when empty.
pub fn language_badge(language: &str) -> Option<String> {
    (!language.is_empty()).then(|| language.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_keep_the_book_ratio_and_tabs_show_counts_on_request() {
        assert_eq!(cover_height(128), 192);
        assert_eq!(tab_label("Reading", Some(4)), "Reading (4)");
        assert_eq!(tab_label("Reading", None), "Reading");
    }

    #[test]
    fn continue_button_is_large_in_cover_only_and_comfortable_grids() {
        assert_eq!(continue_size(LibraryDisplayMode::CoverOnlyGrid), 32);
        assert_eq!(continue_size(LibraryDisplayMode::ComfortableGrid), 32);
        assert_eq!(continue_size(LibraryDisplayMode::CompactGrid), 28);
        assert!(shows_title_overlay(LibraryDisplayMode::CompactGrid));
        assert!(!shows_title_overlay(LibraryDisplayMode::CoverOnlyGrid));
    }

    #[test]
    fn badges_hide_on_zero_and_uppercase_the_language() {
        assert_eq!(count_badge(0), None);
        assert_eq!(count_badge(-1), None);
        assert_eq!(count_badge(12).as_deref(), Some("12"));
        assert_eq!(language_badge(""), None);
        assert_eq!(language_badge("en").as_deref(), Some("EN"));
    }

    #[test]
    fn tabs_show_for_the_setting_or_a_search() {
        assert!(tabs_visible(true, false));
        assert!(tabs_visible(false, true));
        assert!(!tabs_visible(false, false));
    }

    #[test]
    fn auto_columns_come_from_the_width_with_a_128_pixel_minimum() {
        assert_eq!(grid_columns(1000, 0), 7);
        assert_eq!(grid_columns(100, 0), 1);
        assert_eq!(grid_columns(1000, 3), 3);
    }

    #[test]
    fn list_rows_split_the_page_without_integer_division_and_never_shrink_below_48() {
        assert_eq!(list_row_height(600.0, 4), 600.0 / 4.0 - 3.0 / 4.0);
        assert_eq!(list_row_height(600.0, 2), 600.0 / 2.0 - 1.5);
        assert_eq!(list_row_height(100.0, 10), 48.0);
        assert_eq!(list_row_height(100.0, 0), 48.0);
    }
}
