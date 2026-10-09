//! Queue view model for the manga download screen (T-23a-032, T-23a-034). Pure data: source groups, counts, row
//! progress and the messages the view redraws from. No GTK here. The view calls `DownloadManager` directly for
//! actions, and it redraws only after `QueueChanged` (ui-parity section 7).

use std::sync::Arc;
use std::time::Instant;

use crate::events::DownloadEvent;
use crate::model::{DownloadState, QueuedDownload};
use crate::throttle::{PROGRESS_INTERVAL, ProgressThrottle};

/// One queue row. `chapter_id` is the full 64-bit id, which is what row identity uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowView {
    pub chapter_id: i64,
    pub manga_id: i64,
    pub manga_title: String,
    pub chapter_name: String,
    pub status: DownloadState,
    /// Pages in `Ready` state.
    pub downloaded: usize,
    /// Pages in the page list, 0 before the list is known.
    pub total: usize,
    /// Sum of page progress, each page at most 100.
    pub progress_sum: i64,
}

impl RowView {
    pub fn from_item(item: &QueuedDownload) -> Self {
        Self {
            chapter_id: item.chapter.id,
            manga_id: item.manga.id,
            manga_title: item.manga.title.clone(),
            chapter_name: item.chapter.name.clone(),
            status: item.status,
            downloaded: item.downloaded_images(),
            total: item.pages.as_ref().map_or(0, Vec::len),
            progress_sum: item.total_progress(),
        }
    }

    /// Progress bar `(value, maximum)`: the page progress sum over `pages * 100`. Before the page list is known the bar
    /// is `(0, 1)`.
    pub fn bar(&self) -> (i64, i64) {
        if self.total == 0 { (0, 1) } else { (self.progress_sum, self.total as i64 * 100) }
    }

    /// Right-side text: `"<downloaded>/<pages>"`. Before the page list is known it is empty, or `"Downloading…"` while the
    /// chapter is downloading (the proposal in ui-parity 1.3, so a running row is never blank).
    pub fn progress_text(&self) -> String {
        match (self.total, self.status) {
            (0, DownloadState::Downloading) => "Downloading…".to_owned(),
            (0, _) => String::new(),
            _ => format!("{}/{}", self.downloaded, self.total),
        }
    }

    /// DOWNLOADED and ERROR end the row's progress job (behaviour 14.6).
    pub fn is_finished(&self) -> bool {
        matches!(self.status, DownloadState::Downloaded | DownloadState::Error)
    }
}

/// One source group: rows in queue order, the group named by the source's display name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupView {
    pub source_id: i64,
    pub name: String,
    pub rows: Vec<RowView>,
}

impl GroupView {
    /// Header text `"<name> (<count>)"`.
    pub fn header(&self) -> String {
        format!("{} ({})", self.name, self.rows.len())
    }
}

/// Groups by source in first-occurrence order. Rows keep the queue order inside each group.
pub fn build_groups(snapshot: &[Arc<QueuedDownload>]) -> Vec<GroupView> {
    let mut groups: Vec<GroupView> = Vec::new();
    for item in snapshot {
        let source_id = item.source.id();
        let row = RowView::from_item(item);
        match groups.iter_mut().find(|g| g.source_id == source_id) {
            Some(group) => group.rows.push(row),
            None => groups.push(GroupView { source_id, name: item.source.display_name(), rows: vec![row] }),
        }
    }
    groups
}

/// Text of the empty state (`download_queue` screen with no rows).
pub const EMPTY_TEXT: &str = "No downloads";

/// Label of the footer action for the manager state: Pause while the queue runs, Resume otherwise (the anime queue says
/// Stop and Continue in the GUI).
pub fn fab_label(running: bool) -> &'static str {
    if running { "Pause" } else { "Resume" }
}

/// What the view should do after an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// Membership or order changed: rebuild from `DownloadManager::snapshot()`.
    Rebuild,
    /// Progress for one chapter: refresh that row from the snapshot.
    Progress(i64),
    /// A status changed: the row was updated in place.
    Status(i64),
    /// The queue started (`true`) or stopped (`false`): the footer action label follows this.
    Running(bool),
}

/// The screen state: groups plus the progress gate.
pub struct QueueView {
    groups: Vec<GroupView>,
    throttle: ProgressThrottle,
}

impl Default for QueueView {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueView {
    pub fn new() -> Self {
        Self { groups: Vec::new(), throttle: ProgressThrottle::new(PROGRESS_INTERVAL) }
    }

    pub fn groups(&self) -> &[GroupView] {
        &self.groups
    }

    /// Empty state: no rows at all.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn row(&self, chapter_id: i64) -> Option<&RowView> {
        self.groups.iter().flat_map(|g| g.rows.iter()).find(|r| r.chapter_id == chapter_id)
    }

    pub fn rebuild(&mut self, snapshot: &[Arc<QueuedDownload>]) {
        self.groups = build_groups(snapshot);
    }

    /// Refreshes one row's counts and progress from the snapshot. Returns `false` when the row is not in the view.
    pub fn refresh_row(&mut self, item: &QueuedDownload) -> bool {
        let fresh = RowView::from_item(item);
        match self.groups.iter_mut().flat_map(|g| g.rows.iter_mut()).find(|r| r.chapter_id == fresh.chapter_id) {
            Some(row) => {
                *row = fresh;
                true
            }
            None => false,
        }
    }

    /// Applies one engine event at `now` and says what to redraw. Progress is gated to one update per 50 ms per chapter,
    /// and a finished row ignores later progress.
    pub fn on_event(&mut self, event: &DownloadEvent, now: Instant) -> Option<Msg> {
        match event {
            DownloadEvent::QueueChanged | DownloadEvent::Finished { .. } => Some(Msg::Rebuild),
            DownloadEvent::Progress { chapter_id, .. } => {
                let row = self.row(*chapter_id)?;
                if row.is_finished() || !self.throttle.allow(*chapter_id, now) {
                    return None;
                }
                Some(Msg::Progress(*chapter_id))
            }
            DownloadEvent::Status { chapter_id, state } => {
                if matches!(state, DownloadState::Downloaded | DownloadState::Error) {
                    self.throttle.forget(*chapter_id);
                }
                let row = self.groups.iter_mut().flat_map(|g| g.rows.iter_mut()).find(|r| r.chapter_id == *chapter_id);
                match row {
                    Some(row) => {
                        row.status = *state;
                        Some(Msg::Status(*chapter_id))
                    }
                    None => Some(Msg::Rebuild),
                }
            }
            DownloadEvent::Started => Some(Msg::Running(true)),
            DownloadEvent::Paused | DownloadEvent::Complete => Some(Msg::Running(false)),
            DownloadEvent::Dismiss | DownloadEvent::Warning { .. } | DownloadEvent::Error { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_state_text_and_fab_label_follow_the_manager_state() {
        assert_eq!(EMPTY_TEXT, "No downloads");
        assert_eq!(fab_label(true), "Pause");
        assert_eq!(fab_label(false), "Resume");
    }
    use crate::model::{Page, PageState};
    use crate::testing::{download, fake_source};
    use std::time::Duration;

    fn item(source_id: i64, chapter_id: i64, manga_id: i64, status: DownloadState) -> Arc<QueuedDownload> {
        let source = fake_source(source_id);
        Arc::new(download(&source, chapter_id, manga_id, status))
    }

    fn with_pages(item: &Arc<QueuedDownload>, states: &[(PageState, i32)]) -> Arc<QueuedDownload> {
        let mut copy = (**item).clone();
        copy.pages = Some(
            states
                .iter()
                .enumerate()
                .map(|(i, (state, progress))| {
                    let mut page = Page::new(i as u32, "u", None);
                    page.state = *state;
                    page.progress = *progress;
                    page
                })
                .collect(),
        );
        Arc::new(copy)
    }

    #[test]
    fn groups_follow_first_occurrence_and_keep_queue_order_inside() {
        let snapshot = vec![
            item(2, 20, 1, DownloadState::Queue),
            item(1, 10, 1, DownloadState::Queue),
            item(2, 21, 1, DownloadState::Queue),
            item(1, 11, 2, DownloadState::Downloading),
        ];
        let groups = build_groups(&snapshot);
        let order: Vec<i64> = groups.iter().map(|g| g.source_id).collect();
        assert_eq!(order, vec![2, 1]);
        let rows: Vec<i64> = groups[0].rows.iter().map(|r| r.chapter_id).collect();
        assert_eq!(rows, vec![20, 21]);
        assert_eq!(groups[0].header(), "Source 2 (EN) (2)");
        assert_eq!(groups[1].header(), "Source 1 (EN) (2)");
    }

    #[test]
    fn row_identity_uses_the_full_64_bit_chapter_id() {
        let high = (1i64 << 32) + 1;
        let snapshot = vec![item(1, 1, 1, DownloadState::Queue), item(1, high, 1, DownloadState::Queue)];
        let groups = build_groups(&snapshot);
        assert_eq!(groups[0].rows.len(), 2, "ids equal in the low 32 bits are still different rows");
        let mut view = QueueView::new();
        view.rebuild(&snapshot);
        assert_eq!(view.row(high).map(|r| r.chapter_id), Some(high));
        assert_eq!(view.row(1).map(|r| r.chapter_id), Some(1));
    }

    #[test]
    fn bar_is_the_page_progress_sum_over_pages_times_100() {
        let base = item(1, 5, 1, DownloadState::Downloading);
        let row = RowView::from_item(&with_pages(
            &base,
            &[(PageState::Ready, 100), (PageState::DownloadImage, 40), (PageState::Queue, 0)],
        ));
        assert_eq!(row.bar(), (140, 300));
        assert_eq!(row.progress_text(), "1/3");
        let unknown = RowView::from_item(&base);
        assert_eq!(unknown.bar(), (0, 1), "before the page list is known");
        assert_eq!(unknown.progress_text(), "Downloading…");
        let queued = RowView::from_item(&item(1, 6, 1, DownloadState::Queue));
        assert_eq!(queued.progress_text(), "");
    }

    #[test]
    fn status_changes_update_the_row_without_a_rebuild() {
        let snapshot = vec![item(1, 1, 1, DownloadState::Queue), item(1, 2, 1, DownloadState::Queue)];
        let mut view = QueueView::new();
        view.rebuild(&snapshot);
        let now = Instant::now();
        let msg = view.on_event(&DownloadEvent::Status { chapter_id: 2, state: DownloadState::Downloading }, now);
        assert_eq!(msg, Some(Msg::Status(2)));
        assert_eq!(view.row(2).unwrap().status, DownloadState::Downloading);
        assert_eq!(view.groups()[0].rows.len(), 2);
        assert_eq!(view.row(1).unwrap().status, DownloadState::Queue);
    }

    #[test]
    fn progress_is_coalesced_to_one_update_per_fifty_milliseconds() {
        let snapshot = vec![item(1, 1, 1, DownloadState::Downloading)];
        let mut view = QueueView::new();
        view.rebuild(&snapshot);
        let start = Instant::now();
        let progress = DownloadEvent::Progress {
            chapter_id: 1,
            manga_title: "Manga 1".into(),
            chapter_name: "Chapter 1".into(),
            downloaded: 0,
            total: 3,
        };
        let shown: Vec<bool> = [0u64, 10, 49, 50]
            .into_iter()
            .map(|ms| view.on_event(&progress, start + Duration::from_millis(ms)).is_some())
            .collect();
        assert_eq!(shown, vec![true, false, false, true], "TST-20");
    }

    #[test]
    fn a_finished_row_ignores_progress_and_its_gate_is_released() {
        let snapshot = vec![item(1, 1, 1, DownloadState::Downloading)];
        let mut view = QueueView::new();
        view.rebuild(&snapshot);
        let start = Instant::now();
        let progress = DownloadEvent::Progress {
            chapter_id: 1,
            manga_title: String::new(),
            chapter_name: String::new(),
            downloaded: 0,
            total: 1,
        };
        assert!(view.on_event(&progress, start).is_some());
        assert_eq!(
            view.on_event(&DownloadEvent::Status { chapter_id: 1, state: DownloadState::Error }, start),
            Some(Msg::Status(1))
        );
        assert!(view.on_event(&progress, start + Duration::from_secs(1)).is_none(), "ERROR ends the progress job");
    }

    #[test]
    fn queue_events_map_to_view_messages() {
        let mut view = QueueView::new();
        let now = Instant::now();
        assert_eq!(view.on_event(&DownloadEvent::QueueChanged, now), Some(Msg::Rebuild));
        assert_eq!(view.on_event(&DownloadEvent::Finished { chapter_id: 3 }, now), Some(Msg::Rebuild));
        assert_eq!(view.on_event(&DownloadEvent::Started, now), Some(Msg::Running(true)));
        assert_eq!(view.on_event(&DownloadEvent::Paused, now), Some(Msg::Running(false)));
        assert_eq!(view.on_event(&DownloadEvent::Complete, now), Some(Msg::Running(false)));
        assert_eq!(view.on_event(&DownloadEvent::Dismiss, now), None);
        assert!(view.is_empty());
    }

    #[test]
    fn refresh_row_updates_counts_from_the_snapshot() {
        let base = item(1, 1, 1, DownloadState::Downloading);
        let mut view = QueueView::new();
        view.rebuild(std::slice::from_ref(&base));
        assert_eq!(view.row(1).unwrap().total, 0);
        let updated = with_pages(&base, &[(PageState::Ready, 100), (PageState::Ready, 100)]);
        assert!(view.refresh_row(&updated));
        assert_eq!(view.row(1).unwrap().bar(), (200, 200));
        assert!(!view.refresh_row(&item(1, 99, 1, DownloadState::Queue)));
    }
}
