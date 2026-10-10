//! The atomic CSV write (T-35a-074). The text goes to `<name>.kuro-tmp` beside the target, is synced and renamed over
//! it. Any failure removes the temp file and reports the target and the io error, so the old file stays as it was.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ExportError {
    /// The write of the target failed; the temp file is gone.
    Write(PathBuf, std::io::Error),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Write(path, error) => write!(f, "{}: {error}", path.display()),
        }
    }
}

/// The temp file of `target`: the same name with `.kuro-tmp` added, in the same folder.
pub fn temp_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(".kuro-tmp");
    PathBuf::from(name)
}

/// Writes `text` (UTF-8, no byte-order mark) to `target` through the temp file.
pub fn write_csv(target: &Path, text: &str) -> Result<(), ExportError> {
    let temp = temp_path(target);
    let result = (|| -> std::io::Result<()> {
        let mut file = File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, target)
    })();
    result.map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        ExportError::Write(target.to_path_buf(), error)
    })
}

/// [`write_csv`] on the blocking pool, so the GTK thread never waits for the disk (T-35a-074).
pub async fn write_csv_off_thread(target: PathBuf, text: String) -> Result<(), ExportError> {
    let shown = target.clone();
    tokio::task::spawn_blocking(move || write_csv(&target, &text))
        .await
        .unwrap_or_else(|join| Err(ExportError::Write(shown, std::io::Error::other(join))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_replaces_the_existing_file_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("kurobayomi_library.csv");
        std::fs::write(&target, "old").unwrap();
        write_csv(&target, "new\r\nrow").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new\r\nrow");
        assert!(!temp_path(&target).exists());
    }

    #[test]
    fn a_failed_write_removes_the_temp_file_and_names_the_target() {
        // The target is a folder, so the rename over it fails after the temp file was written.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("taken");
        std::fs::create_dir(&target).unwrap();
        let error = write_csv(&target, "data").unwrap_err();
        assert!(matches!(&error, ExportError::Write(path, _) if path == &target));
        assert!(!temp_path(&target).exists());
        assert!(error.to_string().contains("taken"));
    }

    #[test]
    fn a_missing_folder_is_a_write_error_with_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("missing").join("out.csv");
        assert!(write_csv(&target, "x").is_err());
        assert!(!temp_path(&target).exists());
    }

    #[tokio::test]
    async fn the_off_thread_write_reports_success() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out.csv");
        write_csv_off_thread(target.clone(), "a,b".into()).await.unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "a,b");
    }
}
