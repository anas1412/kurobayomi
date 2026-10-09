//! Backup errors. `Display` strings are for logs only; user-visible text comes from `key` and `user_text` (B-16).

use crate::wire::WireError;

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("couldn't create a backup file")]
    CreateFile,
    #[error("no library entries to back up")]
    EmptyBackup,
    #[error("backup is already in progress")]
    InProgress,
    #[error("JSON backup not supported")]
    JsonNotSupported,
    /// Decode side: truncated input, corrupt gzip, wire error, bad enum. Logged with the detail.
    #[error("backup file is corrupted: {0}")]
    Corrupt(String),
    /// Encode side: an entry cannot be written (unsupported value or blocked layout).
    #[error("encoding failed: {0}")]
    Encode(WireError),
    /// A converted integer does not fit its format field. The row id is in the log.
    #[error("{entry} {id}: {field} does not fit the backup format")]
    Overflow { entry: &'static str, id: i64, field: &'static str },
    /// A decoded backup with no entries, preferences, categories or extensions (E-04, restore only).
    #[error("backup does not contain any library entries")]
    NoEntries,
    /// The library reader failed (database error, lock timeout).
    #[error("library read failed: {0}")]
    Read(String),
    #[error("backup failed: {0}")]
    Io(#[from] std::io::Error),
}

impl BackupError {
    /// Message key from B-16. Every variant maps to exactly one key.
    pub fn key(&self) -> &'static str {
        match self {
            Self::CreateFile => "create_backup_file_error",
            Self::EmptyBackup => "empty_backup_error",
            Self::InProgress => "backup_in_progress",
            Self::JsonNotSupported => "invalid_backup_file_json",
            Self::NoEntries => "invalid_backup_file_missing_manga",
            Self::Corrupt(_) => "invalid_backup_file_unknown",
            Self::Encode(_) | Self::Overflow { .. } | Self::Read(_) | Self::Io(_) => "creating_backup_error",
        }
    }

    /// English text for the key (B-16). Restore and create UIs show this, never `Display`.
    pub fn user_text(&self) -> &'static str {
        match self.key() {
            "create_backup_file_error" => "Couldn't create a backup file",
            "empty_backup_error" => "No library entries to back up",
            "backup_in_progress" => "Backup is already in progress",
            "invalid_backup_file_json" => "JSON backup not supported",
            "invalid_backup_file_missing_manga" => "Backup does not contain any library entries",
            "invalid_backup_file_unknown" => "Backup file is corrupted",
            _ => "Backup failed",
        }
    }
}

impl From<WireError> for BackupError {
    fn from(e: WireError) -> Self {
        Self::Corrupt(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_maps_to_one_key() {
        let cases: Vec<(BackupError, &str)> = vec![
            (BackupError::CreateFile, "create_backup_file_error"),
            (BackupError::EmptyBackup, "empty_backup_error"),
            (BackupError::InProgress, "backup_in_progress"),
            (BackupError::JsonNotSupported, "invalid_backup_file_json"),
            (BackupError::Corrupt("x".into()), "invalid_backup_file_unknown"),
            (BackupError::NoEntries, "invalid_backup_file_missing_manga"),
            (BackupError::Encode(WireError::Truncated), "creating_backup_error"),
            (BackupError::Overflow { entry: "manga", id: 1, field: "status" }, "creating_backup_error"),
            (BackupError::Read("x".into()), "creating_backup_error"),
            (BackupError::Io(std::io::Error::other("disk")), "creating_backup_error"),
        ];
        for (e, key) in cases {
            assert_eq!(e.key(), key);
            assert!(!e.user_text().is_empty());
        }
    }

    #[test]
    fn corrupt_text_is_the_key_text_not_the_detail() {
        let e = BackupError::from(WireError::Truncated);
        assert_eq!(e.user_text(), "Backup file is corrupted");
        assert!(!e.user_text().contains("truncated"));
    }
}
