//! English copy of area 42. Section 6 strings are read through `tr` from the `kuro-i18n` table. The five additions of
//! T-42a-062 ("Scanning downloads…", "Showing latest 500", "Library update already running", "Source not installed;
//! files not deleted", "No downloads in this category") have no key yet: the table is generated from
//! `kurobayomi-extra.ftl` and `kuro-i18n` pins its message count in two tests, so a key added from this lane would
//! break that crate. They stay constants here until `kuro-i18n` takes them.

use kuro_i18n::{MsgKey, tr};

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
