//! Scheduling contract for the periodic jobs (T-11a-004 to T-11a-007): the job spec, the constraint check, the
//! [`JobScheduler`] trait the migration steps call, and an in-process scheduler that follows the WorkManager rules
//! the Android jobs relied on. Job bodies are not here: a [`JobRunner`] runs them.
//!
//! The in-process scheduler keeps its jobs in memory. A periodic job is first due one interval after it was
//! registered (or after its last run), and may run in the last `flex` of that interval. Runs whose constraints are
//! unmet are skipped, not queued, and checked again when [`PowerNetworkState`] reports a change or after
//! [`RECHECK`]. A job that missed its window by more than one interval runs once, [`STARTUP_DELAY`] after the process
//! started, and then continues from that run. Nothing runs while a restore is in progress.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use kuro_platform::power_network::{Conditions, PowerNetworkState};

/// The last part of an interval in which a job may run.
pub const FLEX: Duration = Duration::from_secs(10 * 60);
/// How long after the process start an overdue job waits, so the first window is usable.
pub const STARTUP_DELAY: Duration = Duration::from_secs(60);
/// A deferred job is checked again at least this often, whatever the platform reports.
pub const RECHECK: Duration = Duration::from_secs(15 * 60);
/// The longest wait after a failed run.
pub const MAX_BACKOFF: Duration = Duration::from_secs(5 * 60 * 60);
/// The base delay of the retry backoff.
pub const BACKOFF_BASE: Duration = Duration::from_secs(10 * 60);

/// What a job needs from the device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Constraints {
    /// A connection that reaches the network (`NetworkType.CONNECTED`).
    pub requires_connected: bool,
    /// A connection that is not metered (`NetworkType.UNMETERED`).
    pub requires_unmetered: bool,
    /// External power.
    pub requires_ac: bool,
    /// The battery is not at its low warning level.
    pub battery_not_low: bool,
}

/// One condition a job is waiting for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unmet {
    Connection,
    Unmetered,
    Power,
    Battery,
}

impl Constraints {
    pub const NONE: Self =
        Self { requires_connected: false, requires_unmetered: false, requires_ac: false, battery_not_low: false };

    /// The constraints that do not hold under `conditions`, in a fixed order.
    pub fn unmet(&self, conditions: &Conditions) -> Vec<Unmet> {
        let mut unmet = Vec::new();
        if self.requires_connected && !conditions.connected {
            unmet.push(Unmet::Connection);
        }
        if self.requires_unmetered && (!conditions.connected || conditions.metered) {
            unmet.push(Unmet::Unmetered);
        }
        if self.requires_ac && !conditions.on_ac {
            unmet.push(Unmet::Power);
        }
        if self.battery_not_low && conditions.battery_low {
            unmet.push(Unmet::Battery);
        }
        unmet
    }
}

/// How the wait after a failed run grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backoff {
    /// `base * attempt`.
    Linear(Duration),
    /// `base * 2^(attempt - 1)`.
    Exponential(Duration),
}

impl Backoff {
    /// The wait after the `attempt`-th failure in a row (the first failure is attempt 1), at most [`MAX_BACKOFF`].
    pub fn delay(&self, attempt: u32) -> Duration {
        let attempt = attempt.max(1);
        let delay = match *self {
            Self::Linear(base) => base.saturating_mul(attempt),
            Self::Exponential(base) => base.saturating_mul(1u32.checked_shl(attempt - 1).unwrap_or(u32::MAX)),
        };
        delay.min(MAX_BACKOFF)
    }
}

/// A unique periodic job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeriodicSpec {
    pub name: &'static str,
    pub interval: Duration,
    pub flex: Duration,
    pub constraints: Constraints,
    pub backoff: Backoff,
}

/// What the migration steps and the settings pages call. Both calls are idempotent and do not block.
pub trait JobScheduler: Send + Sync {
    /// Registers the job, or replaces the stored definition with the same name. A replaced job keeps its last run and
    /// a run in progress (`ExistingPeriodicWorkPolicy.UPDATE`).
    fn enqueue_unique_periodic(&self, spec: PeriodicSpec);
    /// Removes the job. Nothing happens when it is absent.
    fn cancel_unique(&self, name: &str);
}

/// Runs the body of a job. `true` is success; `false` starts the retry backoff.
#[async_trait]
pub trait JobRunner: Send + Sync {
    async fn run(&self, name: &'static str) -> bool;
}

/// Milliseconds since the Unix epoch.
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// The wall clock.
pub fn system_clock() -> Clock {
    Arc::new(|| {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
    })
}

/// A clock that starts at `start_wall_ms` and then follows tokio's time, so a test with paused time moves it with
/// `sleep` and `advance`.
pub fn tokio_clock(start_wall_ms: i64) -> Clock {
    let start = tokio::time::Instant::now();
    Arc::new(move || start_wall_ms + i64::try_from(start.elapsed().as_millis()).unwrap_or(i64::MAX))
}

/// What the scheduler decided for one job at one moment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Not in its window yet; check again at this time (epoch milliseconds).
    Wait { at_ms: i64 },
    /// In its window, but a condition does not hold. Checked again when the conditions change.
    Defer(Vec<Unmet>),
    /// In its window, but a restore is in progress.
    Restoring,
    Run,
}

/// The state of one registered job.
#[derive(Clone, Debug)]
struct Job {
    spec: PeriodicSpec,
    enqueued_ms: i64,
    last_run_ms: Option<i64>,
    running: bool,
    failures: u32,
    retry_at_ms: Option<i64>,
}

fn ms(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

impl Job {
    fn new(spec: PeriodicSpec, now_ms: i64) -> Self {
        Self { spec, enqueued_ms: now_ms, last_run_ms: None, running: false, failures: 0, retry_at_ms: None }
    }

    /// When the job is next due: one interval after its last run, or after it was registered.
    fn next_due_ms(&self) -> i64 {
        self.last_run_ms.unwrap_or(self.enqueued_ms).saturating_add(ms(self.spec.interval))
    }

    fn decide(&self, now_ms: i64, started_ms: i64, conditions: &Conditions, restore_in_progress: bool) -> Decision {
        let due = self.next_due_ms();
        let overdue = now_ms > due.saturating_add(ms(self.spec.interval));
        let mut eligible = if overdue {
            started_ms.saturating_add(ms(STARTUP_DELAY))
        } else {
            due.saturating_sub(ms(self.spec.flex)).max(self.last_run_ms.unwrap_or(self.enqueued_ms))
        };
        if let Some(retry) = self.retry_at_ms {
            eligible = eligible.max(retry);
        }
        if now_ms < eligible {
            return Decision::Wait { at_ms: eligible };
        }
        if restore_in_progress {
            return Decision::Restoring;
        }
        let unmet = self.spec.constraints.unmet(conditions);
        if unmet.is_empty() { Decision::Run } else { Decision::Defer(unmet) }
    }
}

/// A job as the worker info page lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobInfo {
    pub name: &'static str,
    pub spec: PeriodicSpec,
    pub last_run_ms: Option<i64>,
    pub next_due_ms: i64,
    pub running: bool,
}

/// The jobs due now and when to look again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tick {
    /// Jobs to start. Each is marked running until [`InProcessScheduler::finish`].
    pub run: Vec<&'static str>,
    /// The earliest time a waiting job enters its window, when there is one.
    pub wake_at_ms: Option<i64>,
}

/// The default [`JobScheduler`]: keeps the jobs in memory and decides, from the clock and the device state, which
/// ones run.
pub struct InProcessScheduler {
    jobs: Mutex<BTreeMap<&'static str, Job>>,
    clock: Clock,
}

impl Default for InProcessScheduler {
    fn default() -> Self {
        Self::with_clock(system_clock())
    }
}

impl InProcessScheduler {
    pub fn with_clock(clock: Clock) -> Self {
        Self { jobs: Mutex::new(BTreeMap::new()), clock }
    }

    fn jobs(&self) -> std::sync::MutexGuard<'_, BTreeMap<&'static str, Job>> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The registered jobs by name.
    pub fn snapshot(&self) -> Vec<JobInfo> {
        self.jobs()
            .values()
            .map(|job| JobInfo {
                name: job.spec.name,
                spec: job.spec.clone(),
                last_run_ms: job.last_run_ms,
                next_due_ms: job.next_due_ms(),
                running: job.running,
            })
            .collect()
    }

    /// The stored definition of `name`.
    pub fn spec(&self, name: &str) -> Option<PeriodicSpec> {
        self.jobs().get(name).map(|job| job.spec.clone())
    }

    /// Records when `name` last ran, from the state file written by an earlier run of the app. A job that missed its
    /// window by more than one interval then runs once after the start.
    pub fn set_last_run(&self, name: &str, last_run_ms: i64) {
        if let Some(job) = self.jobs().get_mut(name) {
            job.last_run_ms = Some(last_run_ms);
        }
    }

    /// The decision for `name` now.
    pub fn decide(
        &self,
        name: &str,
        started_ms: i64,
        conditions: &Conditions,
        restore_in_progress: bool,
    ) -> Option<Decision> {
        let now = (self.clock)();
        self.jobs().get(name).map(|job| job.decide(now, started_ms, conditions, restore_in_progress))
    }

    /// Decides for every job that is not running. The jobs to start are marked running, so a second tick does not
    /// start them again before [`InProcessScheduler::finish`].
    pub fn tick(&self, started_ms: i64, conditions: &Conditions, restore_in_progress: bool) -> Tick {
        let now = (self.clock)();
        let mut tick = Tick::default();
        for job in self.jobs().values_mut().filter(|job| !job.running) {
            match job.decide(now, started_ms, conditions, restore_in_progress) {
                Decision::Run => {
                    job.running = true;
                    tick.run.push(job.spec.name);
                }
                Decision::Wait { at_ms } => {
                    tick.wake_at_ms = Some(tick.wake_at_ms.map_or(at_ms, |earlier| earlier.min(at_ms)));
                }
                Decision::Defer(_) | Decision::Restoring => {}
            }
        }
        tick
    }

    /// Ends a run. Success moves the schedule on from now. A failure waits for the job's backoff before the next try.
    /// A job that was cancelled while it ran is simply gone.
    pub fn finish(&self, name: &str, success: bool) {
        let now = (self.clock)();
        if let Some(job) = self.jobs().get_mut(name) {
            job.running = false;
            if success {
                job.last_run_ms = Some(now);
                job.failures = 0;
                job.retry_at_ms = None;
            } else {
                job.failures += 1;
                job.retry_at_ms = Some(now.saturating_add(ms(job.spec.backoff.delay(job.failures))));
            }
        }
    }

    /// Drives the jobs until the task is dropped or aborted. It looks at the jobs when the process starts, when
    /// `state` reports a change, when a waiting job enters its window, and at least every [`RECHECK`]. Jobs run on
    /// their own tasks, never on the caller's thread.
    pub async fn run_loop(
        self: Arc<Self>,
        state: Arc<PowerNetworkState>,
        runner: Arc<dyn JobRunner>,
        restore_in_progress: Arc<AtomicBool>,
    ) {
        let started = (self.clock)();
        let mut changes = state.subscribe();
        loop {
            let tick = self.tick(started, &state.conditions(), restore_in_progress.load(Ordering::Relaxed));
            for name in tick.run {
                let (scheduler, runner) = (Arc::clone(&self), Arc::clone(&runner));
                tokio::spawn(async move {
                    let success = runner.run(name).await;
                    scheduler.finish(name, success);
                });
            }
            let wait = tick
                .wake_at_ms
                .map_or(RECHECK, |at| Duration::from_millis(u64::try_from(at - (self.clock)()).unwrap_or(0)))
                .min(RECHECK);
            tokio::select! {
                changed = changes.changed() => if changed.is_err() { return },
                () = tokio::time::sleep(wait) => {}
            }
        }
    }
}

impl JobScheduler for InProcessScheduler {
    fn enqueue_unique_periodic(&self, spec: PeriodicSpec) {
        let now = (self.clock)();
        let mut jobs = self.jobs();
        match jobs.get_mut(spec.name) {
            Some(job) => job.spec = spec,
            None => {
                jobs.insert(spec.name, Job::new(spec, now));
            }
        }
    }

    fn cancel_unique(&self, name: &str) {
        self.jobs().remove(name);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicI64;

    use super::*;

    const HOUR: i64 = 3_600_000;
    const MINUTE: i64 = 60_000;

    fn spec(name: &'static str, hours: u64, constraints: Constraints) -> PeriodicSpec {
        PeriodicSpec {
            name,
            interval: Duration::from_secs(hours * 3600),
            flex: FLEX,
            constraints,
            backoff: Backoff::Linear(BACKOFF_BASE),
        }
    }

    /// A scheduler over a clock the test moves by hand.
    fn scheduler() -> (InProcessScheduler, Arc<AtomicI64>) {
        let now = Arc::new(AtomicI64::new(1_000_000_000));
        let reader = Arc::clone(&now);
        (InProcessScheduler::with_clock(Arc::new(move || reader.load(Ordering::SeqCst))), now)
    }

    fn at(now: &AtomicI64, delta: i64) {
        now.fetch_add(delta, Ordering::SeqCst);
    }

    const ONLINE: Conditions = Conditions::DESKTOP;
    const OFFLINE: Conditions = Conditions { connected: false, metered: true, on_ac: true, battery_low: false };

    #[test]
    fn constraints_name_each_unmet_condition() {
        let all = Constraints { requires_connected: true, requires_unmetered: true, requires_ac: true, battery_not_low: true };
        let bad = Conditions { connected: false, metered: true, on_ac: false, battery_low: true };
        assert_eq!(all.unmet(&bad), vec![Unmet::Connection, Unmet::Unmetered, Unmet::Power, Unmet::Battery]);
        assert!(all.unmet(&Conditions::DESKTOP).is_empty());
        assert!(Constraints::NONE.unmet(&bad).is_empty());
        let metered = Conditions { metered: true, ..Conditions::DESKTOP };
        let unmetered_only = Constraints { requires_unmetered: true, ..Constraints::NONE };
        assert_eq!(unmetered_only.unmet(&metered), vec![Unmet::Unmetered]);
        let connected_only = Constraints { requires_connected: true, ..Constraints::NONE };
        assert!(connected_only.unmet(&metered).is_empty(), "a metered connection is still a connection");
    }

    #[test]
    fn backoff_grows_by_kind_and_stops_at_five_hours() {
        let linear = Backoff::Linear(BACKOFF_BASE);
        assert_eq!([1, 2, 3].map(|n| linear.delay(n).as_secs()), [600, 1200, 1800]);
        let exponential = Backoff::Exponential(BACKOFF_BASE);
        assert_eq!([1, 2, 3, 4].map(|n| exponential.delay(n).as_secs()), [600, 1200, 2400, 4800]);
        assert_eq!(exponential.delay(40), MAX_BACKOFF);
        assert_eq!(linear.delay(1000), MAX_BACKOFF);
    }

    #[test]
    fn enqueueing_twice_keeps_one_job_with_the_latest_spec() {
        let (sched, _) = scheduler();
        sched.enqueue_unique_periodic(spec("LibraryUpdate", 6, Constraints::NONE));
        sched.enqueue_unique_periodic(spec("LibraryUpdate", 12, Constraints::NONE));
        assert_eq!(sched.snapshot().len(), 1);
        assert_eq!(sched.spec("LibraryUpdate").unwrap().interval, Duration::from_secs(12 * 3600));
    }

    #[test]
    fn replacing_a_job_keeps_its_last_run_and_cancelling_removes_only_that_job() {
        let (sched, now) = scheduler();
        sched.enqueue_unique_periodic(spec("A", 1, Constraints::NONE));
        sched.enqueue_unique_periodic(spec("B", 1, Constraints::NONE));
        at(&now, HOUR);
        assert_eq!(sched.tick(0, &ONLINE, false).run, vec!["A", "B"]);
        sched.finish("A", true);
        let ran = sched.snapshot()[0].last_run_ms;
        assert!(ran.is_some());
        sched.enqueue_unique_periodic(spec("A", 2, Constraints::NONE));
        assert_eq!(sched.snapshot()[0].last_run_ms, ran, "UPDATE keeps the last run");
        sched.cancel_unique("A");
        sched.cancel_unique("missing");
        assert_eq!(sched.snapshot().iter().map(|j| j.name).collect::<Vec<_>>(), vec!["B"]);
    }

    #[test]
    fn a_job_is_first_due_in_the_last_flex_of_its_first_interval() {
        let (sched, now) = scheduler();
        sched.enqueue_unique_periodic(spec("A", 1, Constraints::NONE));
        let start = now.load(Ordering::SeqCst);
        let wait = sched.decide("A", start, &ONLINE, false).unwrap();
        assert_eq!(wait, Decision::Wait { at_ms: start + HOUR - 10 * MINUTE });
        at(&now, HOUR - 10 * MINUTE - 1);
        assert!(matches!(sched.decide("A", start, &ONLINE, false), Some(Decision::Wait { .. })));
        at(&now, 1);
        assert_eq!(sched.decide("A", start, &ONLINE, false), Some(Decision::Run));
    }

    #[test]
    fn a_job_with_unmet_constraints_is_deferred_not_dropped() {
        let (sched, now) = scheduler();
        let needs_network = Constraints { requires_connected: true, ..Constraints::NONE };
        sched.enqueue_unique_periodic(spec("A", 1, needs_network));
        let start = now.load(Ordering::SeqCst);
        at(&now, HOUR);
        assert_eq!(sched.tick(start, &OFFLINE, false).run, Vec::<&str>::new());
        assert_eq!(sched.decide("A", start, &OFFLINE, false), Some(Decision::Defer(vec![Unmet::Connection])));
        assert_eq!(sched.tick(start, &ONLINE, false).run, vec!["A"], "it runs as soon as the condition holds");
    }

    #[test]
    fn a_running_job_is_not_started_twice_and_a_finished_one_moves_on_by_an_interval() {
        let (sched, now) = scheduler();
        sched.enqueue_unique_periodic(spec("A", 1, Constraints::NONE));
        let start = now.load(Ordering::SeqCst);
        at(&now, HOUR);
        assert_eq!(sched.tick(start, &ONLINE, false).run, vec!["A"]);
        assert!(sched.tick(start, &ONLINE, false).run.is_empty());
        sched.finish("A", true);
        let tick = sched.tick(start, &ONLINE, false);
        assert!(tick.run.is_empty());
        assert_eq!(tick.wake_at_ms, Some(now.load(Ordering::SeqCst) + HOUR - 10 * MINUTE));
    }

    #[test]
    fn a_failed_run_waits_for_the_backoff() {
        let (sched, now) = scheduler();
        sched.enqueue_unique_periodic(spec("A", 1, Constraints::NONE));
        let start = now.load(Ordering::SeqCst);
        at(&now, HOUR);
        assert_eq!(sched.tick(start, &ONLINE, false).run, vec!["A"]);
        sched.finish("A", false);
        assert_eq!(sched.decide("A", start, &ONLINE, false), Some(Decision::Wait { at_ms: now.load(Ordering::SeqCst) + 10 * MINUTE }));
        at(&now, 10 * MINUTE);
        assert_eq!(sched.tick(start, &ONLINE, false).run, vec!["A"]);
        sched.finish("A", false);
        assert_eq!(sched.decide("A", start, &ONLINE, false), Some(Decision::Wait { at_ms: now.load(Ordering::SeqCst) + 20 * MINUTE }), "linear: 10, 20, 30 minutes");
    }

    #[test]
    fn a_job_that_missed_more_than_one_period_runs_once_shortly_after_start() {
        let (sched, now) = scheduler();
        sched.enqueue_unique_periodic(spec("A", 24, Constraints::NONE));
        let start = now.load(Ordering::SeqCst);
        // The app was closed for ten days; the last run was ten days before the start.
        sched.set_last_run("A", start - 10 * 24 * HOUR);
        assert_eq!(
            sched.decide("A", start, &ONLINE, false),
            Some(Decision::Wait { at_ms: start + MINUTE }),
            "it waits for the first window to be usable"
        );
        at(&now, MINUTE);
        assert_eq!(sched.tick(start, &ONLINE, false).run, vec!["A"]);
        sched.finish("A", true);
        let next = sched.snapshot()[0].next_due_ms;
        assert_eq!(next, now.load(Ordering::SeqCst) + 24 * HOUR, "missed runs are not repeated; the schedule restarts from now");
        assert!(sched.tick(start, &ONLINE, false).run.is_empty());
    }

    #[test]
    fn the_overdue_run_does_not_fire_while_a_restore_is_in_progress() {
        let (sched, now) = scheduler();
        sched.enqueue_unique_periodic(spec("A", 24, Constraints::NONE));
        let start = now.load(Ordering::SeqCst);
        sched.set_last_run("A", start - 10 * 24 * HOUR);
        at(&now, 5 * MINUTE);
        assert_eq!(sched.decide("A", start, &ONLINE, true), Some(Decision::Restoring));
        assert!(sched.tick(start, &ONLINE, true).run.is_empty());
        assert_eq!(sched.tick(start, &ONLINE, false).run, vec!["A"], "it runs once the restore is over");
    }

    #[test]
    fn a_job_one_period_late_is_not_overdue() {
        let (sched, now) = scheduler();
        sched.enqueue_unique_periodic(spec("A", 24, Constraints::NONE));
        let start = now.load(Ordering::SeqCst);
        // Last ran 30 hours ago: due 6 hours ago, late by less than one period.
        sched.set_last_run("A", start - 30 * HOUR);
        assert_eq!(sched.decide("A", start, &ONLINE, false), Some(Decision::Run), "no startup delay for a job that is merely late");
    }

    struct Counting(std::sync::atomic::AtomicUsize);

    #[async_trait]
    impl JobRunner for Counting {
        async fn run(&self, _name: &'static str) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst);
            true
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_loop_waits_for_the_network_and_runs_the_job_when_it_returns() {
        let sched = Arc::new(InProcessScheduler::with_clock(tokio_clock(1_000_000_000)));
        let needs_network = Constraints { requires_connected: true, ..Constraints::NONE };
        sched.enqueue_unique_periodic(spec("A", 1, needs_network));
        let state = PowerNetworkState::new(OFFLINE);
        let runner = Arc::new(Counting(Default::default()));
        let looping = tokio::spawn(Arc::clone(&sched).run_loop(Arc::clone(&state), runner.clone(), Arc::default()));

        // Two hours pass offline: the window opened, and the job waited.
        tokio::time::sleep(Duration::from_secs(2 * 3600)).await;
        assert_eq!(runner.0.load(Ordering::SeqCst), 0, "unmet constraints defer the job");

        state.apply(ONLINE);
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(runner.0.load(Ordering::SeqCst), 1, "the change of the watch value rechecks it");
        assert!(sched.snapshot()[0].last_run_ms.is_some());
        looping.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn the_loop_runs_a_job_without_constraints_inside_its_window() {
        let sched = Arc::new(InProcessScheduler::with_clock(tokio_clock(1_000_000_000)));
        sched.enqueue_unique_periodic(spec("A", 1, Constraints::NONE));
        let state = PowerNetworkState::new(ONLINE);
        let runner = Arc::new(Counting(Default::default()));
        let looping = tokio::spawn(Arc::clone(&sched).run_loop(state, runner.clone(), Arc::default()));
        tokio::time::sleep(Duration::from_secs(49 * 60)).await;
        assert_eq!(runner.0.load(Ordering::SeqCst), 0, "the first window opens ten minutes before the hour");
        tokio::time::sleep(Duration::from_secs(2 * 60)).await;
        assert_eq!(runner.0.load(Ordering::SeqCst), 1);
        looping.abort();
    }
}
