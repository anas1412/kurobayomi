//! Chapter row actions of the manga page that are pure decisions (T-38b-017): the download indicator mapping, which
//! rows a download action may act on, and mark previous as read. Upstream: `MangaScreenModel.kt` (download actions,
//! `markPreviousAsRead`) and `ChapterDownloadIndicator.kt`. Behaviour 6.9.1, 6.9.8; E-DL-1, E-DL-2, E-CANCEL-1, E-PREV-1.

use kuro_downloads::DownloadState;

/// The action the download indicator of a row starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndicatorAction {
    StartNow,
    Cancel,
    Delete,
}

pub fn indicator_action(state: DownloadState) -> IndicatorAction {
    match state {
        DownloadState::NotDownloaded | DownloadState::Error => IndicatorAction::StartNow,
        DownloadState::Queue | DownloadState::Downloading => IndicatorAction::Cancel,
        DownloadState::Downloaded => IndicatorAction::Delete,
    }
}

/// START_NOW and CANCEL act only on exactly one row (upstream ignores several, E-DL-1, E-DL-2). Delete acts on any.
pub fn acts_on(action: IndicatorAction, rows: usize) -> bool {
    match action {
        IndicatorAction::StartNow | IndicatorAction::Cancel => rows == 1,
        IndicatorAction::Delete => rows > 0,
    }
}

/// The local state a cancel sets at once. A later downloader event overrides it (E-CANCEL-1).
pub fn state_after_cancel() -> DownloadState {
    DownloadState::NotDownloaded
}

/// The chapters chronologically before `pointer`, in chronological order (6.9.8). `view` is the displayed list. In a
/// descending view the earlier chapters come after the pointer; the list is reversed to chronological order. A pointer
/// that is not in the view marks nothing (E-PREV-1).
pub fn previous_ids(view: &[i64], pointer: i64, descending: bool) -> Vec<i64> {
    let Some(at) = view.iter().position(|id| *id == pointer) else {
        return Vec::new();
    };
    if descending {
        let mut earlier = view[at + 1..].to_vec();
        earlier.reverse();
        earlier
    } else {
        view[..at].to_vec()
    }
}

/// The Resume or Start button: visible when the processed list has an unread chapter and nothing is selected. It
/// reads "Resume" when the unfiltered list has a read chapter, else "Start" (MangaScreen.kt:346-361).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResumeFab {
    pub visible: bool,
    pub resume: bool,
}

pub fn resume_fab(processed_has_unread: bool, unfiltered_has_read: bool, selected_total: usize) -> ResumeFab {
    ResumeFab { visible: processed_has_unread && selected_total == 0, resume: unfiltered_has_read }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicator_maps_each_state() {
        assert_eq!(indicator_action(DownloadState::NotDownloaded), IndicatorAction::StartNow);
        assert_eq!(indicator_action(DownloadState::Error), IndicatorAction::StartNow);
        assert_eq!(indicator_action(DownloadState::Queue), IndicatorAction::Cancel);
        assert_eq!(indicator_action(DownloadState::Downloading), IndicatorAction::Cancel);
        assert_eq!(indicator_action(DownloadState::Downloaded), IndicatorAction::Delete);
    }

    #[test]
    fn start_now_and_cancel_need_one_row() {
        assert!(acts_on(IndicatorAction::StartNow, 1));
        assert!(!acts_on(IndicatorAction::StartNow, 2));
        assert!(acts_on(IndicatorAction::Cancel, 1));
        assert!(!acts_on(IndicatorAction::Cancel, 3));
        assert!(acts_on(IndicatorAction::Delete, 3));
    }

    #[test]
    fn cancel_sets_not_downloaded_locally() {
        assert_eq!(state_after_cancel(), DownloadState::NotDownloaded);
    }

    #[test]
    fn mark_previous_uses_the_view_vector_of_the_spec() {
        assert_eq!(previous_ids(&[9, 8, 7, 6], 7, true), vec![6]);
        assert_eq!(previous_ids(&[9, 8, 7, 6, 5], 7, true), vec![5, 6], "chronological order");
        assert_eq!(previous_ids(&[5, 6, 7, 8], 7, false), vec![5, 6]);
    }

    #[test]
    fn resume_button_rules() {
        assert_eq!(resume_fab(true, true, 0), ResumeFab { visible: true, resume: true });
        assert_eq!(resume_fab(true, false, 0), ResumeFab { visible: true, resume: false }, "Start");
        assert!(!resume_fab(true, true, 1).visible, "hidden while selecting");
        assert!(!resume_fab(false, true, 0).visible, "nothing unread");
    }

    #[test]
    fn mark_previous_with_a_pointer_outside_the_view_marks_nothing() {
        assert!(previous_ids(&[9, 8, 7], 42, true).is_empty());
        assert!(previous_ids(&[9, 8, 7], 9, true).len() == 2, "descending: everything earlier");
    }
}
