//! The job registry (task T-47b-044): every background job records its id, tags, state, next run and attempt count, so
//! the busy checks and the worker info page read one list. A unique name keeps one job at a time: `UniquePolicy::Keep`
//! returns the job that is enqueued or running under the name, and the check and the enqueue happen under one lock
//! (RACE-06).
//!
//! Decisions: ids are a counter, not UUIDs (the workspace has no uuid crate). The registry records state only; the owner
//! spawns the job future on the runtime and reports its start and outcome here.
#![allow(dead_code)] // the job owners are not in this lane yet; the worker info page reads the registry
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

/// The states a job moves through. `Blocked` is kept for the dump format; nothing here sets it yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Enqueued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Blocked,
}

impl JobState {
    /// The name the worker dump prints (WorkManager's constant names).
    pub fn label(self) -> &'static str {
        match self {
            Self::Enqueued => "ENQUEUED",
            Self::Running => "RUNNING",
            Self::Succeeded => "SUCCEEDED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
            Self::Blocked => "BLOCKED",
        }
    }

    /// Enqueued or running: the states a unique name is held in.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Enqueued | Self::Running)
    }
}

/// What a job reports when it ends one attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobOutcome {
    Success,
    Retry,
    Failure,
}

/// `Keep` returns the active job of the same name; `Replace` cancels it and enqueues the new one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniquePolicy {
    Keep,
    Replace,
}

/// One job as the registry records it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobInfo {
    pub id: u64,
    pub name: String,
    pub tags: Vec<String>,
    pub state: JobState,
    /// Unix milliseconds of the next run, when the job is scheduled.
    pub next_run: Option<i64>,
    /// Attempts that ended in a retry. The dump shows this plus one.
    pub attempts: u32,
}

/// The job an enqueue left under its name, and whether this call created it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enqueued {
    pub id: u64,
    pub created: bool,
}

#[derive(Default)]
struct Table {
    next_id: u64,
    jobs: Vec<JobInfo>,
}

/// The registry. Every method takes the one lock, so a busy check and an enqueue cannot interleave.
#[derive(Default)]
pub struct JobRegistry {
    table: Mutex<Table>,
}

impl JobRegistry {
    fn lock(&self) -> MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Enqueues a job under `name`. With `Keep`, an active job of that name is returned and nothing is enqueued.
    pub fn enqueue_unique(&self, name: &str, tags: &[&str], policy: UniquePolicy, next_run: Option<i64>) -> Enqueued {
        let mut table = self.lock();
        if let Some(existing) = table.jobs.iter_mut().find(|job| job.name == name && job.state.is_active()) {
            match policy {
                UniquePolicy::Keep => return Enqueued { id: existing.id, created: false },
                UniquePolicy::Replace => existing.state = JobState::Cancelled,
            }
        }
        let id = table.next_id;
        table.next_id += 1;
        let tags = tags.iter().map(|tag| (*tag).to_owned()).collect();
        table.jobs.push(JobInfo { id, name: name.to_owned(), tags, state: JobState::Enqueued, next_run, attempts: 0 });
        Enqueued { id, created: true }
    }

    /// Moves an enqueued job to running. False for an unknown or not-enqueued job.
    pub fn start(&self, id: u64) -> bool {
        let mut table = self.lock();
        match table.jobs.iter_mut().find(|job| job.id == id) {
            Some(job) if job.state == JobState::Enqueued => {
                job.state = JobState::Running;
                true
            }
            _ => false,
        }
    }

    /// Records the end of a running attempt. A retry goes back to enqueued with one more attempt.
    pub fn finish(&self, id: u64, outcome: JobOutcome) {
        let mut table = self.lock();
        let Some(job) = table.jobs.iter_mut().find(|job| job.id == id) else { return };
        if job.state != JobState::Running {
            return;
        }
        match outcome {
            JobOutcome::Success => job.state = JobState::Succeeded,
            JobOutcome::Failure => job.state = JobState::Failed,
            JobOutcome::Retry => {
                job.state = JobState::Enqueued;
                job.attempts += 1;
            }
        }
    }

    /// Cancels every active job of `name`.
    pub fn cancel_unique(&self, name: &str) {
        for job in self.lock().jobs.iter_mut().filter(|job| job.name == name && job.state.is_active()) {
            job.state = JobState::Cancelled;
        }
    }

    /// Whether a running job carries `tag` (the busy checks of the backup screens).
    pub fn is_running(&self, tag: &str) -> bool {
        self.lock().jobs.iter().any(|job| job.state == JobState::Running && job.tags.iter().any(|t| t == tag))
    }

    /// Every recorded job in the order they were enqueued.
    pub fn snapshot(&self) -> Vec<JobInfo> {
        self.lock().jobs.clone()
    }
}

/// The registry the app shares.
pub fn global() -> &'static JobRegistry {
    static REGISTRY: OnceLock<JobRegistry> = OnceLock::new();
    REGISTRY.get_or_init(JobRegistry::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_returns_the_active_job_of_the_same_name_and_enqueues_nothing_new() {
        let registry = JobRegistry::default();
        let first = registry.enqueue_unique("backup-create", &["backup"], UniquePolicy::Keep, None);
        let second = registry.enqueue_unique("backup-create", &["backup"], UniquePolicy::Keep, None);
        assert!(first.created);
        assert_eq!(second, Enqueued { id: first.id, created: false });
        assert_eq!(registry.snapshot().len(), 1);
    }

    #[test]
    fn keep_enqueues_again_once_the_earlier_job_has_finished() {
        let registry = JobRegistry::default();
        let first = registry.enqueue_unique("refresh", &[], UniquePolicy::Keep, None);
        assert!(registry.start(first.id));
        registry.finish(first.id, JobOutcome::Success);
        let again = registry.enqueue_unique("refresh", &[], UniquePolicy::Keep, None);
        assert!(again.created);
        assert_ne!(again.id, first.id);
    }

    #[test]
    fn replace_cancels_the_active_job_and_enqueues_the_new_one() {
        let registry = JobRegistry::default();
        let first = registry.enqueue_unique("refresh", &[], UniquePolicy::Keep, None);
        let replaced = registry.enqueue_unique("refresh", &[], UniquePolicy::Replace, Some(5));
        assert!(replaced.created);
        let jobs = registry.snapshot();
        assert_eq!(jobs[0].state, JobState::Cancelled);
        assert_eq!(jobs[0].id, first.id);
        assert_eq!(jobs[1].state, JobState::Enqueued);
        assert_eq!(jobs[1].next_run, Some(5));
    }

    #[test]
    fn a_job_is_busy_only_while_running_and_only_under_its_tag() {
        let registry = JobRegistry::default();
        let job = registry.enqueue_unique("backup-restore", &["backup:restore"], UniquePolicy::Keep, None);
        assert!(!registry.is_running("backup:restore"), "enqueued is not running");
        assert!(registry.start(job.id));
        assert!(registry.is_running("backup:restore"));
        assert!(!registry.is_running("backup:create"));
        registry.finish(job.id, JobOutcome::Success);
        assert!(!registry.is_running("backup:restore"));
    }

    #[test]
    fn a_retry_goes_back_to_enqueued_with_one_more_attempt() {
        let registry = JobRegistry::default();
        let job = registry.enqueue_unique("update", &[], UniquePolicy::Keep, None);
        assert!(registry.start(job.id));
        registry.finish(job.id, JobOutcome::Retry);
        let info = &registry.snapshot()[0];
        assert_eq!((info.state, info.attempts), (JobState::Enqueued, 1));
    }

    #[test]
    fn only_a_running_job_takes_an_outcome() {
        let registry = JobRegistry::default();
        let job = registry.enqueue_unique("update", &[], UniquePolicy::Keep, None);
        registry.finish(job.id, JobOutcome::Failure);
        assert_eq!(registry.snapshot()[0].state, JobState::Enqueued, "an enqueued job ignores an outcome");
        assert!(!registry.start(99), "an unknown id does not start");
    }

    #[test]
    fn cancel_unique_cancels_only_the_active_jobs_of_that_name() {
        let registry = JobRegistry::default();
        let done = registry.enqueue_unique("a", &[], UniquePolicy::Keep, None);
        registry.start(done.id);
        registry.finish(done.id, JobOutcome::Success);
        registry.enqueue_unique("a", &[], UniquePolicy::Keep, None);
        registry.enqueue_unique("b", &[], UniquePolicy::Keep, None);
        registry.cancel_unique("a");
        let states: Vec<JobState> = registry.snapshot().iter().map(|job| job.state).collect();
        assert_eq!(states, [JobState::Succeeded, JobState::Cancelled, JobState::Enqueued]);
    }

    #[test]
    fn the_dump_labels_are_the_work_manager_names() {
        assert_eq!(JobState::Enqueued.label(), "ENQUEUED");
        assert_eq!(JobState::Cancelled.label(), "CANCELLED");
        assert!(JobState::Running.is_active() && !JobState::Failed.is_active());
    }
}
