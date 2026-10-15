//! The chapter and episode page cache of Data and storage (task T-47a-089): its size, clearing it (the journal file
//! stays, as Aniyomi's `ChapterCache` keeps it) and the messages. The auto-clear switch is in `data_storage_rows`.
use kuro_i18n::{MsgKey, tr};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use kuro_platform::paths::Paths;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::rows::text;
use crate::settings_ui::usage::si_size;

/// The chapter page cache directory of this app.
pub fn cache_dir() -> Option<PathBuf> {
    kuro_platform::linux::LinuxPaths::system().ok().map(|p| p.chapter_cache_dir())
}

/// `journal` and `journal.*` are the cache's bookkeeping and stay when the cache is cleared.
fn is_journal(name: &str) -> bool {
    name == "journal" || name.starts_with("journal.")
}

/// The bytes of the regular files directly in `dir` (the cache is flat); 0 when it cannot be read.
pub fn used_bytes(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.metadata().ok())
                .filter(|meta| meta.is_file())
                .map(|meta| meta.len())
                .sum()
        })
        .unwrap_or(0)
}

/// Deletes the cached files of `dir` except the journal and returns how many were deleted.
pub fn clear(dir: &Path) -> io::Result<usize> {
    let mut deleted = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if is_journal(&entry.file_name().to_string_lossy()) {
            continue;
        }
        if entry.file_type()?.is_file() {
            std::fs::remove_file(entry.path())?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// The toast after a clear: `Cache cleared, <n> files deleted`, or the error text.
pub fn cleared_message(result: &io::Result<usize>) -> String {
    match result {
        Ok(deleted) => format!("Cache cleared, {deleted} files deleted"),
        Err(_) => tr(MsgKey::cache_delete_error).to_owned(),
    }
}

/// The "Clear chapter and episode cache" row, subtitled `Used: <size>`. Tapping clears the cache, shows the toast and
/// runs `changed` so the page shows the new size.
pub fn clear_row(changed: Rc<dyn Fn()>) -> PreferenceItem {
    let dir = cache_dir();
    let used = dir.as_deref().map(used_bytes).unwrap_or(0);
    let mut row = text(&tr(MsgKey::pref_clear_chapter_cache), Some(&format!("Used: {}", si_size(used))), true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(move || {
            let result = match &dir {
                Some(dir) => clear(dir),
                None => Err(io::Error::other("no chapter cache directory")),
            };
            if let Err(err) = &result {
                tracing::warn!(%err, "clearing the chapter cache failed");
            }
            crate::toast::notify(cleared_message(&result));
            changed();
        }));
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_deletes_the_cached_files_and_keeps_the_journal() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["page-1.jpg", "page-2.jpg", "journal", "journal.bak"] {
            std::fs::write(dir.path().join(name), b"0123456789").unwrap();
        }
        assert_eq!(used_bytes(dir.path()), 40);
        assert_eq!(clear(dir.path()).unwrap(), 2);
        let mut left: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["journal", "journal.bak"]);
        assert_eq!(used_bytes(dir.path()), 20);
    }

    #[test]
    fn the_toast_reports_the_count_or_the_error() {
        assert_eq!(cleared_message(&Ok(3)), "Cache cleared, 3 files deleted");
        assert_eq!(cleared_message(&Err(io::Error::other("denied"))), "Error occurred while clearing");
    }

    #[test]
    fn a_missing_directory_is_an_error_and_uses_no_bytes() {
        let missing = Path::new("/no/such/kuro/cache");
        assert!(clear(missing).is_err());
        assert_eq!(used_bytes(missing), 0);
    }

    #[test]
    fn the_row_shows_the_used_size_and_is_titled_as_the_spec() {
        let row = clear_row(Rc::new(|| {}));
        assert_eq!(row.title(), "Clear chapter and episode cache");
        let PreferenceItem::Text { common, on_click, .. } = row else { panic!("text row") };
        assert!(common.subtitle.as_deref().is_some_and(|s| s.starts_with("Used: ")));
        assert!(on_click.is_some());
    }
}
