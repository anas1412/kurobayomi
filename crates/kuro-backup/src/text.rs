//! Restore and status texts as message keys with their English defaults (T-22a-043). The GTK layer shows the key's
//! translation. Log lines stay English for parity, so they never read this table.

/// One user-visible text: the `kuro-i18n` key and the English default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Text {
    pub key: &'static str,
    pub english: &'static str,
}

pub const RESTORE_IN_PROGRESS: Text = Text { key: "restore_in_progress", english: "Restore is already in progress" };
pub const RESTORING_BACKUP: Text = Text { key: "restoring_backup", english: "Restoring backup" };
pub const RESTORING_BACKUP_ERROR: Text = Text { key: "restoring_backup_error", english: "Restoring backup failed" };
pub const RESTORING_BACKUP_CANCELED: Text = Text { key: "restoring_backup_canceled", english: "Canceled restore" };
pub const RESTORE_COMPLETED: Text = Text { key: "restore_completed", english: "Restore completed" };
pub const MISSING_SOURCES: Text = Text { key: "backup_restore_missing_sources", english: "Missing sources:" };
pub const TRACKERS_NOT_LOGGED_IN: Text = Text { key: "backup_restore_missing_trackers", english: "Trackers not logged into:" };
pub const SELECT_BACKUP_FILE: Text = Text { key: "file_select_backup", english: "Select backup file" };
pub const NO_FILE_SELECTED: Text = Text { key: "file_null_uri_error", english: "No file selected" };
pub const INVALID_BACKUP_FILE: Text = Text { key: "invalid_backup_file", english: "Invalid backup file:" };
pub const INVALID_BACKUP_FILE_ERROR: Text = Text { key: "invalid_backup_file_error", english: "Full error:" };
pub const MISSING_HINT: Text = Text {
    key: "backup_restore_content_full",
    english: "You may need to install any missing extensions and log in to tracking services afterwards to use them.",
};
// The seven restore option labels, in the order of `RestoreOptions::to_bits` (section 12).
pub const LABEL_LIBRARY: Text = Text { key: "label_library", english: "Library" };
pub const CATEGORIES: Text = Text { key: "categories", english: "Categories" };
pub const APP_SETTINGS: Text = Text { key: "app_settings", english: "App settings" };
pub const EXTENSION_STORES: Text = Text { key: "extensionStores", english: "Extension stores" };
pub const CUSTOM_BUTTON_SETTINGS: Text = Text { key: "custom_button_settings", english: "Custom buttons" };
pub const SOURCE_SETTINGS: Text = Text { key: "source_settings", english: "Source settings" };
pub const LABEL_EXTENSIONS: Text = Text { key: "label_extensions", english: "Extensions" };

// Kurobayomi-only texts: they have no Aniyomi source, so they live in `kuro-i18n/locales/en/kurobayomi-extra.ftl`.
pub const CHECKING_BACKUP: Text = Text { key: "restore_checking_backup", english: "Checking backup file…" };
pub const KEEP_RUNNING: Text = Text { key: "restore_close_keep_running", english: "Keep running" };
pub const CANCEL_AND_CLOSE: Text = Text { key: "restore_close_cancel_and_close", english: "Cancel restore and close" };
pub const PROGRESS_ANIME_CATEGORIES: Text = Text { key: "restore_progress_anime_categories", english: "Anime categories" };
pub const PROGRESS_MANGA_CATEGORIES: Text = Text { key: "restore_progress_manga_categories", english: "Manga categories" };

/// The info line under the Extensions card of Create backup (T-21a-019).
pub const BACKUP_EXTENSIONS_INFO: Text = Text {
    key: "backup_extensions_info",
    english: "Extension packages are saved as Kurobayomi extension files. Android APK extensions are replaced by them and are not included.",
};

/// The restore option labels in bit order (`RestoreOptions::to_bits`).
pub const RESTORE_OPTION_LABELS: [Text; 7] =
    [LABEL_LIBRARY, CATEGORIES, APP_SETTINGS, EXTENSION_STORES, CUSTOM_BUTTON_SETTINGS, SOURCE_SETTINGS, LABEL_EXTENSIONS];

#[cfg(test)]
mod tests {
    use super::*;

    const FTL: &str = include_str!("../../kuro-i18n/locales/en/kurobayomi.ftl");

    fn catalogue_line(key: &str) -> Option<&'static str> {
        FTL.lines().find(|l| l.starts_with(&format!("{key} = ")))
    }

    /// Every catalogue key must exist in the English catalogue with the same English default, so a translator sees it.
    #[test]
    fn every_key_exists_in_the_english_catalogue_with_the_english_default() {
        let mut all = vec![
            RESTORE_IN_PROGRESS,
            RESTORING_BACKUP,
            RESTORING_BACKUP_ERROR,
            RESTORING_BACKUP_CANCELED,
            RESTORE_COMPLETED,
            MISSING_SOURCES,
            TRACKERS_NOT_LOGGED_IN,
            SELECT_BACKUP_FILE,
            NO_FILE_SELECTED,
            INVALID_BACKUP_FILE,
            INVALID_BACKUP_FILE_ERROR,
            MISSING_HINT,
            CHECKING_BACKUP,
            KEEP_RUNNING,
            CANCEL_AND_CLOSE,
            PROGRESS_ANIME_CATEGORIES,
            PROGRESS_MANGA_CATEGORIES,
            BACKUP_EXTENSIONS_INFO,
        ];
        all.extend(RESTORE_OPTION_LABELS);
        for text in all {
            assert_eq!(catalogue_line(text.key), Some(format!("{} = {}", text.key, text.english).as_str()), "key {}", text.key);
        }
    }
}
