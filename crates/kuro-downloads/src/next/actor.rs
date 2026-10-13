//! The queue actor: one task owns both lanes, the running units, the network gate and the progress flush
//! (docs/design/unified-downloads.md 2.5). Commands arrive on an unbounded channel, so none is dropped. Every store
//! write and every disk touch runs in `spawn_blocking`. With nothing running no timer runs.

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use kuro_data::entry::EntryRepo;
use kuro_data::item::ItemRepo;
use kuro_db::Db;
use kuro_platform::kind::Kind;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::task::{AbortHandle, Id, JoinError, JoinSet};
use tokio::time::{Instant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

use super::events::DownloadEvent;
use super::gate::{Connectivity, DEBOUNCE, Gate, GateAction, verdict};
use super::index::DownloadIndex;
use super::layout::{Layout, check_free_space};
use super::queue::{CancelTarget, EnqueueRequest, Enqueued, Place, QueueParams, Layouts, Refusal, Snapshot, bulk_warning, plan_enqueue};
use super::runner::{RunCtx, SourceDirectory, UnitDone, UnitJob, UnitRunner};
use super::state::{Lane, RunState, StartMode};
use super::store::{QueueStore, StoredRow};
use super::unit::{Availability, EntryRef, QueuedUnit, SourceKey, UnitRef};
use crate::downloader::unix_millis;
use crate::error::{DownloadError, FailureKind};
use crate::model::DownloadState;
use crate::pipeline::FreeSpaceFn;

/// Progress events are coalesced to this interval per unit (10 Hz).
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

pub(crate) enum Command {
    Enqueue { req: EnqueueRequest, reply: oneshot::Sender<Result<Enqueued, DownloadError>> },
    StartNow { req: EnqueueRequest, reply: oneshot::Sender<Result<(), DownloadError>> },
    Start { kind: Kind },
    Pause { kind: Kind },
    Clear { kind: Kind },
    Reorder { kind: Kind, unit_ids: Vec<i64> },
    Cancel { kind: Kind, target: CancelTarget, reply: oneshot::Sender<Vec<i64>> },
    /// Replies `true` when a run was holding a unit of the entry and is now stopped.
    Hold { kind: Kind, entry_id: i64, reply: oneshot::Sender<bool> },
    Release { kind: Kind },
    SourcesChanged,
}

fn slot(kind: Kind) -> usize {
    match kind {
        Kind::Manga => 0,
        Kind::Anime => 1,
    }
}

struct Lanes {
    manga: Lane,
    anime: Lane,
}

impl Lanes {
    fn lane(&self, kind: Kind) -> &Lane {
        match kind {
            Kind::Manga => &self.manga,
            Kind::Anime => &self.anime,
        }
    }

    fn lane_mut(&mut self, kind: Kind) -> &mut Lane {
        match kind {
            Kind::Manga => &mut self.manga,
            Kind::Anime => &mut self.anime,
        }
    }
}

struct RunningTask {
    id: Id,
    abort: AbortHandle,
    cancel: CancellationToken,
    /// The folder the unit downloads into, as named when it started.
    source_dir: String,
}

struct Actor {
    db: Arc<Db>,
    store: Arc<QueueStore>,
    runners: Vec<Arc<dyn UnitRunner>>,
    directory: Arc<dyn SourceDirectory>,
    layouts: Layouts,
    index: DownloadIndex,
    free_space: FreeSpaceFn,
    max_active: Arc<dyn Fn() -> usize + Send + Sync>,
    connectivity: watch::Receiver<Connectivity>,
    connectivity_open: bool,
    only_over_wifi: bool,
    lanes: Lanes,
    gate: Gate,
    /// Units the network stopped, per lane. They go back to QUEUE when it returns.
    network_failed: [HashSet<i64>; 2],
    /// Open [`super::queue::EntryHold`]s per lane. A held lane starts nothing.
    holds: [usize; 2],
    /// A unit finished since the lane last started: the end of the run announces it.
    finished_in_run: [bool; 2],
    /// When the gate looks again at a block that has not lasted long enough to stop the lanes.
    recheck: Option<Instant>,
    tasks: JoinSet<Result<UnitDone, DownloadError>>,
    running: HashMap<(Kind, i64), RunningTask>,
    task_key: HashMap<Id, (Kind, i64)>,
    /// The last progress version sent per running unit.
    seen_progress: HashMap<(Kind, i64), u64>,
    snapshot: watch::Sender<Arc<Snapshot>>,
    events: broadcast::Sender<DownloadEvent>,
}

/// The actor loop. Returns when every `Queue` handle is gone.
pub(crate) async fn run(
    params: QueueParams,
    mut commands: mpsc::UnboundedReceiver<Command>,
    snapshot: watch::Sender<Arc<Snapshot>>,
    events: broadcast::Sender<DownloadEvent>,
) {
    let mut actor = Actor::new(params, snapshot, events);
    actor.restore().await;
    actor.apply_gate();
    actor.publish();
    let mut flush = tokio::time::interval(PROGRESS_INTERVAL);
    flush.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let recheck = actor.recheck;
        tokio::select! {
            command = commands.recv() => match command {
                Some(command) => actor.handle(command).await,
                None => break,
            },
            Some(joined) = actor.tasks.join_next_with_id(), if !actor.tasks.is_empty() => {
                actor.on_joined(joined).await;
            }
            changed = actor.connectivity.changed(), if actor.connectivity_open => {
                if changed.is_err() {
                    actor.connectivity_open = false;
                }
            }
            _ = flush.tick(), if !actor.running.is_empty() => actor.flush_progress(),
            _ = until(recheck) => actor.recheck = None,
        }
        actor.apply_gate();
        actor.reconcile();
        actor.publish();
    }
    for kind in Kind::ALL {
        actor.abort_kind(kind);
    }
}

async fn until(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

impl Actor {
    fn new(params: QueueParams, snapshot: watch::Sender<Arc<Snapshot>>, events: broadcast::Sender<DownloadEvent>) -> Self {
        Self {
            db: params.db,
            store: params.store,
            runners: params.runners,
            directory: params.directory,
            layouts: params.layouts,
            index: params.index,
            free_space: params.free_space,
            max_active: params.max_active,
            connectivity: params.connectivity,
            connectivity_open: true,
            only_over_wifi: params.only_over_wifi,
            lanes: Lanes { manga: Lane::new(Kind::Manga), anime: Lane::new(Kind::Anime) },
            gate: Gate::default(),
            network_failed: [HashSet::new(), HashSet::new()],
            holds: [0, 0],
            finished_in_run: [false, false],
            recheck: None,
            tasks: JoinSet::new(),
            running: HashMap::new(),
            task_key: HashMap::new(),
            seen_progress: HashMap::new(),
            snapshot,
            events,
        }
    }

    fn emit(&self, event: DownloadEvent) {
        let _ = self.events.send(event);
    }

    fn emit_status(&self, kind: Kind, ids: Vec<i64>, state: DownloadState) {
        for unit_id in ids {
            self.emit(DownloadEvent::Status { kind, unit_id, state });
        }
    }

    fn runner(&self, kind: Kind) -> Option<&Arc<dyn UnitRunner>> {
        self.runners.iter().find(|r| r.kind() == kind)
    }

    fn can_run(&self, kind: Kind, source: &SourceKey) -> bool {
        self.runner(kind).is_some_and(|r| r.can_run(source))
    }

    /// Whether a unit of this source waits: the source is not installed, or its runner cannot reach it.
    fn waiting_for(&self, kind: Kind, source_id: i64) -> bool {
        match self.directory.source_key(kind, source_id) {
            None => true,
            Some(key) => key.availability == Availability::Stub || !self.can_run(kind, &key),
        }
    }

    // ----- startup -----

    /// Restores both lanes from the store and sweeps the staging folders nothing restored owns (B20). The lanes are
    /// restored as QUEUE and wait for a start.
    async fn restore(&mut self) {
        for kind in Kind::ALL {
            let Some(runner) = self.runner(kind).cloned() else { continue };
            let store = self.store.clone();
            let db = self.db.clone();
            let directory = self.directory.clone();
            let layout = self.layouts.of(kind).clone();
            let index = self.index.clone();
            let restored = tokio::task::spawn_blocking(move || {
                restore_kind(kind, &store, &db, directory.as_ref(), runner.as_ref(), &layout, &index)
            })
            .await;
            match restored {
                Ok(units) => *self.lanes.lane_mut(kind) = Lane::restored(kind, units),
                Err(err) => tracing::error!(target: "kuro_downloads", error = %err, "restoring the queue failed"),
            }
        }
    }

    // ----- commands -----

    async fn handle(&mut self, command: Command) {
        match command {
            Command::Enqueue { req, reply } => {
                let outcome = self.enqueue(req).await;
                let _ = reply.send(outcome);
            }
            Command::StartNow { req, reply } => {
                let outcome = self.start_now(req).await;
                let _ = reply.send(outcome);
            }
            Command::Start { kind } => {
                // The user took the lane in hand: the network no longer starts it by itself.
                self.gate.user_paused(kind);
                self.start_lane(kind, StartMode::Explicit);
            }
            Command::Pause { kind } => self.pause(kind),
            Command::Clear { kind } => self.clear(kind).await,
            Command::Reorder { kind, unit_ids } => self.reorder(kind, &unit_ids).await,
            Command::Cancel { kind, target, reply } => {
                let ids = self.cancel(kind, target).await;
                let _ = reply.send(ids);
            }
            Command::Hold { kind, entry_id, reply } => {
                let lane = self.lanes.lane(kind);
                let held = lane.run_state() == RunState::Running && lane.items().iter().any(|d| d.entry.id == entry_id);
                if held {
                    self.holds[slot(kind)] += 1;
                    self.requeue_running(kind);
                }
                let _ = reply.send(held);
            }
            Command::Release { kind } => {
                let holds = &mut self.holds[slot(kind)];
                *holds = holds.saturating_sub(1);
            }
            Command::SourcesChanged => self.sources_changed(),
        }
    }

    fn new_unit(&self, req: &EnqueueRequest, unit: UnitRef) -> QueuedUnit {
        let mut queued = QueuedUnit::new(req.kind, req.entry.clone(), unit, req.source.clone());
        queued.waiting = req.source.availability == Availability::Stub || !self.can_run(req.kind, &req.source);
        queued
    }

    fn is_downloaded(&self, req: &EnqueueRequest, unit: &UnitRef) -> bool {
        self.index.is_downloaded(req.kind, req.source.id, &req.entry.title, &unit.name, unit.scanlator.as_deref())
    }

    async fn enqueue(&mut self, req: EnqueueRequest) -> Result<Enqueued, DownloadError> {
        let kind = req.kind;
        if req.units.is_empty() {
            return Ok(Enqueued::refused(Refusal::NothingToDo));
        }
        if req.source.availability == Availability::Local {
            return Ok(Enqueued::refused(Refusal::LocalSource));
        }
        if self.runner(kind).is_none() {
            return Err(DownloadError::Internal(format!("no {} runner", kind.as_str())));
        }
        let lane = self.lanes.lane(kind);
        let was_empty = lane.is_empty();
        let was_running = lane.run_state() == RunState::Running;
        let picks = plan_enqueue(&req.units, req.skip_fillermarked, |id| lane.contains(id), |u| self.is_downloaded(&req, u));
        let units: Vec<QueuedUnit> = picks.into_iter().map(|u| self.new_unit(&req, u)).collect();
        let lane = self.lanes.lane_mut(kind);
        let added = match req.place {
            Place::Back => lane.add_all(units),
            Place::Front => lane.add_front(units),
        };
        if added.is_empty() {
            return Ok(Enqueued::refused(Refusal::NothingToDo));
        }
        self.persist(kind).await;
        self.emit(DownloadEvent::QueueChanged { kind });
        let mut out = Enqueued {
            added,
            started: false,
            warning: None,
            continue_toast: !was_empty && !was_running,
            refused: None,
        };
        if req.auto_start && (was_empty || req.place == Place::Front) {
            out.warning = bulk_warning(self.lanes.lane(kind).items().iter().map(|d| (d.source.id, d.source.unmetered)));
            if let Some(warning) = &out.warning {
                self.emit(DownloadEvent::Warning {
                    kind,
                    text: warning.text.to_owned(),
                    timeout_secs: Some(warning.timeout_secs),
                    help_url: Some(warning.help_url.to_owned()),
                });
            }
            // A new download retries nothing that failed before.
            out.started = self.start_lane(kind, StartMode::Resume(HashSet::new()));
        }
        Ok(out)
    }

    async fn start_now(&mut self, req: EnqueueRequest) -> Result<(), DownloadError> {
        let kind = req.kind;
        if req.source.availability == Availability::Local {
            return Err(DownloadError::Internal("a local source is not downloaded".to_owned()));
        }
        if self.runner(kind).is_none() {
            return Err(DownloadError::Internal(format!("no {} runner", kind.as_str())));
        }
        if req.units.iter().any(|u| self.is_downloaded(&req, u)) {
            return Err(DownloadError::AlreadyDownloaded);
        }
        let ids: Vec<i64> = req.units.iter().map(|u| u.id).collect();
        let units: Vec<QueuedUnit> = req.units.iter().map(|u| self.new_unit(&req, u.clone())).collect();
        // A unit that is already queued moves to the front and starts over, an ERROR one included.
        for removed in self.lanes.lane_mut(kind).remove_units(&ids) {
            self.abort_unit(kind, removed.unit.id);
            self.network_failed[slot(kind)].remove(&removed.unit.id);
        }
        self.lanes.lane_mut(kind).add_front(units);
        self.persist(kind).await;
        self.emit(DownloadEvent::QueueChanged { kind });
        self.start_lane(kind, StartMode::Resume(HashSet::new()));
        Ok(())
    }

    /// Begins a run. Does nothing for an empty lane or one that already runs. Returns whether a run began.
    fn start_lane(&mut self, kind: Kind, mode: StartMode) -> bool {
        let lane = self.lanes.lane_mut(kind);
        if lane.run_state() == RunState::Running || lane.is_empty() {
            return false;
        }
        let explicit = matches!(mode, StartMode::Explicit);
        let retried = lane.begin(mode);
        if explicit {
            self.network_failed[slot(kind)].clear();
        }
        self.finished_in_run[slot(kind)] = false;
        self.emit_status(kind, retried, DownloadState::Queue);
        self.emit(DownloadEvent::Started { kind });
        true
    }

    fn pause(&mut self, kind: Kind) {
        let lane = self.lanes.lane(kind);
        if lane.is_empty() || lane.run_state() == RunState::Paused {
            return;
        }
        self.abort_kind(kind);
        let requeued = self.lanes.lane_mut(kind).pause_running();
        self.emit_status(kind, requeued, DownloadState::Queue);
        self.network_failed[slot(kind)].clear();
        self.gate.user_paused(kind);
        self.emit(DownloadEvent::Paused { kind });
    }

    async fn clear(&mut self, kind: Kind) {
        self.abort_kind(kind);
        let lane = self.lanes.lane_mut(kind);
        lane.clear();
        lane.set_run_state(RunState::Stopped);
        self.network_failed[slot(kind)].clear();
        self.gate.user_paused(kind);
        self.emit(DownloadEvent::Dismiss { kind });
        self.persist(kind).await;
        self.emit(DownloadEvent::QueueChanged { kind });
    }

    /// The scheduler restarts what the new order displaces; a unit that keeps its slot keeps running.
    async fn reorder(&mut self, kind: Kind, unit_ids: &[i64]) {
        if !self.lanes.lane_mut(kind).reorder(unit_ids) {
            return;
        }
        self.persist(kind).await;
        self.emit(DownloadEvent::QueueChanged { kind });
    }

    /// Removes units from the lane. Only the removed units stop; no other unit changes state, so a failed unit is not
    /// retried (B16).
    async fn cancel(&mut self, kind: Kind, target: CancelTarget) -> Vec<i64> {
        let lane = self.lanes.lane_mut(kind);
        let removed = match target {
            CancelTarget::Units(ids) => lane.remove_units(&ids),
            CancelTarget::Entry(entry_id) => lane.remove_entry(entry_id),
        };
        let ids: Vec<i64> = removed.iter().map(|d| d.unit.id).collect();
        for id in &ids {
            self.abort_unit(kind, *id);
            self.network_failed[slot(kind)].remove(id);
        }
        if !ids.is_empty() {
            self.persist(kind).await;
            self.emit(DownloadEvent::QueueChanged { kind });
        }
        ids
    }

    /// Asks the directory and the runner again about every unit's source and flips the waiting flags.
    fn sources_changed(&mut self) {
        for kind in Kind::ALL {
            let flags: Vec<(i64, bool)> =
                self.lanes.lane(kind).items().iter().map(|d| (d.unit.id, self.waiting_for(kind, d.source.id))).collect();
            let lane = self.lanes.lane_mut(kind);
            let mut changed = false;
            for (unit_id, waiting) in flags {
                changed |= lane.set_waiting(unit_id, waiting);
            }
            if changed {
                self.emit(DownloadEvent::QueueChanged { kind });
            }
        }
    }

    // ----- running units -----

    fn abort_unit(&mut self, kind: Kind, unit_id: i64) {
        if let Some(task) = self.running.remove(&(kind, unit_id)) {
            task.cancel.cancel();
            task.abort.abort();
            self.task_key.remove(&task.id);
        }
        self.seen_progress.remove(&(kind, unit_id));
    }

    fn abort_kind(&mut self, kind: Kind) {
        let ids: Vec<i64> = self.running.keys().filter(|(k, _)| *k == kind).map(|(_, id)| *id).collect();
        for id in ids {
            self.abort_unit(kind, id);
        }
    }

    /// Aborts the lane's running units and puts them back in QUEUE. The run state stays.
    fn requeue_running(&mut self, kind: Kind) {
        self.abort_kind(kind);
        let requeued = self.lanes.lane_mut(kind).requeue_running();
        self.emit_status(kind, requeued, DownloadState::Queue);
    }

    /// Stops the lane for a failure: running units become ERROR. With a reason (the network) they are remembered, so
    /// the return of the network retries exactly them, and a warning goes out.
    fn stop_lane(&mut self, kind: Kind, reason: Option<&'static str>) {
        self.abort_kind(kind);
        let errored = self.lanes.lane_mut(kind).stop_running();
        if reason.is_some() {
            self.network_failed[slot(kind)].extend(errored.iter().copied());
        }
        self.emit_status(kind, errored, DownloadState::Error);
        if let Some(text) = reason {
            self.emit(DownloadEvent::Warning { kind, text: text.to_owned(), timeout_secs: None, help_url: None });
        }
    }

    /// Starts and aborts units so they match the scheduler's pick, and ends a run that has nothing left to do.
    fn reconcile(&mut self) {
        for kind in Kind::ALL {
            self.reconcile_kind(kind);
        }
    }

    fn reconcile_kind(&mut self, kind: Kind) {
        let lane = self.lanes.lane(kind);
        if lane.run_state() != RunState::Running || self.holds[slot(kind)] > 0 {
            return;
        }
        let wanted = lane.pick((self.max_active)().max(1));
        let stale: Vec<i64> =
            self.running.keys().filter(|(k, id)| *k == kind && !wanted.contains(id)).map(|(_, id)| *id).collect();
        for unit_id in stale {
            self.abort_unit(kind, unit_id);
            self.requeue(kind, unit_id);
        }
        for unit_id in wanted {
            if !self.running.contains_key(&(kind, unit_id)) {
                self.spawn_unit(kind, unit_id);
            }
        }
        // A waiting unit keeps the run open: it starts by itself when its source appears. Units the network failed
        // keep it open while the gate is about to stop the lane, so the network's return can resume them.
        let lane = self.lanes.lane(kind);
        let nothing_active = lane.items().iter().all(|d| !d.state.is_active());
        let network_pending = !self.network_failed[slot(kind)].is_empty() && self.blocked();
        let running_any = self.running.keys().any(|(k, _)| *k == kind);
        if nothing_active && !running_any && !network_pending {
            self.lanes.lane_mut(kind).set_run_state(RunState::Stopped);
            if std::mem::take(&mut self.finished_in_run[slot(kind)]) {
                self.emit(DownloadEvent::Complete { kind });
            }
        }
    }

    fn requeue(&mut self, kind: Kind, unit_id: i64) {
        if let Ok(true) = self.lanes.lane_mut(kind).set_state(unit_id, DownloadState::Queue) {
            self.emit(DownloadEvent::Status { kind, unit_id, state: DownloadState::Queue });
        }
    }

    fn spawn_unit(&mut self, kind: Kind, unit_id: i64) {
        let Some(item) = self.lanes.lane(kind).get(unit_id).cloned() else { return };
        let Some(runner) = self.runner(kind).cloned() else {
            let err = DownloadError::Internal(format!("no {} runner", kind.as_str()));
            self.fail_unit(kind, unit_id, err);
            return;
        };
        // A unit that waited as a stub runs with the source's current name, which names its folder.
        let source = self.directory.source_key(kind, item.source.id).unwrap_or_else(|| item.source.clone());
        if let Ok(true) = self.lanes.lane_mut(kind).set_state(unit_id, DownloadState::Downloading) {
            self.emit(DownloadEvent::Status { kind, unit_id, state: DownloadState::Downloading });
        }
        let cancel = CancellationToken::new();
        let ctx = RunCtx {
            cancel: cancel.clone(),
            progress: item.progress.clone(),
            layout: self.layouts.of(kind).clone(),
            index: self.index.clone(),
        };
        let job = UnitJob { entry: item.entry.clone(), unit: item.unit.clone(), source: source.clone() };
        // The free-space precheck creates the entry folder, so it runs off the actor.
        let layout = ctx.layout.clone();
        let free_space = self.free_space;
        let dir_name = source_dir(&self.index, kind, &source, &item.entry.title);
        let title = item.entry.title.clone();
        let blocking_dir = dir_name.clone();
        let abort = self.tasks.spawn(async move {
            let checked = tokio::task::spawn_blocking(move || -> Result<(), DownloadError> {
                let dir = layout.entry_dir(&blocking_dir, &title)?;
                check_free_space(free_space(&dir))
            })
            .await;
            match checked {
                Ok(Ok(())) => {}
                Ok(Err(err)) => return Err(err),
                Err(err) => return Err(DownloadError::Internal(err.to_string())),
            }
            runner.run(job, ctx).await
        });
        self.task_key.insert(abort.id(), (kind, unit_id));
        self.seen_progress.insert((kind, unit_id), item.progress.version());
        self.running.insert((kind, unit_id), RunningTask { id: abort.id(), abort, cancel, source_dir: dir_name });
    }

    async fn on_joined(&mut self, joined: Result<(Id, Result<UnitDone, DownloadError>), JoinError>) {
        let (id, outcome) = match joined {
            Ok((id, outcome)) => (id, outcome),
            Err(err) => {
                let id = err.id();
                let message = if err.is_panic() { panic_message(err.into_panic()) } else { "the download was cancelled".to_owned() };
                (id, Err(DownloadError::Internal(message)))
            }
        };
        // A task we aborted has no entry here, so its late result is dropped.
        let Some((kind, unit_id)) = self.task_key.remove(&id) else { return };
        let task = self.running.remove(&(kind, unit_id));
        self.seen_progress.remove(&(kind, unit_id));
        match outcome {
            Ok(done) => {
                let dir = task.map(|t| t.source_dir);
                self.finished(kind, unit_id, dir, done).await;
            }
            Err(DownloadError::Cancelled) => self.requeue(kind, unit_id),
            Err(err) => self.fail_unit(kind, unit_id, err),
        }
    }

    async fn finished(&mut self, kind: Kind, unit_id: i64, dir: Option<String>, done: UnitDone) {
        let Some(item) = self.lanes.lane(kind).get(unit_id).cloned() else { return };
        let dir = dir.unwrap_or_else(|| source_dir(&self.index, kind, &item.source, &item.entry.title));
        self.index.add_unit(kind, item.source.id, &dir, &item.entry.title, &done.dir_name);
        self.lanes.lane_mut(kind).finish(unit_id);
        self.network_failed[slot(kind)].remove(&unit_id);
        self.finished_in_run[slot(kind)] = true;
        self.persist(kind).await;
        self.emit(DownloadEvent::Status { kind, unit_id, state: DownloadState::Downloaded });
        self.emit(DownloadEvent::Finished { kind, unit_id, entry_id: item.entry.id });
        self.emit(DownloadEvent::QueueChanged { kind });
    }

    fn fail_unit(&mut self, kind: Kind, unit_id: i64, err: DownloadError) {
        let Some(item) = self.lanes.lane(kind).get(unit_id).cloned() else { return };
        tracing::warn!(target: "kuro_downloads", kind = kind.as_str(), unit = unit_id, error = %err, "download failed");
        if err.failure_kind() == FailureKind::Network {
            self.network_failed[slot(kind)].insert(unit_id);
        }
        if let Ok(true) = self.lanes.lane_mut(kind).set_state(unit_id, DownloadState::Error) {
            self.emit(DownloadEvent::Status { kind, unit_id, state: DownloadState::Error });
        }
        // Per-page failures were already reported one by one.
        if !matches!(err, DownloadError::PagesFailed(_)) {
            self.emit(DownloadEvent::Error {
                kind,
                unit_id: Some(unit_id),
                entry_id: Some(item.entry.id),
                entry_title: Some(item.entry.title.clone()),
                unit_name: Some(item.unit.name.clone()),
                message: err.to_string(),
            });
        }
        // The folder cannot be written: nothing in this lane can run (FM-01).
        if matches!(err, DownloadError::InvalidLocation(_)) {
            self.stop_lane(kind, None);
        }
    }

    fn flush_progress(&mut self) {
        let keys: Vec<(Kind, i64)> = self.running.keys().copied().collect();
        for (kind, unit_id) in keys {
            let Some(item) = self.lanes.lane(kind).get(unit_id) else { continue };
            let version = item.progress.version();
            if self.seen_progress.insert((kind, unit_id), version) == Some(version) {
                continue;
            }
            let (done, total) = item.progress.pages().unwrap_or((0, 0));
            self.emit(DownloadEvent::Progress {
                kind,
                unit_id,
                entry_id: item.entry.id,
                entry_title: item.entry.title.clone(),
                unit_name: item.unit.name.clone(),
                done,
                total,
                percent: item.progress.percent(),
            });
        }
    }

    // ----- network -----

    fn blocked(&self) -> bool {
        verdict(*self.connectivity.borrow(), self.only_over_wifi).is_some()
    }

    /// Feeds the gate the current connectivity and acts on its answer: a block that lasted stops the running lanes, the
    /// return of the network starts the lanes it stopped (and only those).
    fn apply_gate(&mut self) {
        let now = Instant::now();
        let connectivity = *self.connectivity.borrow_and_update();
        let running = Kind::ALL.map(|kind| self.lanes.lane(kind).run_state() == RunState::Running);
        let actions = self.gate.observe(now.into_std(), connectivity, self.only_over_wifi, running);
        for (kind, action) in Kind::ALL.into_iter().zip(actions) {
            match action {
                GateAction::Stop(reason) => self.stop_lane(kind, Some(reason)),
                GateAction::Resume => {
                    let ids = std::mem::take(&mut self.network_failed[slot(kind)]);
                    self.start_lane(kind, StartMode::Resume(ids));
                }
                GateAction::Nothing => {}
            }
        }
        // A block shorter than the debounce stops nothing yet: look again when it has lasted long enough.
        let still_running = Kind::ALL.iter().any(|k| self.lanes.lane(*k).run_state() == RunState::Running);
        self.recheck = if verdict(connectivity, self.only_over_wifi).is_some() && still_running {
            self.recheck.or(Some(now + DEBOUNCE))
        } else {
            None
        };
    }

    // ----- state out -----

    /// Saves the lane. The returned future owns its data, so the actor does not hold itself across the wait.
    fn persist(&self, kind: Kind) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let now = unix_millis();
        let rows: Vec<StoredRow> = self
            .lanes
            .lane(kind)
            .items()
            .iter()
            .enumerate()
            .map(|(position, d)| StoredRow {
                unit_id: d.unit.id,
                entry_id: d.entry.id,
                source: d.source.id,
                position: position as i64,
                state: d.state as u8,
                // The queue does not keep the enqueue instant: it is written at save time.
                enqueued_at: now,
            })
            .collect();
        let store = self.store.clone();
        Box::pin(async move {
            match tokio::task::spawn_blocking(move || store.replace(kind, &rows)).await {
                Ok(Ok(_)) => {}
                Ok(Err(err)) => {
                    tracing::error!(target: "kuro_downloads", kind = kind.as_str(), error = %err, "could not save the queue")
                }
                Err(err) => {
                    tracing::error!(target: "kuro_downloads", kind = kind.as_str(), error = %err, "queue save task failed")
                }
            }
        })
    }

    fn publish(&self) {
        let items = Kind::ALL.iter().flat_map(|k| self.lanes.lane(*k).items().iter().cloned()).collect();
        let next = Snapshot { items, manga: self.lanes.manga.run_state(), anime: self.lanes.anime.run_state() };
        self.snapshot.send_if_modified(move |current| {
            if current.same_as(&next) {
                return false;
            }
            *current = Arc::new(next);
            true
        });
    }
}

/// The folder a source's units live in. A source that is not installed (a stub) may have its folder on disk under a
/// name the index knows better than the stub does.
fn source_dir(index: &DownloadIndex, kind: Kind, source: &SourceKey, title: &str) -> String {
    let computed = Layout::source_dir_name(&source.name, &source.lang);
    if source.availability == Availability::Stub {
        return index.entry_source_dir(kind, source.id, title).unwrap_or(computed);
    }
    computed
}

/// Reads one lane's saved rows back as queued units (blocking). A row whose item or entry is gone is dropped; a row
/// whose source cannot be named is kept as a stub that waits (D5). Then the staging folders of every restored unit are
/// kept and the rest are swept. When the saved rows could not be read nothing is swept: the units that own the
/// staging folders are unknown.
fn restore_kind(
    kind: Kind,
    store: &QueueStore,
    db: &Arc<Db>,
    directory: &dyn SourceDirectory,
    runner: &dyn UnitRunner,
    layout: &Layout,
    index: &DownloadIndex,
) -> Vec<QueuedUnit> {
    let rows = match store.load(kind) {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(target: "kuro_downloads", kind = kind.as_str(), error = %err, "could not read the saved queue");
            return Vec::new();
        }
    };
    let entries = EntryRepo::new(db.clone());
    let items = ItemRepo::new(db.clone());
    let mut units = Vec::new();
    for row in rows {
        let Ok(Some(item)) = items.by_id(row.unit_id) else { continue };
        let Ok(entry) = entries.by_id(row.entry_id) else { continue };
        if entry.kind != kind {
            continue;
        }
        let (source, waiting) = match directory.source_key(kind, row.source) {
            Some(key) => {
                let waiting = key.availability == Availability::Stub || !runner.can_run(&key);
                (key, waiting)
            }
            None => {
                let stub = SourceKey {
                    id: row.source,
                    name: String::new(),
                    lang: String::new(),
                    unmetered: false,
                    availability: Availability::Stub,
                };
                (stub, true)
            }
        };
        let mut unit = QueuedUnit::new(kind, EntryRef::from(&entry), UnitRef::from(&item), source);
        unit.waiting = waiting;
        units.push(unit);
    }
    let mut keep = Vec::new();
    for unit in &units {
        let dir = source_dir(index, kind, &unit.source, &unit.entry.title);
        if let Some(entry_dir) = layout.find_entry_dir(&dir, &unit.entry.title) {
            keep.extend(runner.staging_names(&unit.unit).into_iter().map(|name| entry_dir.join(name)));
        }
    }
    let removed = layout.sweep_staging(&keep);
    tracing::debug!(target: "kuro_downloads", kind = kind.as_str(), removed, "startup sweep");
    units
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "the download task panicked".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    use async_trait::async_trait;
    use kuro_data::entry::Entry;
    use kuro_data::item::Item;
    use kuro_source::SourceError;

    use crate::next::index::IndexRoots;
    use crate::next::naming::{UnitName, entry_dir_name, staging_dir_names, unit_dir_name};
    use crate::next::queue::Queue;
    use crate::next::unit::fixtures;

    const LONG: u64 = 3_600_000;

    struct InFlight<'a>(&'a AtomicUsize);

    impl Drop for InFlight<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Runs for a set time, then fails for the ids in `fail` and succeeds otherwise. Counts what is in flight now and at
    /// most, and records every start.
    struct FakeRunner {
        kind: Kind,
        delay_ms: AtomicU64,
        unit_delay_ms: Mutex<HashMap<i64, u64>>,
        fail: Mutex<HashMap<i64, fn() -> DownloadError>>,
        in_flight: AtomicUsize,
        peak: AtomicUsize,
        started: Mutex<Vec<i64>>,
    }

    impl FakeRunner {
        fn new(kind: Kind) -> Arc<Self> {
            Arc::new(Self {
                kind,
                delay_ms: AtomicU64::new(0),
                unit_delay_ms: Mutex::new(HashMap::new()),
                fail: Mutex::new(HashMap::new()),
                in_flight: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
                started: Mutex::new(Vec::new()),
            })
        }

        fn set_delay(&self, ms: u64) {
            self.delay_ms.store(ms, Ordering::SeqCst);
        }

        fn set_unit_delay(&self, unit_id: i64, ms: u64) {
            self.unit_delay_ms.lock().unwrap().insert(unit_id, ms);
        }

        fn fail_with(&self, unit_id: i64, make: fn() -> DownloadError) {
            self.fail.lock().unwrap().insert(unit_id, make);
        }

        fn in_flight(&self) -> usize {
            self.in_flight.load(Ordering::SeqCst)
        }

        fn peak(&self) -> usize {
            self.peak.load(Ordering::SeqCst)
        }

        fn started(&self) -> Vec<i64> {
            self.started.lock().unwrap().clone()
        }

        fn starts_of(&self, unit_id: i64) -> usize {
            self.started().iter().filter(|id| **id == unit_id).count()
        }
    }

    #[async_trait]
    impl UnitRunner for FakeRunner {
        fn kind(&self) -> Kind {
            self.kind
        }

        fn can_run(&self, source: &SourceKey) -> bool {
            source.availability != Availability::Stub
        }

        fn staging_names(&self, unit: &UnitRef) -> Vec<String> {
            staging_dir_names(self.kind, UnitName::new(&unit.name, unit.scanlator.as_deref()))
        }

        async fn run(&self, job: UnitJob, ctx: RunCtx) -> Result<UnitDone, DownloadError> {
            self.started.lock().unwrap().push(job.unit.id);
            let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            // The guard counts the unit out when the task is aborted too.
            let _in_flight = InFlight(&self.in_flight);
            ctx.progress.set_percent(50);
            let own = self.unit_delay_ms.lock().unwrap().get(&job.unit.id).copied();
            let delay = Duration::from_millis(own.unwrap_or_else(|| self.delay_ms.load(Ordering::SeqCst)));
            let failure = self.fail.lock().unwrap().get(&job.unit.id).copied();
            tokio::select! {
                _ = ctx.cancel.cancelled() => Err(DownloadError::Cancelled),
                _ = tokio::time::sleep(delay) => match failure {
                    Some(make) => Err(make()),
                    None => Ok(UnitDone {
                        dir_name: unit_dir_name(self.kind, UnitName::new(&job.unit.name, job.unit.scanlator.as_deref())),
                    }),
                },
            }
        }
    }

    #[derive(Default)]
    struct FakeDirectory {
        sources: Mutex<HashMap<(Kind, i64), SourceKey>>,
    }

    impl FakeDirectory {
        fn add(&self, kind: Kind, source: SourceKey) {
            self.sources.lock().unwrap().insert((kind, source.id), source);
        }
    }

    impl SourceDirectory for FakeDirectory {
        fn source_key(&self, kind: Kind, source_id: i64) -> Option<SourceKey> {
            self.sources.lock().unwrap().get(&(kind, source_id)).cloned()
        }
    }

    fn plenty(_: &Path) -> Option<u64> {
        Some(u64::MAX)
    }

    fn low(_: &Path) -> Option<u64> {
        Some(1)
    }

    const OFFLINE: Connectivity = Connectivity { online: false, metered: false };
    const ONLINE: Connectivity = Connectivity { online: true, metered: false };

    /// Everything a queue needs, built before the queue starts so a test can seed the database first. Sources 1 to 9
    /// are installed for both kinds.
    struct Harness {
        db: Arc<Db>,
        store: Arc<QueueStore>,
        directory: Arc<FakeDirectory>,
        runners: [Arc<FakeRunner>; 2],
        index: DownloadIndex,
        connectivity: watch::Sender<Connectivity>,
        max: Arc<AtomicUsize>,
        free: FreeSpaceFn,
        root: tempfile::TempDir,
    }

    impl Harness {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let path = kuro_db::schema_unified::db_path(root.path());
            let writer = kuro_db::schema_unified::open_file(&path).unwrap();
            let db = Arc::new(Db::new(&path, writer).unwrap());
            let directory = Arc::new(FakeDirectory::default());
            for kind in Kind::ALL {
                for id in 1..=9 {
                    directory.add(kind, fixtures::source(id));
                }
            }
            let cache = root.path().join("cache");
            std::fs::create_dir_all(&cache).unwrap();
            let roots = IndexRoots { manga: root.path().join("manga"), anime: root.path().join("anime") };
            Self {
                store: Arc::new(QueueStore::new(db.clone())),
                db,
                directory,
                runners: Kind::ALL.map(FakeRunner::new),
                index: DownloadIndex::open(roots, &cache),
                connectivity: watch::channel(ONLINE).0,
                max: Arc::new(AtomicUsize::new(5)),
                free: plenty,
                root,
            }
        }

        fn runner(&self, kind: Kind) -> &Arc<FakeRunner> {
            &self.runners[slot(kind)]
        }

        fn layouts(&self) -> Layouts {
            Layouts {
                manga: Layout::new(Kind::Manga, self.root.path().join("manga")),
                anime: Layout::new(Kind::Anime, self.root.path().join("anime")),
            }
        }

        fn spawn(&self) -> Queue {
            let max = self.max.clone();
            Queue::spawn(QueueParams {
                db: self.db.clone(),
                store: self.store.clone(),
                runners: self.runners.iter().map(|r| r.clone() as Arc<dyn UnitRunner>).collect(),
                directory: self.directory.clone(),
                layouts: self.layouts(),
                index: self.index.clone(),
                free_space: self.free,
                max_active: Arc::new(move || max.load(Ordering::SeqCst)),
                connectivity: self.connectivity.subscribe(),
                only_over_wifi: false,
            })
        }

        /// A saved queue row for a new entry and item of `kind` whose source is `source`. Returns `(entry id, unit id)`.
        fn seed(&self, kind: Kind, source: i64) -> (i64, i64) {
            let entry = Entry { url: "/seeded".into(), title: "Seeded".into(), source, status: 1, ..Entry::create(kind) };
            let entry_id = EntryRepo::new(self.db.clone()).insert(&entry).unwrap();
            let item = Item { entry_id, url: "/seeded/1".into(), name: "Unit".into(), ..Item::create(kind) };
            let unit_id = ItemRepo::new(self.db.clone()).add_all(&[item]).unwrap()[0].id;
            let row = StoredRow { unit_id, entry_id, source, position: 0, state: 1, enqueued_at: 0 };
            self.store.replace(kind, &[row]).unwrap();
            (entry_id, unit_id)
        }
    }

    fn unit(id: i64, source_order: i64) -> UnitRef {
        UnitRef {
            id,
            entry_id: 1,
            url: format!("/u{id}"),
            name: format!("Unit {id}"),
            scanlator: None,
            done: false,
            bookmark: false,
            fillermark: false,
            source_order,
            date_upload: 0,
            number: id as f64,
        }
    }

    /// An enqueue of `units` for entry `entry_id` of `source_id`, at the back.
    fn request(kind: Kind, source_id: i64, entry_id: i64, units: Vec<UnitRef>, auto_start: bool) -> EnqueueRequest {
        let entry = EntryRef {
            id: entry_id,
            kind,
            source: source_id,
            url: format!("/e{entry_id}"),
            title: format!("Title {entry_id}"),
        };
        EnqueueRequest {
            kind,
            entry,
            source: fixtures::source(source_id),
            units,
            place: Place::Back,
            auto_start,
            skip_fillermarked: false,
        }
    }

    /// Units 1, 2 and 3 of source 1 in queue order 1, 2, 3.
    fn three(kind: Kind, auto_start: bool) -> EnqueueRequest {
        request(kind, 1, 1, vec![unit(1, 3), unit(2, 2), unit(3, 1)], auto_start)
    }

    fn ids(q: &Queue, kind: Kind) -> Vec<i64> {
        q.snapshot().of(kind).map(|d| d.unit.id).collect()
    }

    fn state(q: &Queue, kind: Kind, unit_id: i64) -> Option<DownloadState> {
        q.queued(kind, unit_id).map(|d| d.state)
    }

    fn drain(events: &mut broadcast::Receiver<DownloadEvent>) -> Vec<DownloadEvent> {
        let mut out = Vec::new();
        while let Ok(event) = events.try_recv() {
            out.push(event);
        }
        out
    }

    async fn settle(cond: impl Fn() -> bool) {
        for _ in 0..1000 {
            if cond() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("condition never held");
    }

    async fn pause_a_bit() {
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    #[tokio::test]
    async fn pause_returns_running_units_to_queue_and_start_completes_all_with_one_complete() {
        for kind in Kind::ALL {
            let h = Harness::new();
            h.runner(kind).set_delay(LONG);
            let q = h.spawn();
            let mut events = q.events();
            q.enqueue(three(kind, true)).await.unwrap();
            settle(|| h.runner(kind).in_flight() == 1).await;
            q.pause(kind);
            settle(|| q.snapshot().run(kind) == RunState::Paused && h.runner(kind).in_flight() == 0).await;
            assert!(q.snapshot().of(kind).all(|d| d.state == DownloadState::Queue));
            assert_eq!(ids(&q, kind), vec![1, 2, 3]);

            h.runner(kind).set_delay(0);
            q.start(kind);
            settle(|| ids(&q, kind).is_empty()).await;
            let events = drain(&mut events);
            let completes = events.iter().filter(|e| matches!(e, DownloadEvent::Complete { kind: k } if *k == kind)).count();
            assert_eq!(completes, 1, "one Complete for {kind:?}");
            assert_eq!(q.snapshot().run(kind), RunState::Stopped);
            assert!(events.iter().any(|e| matches!(e, DownloadEvent::Paused { kind: k } if *k == kind)));
        }
    }

    #[tokio::test]
    async fn the_lanes_are_independent() {
        let h = Harness::new();
        h.runner(Kind::Manga).set_delay(LONG);
        h.runner(Kind::Anime).set_delay(LONG);
        let q = h.spawn();
        q.enqueue(three(Kind::Manga, true)).await.unwrap();
        q.enqueue(three(Kind::Anime, true)).await.unwrap();
        settle(|| h.runner(Kind::Manga).in_flight() == 1 && h.runner(Kind::Anime).in_flight() == 1).await;
        q.pause(Kind::Anime);
        settle(|| q.snapshot().run(Kind::Anime) == RunState::Paused && h.runner(Kind::Anime).in_flight() == 0).await;
        assert_eq!(q.snapshot().run(Kind::Manga), RunState::Running);
        assert_eq!(h.runner(Kind::Manga).in_flight(), 1, "the manga lane keeps running");
        q.clear(Kind::Manga);
        settle(|| ids(&q, Kind::Manga).is_empty()).await;
        assert_eq!(ids(&q, Kind::Anime), vec![1, 2, 3], "clearing one lane leaves the other");
    }

    #[tokio::test]
    async fn reorder_puts_listed_units_first_and_a_no_op_restarts_nothing() {
        for kind in Kind::ALL {
            let h = Harness::new();
            h.runner(kind).set_delay(LONG);
            let q = h.spawn();
            q.enqueue(three(kind, false)).await.unwrap();
            assert_eq!(ids(&q, kind), vec![1, 2, 3]);

            q.reorder(kind, vec![3, 99]);
            settle(|| ids(&q, kind) == vec![3, 1, 2]).await;
            q.reorder(kind, Vec::new());
            pause_a_bit().await;
            assert_eq!(ids(&q, kind), vec![3, 1, 2], "an empty reorder never clears or moves");

            q.start(kind);
            settle(|| h.runner(kind).in_flight() == 1).await;
            assert_eq!(h.runner(kind).started(), vec![3]);
            q.reorder(kind, vec![3, 1, 2]);
            pause_a_bit().await;
            assert_eq!(h.runner(kind).started(), vec![3], "a reorder that changes nothing does not restart the run");

            q.reorder(kind, vec![1]);
            settle(|| h.runner(kind).starts_of(1) == 1 && h.runner(kind).in_flight() == 1).await;
            assert_eq!(ids(&q, kind), vec![1, 3, 2]);
            assert_eq!(state(&q, kind, 3), Some(DownloadState::Queue), "the displaced unit went back to QUEUE");
        }
    }

    #[tokio::test]
    async fn cancelling_one_unit_does_not_retry_failed_units_or_restart_running_ones() {
        for kind in Kind::ALL {
            let h = Harness::new();
            let runner = h.runner(kind);
            runner.set_delay(LONG);
            runner.set_unit_delay(1, 0);
            runner.fail_with(1, || DownloadError::NoVideo);
            let q = h.spawn();
            q.enqueue(request(kind, 1, 1, vec![unit(1, 2), unit(2, 1)], false)).await.unwrap();
            q.enqueue(request(kind, 2, 2, vec![unit(3, 1)], false)).await.unwrap();
            q.start(kind);
            settle(|| {
                state(&q, kind, 1) == Some(DownloadState::Error)
                    && state(&q, kind, 2) == Some(DownloadState::Downloading)
                    && state(&q, kind, 3) == Some(DownloadState::Downloading)
            })
            .await;

            let removed = q.cancel(kind, CancelTarget::Units(vec![3])).await.unwrap();
            assert_eq!(removed, vec![3]);
            pause_a_bit().await;
            assert_eq!(state(&q, kind, 1), Some(DownloadState::Error), "the failed unit stays failed");
            assert_eq!(runner.starts_of(1), 1, "and is not run again");
            assert_eq!(runner.starts_of(2), 1, "a running unit is not restarted by a cancel");
            assert_eq!(state(&q, kind, 2), Some(DownloadState::Downloading));
        }
    }

    #[tokio::test]
    async fn cancelling_an_entry_removes_all_its_units() {
        let h = Harness::new();
        h.runner(Kind::Manga).set_delay(LONG);
        let q = h.spawn();
        q.enqueue(three(Kind::Manga, false)).await.unwrap();
        q.enqueue(request(Kind::Manga, 2, 2, vec![unit(7, 1)], false)).await.unwrap();
        let removed = q.cancel(Kind::Manga, CancelTarget::Entry(1)).await.unwrap();
        assert_eq!(removed, vec![1, 2, 3]);
        assert_eq!(ids(&q, Kind::Manga), vec![7]);
    }

    #[tokio::test]
    async fn an_anime_unit_emits_the_full_event_set() {
        let h = Harness::new();
        h.runner(Kind::Anime).set_delay(250);
        let q = h.spawn();
        let mut events = q.events();
        q.enqueue(request(Kind::Anime, 1, 1, vec![unit(1, 1)], true)).await.unwrap();
        settle(|| ids(&q, Kind::Anime).is_empty()).await;
        let events = drain(&mut events);
        let at = |pred: &dyn Fn(&DownloadEvent) -> bool| events.iter().position(|e| pred(e)).expect("event missing");
        let started = at(&|e| matches!(e, DownloadEvent::Started { kind: Kind::Anime }));
        let running = at(&|e| matches!(e, DownloadEvent::Status { state: DownloadState::Downloading, .. }));
        let progress = at(&|e| matches!(e, DownloadEvent::Progress { percent: 50, .. }));
        let downloaded = at(&|e| matches!(e, DownloadEvent::Status { state: DownloadState::Downloaded, .. }));
        let finished = at(&|e| matches!(e, DownloadEvent::Finished { unit_id: 1, entry_id: 1, .. }));
        let complete = at(&|e| matches!(e, DownloadEvent::Complete { kind: Kind::Anime }));
        assert!(started < running && running < progress && progress < downloaded && downloaded < finished && finished < complete);
        assert!(events.iter().all(|e| e.kind() == Kind::Anime), "nothing leaks to the other lane");
    }

    #[tokio::test]
    async fn a_finished_unit_is_recorded_in_the_index_and_leaves_the_saved_queue() {
        for kind in Kind::ALL {
            let h = Harness::new();
            let q = h.spawn();
            q.enqueue(request(kind, 1, 1, vec![unit(1, 1)], true)).await.unwrap();
            settle(|| ids(&q, kind).is_empty()).await;
            assert!(h.index.is_downloaded(kind, 1, "Title 1", "Unit 1", None));
            assert!(h.store.load(kind).unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn the_maximum_downloads_setting_limits_the_sources_of_each_lane() {
        for kind in Kind::ALL {
            let h = Harness::new();
            h.runner(kind).set_delay(LONG);
            h.max.store(2, Ordering::SeqCst);
            let q = h.spawn();
            for source in 1..=3 {
                q.enqueue(request(kind, source, source, vec![unit(source, 1)], false)).await.unwrap();
            }
            q.start(kind);
            settle(|| h.runner(kind).in_flight() == 2).await;
            pause_a_bit().await;
            assert_eq!(h.runner(kind).peak(), 2, "two at once, never three");

            h.max.store(3, Ordering::SeqCst);
            q.sources_changed();
            settle(|| h.runner(kind).in_flight() == 3).await;
        }
    }

    #[tokio::test]
    async fn one_source_runs_one_unit_at_a_time_in_each_lane() {
        for kind in Kind::ALL {
            let h = Harness::new();
            h.runner(kind).set_delay(LONG);
            let q = h.spawn();
            q.enqueue(three(kind, true)).await.unwrap();
            settle(|| h.runner(kind).in_flight() == 1).await;
            pause_a_bit().await;
            assert_eq!(h.runner(kind).peak(), 1);
        }
    }

    #[tokio::test]
    async fn a_hand_pick_keeps_fillermarked_units_and_the_automatic_download_skips_them() {
        let h = Harness::new();
        let q = h.spawn();
        let mut filler = unit(5, 9);
        filler.fillermark = true;
        let mut auto = request(Kind::Anime, 1, 1, vec![unit(1, 1), filler.clone()], false);
        auto.skip_fillermarked = true;
        assert_eq!(q.enqueue(auto).await.unwrap().added, vec![1]);
        let pick = request(Kind::Anime, 1, 1, vec![filler], false);
        assert_eq!(q.enqueue(pick).await.unwrap().added, vec![5], "the hand pick is queued");
    }

    #[tokio::test]
    async fn enqueue_skips_downloaded_and_queued_units_and_refuses_a_local_source() {
        let h = Harness::new();
        let q = h.spawn();
        h.index.add_unit(Kind::Manga, 1, "Source 1 (en)", "Title 1", "Unit 1");
        let first = q.enqueue(three(Kind::Manga, false)).await.unwrap();
        assert_eq!(first.added, vec![2, 3], "unit 1 is on disk");
        assert!(!first.started);
        let again = q.enqueue(three(Kind::Manga, false)).await.unwrap();
        assert!(again.added.is_empty() && again.refused == Some(Refusal::NothingToDo));

        let mut local = request(Kind::Manga, 9, 9, vec![unit(90, 1)], true);
        local.source.availability = Availability::Local;
        let refused = q.enqueue(local).await.unwrap();
        assert_eq!(refused.refused, Some(Refusal::LocalSource));
        assert!(ids(&q, Kind::Manga).iter().all(|id| *id != 90));
    }

    #[tokio::test]
    async fn auto_start_warns_over_the_bulk_threshold_and_only_starts_an_empty_lane() {
        for kind in Kind::ALL {
            let h = Harness::new();
            h.runner(kind).set_delay(LONG);
            let q = h.spawn();
            let mut events = q.events();
            let big: Vec<UnitRef> = (1..=16).map(|i| unit(i, i)).collect();
            let first = q.enqueue(request(kind, 1, 1, big, true)).await.unwrap();
            assert!(first.started, "the lane was empty");
            let warning = first.warning.expect("16 units of one source warn");
            assert_eq!(warning.timeout_secs, 30);
            assert!(drain(&mut events).iter().any(|e| matches!(e, DownloadEvent::Warning { kind: k, .. } if *k == kind)));

            let second = q.enqueue(request(kind, 2, 2, vec![unit(100, 1)], true)).await.unwrap();
            assert!(!second.started && second.warning.is_none(), "the lane was not empty");
            assert!(!second.continue_toast, "the lane runs, so no Continue is offered");
        }
    }

    #[tokio::test]
    async fn units_added_to_a_stopped_lane_ask_for_continue() {
        let h = Harness::new();
        let q = h.spawn();
        q.enqueue(request(Kind::Manga, 1, 1, vec![unit(1, 1)], false)).await.unwrap();
        let second = q.enqueue(request(Kind::Manga, 1, 1, vec![unit(2, 2)], true)).await.unwrap();
        assert!(second.continue_toast && !second.started);
    }

    #[tokio::test]
    async fn start_now_refuses_a_unit_on_disk_and_otherwise_runs_it_first() {
        for kind in Kind::ALL {
            let h = Harness::new();
            h.runner(kind).set_delay(LONG);
            let q = h.spawn();
            q.enqueue(request(kind, 1, 1, vec![unit(3, 1)], false)).await.unwrap();

            h.index.add_unit(kind, 1, "Source 1 (en)", "Title 1", "Unit 9");
            let on_disk = q.start_now(request(kind, 1, 1, vec![unit(9, 1)], false)).await;
            assert!(matches!(on_disk, Err(DownloadError::AlreadyDownloaded)));
            assert_eq!(ids(&q, kind), vec![3]);

            q.start_now(request(kind, 1, 1, vec![unit(2, 1)], false)).await.unwrap();
            settle(|| h.runner(kind).in_flight() == 1).await;
            assert_eq!(ids(&q, kind), vec![2, 3]);
            assert_eq!(h.runner(kind).started(), vec![2]);
        }
    }

    #[tokio::test]
    async fn start_now_retries_the_failed_unit_it_names_and_no_other() {
        let h = Harness::new();
        let runner = h.runner(Kind::Anime);
        runner.fail_with(1, || DownloadError::NoVideo);
        runner.fail_with(2, || DownloadError::NoVideo);
        let q = h.spawn();
        q.enqueue(request(Kind::Anime, 1, 1, vec![unit(1, 2), unit(2, 1)], true)).await.unwrap();
        settle(|| {
            state(&q, Kind::Anime, 1) == Some(DownloadState::Error) && state(&q, Kind::Anime, 2) == Some(DownloadState::Error)
        })
        .await;
        runner.fail.lock().unwrap().remove(&1);
        q.start_now(request(Kind::Anime, 1, 1, vec![unit(1, 2)], false)).await.unwrap();
        settle(|| state(&q, Kind::Anime, 1).is_none()).await;
        pause_a_bit().await;
        assert_eq!(runner.starts_of(2), 1, "the other failed unit was not retried");
        assert_eq!(state(&q, Kind::Anime, 2), Some(DownloadState::Error));
    }

    #[tokio::test]
    async fn clear_empties_the_lane_and_dismisses_without_announcing_an_end() {
        let h = Harness::new();
        h.runner(Kind::Manga).set_delay(LONG);
        let q = h.spawn();
        let mut events = q.events();
        q.enqueue(three(Kind::Manga, true)).await.unwrap();
        settle(|| h.runner(Kind::Manga).in_flight() == 1).await;
        q.clear(Kind::Manga);
        settle(|| ids(&q, Kind::Manga).is_empty() && h.runner(Kind::Manga).in_flight() == 0).await;
        assert_eq!(q.snapshot().run(Kind::Manga), RunState::Stopped);
        let events = drain(&mut events);
        assert!(events.iter().any(|e| matches!(e, DownloadEvent::Dismiss { kind: Kind::Manga })));
        assert!(!events.iter().any(|e| matches!(e, DownloadEvent::Complete { .. })), "a clear is not a finished download");
    }

    #[tokio::test]
    async fn two_hundred_commands_are_all_applied() {
        let h = Harness::new();
        let q = h.spawn();
        q.enqueue(three(Kind::Manga, false)).await.unwrap();
        for i in 0..200 {
            q.reorder(Kind::Manga, if i % 2 == 0 { vec![3, 2, 1] } else { vec![1, 2, 3] });
        }
        q.reorder(Kind::Manga, vec![2, 3, 1]);
        settle(|| ids(&q, Kind::Manga) == vec![2, 3, 1]).await;
    }

    #[tokio::test]
    async fn holding_an_entry_stops_its_run_and_dropping_the_hold_resumes_it() {
        for kind in Kind::ALL {
            let h = Harness::new();
            h.runner(kind).set_delay(LONG);
            let q = h.spawn();
            q.enqueue(three(kind, true)).await.unwrap();
            settle(|| h.runner(kind).in_flight() == 1).await;

            let other = q.hold_entry(kind, 9).await.unwrap();
            drop(other);
            assert_eq!(h.runner(kind).in_flight(), 1, "an entry with no queued unit holds nothing");

            let hold = q.hold_entry(kind, 1).await.unwrap();
            settle(|| h.runner(kind).in_flight() == 0).await;
            assert_eq!(state(&q, kind, 1), Some(DownloadState::Queue));
            pause_a_bit().await;
            assert_eq!(h.runner(kind).started(), vec![1], "nothing starts while the lane is held");
            drop(hold);
            settle(|| h.runner(kind).in_flight() == 1).await;
            assert_eq!(h.runner(kind).started(), vec![1, 1]);
        }
    }

    #[tokio::test]
    async fn a_low_disk_marks_the_unit_failed_without_running_it() {
        let mut h = Harness::new();
        h.free = low;
        let q = h.spawn();
        let mut events = q.events();
        q.enqueue(request(Kind::Anime, 1, 1, vec![unit(1, 1)], true)).await.unwrap();
        settle(|| state(&q, Kind::Anime, 1) == Some(DownloadState::Error)).await;
        assert!(h.runner(Kind::Anime).started().is_empty());
        let events = drain(&mut events);
        assert!(events.iter().any(|e| matches!(e, DownloadEvent::Error { kind: Kind::Anime, unit_id: Some(1), .. })));
    }

    #[tokio::test]
    async fn a_failing_unit_emits_one_error_and_the_lane_ends() {
        let h = Harness::new();
        let runner = h.runner(Kind::Anime);
        runner.fail_with(1, || DownloadError::Source(SourceError::Transport("reset".into())));
        let q = h.spawn();
        let mut events = q.events();
        q.enqueue(request(Kind::Anime, 1, 1, vec![unit(1, 1)], true)).await.unwrap();
        settle(|| state(&q, Kind::Anime, 1) == Some(DownloadState::Error) && q.snapshot().run(Kind::Anime) == RunState::Stopped).await;
        let errors = drain(&mut events).into_iter().filter(|e| matches!(e, DownloadEvent::Error { .. })).count();
        assert_eq!(errors, 1);
    }

    #[tokio::test]
    async fn an_invalid_location_stops_the_lane_for_both_kinds() {
        for kind in Kind::ALL {
            let h = Harness::new();
            let runner = h.runner(kind);
            runner.set_delay(LONG);
            runner.set_unit_delay(1, 0);
            runner.fail_with(1, || DownloadError::InvalidLocation("/nowhere".to_owned()));
            let q = h.spawn();
            q.enqueue(request(kind, 1, 1, vec![unit(1, 1)], false)).await.unwrap();
            q.enqueue(request(kind, 2, 2, vec![unit(2, 1)], false)).await.unwrap();
            q.start(kind);
            settle(|| q.snapshot().run(kind) == RunState::Stopped).await;
            assert_eq!(state(&q, kind, 1), Some(DownloadState::Error));
            assert_eq!(state(&q, kind, 2), Some(DownloadState::Error), "the other running unit stopped with the lane");
            settle(|| runner.in_flight() == 0).await;
        }
    }

    #[tokio::test]
    async fn the_network_stops_both_lanes_and_its_return_resumes_them() {
        let h = Harness::new();
        for kind in Kind::ALL {
            h.runner(kind).set_delay(LONG);
        }
        let q = h.spawn();
        let mut events = q.events();
        for kind in Kind::ALL {
            q.enqueue(three(kind, true)).await.unwrap();
        }
        settle(|| Kind::ALL.iter().all(|k| h.runner(*k).in_flight() == 1)).await;

        h.connectivity.send(OFFLINE).unwrap();
        settle(|| Kind::ALL.iter().all(|k| q.snapshot().run(*k) == RunState::Stopped)).await;
        for kind in Kind::ALL {
            assert_eq!(state(&q, kind, 1), Some(DownloadState::Error), "the running unit is in ERROR");
            assert_eq!(state(&q, kind, 2), Some(DownloadState::Queue));
        }
        let events = drain(&mut events);
        for kind in Kind::ALL {
            let warnings = events.iter().filter(|e| matches!(e, DownloadEvent::Warning { kind: k, .. } if *k == kind)).count();
            assert_eq!(warnings, 1, "one warning for {kind:?}");
        }

        h.connectivity.send(ONLINE).unwrap();
        settle(|| Kind::ALL.iter().all(|k| q.snapshot().run(*k) == RunState::Running && h.runner(*k).in_flight() == 1)).await;
        for kind in Kind::ALL {
            assert_eq!(h.runner(kind).started(), vec![1, 1], "the unit the network stopped runs again");
        }
    }

    #[tokio::test]
    async fn a_lane_the_user_paused_stays_paused_when_the_network_returns() {
        let h = Harness::new();
        for kind in Kind::ALL {
            h.runner(kind).set_delay(LONG);
        }
        let q = h.spawn();
        for kind in Kind::ALL {
            q.enqueue(three(kind, true)).await.unwrap();
        }
        settle(|| Kind::ALL.iter().all(|k| h.runner(*k).in_flight() == 1)).await;
        h.connectivity.send(OFFLINE).unwrap();
        settle(|| Kind::ALL.iter().all(|k| q.snapshot().run(*k) == RunState::Stopped)).await;

        q.pause(Kind::Anime);
        settle(|| q.snapshot().run(Kind::Anime) == RunState::Paused).await;
        h.connectivity.send(ONLINE).unwrap();
        settle(|| q.snapshot().run(Kind::Manga) == RunState::Running && h.runner(Kind::Manga).in_flight() == 1).await;
        pause_a_bit().await;
        assert_eq!(q.snapshot().run(Kind::Anime), RunState::Paused);
        assert_eq!(h.runner(Kind::Anime).in_flight(), 0);
    }

    #[tokio::test]
    async fn a_failure_that_is_not_the_network_is_not_retried_when_the_network_returns() {
        let h = Harness::new();
        let runner = h.runner(Kind::Manga);
        runner.set_delay(LONG);
        runner.set_unit_delay(1, 0);
        runner.fail_with(1, || DownloadError::NoVideo);
        let q = h.spawn();
        q.enqueue(request(Kind::Manga, 1, 1, vec![unit(1, 2), unit(2, 1)], false)).await.unwrap();
        q.enqueue(request(Kind::Manga, 2, 2, vec![unit(3, 1)], false)).await.unwrap();
        q.start(Kind::Manga);
        settle(|| state(&q, Kind::Manga, 1) == Some(DownloadState::Error) && runner.in_flight() == 2).await;

        h.connectivity.send(OFFLINE).unwrap();
        settle(|| q.snapshot().run(Kind::Manga) == RunState::Stopped).await;
        h.connectivity.send(ONLINE).unwrap();
        settle(|| q.snapshot().run(Kind::Manga) == RunState::Running && runner.in_flight() == 2).await;
        assert_eq!(runner.starts_of(1), 1, "unit 1 failed for another reason and stays failed");
        assert_eq!(state(&q, Kind::Manga, 1), Some(DownloadState::Error));
    }

    #[tokio::test]
    async fn a_restored_unit_whose_source_is_missing_waits_and_runs_when_the_source_appears() {
        for kind in Kind::ALL {
            let h = Harness::new();
            let (_, unit_id) = h.seed(kind, 7);
            let q = h.spawn();
            settle(|| !ids(&q, kind).is_empty()).await;
            let queued = q.queued(kind, unit_id).unwrap();
            assert!(queued.waiting, "the source is not installed");
            assert_eq!(queued.source.availability, Availability::Stub);
            assert_eq!(queued.state, DownloadState::Queue);

            // Another save keeps the waiting row.
            q.enqueue(request(kind, 1, 1, vec![unit(900, 1)], false)).await.unwrap();
            assert!(h.store.load(kind).unwrap().iter().any(|row| row.unit_id == unit_id));

            q.start(kind);
            settle(|| h.runner(kind).started() == vec![900]).await;
            assert_eq!(state(&q, kind, unit_id), Some(DownloadState::Queue), "a waiting unit is skipped, not failed");

            h.directory.add(kind, fixtures::source(7));
            q.sources_changed();
            settle(|| ids(&q, kind).is_empty()).await;
            assert!(h.runner(kind).started().contains(&unit_id));
        }
    }

    #[tokio::test]
    async fn a_restored_unit_keeps_its_staging_folder_and_a_stray_one_is_swept() {
        for kind in Kind::ALL {
            let h = Harness::new();
            h.seed(kind, 1);
            let source = fixtures::source(1);
            let entry_dir = h
                .root
                .path()
                .join(kind.as_str())
                .join(Layout::source_dir_name(&source.name, &source.lang))
                .join(entry_dir_name("Seeded"));
            let names = staging_dir_names(kind, UnitName::new("Unit", None));
            let kept = entry_dir.join(&names[0]);
            let stray = entry_dir.join("Other_tmp");
            std::fs::create_dir_all(&kept).unwrap();
            std::fs::create_dir_all(&stray).unwrap();

            let q = h.spawn();
            settle(|| !ids(&q, kind).is_empty()).await;
            assert!(kept.exists(), "the restored unit's staging folder survives the start");
            assert!(!stray.exists(), "an unrelated staging folder is swept");
        }
    }
}
