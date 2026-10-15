//! English copy of area 42. Section 6 strings are read through `tr` from the `kuro-i18n` table. The five additions of
//! T-42a-062 ("Scanning downloads…", "Showing latest 500", "Library update already running", "Source not installed;
//! files not deleted", "No downloads in this category") have no key yet: the table is generated from
//! `kurobayomi-extra.ftl` and `kuro-i18n` pins its message count in two tests, so a key added from this lane would
//! break that crate. They stay constants here until `kuro-i18n` takes them.

use kuro_i18n::{MsgKey, tr};

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
pub fn storage_scanning() -> String {
    tr(MsgKey::storage_storage_scanning)
}
pub fn storage_empty_category() -> String {
    tr(MsgKey::storage_storage_empty_category)
}
pub fn storage_source_missing() -> String {
    tr(MsgKey::storage_storage_source_missing)
}

/// Updates copy (T-42a-032, T-42a-038, T-42b-003, T-42b-016).
pub fn showing_latest_500() -> String {
    tr(MsgKey::storage_showing_latest_500)
}
pub fn delete_source_missing() -> String {
    tr(MsgKey::storage_delete_source_missing)
}
pub fn confirm_delete_chapters() -> String {
    tr(MsgKey::confirm_delete_chapters)
}
pub fn confirm_delete_episodes() -> String {
    tr(MsgKey::confirm_delete_episodes)
}
pub fn internal_error() -> String {
    tr(MsgKey::internal_error)
}
