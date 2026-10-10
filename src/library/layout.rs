//! Grid geometry and badge text (T-37a-018 to T-37a-020, T-37a-024). Pure numbers and strings, so the card widgets
//! only place them.

use kuro_prefs::library::LibraryDisplayMode;

/// Cover height for a cover of `width` pixels: the 2:3 book ratio.
pub fn cover_height(width: i32) -> i32 {
    width * 3 / 2
}

/// Whether the category tabs and the toolbar's tab-aware title show: the setting, or any active search (E-19, E-44).
/// The tab strip and the toolbar read this one answer.
pub fn tabs_visible(category_tabs: bool, search_active: bool) -> bool {
    category_tabs || search_active
}

/// Height of one list row when `entries` rows share `page_height` pixels: `height / n - 3 / n` in floating point,
/// at least 48 (E-23, an intended difference from the upstream integer division). A list size of 0 (Auto) gives the
/// fixed 76 px row (T-37a-021).
pub fn list_row_height(page_height: f64, entries: usize) -> f64 {
    if entries == 0 {
        return 76.0;
    }
    let n = entries as f64;
    (page_height / n - 3.0 / n).max(48.0)
}

/// The list size of the page: the portrait or landscape slider value for a window `width` pixels wide (the same
/// split the grids read), clamped to `0..=10`. 0 is Auto.
pub fn list_size(width: i32, portrait: i32, landscape: i32) -> usize {
    use kuro_domain::library::columns::{PORTRAIT_BREAKPOINT_PX, clamp_columns};
    clamp_columns(if width < PORTRAIT_BREAKPOINT_PX { portrait } else { landscape }) as usize
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

/// The tooltip of the unread badge: a season parent counts seasons, not episodes (T-37a-048). None keeps the default.
pub fn unread_badge_tooltip(season_parent: bool) -> Option<&'static str> {
    season_parent.then_some("unseen seasons")
}

/// The language badge: the source language in upper case, or None when empty.
pub fn language_badge(language: &str) -> Option<String> {
    (!language.is_empty()).then(|| language.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_season_parent_unread_badge_says_unseen_seasons() {
        // T-37a-048: the tooltip names seasons for season parents only.
        assert_eq!(unread_badge_tooltip(true), Some("unseen seasons"));
        assert_eq!(unread_badge_tooltip(false), None);
    }

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
    fn list_rows_split_the_page_without_integer_division_and_never_shrink_below_48() {
        assert_eq!(list_row_height(600.0, 4), 600.0 / 4.0 - 3.0 / 4.0);
        assert_eq!(list_row_height(600.0, 2), 600.0 / 2.0 - 1.5);
        assert_eq!(list_row_height(100.0, 10), 48.0);
        assert_eq!(list_row_height(100.0, 0), 76.0, "list size 0 is the fixed 76 px row");
        assert_eq!(list_row_height(0.0, 0), 76.0);
    }

    #[test]
    fn the_list_size_follows_the_orientation_key_and_stays_in_range() {
        assert_eq!(list_size(899, 3, 7), 3);
        assert_eq!(list_size(900, 3, 7), 7);
        assert_eq!(list_size(500, 0, 7), 0);
        assert_eq!(list_size(500, 99, 7), 10);
        assert_eq!(list_size(500, -4, 7), 0);
    }
}
