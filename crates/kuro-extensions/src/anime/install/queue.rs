//! The install queue (T-19a-034, T-19a-035, `p1-rust-design.md` sections 6.2 and 6.6). Requests start in FIFO order. The
//! queue keeps the waiting entries and the running ones. The tracker keeps each package's step.

use std::collections::VecDeque;

use super::{Begin, InstallTracker, Ticket};
use crate::anime::model::InstallStep;

#[derive(Debug, Default)]
pub struct InstallQueue {
    waiting: VecDeque<Ticket>,
    running: Vec<Ticket>,
}

impl InstallQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues an install of `pkg`. A package that is already waiting or running gets its step back, and nothing is
    /// queued a second time (E-INS-12).
    pub fn request(&mut self, tracker: &mut InstallTracker, pkg: &str) -> Begin {
        let begin = tracker.begin(pkg);
        if let Begin::Started(ticket) = &begin {
            self.waiting.push_back(ticket.clone());
        }
        begin
    }

    /// The next entry to run, in FIFO order. It moves to the running list.
    pub fn next(&mut self) -> Option<Ticket> {
        let ticket = self.waiting.pop_front()?;
        self.running.push(ticket.clone());
        Some(ticket)
    }

    /// The entry behind `ticket` finished, with any outcome.
    pub fn finished(&mut self, ticket: &Ticket) {
        self.running.retain(|running| running != ticket);
    }

    /// Cancels the install of `pkg`. A waiting entry leaves the queue and its step becomes `Idle` without a download
    /// (E-INS-13). A running entry has its token fired, and its task then reports `Idle`. Returns false when nothing is
    /// queued or running for `pkg`.
    pub fn cancel(&mut self, tracker: &mut InstallTracker, pkg: &str) -> bool {
        if let Some(position) = self.waiting.iter().position(|ticket| ticket.pkg == pkg) {
            if let Some(ticket) = self.waiting.remove(position) {
                ticket.cancel.cancel();
                tracker.set_step(&ticket, InstallStep::Idle);
                return true;
            }
        }
        tracker.cancel(pkg)
    }

    /// The owner is going away (`onDestroy`, E-INS-14). Waiting entries become `Error`. The running entry is cancelled
    /// and becomes `Idle`. The queue is empty afterwards.
    pub fn shutdown(&mut self, tracker: &mut InstallTracker) {
        for ticket in self.waiting.drain(..) {
            tracker.set_step(&ticket, InstallStep::Error);
        }
        for ticket in self.running.drain(..) {
            ticket.cancel.cancel();
            tracker.set_step(&ticket, InstallStep::Idle);
        }
    }

    /// The packages waiting to start, in order.
    pub fn waiting(&self) -> Vec<&str> {
        self.waiting
            .iter()
            .map(|ticket| ticket.pkg.as_str())
            .collect()
    }

    /// The packages running now.
    pub fn running(&self) -> Vec<&str> {
        self.running
            .iter()
            .map(|ticket| ticket.pkg.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(begin: Begin) -> Ticket {
        match begin {
            Begin::Started(ticket) => ticket,
            Begin::Running(step) => panic!("expected a new install, got {step:?}"),
        }
    }

    #[test]
    fn requests_start_in_fifo_order() {
        let mut tracker = InstallTracker::new();
        let mut queue = InstallQueue::new();
        queue.request(&mut tracker, "eu.a");
        queue.request(&mut tracker, "eu.b");
        queue.request(&mut tracker, "eu.c");
        assert_eq!(
            queue.next().map(|ticket| ticket.pkg),
            Some("eu.a".to_owned())
        );
        assert_eq!(
            queue.next().map(|ticket| ticket.pkg),
            Some("eu.b".to_owned())
        );
        assert_eq!(
            queue.running(),
            ["eu.a", "eu.b"],
            "downloads run in parallel"
        );
        assert_eq!(queue.waiting(), ["eu.c"]);
    }

    #[test]
    fn tst_19_085_a_second_request_for_a_queued_or_running_package_gets_the_existing_entry() {
        let mut tracker = InstallTracker::new();
        let mut queue = InstallQueue::new();
        queue.request(&mut tracker, "eu.a");
        assert_eq!(
            queue.request(&mut tracker, "eu.a"),
            Begin::Running(InstallStep::Pending)
        );
        assert_eq!(queue.waiting(), ["eu.a"], "one entry, so one download");
        let ticket = started(queue.request(&mut tracker, "eu.b"));
        assert_eq!(ticket.pkg, "eu.b");
        queue.next();
        assert_eq!(
            queue.request(&mut tracker, "eu.a"),
            Begin::Running(InstallStep::Pending)
        );
    }

    #[test]
    fn tst_19_086_cancel_of_a_queued_entry_removes_it_and_emits_idle_while_the_others_run() {
        let mut tracker = InstallTracker::new();
        let mut queue = InstallQueue::new();
        queue.request(&mut tracker, "eu.a");
        let second = started(queue.request(&mut tracker, "eu.b"));
        queue.request(&mut tracker, "eu.c");
        assert!(queue.cancel(&mut tracker, "eu.b"));
        assert_eq!(tracker.step("eu.b"), Some(InstallStep::Idle));
        assert!(
            second.cancel.is_cancelled(),
            "the queued entry's token fires"
        );
        assert_eq!(queue.waiting(), ["eu.a", "eu.c"]);
        assert_eq!(
            queue.next().map(|ticket| ticket.pkg),
            Some("eu.a".to_owned())
        );
        assert_eq!(
            queue.next().map(|ticket| ticket.pkg),
            Some("eu.c".to_owned())
        );
    }

    #[test]
    fn cancel_of_a_package_that_is_not_queued_is_false() {
        let mut tracker = InstallTracker::new();
        let mut queue = InstallQueue::new();
        assert!(!queue.cancel(&mut tracker, "eu.none"));
    }

    #[test]
    fn tst_19_087_shutdown_sets_queued_entries_to_error_and_the_active_one_to_idle() {
        let mut tracker = InstallTracker::new();
        let mut queue = InstallQueue::new();
        let active = started(queue.request(&mut tracker, "eu.active"));
        queue.request(&mut tracker, "eu.waiting");
        queue.next();
        tracker.set_step(&active, InstallStep::Downloading);
        queue.shutdown(&mut tracker);
        assert_eq!(tracker.step("eu.waiting"), Some(InstallStep::Error));
        assert_eq!(tracker.step("eu.active"), Some(InstallStep::Idle));
        assert!(active.cancel.is_cancelled());
        assert!(queue.waiting().is_empty() && queue.running().is_empty());
    }

    #[test]
    fn a_finished_entry_leaves_the_running_list() {
        let mut tracker = InstallTracker::new();
        let mut queue = InstallQueue::new();
        queue.request(&mut tracker, "eu.a");
        let ticket = queue.next().expect("one entry");
        queue.finished(&ticket);
        assert!(queue.running().is_empty());
    }
}
