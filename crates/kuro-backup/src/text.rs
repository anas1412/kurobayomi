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
pub const SYNCING_LIBRARY: Text = Text { key: "syncing_library", english: "Syncing library" };
pub const LIBRARY_SYNC_COMPLETE: Text = Text { key: "library_sync_complete", english: "Library sync complete" };
pub const MISSING_SOURCES: Text = Text { key: "backup_restore_missing_sources", english: "Missing sources:" };
pub const TRACKERS_NOT_LOGGED_IN: Text = Text { key: "backup_restore_missing_trackers", english: "Trackers not logged into:" };
pub const SELECT_BACKUP_FILE: Text = Text { key: "file_select_backup", english: "Select backup file" };
pub const NO_FILE_SELECTED: Text = Text { key: "file_null_uri_error", english: "No file selected" };

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key must exist in the English catalogue, so a translator sees it.
    #[test]
    fn every_key_exists_in_the_english_catalogue_with_the_english_default() {
        let ftl = include_str!("../../kuro-i18n/locales/en/kurobayomi.ftl");
        for text in [
            RESTORE_IN_PROGRESS,
            RESTORING_BACKUP,
            RESTORING_BACKUP_ERROR,
            RESTORING_BACKUP_CANCELED,
            RESTORE_COMPLETED,
            SYNCING_LIBRARY,
            LIBRARY_SYNC_COMPLETE,
            MISSING_SOURCES,
            TRACKERS_NOT_LOGGED_IN,
            SELECT_BACKUP_FILE,
            NO_FILE_SELECTED,
        ] {
            let line = ftl.lines().find(|l| l.starts_with(&format!("{} = ", text.key)));
            assert_eq!(line, Some(format!("{} = {}", text.key, text.english).as_str()), "key {}", text.key);
        }
    }
}
