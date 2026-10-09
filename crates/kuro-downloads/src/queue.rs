//! Pure in-memory queue state machine: no I/O, no clocks, no async. Every transition of `MangaDownload.State` in
//! `p1-behavior.md` section 1.1 that touches the queue goes through here.

use std::collections::HashSet;
use std::sync::Arc;

use crate::model::{DownloadState, QueuedDownload};
use crate::scheduler::pick_active;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QueueError {
    #[error("chapter {0} is not in the queue")]
    UnknownId(i64),
    #[error("chapter {id} cannot go from {from:?} to {to:?}")]
    Transition { id: i64, from: DownloadState, to: DownloadState },
}

/// The queue in order, plus the user pause flag. Invariants kept by every method:
/// a chapter id is in the queue at most once, and no item is `Downloaded` (finished items are removed).
#[derive(Default)]
pub struct QueueState {
    items: Vec<Arc<QueuedDownload>>,
    paused: bool,
}

impl QueueState {
    pub fn new() -> Self {
        Self::default()
    }

    /// A queue restored at startup: every item is QUEUE, repeats are skipped (E-Q02).
    pub fn restored(downloads: Vec<QueuedDownload>) -> Self {
        let mut state = Self::new();
        state.add_all(downloads);
        state
    }

    pub fn items(&self) -> &[Arc<QueuedDownload>] {
        &self.items
    }

    pub fn snapshot(&self) -> Vec<Arc<QueuedDownload>> {
        self.items.clone()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, chapter_id: i64) -> Option<&Arc<QueuedDownload>> {
        self.items.iter().find(|d| d.chapter.id == chapter_id)
    }

    pub fn contains(&self, chapter_id: i64) -> bool {
        self.get(chapter_id).is_some()
    }

    /// `true` after a user pause, until the next explicit start.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Appends every download as QUEUE. Chapters already in the queue, and repeats within `downloads`, are skipped.
    /// Returns the ids that were added.
    pub fn add_all(&mut self, downloads: Vec<QueuedDownload>) -> Vec<i64> {
        let mut added = Vec::new();
        for mut download in downloads {
            if self.contains(download.chapter.id) {
                continue;
            }
            download.status = DownloadState::Queue;
            added.push(download.chapter.id);
            self.items.push(Arc::new(download));
        }
        added
    }

    /// Removes the chapters. Returned copies carry the status they had when removed, but DOWNLOADING and QUEUE
    /// become NOT_DOWNLOADED (`Dl.removeFromQueueIf`).
    pub fn remove_chapters(&mut self, ids: &[i64]) -> Vec<QueuedDownload> {
        self.remove_where(|d| ids.contains(&d.chapter.id))
    }

    /// Removes every chapter of one manga, with the same status rule as [`Self::remove_chapters`].
    pub fn remove_manga(&mut self, manga_id: i64) -> Vec<QueuedDownload> {
        self.remove_where(|d| d.manga.id == manga_id)
    }

    /// Empties the queue (`Dl.internalClearQueue`).
    pub fn clear(&mut self) -> Vec<QueuedDownload> {
        self.remove_where(|_| true)
    }

    fn remove_where(&mut self, pred: impl Fn(&QueuedDownload) -> bool) -> Vec<QueuedDownload> {
        let (removed, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.items).into_iter().partition(|d| pred(d));
        self.items = kept;
        removed
            .into_iter()
            .map(|d| {
                let mut owned = Arc::unwrap_or_clone(d);
                if matches!(owned.status, DownloadState::Downloading | DownloadState::Queue) {
                    owned.status = DownloadState::NotDownloaded;
                }
                owned
            })
            .collect()
    }

    /// DOWNLOADING becomes QUEUE. The paused flag is untouched (used for a removal that restarts afterwards).
    pub fn requeue_running(&mut self) -> Vec<i64> {
        self.set_all_with_status(DownloadState::Downloading, DownloadState::Queue)
    }

    /// `Dl.pause`: DOWNLOADING becomes QUEUE and the queue is marked paused.
    pub fn pause_running(&mut self) -> Vec<i64> {
        self.paused = true;
        self.requeue_running()
    }

    /// `Dl.stop`: DOWNLOADING becomes ERROR.
    pub fn stop_running(&mut self) -> Vec<i64> {
        self.set_all_with_status(DownloadState::Downloading, DownloadState::Error)
    }

    /// `Dl.start`: every item that is not QUEUE becomes QUEUE, so ERROR items are retried.
    pub fn reset_for_start(&mut self) -> Vec<i64> {
        let mut changed = Vec::new();
        for d in self.items.iter_mut().filter(|d| d.status != DownloadState::Queue) {
            Arc::make_mut(d).status = DownloadState::Queue;
            changed.push(d.chapter.id);
        }
        changed
    }

    /// Automatic resume after the network returns: only the ERROR items in `ids` go back to QUEUE.
    pub fn requeue_errors(&mut self, ids: &HashSet<i64>) -> Vec<i64> {
        let mut changed = Vec::new();
        for d in self.items.iter_mut().filter(|d| d.status == DownloadState::Error && ids.contains(&d.chapter.id)) {
            Arc::make_mut(d).status = DownloadState::Queue;
            changed.push(d.chapter.id);
        }
        changed
    }

    /// Puts the queue in the given order. Unknown and repeated ids are ignored, queued items that are not listed follow in
    /// their current order, and an empty list does nothing (E-15). ERROR items keep their status (Proposal, unlike
    /// Kotlin `updateQueue`).
    pub fn reorder(&mut self, ordered_ids: &[i64]) {
        if ordered_ids.is_empty() {
            return;
        }
        let mut rest = std::mem::take(&mut self.items);
        let mut out = Vec::with_capacity(rest.len());
        for id in ordered_ids {
            if let Some(pos) = rest.iter().position(|d| d.chapter.id == *id) {
                out.push(rest.remove(pos));
            }
        }
        out.append(&mut rest);
        self.items = out;
    }

    /// Checked status change. Starting a download needs QUEUE. ERROR and QUEUE can be set from any state. Removal
    /// and completion use [`Self::remove_chapters`] and [`Self::mark_downloaded`], never this method.
    pub fn set_status(&mut self, chapter_id: i64, to: DownloadState) -> Result<bool, QueueError> {
        let item = self.items.iter_mut().find(|d| d.chapter.id == chapter_id).ok_or(QueueError::UnknownId(chapter_id))?;
        let from = item.status;
        if from == to {
            return Ok(false);
        }
        let allowed = match to {
            DownloadState::Downloading => from == DownloadState::Queue,
            DownloadState::Queue | DownloadState::Error => true,
            DownloadState::NotDownloaded | DownloadState::Downloaded => false,
        };
        if !allowed {
            return Err(QueueError::Transition { id: chapter_id, from, to });
        }
        Arc::make_mut(item).status = to;
        Ok(true)
    }

    /// Completion: removes the chapter and returns it with status DOWNLOADED.
    pub fn mark_downloaded(&mut self, chapter_id: i64) -> Option<QueuedDownload> {
        let pos = self.items.iter().position(|d| d.chapter.id == chapter_id)?;
        let mut done = Arc::unwrap_or_clone(self.items.remove(pos));
        done.status = DownloadState::Downloaded;
        Some(done)
    }

    /// Mutates one item in place. Copies it first only when a snapshot still shares it.
    pub fn update(&mut self, chapter_id: i64, f: impl FnOnce(&mut QueuedDownload)) -> bool {
        match self.items.iter_mut().find(|d| d.chapter.id == chapter_id) {
            Some(item) => {
                f(Arc::make_mut(item));
                true
            }
            None => false,
        }
    }

    /// The chapters the scheduler runs now (see [`pick_active`]).
    pub fn active_ids(&self, max_sources: usize) -> Vec<i64> {
        pick_active(self.items.iter().map(|d| (d.source.id(), d.chapter.id, d.status)), max_sources)
    }

    /// `true` when no item is NOT_DOWNLOADED, QUEUE or DOWNLOADING (`Dl.areAllDownloadsFinished`).
    pub fn all_finished(&self) -> bool {
        self.items.iter().all(|d| !d.status.is_active())
    }

    fn set_all_with_status(&mut self, from: DownloadState, to: DownloadState) -> Vec<i64> {
        let mut changed = Vec::new();
        for d in self.items.iter_mut().filter(|d| d.status == from) {
            Arc::make_mut(d).status = to;
            changed.push(d.chapter.id);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::MAX_ACTIVE_SOURCES;
    use crate::testing::{download, fake_source};
    use DownloadState::*;

    fn queue_with(statuses: &[(i64, DownloadState)]) -> QueueState {
        let mut q = QueueState::new();
        let src = fake_source(1);
        q.add_all(statuses.iter().map(|(id, _)| download(&src, *id, 1, Queue)).collect());
        for (id, status) in statuses {
            q.update(*id, |d| d.status = *status);
        }
        q
    }

    fn statuses(q: &QueueState) -> Vec<(i64, DownloadState)> {
        q.items().iter().map(|d| (d.chapter.id, d.status)).collect()
    }

    #[test]
    fn add_all_sets_queue_status_and_appends_in_order() {
        let mut q = QueueState::new();
        let src = fake_source(1);
        let added = q.add_all(vec![download(&src, 3, 1, Downloaded), download(&src, 1, 1, Error), download(&src, 2, 1, Queue)]);
        assert_eq!(added, vec![3, 1, 2]);
        assert_eq!(statuses(&q), vec![(3, Queue), (1, Queue), (2, Queue)]);
    }

    #[test]
    fn add_all_skips_duplicates_in_queue_and_in_the_input() {
        let mut q = queue_with(&[(1, Queue)]);
        let src = fake_source(1);
        let added = q.add_all(vec![download(&src, 1, 1, Queue), download(&src, 2, 1, Queue), download(&src, 2, 1, Queue)]);
        assert_eq!(added, vec![2]);
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn remove_resets_downloading_and_queue_but_not_error() {
        let mut q = queue_with(&[(1, Downloading), (2, Queue), (3, Error)]);
        let removed = q.remove_chapters(&[1, 2, 3]);
        let got: Vec<_> = removed.iter().map(|d| (d.chapter.id, d.status)).collect();
        assert_eq!(got, vec![(1, NotDownloaded), (2, NotDownloaded), (3, Error)]);
        assert!(q.is_empty());
    }

    #[test]
    fn pause_running_sets_queue_and_the_paused_flag() {
        let mut q = queue_with(&[(1, Downloading), (2, Downloading), (3, Queue)]);
        assert_eq!(q.pause_running(), vec![1, 2]);
        assert_eq!(statuses(&q), vec![(1, Queue), (2, Queue), (3, Queue)]);
        assert!(q.is_paused());
    }

    #[test]
    fn stop_running_sets_error() {
        let mut q = queue_with(&[(1, Downloading), (2, Queue)]);
        assert_eq!(q.stop_running(), vec![1]);
        assert_eq!(statuses(&q), vec![(1, Error), (2, Queue)]);
    }

    #[test]
    fn reset_for_start_retries_error_and_not_downloaded() {
        let mut q = queue_with(&[(1, Error), (2, NotDownloaded), (3, Queue), (4, Downloading)]);
        let changed = q.reset_for_start();
        assert_eq!(changed, vec![1, 2, 4]);
        assert!(q.items().iter().all(|d| d.status == Queue));
    }

    #[test]
    fn requeue_errors_only_touches_the_given_ids() {
        let mut q = queue_with(&[(1, Error), (2, Error), (3, Queue)]);
        let only: HashSet<i64> = [2].into_iter().collect();
        assert_eq!(q.requeue_errors(&only), vec![2]);
        assert_eq!(statuses(&q), vec![(1, Error), (2, Queue), (3, Queue)]);
    }

    #[test]
    fn reorder_ignores_unknown_and_repeats_and_appends_unlisted_in_current_order() {
        let mut q = queue_with(&[(1, Queue), (2, Queue), (3, Queue)]);
        q.reorder(&[3, 9, 3]);
        assert_eq!(statuses(&q), vec![(3, Queue), (1, Queue), (2, Queue)]);
        q.reorder(&[]);
        assert_eq!(statuses(&q), vec![(3, Queue), (1, Queue), (2, Queue)], "an empty list is a no-op");
    }

    #[test]
    fn reorder_keeps_error_status() {
        let mut q = queue_with(&[(1, Error), (2, Queue)]);
        q.reorder(&[2, 1]);
        assert_eq!(statuses(&q), vec![(2, Queue), (1, Error)]);
    }

    #[test]
    fn set_status_enforces_the_transition_table() {
        let mut q = queue_with(&[(1, Queue)]);
        assert_eq!(q.set_status(1, Downloading), Ok(true));
        assert_eq!(q.set_status(1, Downloading), Ok(false));
        assert_eq!(q.set_status(1, Error), Ok(true));
        assert!(matches!(q.set_status(1, Downloaded), Err(QueueError::Transition { .. })));
        assert!(matches!(q.set_status(1, NotDownloaded), Err(QueueError::Transition { .. })));
        assert_eq!(q.set_status(1, Queue), Ok(true));
        assert!(q.set_status(1, Downloading).is_ok());
        assert_eq!(q.set_status(7, Queue), Err(QueueError::UnknownId(7)));
    }

    #[test]
    fn mark_downloaded_removes_the_item_so_no_finished_item_stays() {
        let mut q = queue_with(&[(1, Downloading), (2, Queue)]);
        let done = q.mark_downloaded(1).unwrap();
        assert_eq!(done.status, Downloaded);
        assert_eq!(statuses(&q), vec![(2, Queue)]);
        assert!(q.mark_downloaded(1).is_none());
    }

    #[test]
    fn remove_manga_only_removes_rows_with_that_manga_id() {
        let mut q = QueueState::new();
        let src = fake_source(1);
        q.add_all(vec![download(&src, 1, 7, Queue), download(&src, 2, 8, Queue), download(&src, 3, 7, Error)]);
        let removed = q.remove_manga(7);
        assert_eq!(removed.len(), 2);
        assert_eq!(statuses(&q), vec![(2, Queue)]);
    }

    #[test]
    fn clear_empties_and_resets_active_statuses() {
        let mut q = queue_with(&[(1, Downloading), (2, Error)]);
        let removed = q.clear();
        assert_eq!(removed.len(), 2);
        assert_eq!(removed[0].status, NotDownloaded);
        assert!(q.is_empty());
        assert!(q.all_finished());
    }

    #[test]
    fn active_ids_and_all_finished_follow_the_scheduler_rules() {
        let q = queue_with(&[(1, Error), (2, Queue)]);
        assert_eq!(q.active_ids(MAX_ACTIVE_SOURCES), vec![2]);
        assert!(!q.all_finished());
        let finished = queue_with(&[(1, Error)]);
        assert!(finished.active_ids(MAX_ACTIVE_SOURCES).is_empty());
        assert!(finished.all_finished());
    }

    #[test]
    fn snapshots_keep_their_copy_when_the_queue_changes() {
        let mut q = queue_with(&[(1, Queue)]);
        let before = q.snapshot();
        q.set_status(1, Downloading).unwrap();
        assert_eq!(before[0].status, Queue);
        assert_eq!(q.get(1).unwrap().status, Downloading);
    }
}
