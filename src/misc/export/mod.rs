//! The library CSV export (area 35: T-35a-070 to T-35a-077 and T-35a-095). The rows and their bytes are in
//! `library_csv`, the atomic write in `write`. The dialog and the file chooser are in the settings page (another lane);
//! they call `save`, `toast` and `ExportSession`.

#![allow(dead_code)] // the settings page calls these from the settings lane

pub mod library_csv;
pub mod write;

use std::path::Path;

use kuro_data::entry::Entry;

use library_csv::{ExportOptions, favourite_entries, render};
use write::ExportError;

/// The toast after a successful write (T-35a-075, `export.done`).
pub const DONE: &str = "Library Exported";
/// The toast after a successful write of an empty library (T-35a-075, `export.done_empty`).
pub const DONE_EMPTY: &str = "Library Exported (0 entries)";

/// Snapshots the favourites and writes the file (T-35a-076). Call it when Save is confirmed, with the lists read then,
/// so a favourite changed after the page opened is in the file. Returns the number of rows written.
pub async fn save(target: &Path, favourites: &[Entry], options: ExportOptions) -> Result<usize, ExportError> {
    let entries = favourite_entries(favourites);
    let text = render(&entries, options);
    write::write_csv_off_thread(target.to_path_buf(), text).await?;
    Ok(entries.len())
}

/// The toast of a finished save (T-35a-075). `None` is a dismissed file chooser, which stays silent (T-35a-073).
pub fn toast(result: Option<&Result<usize, ExportError>>) -> Option<String> {
    match result? {
        Ok(0) => Some(DONE_EMPTY.to_owned()),
        Ok(_) => Some(DONE.to_owned()),
        Err(error) => Some(format!("Export failed: {error}")),
    }
}

/// The export options the settings page keeps while it is open (T-35a-077). They are reset to all checked when the
/// page is left, and they are never written to preferences.
#[derive(Debug, Clone, Copy)]
pub struct ExportSession {
    options: ExportOptions,
}

impl Default for ExportSession {
    fn default() -> Self {
        Self { options: ExportOptions::ALL }
    }
}

impl ExportSession {
    pub fn options(&self) -> ExportOptions {
        self.options
    }

    pub fn set(&mut self, options: ExportOptions) {
        self.options = options;
    }

    /// Called when the settings page closes.
    pub fn leave(&mut self) {
        self.options = ExportOptions::ALL;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toast_follows_the_result_and_a_dismissed_chooser_is_silent() {
        assert_eq!(toast(None), None);
        assert_eq!(toast(Some(&Ok(0))).as_deref(), Some("Library Exported (0 entries)"));
        assert_eq!(toast(Some(&Ok(3))).as_deref(), Some("Library Exported"));
        let failed = Err(ExportError::Write("/x.csv".into(), std::io::Error::other("disk full")));
        assert_eq!(toast(Some(&failed)).as_deref(), Some("Export failed: /x.csv: disk full"));
    }

    #[test]
    fn options_are_kept_while_open_and_reset_when_the_page_is_left() {
        let mut session = ExportSession::default();
        assert_eq!(session.options(), ExportOptions::ALL);
        session.set(ExportOptions::ALL.with_title(false));
        assert!(!session.options().title);
        session.leave();
        assert_eq!(session.options(), ExportOptions::ALL);
    }

    #[tokio::test]
    async fn an_empty_library_writes_an_empty_file_and_reports_zero_rows() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("kurobayomi_library.csv");
        let written = save(&target, &[], ExportOptions::ALL).await.unwrap();
        assert_eq!(written, 0);
        assert_eq!(std::fs::read(&target).unwrap(), b"");
    }
}
