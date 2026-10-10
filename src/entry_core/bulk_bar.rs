//! The bulk action bar of the manga chapter selection (T-38a-024, behaviour 7.7). Pure: which buttons show for the
//! selected chapters, in order, with their labels and icons. The widget in `route` draws them and sends the action.
//! Upstream: `EntryBottomActionMenu.kt` (the manga branch: no fillermark and no play buttons).

use kuro_downloads::DownloadState;
use kuro_i18n::MsgKey;

use crate::entry_core::chapter::ChapterRow;

/// The buttons of the bar, in the order they show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulkAction {
    Bookmark,
    Unbookmark,
    MarkRead,
    MarkUnread,
    MarkPrevious,
    Download,
    Delete,
}

impl BulkAction {
    /// The label: the button's tooltip and accessible name.
    pub fn label(self) -> MsgKey {
        match self {
            Self::Bookmark => MsgKey::action_bookmark,
            Self::Unbookmark => MsgKey::action_remove_bookmark,
            Self::MarkRead => MsgKey::action_mark_as_read,
            Self::MarkUnread => MsgKey::action_mark_as_unread,
            Self::MarkPrevious => MsgKey::action_mark_previous_as_read,
            Self::Download => MsgKey::action_download,
            Self::Delete => MsgKey::action_delete,
        }
    }

    /// The icon name, from the Adwaita set.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Bookmark => "starred-symbolic",
            Self::Unbookmark => "non-starred-symbolic",
            Self::MarkRead => "object-select-symbolic",
            Self::MarkUnread => "edit-undo-symbolic",
            Self::MarkPrevious => "go-up-symbolic",
            Self::Download => "folder-download-symbolic",
            Self::Delete => "user-trash-symbolic",
        }
    }
}

/// The buttons for the selected chapters. Nothing selected shows nothing. A local manga has no Download button, since
/// its chapters are already on disk.
pub fn bulk_actions(selected: &[&ChapterRow], local: bool) -> Vec<BulkAction> {
    if selected.is_empty() {
        return Vec::new();
    }
    let any = |f: &dyn Fn(&ChapterRow) -> bool| selected.iter().any(|c| f(c));
    let all = |f: &dyn Fn(&ChapterRow) -> bool| selected.iter().all(|c| f(c));
    let downloaded = |c: &ChapterRow| c.download == DownloadState::Downloaded;
    let mut out = Vec::new();
    if any(&|c| !c.bookmark) {
        out.push(BulkAction::Bookmark);
    }
    if all(&|c| c.bookmark) {
        out.push(BulkAction::Unbookmark);
    }
    if any(&|c| !c.read) {
        out.push(BulkAction::MarkRead);
    }
    if any(&|c| c.read || c.last_page_read > 0) {
        out.push(BulkAction::MarkUnread);
    }
    if selected.len() == 1 {
        out.push(BulkAction::MarkPrevious);
    }
    if !local && any(&|c| !downloaded(c)) {
        out.push(BulkAction::Download);
    }
    if any(&downloaded) {
        out.push(BulkAction::Delete);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(read: bool, bookmark: bool, last_page_read: i64, download: DownloadState) -> ChapterRow {
        ChapterRow { read, bookmark, last_page_read, download, ..ChapterRow::new(1, "c", 1.0) }
    }

    #[test]
    fn nothing_selected_shows_no_buttons() {
        assert!(bulk_actions(&[], false).is_empty());
    }

    #[test]
    fn the_bookmark_pair_follows_the_selection() {
        let plain = row(false, false, 0, DownloadState::NotDownloaded);
        let marked = row(false, true, 0, DownloadState::NotDownloaded);
        assert_eq!(bulk_actions(&[&plain], false)[0], BulkAction::Bookmark);
        assert!(!bulk_actions(&[&plain], false).contains(&BulkAction::Unbookmark));
        assert!(bulk_actions(&[&marked], false).contains(&BulkAction::Unbookmark));
        let mixed = bulk_actions(&[&plain, &marked], false);
        assert!(mixed.contains(&BulkAction::Bookmark) && !mixed.contains(&BulkAction::Unbookmark), "any unmarked shows Bookmark");
    }

    #[test]
    fn mark_read_and_unread_follow_the_read_state_and_progress() {
        let unread = row(false, false, 0, DownloadState::NotDownloaded);
        let started = row(false, false, 4, DownloadState::NotDownloaded);
        let read = row(true, false, 0, DownloadState::NotDownloaded);
        let actions = bulk_actions(&[&unread], false);
        assert!(actions.contains(&BulkAction::MarkRead) && !actions.contains(&BulkAction::MarkUnread));
        assert!(bulk_actions(&[&started], false).contains(&BulkAction::MarkUnread), "progress counts as read");
        assert!(bulk_actions(&[&read], false).contains(&BulkAction::MarkUnread));
        assert!(!bulk_actions(&[&read], false).contains(&BulkAction::MarkRead));
    }

    #[test]
    fn mark_previous_needs_exactly_one_chapter() {
        let a = row(false, false, 0, DownloadState::NotDownloaded);
        let b = row(false, false, 0, DownloadState::NotDownloaded);
        assert!(bulk_actions(&[&a], false).contains(&BulkAction::MarkPrevious));
        assert!(!bulk_actions(&[&a, &b], false).contains(&BulkAction::MarkPrevious));
    }

    #[test]
    fn download_is_hidden_for_local_and_delete_needs_a_download() {
        let fresh = row(false, false, 0, DownloadState::NotDownloaded);
        let done = row(false, false, 0, DownloadState::Downloaded);
        assert!(bulk_actions(&[&fresh], false).contains(&BulkAction::Download));
        assert!(!bulk_actions(&[&fresh], true).contains(&BulkAction::Download), "a local manga has no Download");
        assert!(!bulk_actions(&[&fresh], false).contains(&BulkAction::Delete));
        assert!(bulk_actions(&[&done], false).contains(&BulkAction::Delete));
        assert!(!bulk_actions(&[&done], false).contains(&BulkAction::Download));
    }
}
