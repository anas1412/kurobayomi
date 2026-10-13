//! The pure lane state machine: no I/O, no clocks, no async. One lane per kind (docs/design/unified-downloads.md 2.4).
//! Ported from `queue.rs`; the rules the anime queue had inline now live here for both kinds.

use std::collections::HashSet;
use std::sync::Arc;

use kuro_platform::kind::Kind;

use crate::unit::QueuedUnit;
use crate::model::DownloadState;
use crate::scheduler::pick_active;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QueueError {
    #[error("unit {0} is not in the queue")]
    UnknownId(i64),
    #[error("unit {id} cannot go from {from:?} to {to:?}")]
    Transition { id: i64, from: DownloadState, to: DownloadState },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Stopped,
    Running,
    Paused,
}

/// How a run begins. `Explicit` (the user pressed start) retries every ERROR unit. `Resume` retries only the given ids,
/// the units the network stopped, so an automatic resume never retries a unit that failed for another reason.
pub enum StartMode {
    Explicit,
    Resume(HashSet<i64>),
}

/// The units of one kind in queue order, plus the run state. Invariants: a unit id is queued at most once, and no unit
/// is `Downloaded` (finished units are removed).
pub struct Lane {
    kind: Kind,
    items: Vec<Arc<QueuedUnit>>,
    run: RunState,
}

impl Lane {
    pub fn new(kind: Kind) -> Self {
        Self { kind, items: Vec::new(), run: RunState::Stopped }
    }

    /// A lane restored at startup: every unit is QUEUE, repeats are skipped.
    pub fn restored(kind: Kind, units: Vec<QueuedUnit>) -> Self {
        let mut lane = Self::new(kind);
        lane.add_all(units);
        lane
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn items(&self) -> &[Arc<QueuedUnit>] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, unit_id: i64) -> Option<&Arc<QueuedUnit>> {
        self.items.iter().find(|d| d.unit.id == unit_id)
    }

    pub fn contains(&self, unit_id: i64) -> bool {
        self.get(unit_id).is_some()
    }

    pub fn run_state(&self) -> RunState {
        self.run
    }

    /// The actor ends a run that has nothing left to do with `Stopped`.
    pub fn set_run_state(&mut self, run: RunState) {
        self.run = run;
    }

    /// Appends every unit as QUEUE. Units already queued, and repeats within `units`, are skipped. Returns the ids added.
    pub fn add_all(&mut self, units: Vec<QueuedUnit>) -> Vec<i64> {
        let mut added = Vec::new();
        for mut unit in units {
            if self.contains(unit.unit.id) {
                continue;
            }
            unit.state = DownloadState::Queue;
            added.push(unit.unit.id);
            self.items.push(Arc::new(unit));
        }
        added
    }

    /// Adds the units at the front, in the order given. Units already queued are skipped.
    pub fn add_front(&mut self, units: Vec<QueuedUnit>) -> Vec<i64> {
        let added = self.add_all(units);
        self.items.rotate_right(added.len());
        added
    }

    /// Removes the units and returns them as they were (state included). The caller decides what a removed unit becomes.
    pub fn remove_units(&mut self, ids: &[i64]) -> Vec<Arc<QueuedUnit>> {
        self.remove_where(|d| ids.contains(&d.unit.id))
    }

    /// Removes every unit of one entry.
    pub fn remove_entry(&mut self, entry_id: i64) -> Vec<Arc<QueuedUnit>> {
        self.remove_where(|d| d.entry.id == entry_id)
    }

    pub fn clear(&mut self) -> Vec<Arc<QueuedUnit>> {
        self.remove_where(|_| true)
    }

    fn remove_where(&mut self, pred: impl Fn(&QueuedUnit) -> bool) -> Vec<Arc<QueuedUnit>> {
        let (removed, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.items).into_iter().partition(|d| pred(d));
        self.items = kept;
        removed
    }

    /// Puts the lane in the given order. Unknown and repeated ids are ignored, units that are not listed follow in their
    /// current order, and an empty list does nothing. Returns `false` when the order does not change, so a drop at the
    /// original place never restarts the run. ERROR units keep their state.
    pub fn reorder(&mut self, ordered: &[i64]) -> bool {
        if ordered.is_empty() {
            return false;
        }
        let before: Vec<i64> = self.items.iter().map(|d| d.unit.id).collect();
        let mut rest = std::mem::take(&mut self.items);
        let mut out = Vec::with_capacity(rest.len());
        for id in ordered {
            if let Some(pos) = rest.iter().position(|d| d.unit.id == *id) {
                out.push(rest.remove(pos));
            }
        }
        out.append(&mut rest);
        self.items = out;
        let after: Vec<i64> = self.items.iter().map(|d| d.unit.id).collect();
        before != after
    }

    /// Checked state change. Starting a unit needs QUEUE. ERROR and QUEUE can be set from any state. Removal and
    /// completion use [`Self::remove_units`] and [`Self::finish`], never this method. `Ok(false)`: nothing changed.
    pub fn set_state(&mut self, unit_id: i64, to: DownloadState) -> Result<bool, QueueError> {
        let item = self.items.iter_mut().find(|d| d.unit.id == unit_id).ok_or(QueueError::UnknownId(unit_id))?;
        let from = item.state;
        if from == to {
            return Ok(false);
        }
        let allowed = match to {
            DownloadState::Downloading => from == DownloadState::Queue,
            DownloadState::Queue | DownloadState::Error => true,
            DownloadState::NotDownloaded | DownloadState::Downloaded => false,
        };
        if !allowed {
            return Err(QueueError::Transition { id: unit_id, from, to });
        }
        Arc::make_mut(item).state = to;
        Ok(true)
    }

    /// Sets the waiting flag. Returns whether it changed (`false` too for an unknown id).
    pub fn set_waiting(&mut self, unit_id: i64, waiting: bool) -> bool {
        match self.items.iter_mut().find(|d| d.unit.id == unit_id) {
            Some(item) if item.waiting != waiting => {
                Arc::make_mut(item).waiting = waiting;
                true
            }
            _ => false,
        }
    }

    /// Starts the run. Returns the ids put back to QUEUE (the ERROR units the mode retries).
    pub fn begin(&mut self, mode: StartMode) -> Vec<i64> {
        self.run = RunState::Running;
        let mut changed = Vec::new();
        for d in self.items.iter_mut().filter(|d| d.state == DownloadState::Error) {
            let retry = match &mode {
                StartMode::Explicit => true,
                StartMode::Resume(ids) => ids.contains(&d.unit.id),
            };
            if retry {
                Arc::make_mut(d).state = DownloadState::Queue;
                changed.push(d.unit.id);
            }
        }
        changed
    }

    /// The user paused: DOWNLOADING becomes QUEUE and the run is `Paused`.
    pub fn pause_running(&mut self) -> Vec<i64> {
        self.run = RunState::Paused;
        self.requeue_running()
    }

    /// A failure stop (network, bad location): DOWNLOADING becomes ERROR and the run is `Stopped`.
    pub fn stop_running(&mut self) -> Vec<i64> {
        self.run = RunState::Stopped;
        self.set_all_with_state(DownloadState::Downloading, DownloadState::Error)
    }

    /// For a mutation (remove, reorder, delete): DOWNLOADING becomes QUEUE, the run is unchanged.
    pub fn requeue_running(&mut self) -> Vec<i64> {
        self.set_all_with_state(DownloadState::Downloading, DownloadState::Queue)
    }

    /// The units the scheduler runs now: [`pick_active`] over the units that are not waiting, so a waiting unit is
    /// skipped like an ERROR one and the next unit of its source runs.
    pub fn pick(&self, max_sources: usize) -> Vec<i64> {
        pick_active(
            self.items.iter().filter(|d| !d.waiting).map(|d| (d.source.id, d.unit.id, d.state)),
            max_sources,
        )
    }

    /// Completion: removes the unit and returns it.
    pub fn finish(&mut self, unit_id: i64) -> Option<Arc<QueuedUnit>> {
        let pos = self.items.iter().position(|d| d.unit.id == unit_id)?;
        Some(self.items.remove(pos))
    }

    /// `true` when no unit that can run is QUEUE or DOWNLOADING: the run has nothing left to do.
    pub fn all_idle(&self) -> bool {
        self.items.iter().all(|d| d.waiting || !d.state.is_active())
    }

    fn set_all_with_state(&mut self, from: DownloadState, to: DownloadState) -> Vec<i64> {
        let mut changed = Vec::new();
        for d in self.items.iter_mut().filter(|d| d.state == from) {
            Arc::make_mut(d).state = to;
            changed.push(d.unit.id);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::fixtures::queued;
    use DownloadState::*;

    /// A lane of `kind` with one unit per `(id, state)`, all of source 1 and entry 1.
    fn lane_with(kind: Kind, states: &[(i64, DownloadState)]) -> Lane {
        let mut lane = Lane::new(kind);
        lane.add_all(states.iter().map(|(id, _)| queued(kind, 1, *id, 1)).collect());
        for (id, state) in states {
            Arc::make_mut(lane.items.iter_mut().find(|d| d.unit.id == *id).unwrap()).state = *state;
        }
        lane
    }

    fn states(lane: &Lane) -> Vec<(i64, DownloadState)> {
        lane.items().iter().map(|d| (d.unit.id, d.state)).collect()
    }

    fn ids(items: &[Arc<QueuedUnit>]) -> Vec<i64> {
        items.iter().map(|d| d.unit.id).collect()
    }

    #[test]
    fn add_front_puts_the_new_units_first_in_the_given_order() {
        for kind in Kind::ALL {
            let mut lane = lane_with(kind, &[(1, Queue), (2, Queue)]);
            let added = lane.add_front(vec![queued(kind, 1, 7, 1), queued(kind, 1, 8, 1), queued(kind, 1, 1, 1)]);
            assert_eq!(added, vec![7, 8], "a unit already queued is skipped");
            assert_eq!(states(&lane), vec![(7, Queue), (8, Queue), (1, Queue), (2, Queue)]);
        }
    }

    #[test]
    fn add_all_sets_queue_state_and_appends_in_order() {
        let mut lane = Lane::new(Kind::Anime);
        let mut done = queued(Kind::Anime, 1, 3, 1);
        done.state = Downloaded;
        let mut failed = queued(Kind::Anime, 1, 1, 1);
        failed.state = Error;
        let added = lane.add_all(vec![done, failed, queued(Kind::Anime, 1, 2, 1)]);
        assert_eq!(added, vec![3, 1, 2]);
        assert_eq!(states(&lane), vec![(3, Queue), (1, Queue), (2, Queue)]);
    }

    #[test]
    fn add_all_skips_duplicates_in_the_lane_and_in_the_input() {
        let mut lane = lane_with(Kind::Manga, &[(1, Queue)]);
        let added = lane.add_all(vec![queued(Kind::Manga, 1, 1, 1), queued(Kind::Manga, 1, 2, 1), queued(Kind::Manga, 1, 2, 1)]);
        assert_eq!(added, vec![2]);
        assert_eq!(lane.len(), 2);
    }

    #[test]
    fn restored_skips_repeats() {
        let lane = Lane::restored(Kind::Manga, vec![queued(Kind::Manga, 1, 1, 1), queued(Kind::Manga, 1, 1, 1)]);
        assert_eq!(lane.len(), 1);
        assert_eq!(lane.run_state(), RunState::Stopped);
    }

    #[test]
    fn remove_units_returns_them_as_they_were() {
        let mut lane = lane_with(Kind::Anime, &[(1, Downloading), (2, Queue), (3, Error)]);
        let removed = lane.remove_units(&[1, 3, 9]);
        assert_eq!(removed.iter().map(|d| (d.unit.id, d.state)).collect::<Vec<_>>(), vec![(1, Downloading), (3, Error)]);
        assert_eq!(states(&lane), vec![(2, Queue)]);
    }

    #[test]
    fn pause_running_requeues_and_marks_the_lane_paused() {
        for kind in Kind::ALL {
            let mut lane = lane_with(kind, &[(1, Downloading), (2, Downloading), (3, Queue)]);
            lane.begin(StartMode::Explicit);
            assert_eq!(lane.pause_running(), vec![1, 2]);
            assert_eq!(states(&lane), vec![(1, Queue), (2, Queue), (3, Queue)]);
            assert_eq!(lane.run_state(), RunState::Paused);
        }
    }

    #[test]
    fn stop_running_sets_error_and_stops_the_lane() {
        let mut lane = lane_with(Kind::Manga, &[(1, Downloading), (2, Queue)]);
        lane.begin(StartMode::Explicit);
        assert_eq!(lane.stop_running(), vec![1]);
        assert_eq!(states(&lane), vec![(1, Error), (2, Queue)]);
        assert_eq!(lane.run_state(), RunState::Stopped);
    }

    #[test]
    fn resume_after_a_pause_runs_again_and_retries_nothing() {
        for kind in Kind::ALL {
            let mut lane = lane_with(kind, &[(1, Error), (2, Downloading), (3, Queue)]);
            lane.pause_running();
            let retried = lane.begin(StartMode::Resume(HashSet::new()));
            assert!(retried.is_empty());
            assert_eq!(states(&lane), vec![(1, Error), (2, Queue), (3, Queue)]);
            assert_eq!(lane.run_state(), RunState::Running);
        }
    }

    #[test]
    fn explicit_start_retries_every_error_unit() {
        let mut lane = lane_with(Kind::Manga, &[(1, Error), (2, Error), (3, Queue)]);
        assert_eq!(lane.begin(StartMode::Explicit), vec![1, 2]);
        assert!(lane.items().iter().all(|d| d.state == Queue));
    }

    #[test]
    fn resume_retries_only_the_given_ids() {
        let mut lane = lane_with(Kind::Anime, &[(1, Error), (2, Error), (3, Queue)]);
        assert_eq!(lane.begin(StartMode::Resume([2].into_iter().collect())), vec![2]);
        assert_eq!(states(&lane), vec![(1, Error), (2, Queue), (3, Queue)]);
    }

    #[test]
    fn a_mutation_does_not_retry_other_failed_units() {
        // Cancel one unit while another failed: requeue_running, remove, begin(Resume(empty)) leaves the failure alone.
        for kind in Kind::ALL {
            let mut lane = lane_with(kind, &[(1, Error), (2, Downloading), (3, Queue)]);
            lane.begin(StartMode::Explicit);
            lane.set_state(1, Error).unwrap();
            lane.requeue_running();
            lane.remove_units(&[2]);
            lane.begin(StartMode::Resume(HashSet::new()));
            assert_eq!(states(&lane), vec![(1, Error), (3, Queue)]);
        }
    }

    #[test]
    fn reorder_ignores_unknown_and_repeats_and_appends_unlisted_in_current_order() {
        let mut lane = lane_with(Kind::Anime, &[(1, Queue), (2, Queue), (3, Queue)]);
        assert!(lane.reorder(&[3, 9, 3]));
        assert_eq!(ids(lane.items()), vec![3, 1, 2]);
        assert!(!lane.reorder(&[]), "an empty list is a no-op");
        assert_eq!(ids(lane.items()), vec![3, 1, 2]);
    }

    #[test]
    fn a_reorder_that_changes_nothing_reports_false() {
        for kind in Kind::ALL {
            let mut lane = lane_with(kind, &[(1, Queue), (2, Queue), (3, Queue)]);
            assert!(!lane.reorder(&[1, 2, 3]));
            assert!(!lane.reorder(&[1, 99]));
            assert!(lane.reorder(&[1, 3]));
            assert_eq!(ids(lane.items()), vec![1, 3, 2]);
        }
    }

    #[test]
    fn reorder_keeps_error_state() {
        let mut lane = lane_with(Kind::Manga, &[(1, Error), (2, Queue)]);
        lane.reorder(&[2, 1]);
        assert_eq!(states(&lane), vec![(2, Queue), (1, Error)]);
    }

    #[test]
    fn every_edge_of_the_state_diagram_is_allowed_and_the_others_are_refused() {
        let mut lane = lane_with(Kind::Anime, &[(1, Queue)]);
        assert_eq!(lane.set_state(1, Downloading), Ok(true));
        assert_eq!(lane.set_state(1, Error), Ok(true));
        assert_eq!(lane.set_state(1, Queue), Ok(true));
        assert_eq!(lane.set_state(1, Downloading), Ok(true));
        assert_eq!(lane.set_state(1, Queue), Ok(true));
        // ERROR -> DOWNLOADING has to go through QUEUE.
        lane.set_state(1, Error).unwrap();
        assert!(matches!(lane.set_state(1, Downloading), Err(QueueError::Transition { .. })));
        // NOT_DOWNLOADED and DOWNLOADED are never set directly.
        let mut lane = lane_with(Kind::Manga, &[(5, Queue)]);
        assert!(matches!(lane.set_state(5, Downloaded), Err(QueueError::Transition { .. })));
        assert!(matches!(lane.set_state(5, NotDownloaded), Err(QueueError::Transition { .. })));
        assert_eq!(lane.set_state(7, Queue), Err(QueueError::UnknownId(7)));
        assert_eq!(lane.set_state(5, Queue), Ok(false));
    }

    #[test]
    fn finish_removes_the_unit_so_no_finished_unit_stays() {
        let mut lane = lane_with(Kind::Manga, &[(1, Downloading), (2, Queue)]);
        assert_eq!(lane.finish(1).map(|d| d.unit.id), Some(1));
        assert_eq!(states(&lane), vec![(2, Queue)]);
        assert!(lane.finish(1).is_none());
    }

    #[test]
    fn remove_entry_only_removes_units_of_that_entry() {
        let mut lane = Lane::new(Kind::Anime);
        lane.add_all(vec![queued(Kind::Anime, 1, 1, 7), queued(Kind::Anime, 1, 2, 8), queued(Kind::Anime, 1, 3, 7)]);
        assert_eq!(ids(&lane.remove_entry(7)), vec![1, 3]);
        assert_eq!(states(&lane), vec![(2, Queue)]);
    }

    #[test]
    fn clear_empties_the_lane() {
        let mut lane = lane_with(Kind::Manga, &[(1, Downloading), (2, Error)]);
        assert_eq!(lane.clear().len(), 2);
        assert!(lane.is_empty());
        assert!(lane.all_idle());
    }

    #[test]
    fn pick_follows_the_scheduler_rules_and_skips_waiting_units() {
        let mut lane = lane_with(Kind::Manga, &[(1, Error), (2, Queue)]);
        assert_eq!(lane.pick(5), vec![2]);
        assert!(!lane.all_idle());
        // A waiting unit is skipped like an ERROR one, and the next unit of the source runs.
        assert!(lane.set_waiting(2, true));
        assert!(!lane.set_waiting(2, true), "no change the second time");
        assert!(lane.pick(5).is_empty());
        assert!(lane.all_idle(), "only a waiting unit is left");
        lane.add_all(vec![queued(Kind::Manga, 1, 3, 1)]);
        assert_eq!(lane.pick(5), vec![3]);
    }

    #[test]
    fn pick_runs_at_most_max_sources_sources_at_once() {
        let mut lane = Lane::new(Kind::Anime);
        lane.add_all((1..=3).map(|s| queued(Kind::Anime, s, s * 10, s)).collect());
        assert_eq!(lane.pick(2), vec![10, 20]);
        assert_eq!(lane.pick(3), vec![10, 20, 30]);
    }

    #[test]
    fn requeue_running_leaves_the_run_state_alone() {
        let mut lane = lane_with(Kind::Manga, &[(1, Downloading)]);
        lane.begin(StartMode::Explicit);
        assert_eq!(lane.requeue_running(), vec![1]);
        assert_eq!(lane.run_state(), RunState::Running);
    }

    #[test]
    fn snapshots_keep_their_copy_when_the_lane_changes() {
        let mut lane = lane_with(Kind::Anime, &[(1, Queue)]);
        let before = lane.items().to_vec();
        lane.set_state(1, Downloading).unwrap();
        assert_eq!(before[0].state, Queue);
        assert_eq!(lane.get(1).unwrap().state, Downloading);
    }
}
