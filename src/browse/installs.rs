//! The install, update, uninstall and cancel state machine of the extensions tab, and the update-all runner
//! (T-40a-018). Pure: `Installs::apply` takes one input and returns the effects the page must run, so every rule is
//! tested without a runtime. Ports `MangaExtensionsScreenModel` (`installExtension`, `updateExtension`,
//! `cancelInstallUpdateExtension`, `uninstallExtension`), with update-all made sequential (B§5.6, E-40-26).

use std::collections::{BTreeMap, VecDeque};

use kuro_extensions::InstallStep;

use super::extensions::Downloads;

/// What can happen to one package's row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Install, update or retry was pressed.
    Request,
    /// A running job reported a step.
    Report(InstallStep),
    /// Cancel was pressed.
    Cancel,
    /// The package is being uninstalled.
    Uninstall,
    /// The job's flow ended, by completion.
    End,
}

/// What the page must do after a transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Nothing,
    /// Start a download-and-install job.
    StartJob,
    /// Abort the running job.
    AbortJob,
    /// Abort the running job (when `abort`), then remove the package.
    Remove { abort: bool },
}

/// The step of one package after `event`, and what to do about it. A request while a job runs is refused (one install
/// per package); a report for a package with no job is a late event of a cancelled job and is ignored.
pub fn transition(step: InstallStep, event: Event) -> (InstallStep, Action) {
    let busy = !step.is_completed();
    match event {
        Event::Request if busy => (step, Action::Nothing),
        Event::Request => (InstallStep::Pending, Action::StartJob),
        Event::Report(next) if busy => (next, Action::Nothing),
        Event::Report(_) => (step, Action::Nothing),
        Event::Cancel if busy => (InstallStep::Idle, Action::AbortJob),
        Event::Cancel => (step, Action::Nothing),
        Event::Uninstall => (InstallStep::Idle, Action::Remove { abort: busy }),
        Event::End => (InstallStep::Idle, Action::Nothing),
    }
}

/// The queue of update-all: one package at a time, in list order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct UpdateAll {
    queue: VecDeque<String>,
    current: Option<String>,
}

impl UpdateAll {
    pub fn new(pkgs: Vec<String>) -> Self {
        Self { queue: pkgs.into(), current: None }
    }

    /// The next package to update, or `None` while one is running or the queue is empty.
    pub fn next(&mut self) -> Option<String> {
        if self.current.is_some() {
            return None;
        }
        self.current = self.queue.pop_front();
        self.current.clone()
    }

    /// The package that is updating now.
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// The running package ended; the next `next` returns the one after it.
    pub fn finish(&mut self, pkg: &str) {
        if self.current.as_deref() == Some(pkg) {
            self.current = None;
        }
    }

    pub fn is_done(&self) -> bool {
        self.current.is_none() && self.queue.is_empty()
    }
}

/// An input to the whole tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// A button on one row.
    Row { pkg: String, event: Event },
    /// A job's report or end. `job` is the ticket the start effect carried; an older job's events are dropped.
    Job { pkg: String, job: u64, event: Event },
    /// The Update all button, with the packages that have an update.
    UpdateAll(Vec<String>),
    /// Cancels the queue and the running update.
    CancelUpdateAll,
}

/// What the page runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Start { pkg: String, job: u64 },
    Abort { pkg: String, job: u64 },
    Uninstall { pkg: String },
}

/// The steps of every package being installed, the live job of each, and the update-all queue.
#[derive(Debug, Default)]
pub struct Installs {
    downloads: Downloads,
    jobs: BTreeMap<String, u64>,
    next_job: u64,
    update_all: Option<UpdateAll>,
}

impl Installs {
    pub fn step(&self, pkg: &str) -> InstallStep {
        self.downloads.step(pkg)
    }

    pub fn is_empty(&self) -> bool {
        self.downloads.is_empty() && self.jobs.is_empty()
    }

    pub fn updating_all(&self) -> bool {
        self.update_all.is_some()
    }

    /// Applies one input and returns the effects to run, in order.
    pub fn apply(&mut self, input: Input) -> Vec<Effect> {
        let mut effects = Vec::new();
        match input {
            Input::Row { pkg, event } => {
                self.row(&pkg, event, &mut effects);
                if matches!(event, Event::Cancel | Event::Uninstall) {
                    self.advance_if_current(&pkg, &mut effects);
                }
            }
            Input::Job { pkg, job, event } => {
                if self.jobs.get(&pkg) != Some(&job) {
                    return effects;
                }
                match event {
                    Event::End => {
                        self.jobs.remove(&pkg);
                        self.downloads.clear(&pkg);
                        self.advance_if_current(&pkg, &mut effects);
                    }
                    other => self.row(&pkg, other, &mut effects),
                }
            }
            Input::UpdateAll(pkgs) => {
                if self.update_all.is_none() && !pkgs.is_empty() {
                    self.update_all = Some(UpdateAll::new(pkgs));
                    self.advance(&mut effects);
                }
            }
            Input::CancelUpdateAll => {
                // only the package update-all itself is running; installs the user started stay
                let running = self.update_all.take().and_then(|queue| queue.current().map(str::to_owned));
                if let Some(pkg) = running {
                    self.abort(&pkg, &mut effects);
                }
            }
        }
        effects
    }

    fn row(&mut self, pkg: &str, event: Event, effects: &mut Vec<Effect>) {
        let (next, action) = transition(self.downloads.step(pkg), event);
        match action {
            Action::Nothing => {
                if let Event::Report(_) = event {
                    self.downloads.set(pkg, next);
                }
            }
            Action::StartJob => {
                self.next_job += 1;
                // a job that already reported its last step may still be running; its late events are dropped
                self.abort(pkg, effects);
                self.jobs.insert(pkg.to_owned(), self.next_job);
                self.downloads.begin(pkg);
                effects.push(Effect::Start { pkg: pkg.to_owned(), job: self.next_job });
            }
            Action::AbortJob => self.abort(pkg, effects),
            Action::Remove { abort } => {
                if abort {
                    self.abort(pkg, effects);
                }
                effects.push(Effect::Uninstall { pkg: pkg.to_owned() });
            }
        }
    }

    fn abort(&mut self, pkg: &str, effects: &mut Vec<Effect>) {
        if let Some(job) = self.jobs.remove(pkg) {
            effects.push(Effect::Abort { pkg: pkg.to_owned(), job });
        }
        self.downloads.clear(pkg);
    }

    /// When the update-all package ended or was cancelled, start the next one.
    fn advance_if_current(&mut self, pkg: &str, effects: &mut Vec<Effect>) {
        let Some(queue) = self.update_all.as_mut() else { return };
        if queue.current() != Some(pkg) || self.jobs.contains_key(pkg) {
            return;
        }
        queue.finish(pkg);
        self.advance(effects);
    }

    /// Starts the next queued package that is not already installing; the queue ends when none is left.
    fn advance(&mut self, effects: &mut Vec<Effect>) {
        loop {
            let Some(queue) = self.update_all.as_mut() else { return };
            let Some(pkg) = queue.next() else {
                if queue.is_done() {
                    self.update_all = None;
                }
                return;
            };
            let before = effects.len();
            self.row(&pkg, Event::Request, effects);
            if effects.len() > before {
                return;
            }
            // already busy with a job of its own: leave it to that job and take the next package
            if let Some(queue) = self.update_all.as_mut() {
                queue.finish(&pkg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pkg: &str, event: Event) -> Input {
        Input::Row { pkg: pkg.into(), event }
    }

    fn job(pkg: &str, job: u64, event: Event) -> Input {
        Input::Job { pkg: pkg.into(), job, event }
    }

    fn start(pkg: &str, job: u64) -> Effect {
        Effect::Start { pkg: pkg.into(), job }
    }

    #[test]
    fn a_request_on_a_finished_row_starts_a_job_in_pending() {
        for step in [InstallStep::Idle, InstallStep::Installed, InstallStep::Error] {
            assert_eq!(transition(step, Event::Request), (InstallStep::Pending, Action::StartJob), "{step:?}");
        }
    }

    #[test]
    fn a_request_while_a_job_runs_is_refused() {
        for step in [InstallStep::Pending, InstallStep::Downloading, InstallStep::Installing] {
            assert_eq!(transition(step, Event::Request), (step, Action::Nothing), "{step:?}");
        }
    }

    #[test]
    fn cancel_aborts_only_a_running_job() {
        assert_eq!(transition(InstallStep::Downloading, Event::Cancel), (InstallStep::Idle, Action::AbortJob));
        assert_eq!(transition(InstallStep::Error, Event::Cancel), (InstallStep::Error, Action::Nothing));
    }

    #[test]
    fn uninstall_aborts_a_running_job_first() {
        assert_eq!(
            transition(InstallStep::Installing, Event::Uninstall),
            (InstallStep::Idle, Action::Remove { abort: true })
        );
        assert_eq!(
            transition(InstallStep::Idle, Event::Uninstall),
            (InstallStep::Idle, Action::Remove { abort: false })
        );
    }

    #[test]
    fn a_report_for_a_package_with_no_job_is_ignored() {
        assert_eq!(transition(InstallStep::Idle, Event::Report(InstallStep::Installing)).0, InstallStep::Idle);
    }

    #[test]
    fn a_job_walks_pending_downloading_installing_installed_then_ends() {
        let mut installs = Installs::default();
        assert_eq!(installs.apply(row("a.b", Event::Request)), vec![start("a.b", 1)]);
        assert_eq!(installs.step("a.b"), InstallStep::Pending);
        for step in [InstallStep::Downloading, InstallStep::Installing, InstallStep::Installed] {
            installs.apply(job("a.b", 1, Event::Report(step)));
            assert_eq!(installs.step("a.b"), step);
        }
        installs.apply(job("a.b", 1, Event::End));
        assert_eq!(installs.step("a.b"), InstallStep::Idle);
        assert!(installs.is_empty());
    }

    #[test]
    fn cancel_aborts_the_job_and_late_events_of_it_are_dropped() {
        let mut installs = Installs::default();
        installs.apply(row("a.b", Event::Request));
        let effects = installs.apply(row("a.b", Event::Cancel));
        assert_eq!(effects, vec![Effect::Abort { pkg: "a.b".into(), job: 1 }]);
        assert_eq!(installs.step("a.b"), InstallStep::Idle);
        installs.apply(job("a.b", 1, Event::Report(InstallStep::Installing)));
        assert_eq!(installs.step("a.b"), InstallStep::Idle, "a cancelled job cannot bring the row back");
    }

    #[test]
    fn a_restarted_install_ignores_the_end_of_the_old_job() {
        let mut installs = Installs::default();
        installs.apply(row("a.b", Event::Request));
        installs.apply(row("a.b", Event::Cancel));
        assert_eq!(installs.apply(row("a.b", Event::Request)), vec![start("a.b", 2)]);
        installs.apply(job("a.b", 1, Event::End));
        assert_eq!(installs.step("a.b"), InstallStep::Pending, "job 1's end must not clear job 2's row");
    }

    #[test]
    fn retry_after_an_error_starts_a_fresh_job() {
        let mut installs = Installs::default();
        installs.apply(row("a.b", Event::Request));
        installs.apply(job("a.b", 1, Event::Report(InstallStep::Error)));
        let restart = installs.apply(row("a.b", Event::Request));
        assert_eq!(restart, vec![Effect::Abort { pkg: "a.b".into(), job: 1 }, start("a.b", 2)]);
        assert_eq!(installs.step("a.b"), InstallStep::Pending);
        installs.apply(job("a.b", 1, Event::End));
        assert_eq!(installs.step("a.b"), InstallStep::Pending, "the old job's end is stale");
    }

    #[test]
    fn uninstalling_a_busy_package_aborts_then_removes_it() {
        let mut installs = Installs::default();
        installs.apply(row("a.b", Event::Request));
        let effects = installs.apply(row("a.b", Event::Uninstall));
        assert_eq!(effects, vec![Effect::Abort { pkg: "a.b".into(), job: 1 }, Effect::Uninstall { pkg: "a.b".into() }]);
        assert!(installs.is_empty());
    }

    #[test]
    fn uninstalling_an_idle_package_only_removes_it() {
        let mut installs = Installs::default();
        assert_eq!(installs.apply(row("a.b", Event::Uninstall)), vec![Effect::Uninstall { pkg: "a.b".into() }]);
    }

    #[test]
    fn update_all_runs_one_package_at_a_time_in_order() {
        let mut installs = Installs::default();
        let effects = installs.apply(Input::UpdateAll(vec!["a.one".into(), "a.two".into(), "a.three".into()]));
        assert_eq!(effects, vec![start("a.one", 1)], "only the first starts");
        assert_eq!(installs.step("a.two"), InstallStep::Idle);

        installs.apply(job("a.one", 1, Event::Report(InstallStep::Installed)));
        assert_eq!(installs.apply(job("a.one", 1, Event::End)), vec![start("a.two", 2)]);
        assert_eq!(installs.apply(job("a.two", 2, Event::End)), vec![start("a.three", 3)]);
        assert!(installs.updating_all());
        assert!(installs.apply(job("a.three", 3, Event::End)).is_empty());
        assert!(!installs.updating_all(), "the queue ends with the last package");
        assert!(installs.is_empty(), "the map is clear when update-all completes");
    }

    #[test]
    fn a_failed_package_does_not_stop_update_all() {
        let mut installs = Installs::default();
        installs.apply(Input::UpdateAll(vec!["a.one".into(), "a.two".into()]));
        installs.apply(job("a.one", 1, Event::Report(InstallStep::Error)));
        assert_eq!(installs.apply(job("a.one", 1, Event::End)), vec![start("a.two", 2)]);
    }

    #[test]
    fn cancelling_the_running_package_moves_update_all_on() {
        let mut installs = Installs::default();
        installs.apply(Input::UpdateAll(vec!["a.one".into(), "a.two".into()]));
        let effects = installs.apply(row("a.one", Event::Cancel));
        assert_eq!(effects, vec![Effect::Abort { pkg: "a.one".into(), job: 1 }, start("a.two", 2)]);
    }

    #[test]
    fn cancel_update_all_aborts_the_job_drops_the_queue_and_clears_the_map() {
        let mut installs = Installs::default();
        installs.apply(Input::UpdateAll(vec!["a.one".into(), "a.two".into()]));
        installs.apply(job("a.one", 1, Event::Report(InstallStep::Downloading)));
        let effects = installs.apply(Input::CancelUpdateAll);
        assert_eq!(effects, vec![Effect::Abort { pkg: "a.one".into(), job: 1 }]);
        assert!(installs.is_empty());
        assert!(!installs.updating_all());
        assert!(installs.apply(job("a.one", 1, Event::End)).is_empty(), "no next package starts after a cancel");
    }

    #[test]
    fn update_all_skips_a_package_that_is_already_installing() {
        let mut installs = Installs::default();
        installs.apply(row("a.two", Event::Request));
        let effects = installs.apply(Input::UpdateAll(vec!["a.two".into(), "a.one".into()]));
        assert_eq!(effects, vec![start("a.one", 2)]);
    }

    #[test]
    fn a_second_update_all_while_one_runs_is_ignored() {
        let mut installs = Installs::default();
        installs.apply(Input::UpdateAll(vec!["a.one".into()]));
        assert!(installs.apply(Input::UpdateAll(vec!["a.two".into()])).is_empty());
    }

    #[test]
    fn an_update_all_queue_hands_out_one_package_until_it_finishes() {
        let mut queue = UpdateAll::new(vec!["a".into(), "b".into()]);
        assert_eq!(queue.next(), Some("a".into()));
        assert_eq!(queue.next(), None);
        queue.finish("a");
        assert_eq!(queue.next(), Some("b".into()));
        queue.finish("b");
        assert!(queue.is_done());
    }
}
