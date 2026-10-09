//! Library add and remove core of the anime page (T-39b-024). Upstream: `AnimeScreenModel.kt` `toggleFavorite`, the
//! remove branch (behaviour 4.3, lines 391-456). Pure: the caller writes the favorite flag and the cover, then asks.

/// What a successful remove still has to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoveOutcome {
    /// The cover file was deleted, because `removeCovers()` changed the row (the cover timestamp is bumped too).
    pub cover_removed: bool,
    /// Downloads exist, so the delete prompt runs. Dismissing it keeps them.
    pub ask_delete_downloads: bool,
}

/// The remove branch. A failed favorite write does nothing (E-LIB-9): no cover delete and no prompt.
pub fn remove_from_library(favorite_write_ok: bool, covers_changed_row: bool, downloads: usize) -> Option<RemoveOutcome> {
    if !favorite_write_ok {
        return None;
    }
    Some(RemoveOutcome { cover_removed: covers_changed_row, ask_delete_downloads: downloads > 0 })
}

/// The add branch sets the favorite flag only. The prompt's answer decides the downloads (T-LIB-4).
pub fn add_to_library_flag() -> bool {
    true
}

/// Delete removes the downloads of the entry; dismiss keeps them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteAnswer {
    Delete,
    Keep,
}

/// How many downloads the answer removes.
pub fn downloads_removed(answer: DeleteAnswer, downloads: usize) -> usize {
    match answer {
        DeleteAnswer::Delete => downloads,
        DeleteAnswer::Keep => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_favorite_write_does_nothing() {
        assert_eq!(remove_from_library(false, true, 3), None);
    }

    #[test]
    fn remove_deletes_the_cover_only_when_the_row_changed() {
        let out = remove_from_library(true, true, 0).expect("written");
        assert!(out.cover_removed);
        let out = remove_from_library(true, false, 0).expect("written");
        assert!(!out.cover_removed);
    }

    #[test]
    fn remove_asks_about_downloads_only_when_there_are_some() {
        assert!(remove_from_library(true, false, 2).expect("written").ask_delete_downloads);
        assert!(!remove_from_library(true, false, 0).expect("written").ask_delete_downloads);
    }

    #[test]
    fn delete_removes_all_and_dismiss_keeps_them() {
        assert_eq!(downloads_removed(DeleteAnswer::Delete, 4), 4);
        assert_eq!(downloads_removed(DeleteAnswer::Keep, 4), 0);
        assert!(add_to_library_flag());
    }
}
