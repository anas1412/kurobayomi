//! Live download updates for the chapter list: one row patched by chapter id, progress throttled to 10 Hz.
//! Upstream: `MangaScreenModel.kt:493-531` (observeDownloads). Plan: `p2-behavior.md` 6.2; E-DL-6.

use kuro_downloads::DownloadState;

use crate::entry_core::detail::LoadState;

/// Progress updates arrive at most every 100 ms per manga (10 Hz). Status changes are never throttled.
pub const PROGRESS_INTERVAL_MS: u64 = 100;

/// A download event for one chapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadEvent {
    Status {
        chapter_id: i64,
        state: DownloadState,
    },
    Progress {
        chapter_id: i64,
        percent: u8,
    },
}

/// Throttles progress events for one manga. The first event passes, then one per interval.
#[derive(Clone, Debug, Default)]
pub struct ProgressThrottle {
    last_ms: Option<u64>,
}

impl ProgressThrottle {
    pub fn allow(&mut self, now_ms: u64) -> bool {
        match self.last_ms {
            Some(last) if now_ms.saturating_sub(last) < PROGRESS_INTERVAL_MS => false,
            _ => {
                self.last_ms = Some(now_ms);
                true
            }
        }
    }
}

/// Applies one event to the loaded list and returns the index of the row that changed, for one `items_changed`.
/// Events before `Ready` are dropped (upstream drops them too). Local rows never show queue state, and a status
/// event for them changes nothing. Unknown chapter ids are ignored.
pub fn apply_event(
    state: &mut LoadState,
    event: DownloadEvent,
    throttle: &mut ProgressThrottle,
    now_ms: u64,
) -> Option<usize> {
    let LoadState::Ready(page) = state else {
        return None;
    };
    let is_local = page.manga.local;
    let chapter_id = match event {
        DownloadEvent::Status { chapter_id, .. } | DownloadEvent::Progress { chapter_id, .. } => {
            chapter_id
        }
    };
    let index = page.chapters.iter().position(|c| c.id == chapter_id)?;
    let row = &mut page.chapters[index];
    match event {
        DownloadEvent::Status { state, .. } => {
            if is_local || row.download == state {
                return None;
            }
            row.download = state;
            row.download_progress = 0;
            Some(index)
        }
        DownloadEvent::Progress { percent, .. } => {
            if is_local || !throttle.allow(now_ms) || row.download_progress == percent {
                return None;
            }
            row.download_progress = percent;
            Some(index)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_core::chapter::ChapterRow;
    use crate::entry_core::detail::{EntryState, LoadState, MangaRow};
    use crate::entry_core::dialog::DialogSlot;
    use crate::entry_core::flags::ChapterFlags;
    use crate::entry_core::selection::Selection;

    fn page(local: bool, ids: &[i64]) -> LoadState {
        let manga = MangaRow {
            id: 1,
            source: 1,
            url: String::new(),
            title: String::new(),
            author: None,
            artist: None,
            description: None,
            genres: vec![],
            status: 0,
            thumbnail_url: None,
            initialized: true,
            favorite: true,
            local,
            flags: ChapterFlags::default(),
        };
        LoadState::Ready(Box::new(EntryState {
            manga,
            from_source: false,
            chapters: ids
                .iter()
                .map(|&id| ChapterRow::new(id, "c", id as f64))
                .collect(),
            selection: Selection::default(),
            available_scanlators: vec![],
            excluded_scanlators: Default::default(),
            tracking_count: 0,
            has_logged_in_trackers: false,
            has_prompted_add: false,
            is_refreshing: false,
            dialog: DialogSlot::default(),
            downloaded_only: false,
            source: Default::default(),
            skip_filtered: false,
            categories: vec![],
            category_membership: vec![],
        }))
    }

    fn status(id: i64, state: DownloadState) -> DownloadEvent {
        DownloadEvent::Status {
            chapter_id: id,
            state,
        }
    }

    #[test]
    fn events_before_ready_are_dropped() {
        let mut state = LoadState::Loading;
        let mut t = ProgressThrottle::default();
        assert_eq!(
            apply_event(&mut state, status(1, DownloadState::Queue), &mut t, 0),
            None
        );
    }

    #[test]
    fn status_updates_one_row_in_place() {
        let mut state = page(false, &[1, 2, 3]);
        let mut t = ProgressThrottle::default();
        assert_eq!(
            apply_event(&mut state, status(2, DownloadState::Queue), &mut t, 0),
            Some(1)
        );
        assert_eq!(
            apply_event(&mut state, status(2, DownloadState::Queue), &mut t, 0),
            None,
            "no change, no update"
        );
        assert_eq!(
            apply_event(&mut state, status(99, DownloadState::Queue), &mut t, 0),
            None,
            "unknown id"
        );
    }

    #[test]
    fn progress_is_throttled_to_ten_hz_and_status_is_not() {
        let mut state = page(false, &[1]);
        let mut t = ProgressThrottle::default();
        let p = |percent| DownloadEvent::Progress {
            chapter_id: 1,
            percent,
        };
        assert_eq!(apply_event(&mut state, p(10), &mut t, 0), Some(0));
        assert_eq!(
            apply_event(&mut state, p(20), &mut t, 50),
            None,
            "inside the 100 ms window"
        );
        assert_eq!(apply_event(&mut state, p(30), &mut t, 100), Some(0));
        assert_eq!(
            apply_event(
                &mut state,
                status(1, DownloadState::Downloading),
                &mut t,
                101
            ),
            Some(0),
            "status is never throttled"
        );
    }

    #[test]
    fn local_rows_never_show_queue_state() {
        let mut state = page(true, &[1]);
        let mut t = ProgressThrottle::default();
        assert_eq!(
            apply_event(&mut state, status(1, DownloadState::Queue), &mut t, 0),
            None
        );
    }
}
