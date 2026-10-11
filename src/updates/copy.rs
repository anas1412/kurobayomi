//! English copy of area 42. Section 6 strings are read through `tr` from the `kuro-i18n` table. The five additions of
//! T-42a-062 ("Scanning downloads…", "Showing latest 500", "Library update already running", "Source not installed;
//! files not deleted", "No downloads in this category") have no key yet: the table is generated from
//! `kurobayomi-extra.ftl` and `kuro-i18n` pins its message count in two tests, so a key added from this lane would
//! break that crate. They stay constants here until `kuro-i18n` takes them.

use kuro_i18n::{MsgKey, tr};

/// Statistics section titles and captions (p1-ui-parity 4.2).
pub fn stats_overview() -> String {
    tr(MsgKey::label_overview_section)
}
pub fn stats_entries() -> String {
    tr(MsgKey::label_titles_section)
}
pub fn stats_chapters() -> String {
    tr(MsgKey::chapters)
}
pub fn stats_episodes() -> String {
    tr(MsgKey::episodes)
}
pub fn stats_trackers() -> String {
    tr(MsgKey::label_tracker_section)
}
pub fn stats_in_library() -> String {
    tr(MsgKey::in_library)
}
pub fn stats_read_duration() -> String {
    tr(MsgKey::label_read_duration)
}
pub fn stats_watched_duration() -> String {
    tr(MsgKey::label_watched_duration)
}
pub fn stats_completed() -> String {
    tr(MsgKey::label_completed_titles)
}
pub fn stats_in_global_update() -> String {
    tr(MsgKey::label_titles_in_global_update)
}
pub fn stats_started() -> String {
    tr(MsgKey::label_started)
}
pub fn stats_local() -> String {
    tr(MsgKey::label_local)
}
pub fn stats_total() -> String {
    tr(MsgKey::label_total_chapters)
}
pub fn stats_read() -> String {
    tr(MsgKey::label_read_chapters)
}
pub fn stats_watched() -> String {
    tr(MsgKey::label_watched_episodes)
}
pub fn stats_downloaded() -> String {
    tr(MsgKey::label_downloaded)
}
pub fn stats_tracked() -> String {
    tr(MsgKey::label_tracked_titles)
}
pub fn stats_mean_score() -> String {
    tr(MsgKey::label_mean_score)
}
pub fn stats_used() -> String {
    tr(MsgKey::label_used)
}
pub fn none() -> String {
    tr(MsgKey::none)
}
pub fn not_available() -> String {
    tr(MsgKey::not_applicable)
}

pub fn storage_all() -> String {
    tr(MsgKey::label_all)
}
pub fn storage_default() -> String {
    tr(MsgKey::label_default)
}
pub fn delete_title_chapters() -> String {
    tr(MsgKey::delete_downloads_for_manga)
}
pub fn delete_title_episodes() -> String {
    tr(MsgKey::delete_downloads_for_anime)
}
/// Additions of T-42a-062 (no key yet, see the module comment).
pub const STORAGE_SCANNING: &str = "Scanning downloads…";
pub const STORAGE_EMPTY_CATEGORY: &str = "No downloads in this category";
pub const STORAGE_SOURCE_MISSING: &str = "Source not installed; files not deleted";

/// Updates copy (T-42a-032, T-42a-038, T-42b-003, T-42b-016).
pub const SHOWING_LATEST_500: &str = "Showing latest 500";
pub const DELETE_SOURCE_MISSING: &str = "Source not installed, downloads were not deleted";
pub fn confirm_delete_chapters() -> String {
    tr(MsgKey::confirm_delete_chapters)
}
pub fn confirm_delete_episodes() -> String {
    tr(MsgKey::confirm_delete_episodes)
}
pub fn internal_error() -> String {
    tr(MsgKey::internal_error)
}
pub const LAST_UPDATED_PREFIX: &str = "Library last updated: ";
