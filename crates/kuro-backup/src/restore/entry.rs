//! Restore page entry (area 22, T-22a-001): the Restore action, the file dialog and what a pick does. GTK opens the
//! dialog and the page; this module holds the decisions and the texts.

use std::path::PathBuf;

use crate::text::{self, Text};

pub const DIALOG_TITLE: &str = text::SELECT_BACKUP_FILE.english;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFilter {
    pub name: &'static str,
    pub pattern: &'static str,
}

/// Filters of the open dialog: backup files first, then everything.
pub const FILE_FILTERS: [FileFilter; 2] = [
    FileFilter { name: "Aniyomi backup", pattern: "*.tachibk" },
    FileFilter { name: "All files", pattern: "*" },
];

/// Result of the file dialog. `Failed` is a dialog error, not a cancel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    Cancelled,
    Failed,
    Chosen(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Shown when a restore is running; the dialog stays closed.
    Banner(Text),
    OpenDialog,
    Toast(Text),
    OpenPage(PathBuf),
    Nothing,
}

/// The Restore action from Settings > Data.
pub fn on_restore_clicked(restore_running: bool) -> Route {
    if restore_running { Route::Banner(text::RESTORE_IN_PROGRESS) } else { Route::OpenDialog }
}

/// The dialog result.
pub fn on_dialog_result(pick: Pick) -> Route {
    match pick {
        Pick::Cancelled => Route::Nothing,
        Pick::Failed => Route::Toast(text::NO_FILE_SELECTED),
        Pick::Chosen(path) => Route::OpenPage(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_restore_blocks_the_dialog() {
        assert_eq!(on_restore_clicked(true), Route::Banner(text::RESTORE_IN_PROGRESS));
        assert_eq!(on_restore_clicked(false), Route::OpenDialog);
    }

    #[test]
    fn cancel_does_nothing_and_error_says_no_file() {
        assert_eq!(on_dialog_result(Pick::Cancelled), Route::Nothing);
        assert_eq!(on_dialog_result(Pick::Failed), Route::Toast(text::NO_FILE_SELECTED));
    }

    #[test]
    fn chosen_file_opens_the_page() {
        let p = PathBuf::from("/home/u/b.tachibk");
        assert_eq!(on_dialog_result(Pick::Chosen(p.clone())), Route::OpenPage(p));
    }

    #[test]
    fn dialog_title_and_filters() {
        assert_eq!(DIALOG_TITLE, "Select backup file");
        assert_eq!(FILE_FILTERS[0].pattern, "*.tachibk");
        assert_eq!(FILE_FILTERS[1].pattern, "*");
    }
}
