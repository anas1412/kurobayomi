//! The library update started from the Updates page (T-42b-015, T-42b-041, T-42a-033). The run and its single-flight
//! lock belong to the library's update pass. The app registers that runner here at start-up ([`install`]), and this
//! module asks it to start a pass of one kind and turns the answer into the page's message. Pure apart from the host.

use std::cell::RefCell;
use std::rc::Rc;

use kuro_domain::kind::EntryKind;

pub const UPDATING: &str = "Updating library";
pub const ALREADY_RUNNING: &str = "An update is already running";
pub const COULD_NOT_START: &str = "Could not start library update";

/// The app's update runner, as the Updates page sees it.
#[derive(Clone)]
pub struct Host {
    /// Starts a pass of one kind; `false` when a pass of that kind is already running.
    pub start: Rc<dyn Fn(EntryKind) -> bool>,
    /// Whether a pass of that kind is running.
    pub running: Rc<dyn Fn(EntryKind) -> bool>,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

/// Registers the runner. Replaces an earlier one.
pub fn install(host: Host) {
    HOST.with(|h| *h.borrow_mut() = Some(host));
}

/// Registers a runner that was written for the library's refresh button (`start` takes a category, `running` a kind).
/// The Updates page starts a pass of the whole kind, so the category is `None`. This is the one call the runner's
/// start-up makes beside `library::mount::on_update`, handing it the same two closures, so both buttons share one
/// single-flight lock.
pub fn install_shared(start: Rc<dyn Fn(EntryKind, Option<i64>) -> bool>, running: Rc<dyn Fn(EntryKind) -> bool>) {
    install(Host { start: Rc::new(move |kind| start(kind, None)), running });
}

/// What a trigger came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Started,
    AlreadyRunning,
    /// No runner is registered, so the scheduler cannot start.
    Refused,
}

/// Asks the runner for a pass of `kind`. A pass that is already running is never started twice (E-U38).
pub fn trigger(kind: EntryKind) -> Outcome {
    let Some(host) = HOST.with(|h| h.borrow().clone()) else { return Outcome::Refused };
    if (host.running)(kind) {
        Outcome::AlreadyRunning
    } else if (host.start)(kind) {
        Outcome::Started
    } else {
        Outcome::AlreadyRunning
    }
}

/// `true` when a new pass of `kind` was started, `false` when one is running or none can start (T-42b-041).
pub fn start(kind: EntryKind) -> bool {
    trigger(kind) == Outcome::Started
}

/// Whether a pass of `kind` is running. `false` without a runner.
pub fn running(kind: EntryKind) -> bool {
    HOST.with(|h| h.borrow().as_ref().is_some_and(|host| (host.running)(kind)))
}

/// The toast for an outcome.
pub fn message(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Started => UPDATING,
        Outcome::AlreadyRunning => ALREADY_RUNNING,
        Outcome::Refused => COULD_NOT_START,
    }
}

/// True for the key that starts a library pass on the Updates page (T-42b-042): F5.
pub fn is_refresh_key(key: gtk::gdk::Key) -> bool {
    key == gtk::gdk::Key::F5
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    #[test]
    fn f5_is_the_refresh_key_and_other_keys_are_not() {
        assert!(is_refresh_key(gtk::gdk::Key::F5));
        assert!(!is_refresh_key(gtk::gdk::Key::F4));
        assert!(!is_refresh_key(gtk::gdk::Key::Escape));
    }

    fn host(running: bool, accept: bool) -> (Host, Rc<Cell<u32>>) {
        let starts = Rc::new(Cell::new(0));
        let counter = Rc::clone(&starts);
        let host = Host {
            start: Rc::new(move |_| {
                counter.set(counter.get() + 1);
                accept
            }),
            running: Rc::new(move |_| running),
        };
        (host, starts)
    }

    #[test]
    fn no_runner_is_refused_with_the_error_text() {
        assert_eq!(trigger(EntryKind::Manga), Outcome::Refused);
        assert!(!running(EntryKind::Manga));
        assert_eq!(message(Outcome::Refused), "Could not start library update");
    }

    #[test]
    fn a_run_in_progress_is_not_started_again() {
        let (h, starts) = host(true, true);
        install(h);
        assert_eq!(trigger(EntryKind::Anime), Outcome::AlreadyRunning);
        assert_eq!(trigger(EntryKind::Anime), Outcome::AlreadyRunning, "a second click while running starts nothing");
        assert_eq!(starts.get(), 0);
        assert_eq!(message(Outcome::AlreadyRunning), "An update is already running");
    }

    #[test]
    fn an_idle_runner_that_accepts_reports_updating() {
        let (h, starts) = host(false, true);
        install(h);
        assert_eq!(trigger(EntryKind::Manga), Outcome::Started);
        assert_eq!(starts.get(), 1);
        assert_eq!(message(Outcome::Started), "Updating library");
    }

    #[test]
    fn the_shared_runner_gets_the_whole_kind_and_one_flight() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let busy = Rc::new(Cell::new(false));
        let (log, flag) = (Rc::clone(&seen), Rc::clone(&busy));
        let (flag_start, flag_running) = (Rc::clone(&flag), flag);
        install_shared(
            Rc::new(move |kind, category| {
                log.borrow_mut().push((kind, category));
                flag_start.set(true);
                true
            }),
            Rc::new(move |_| flag_running.get()),
        );
        assert!(start(EntryKind::Manga));
        assert!(!start(EntryKind::Manga), "the pass is still running");
        assert!(running(EntryKind::Manga));
        assert_eq!(*seen.borrow(), vec![(EntryKind::Manga, None)]);
        busy.set(false);
        assert!(!running(EntryKind::Manga));
    }

    #[test]
    fn a_refused_start_reads_as_already_running() {
        let (h, _) = host(false, false);
        install(h);
        assert_eq!(trigger(EntryKind::Manga), Outcome::AlreadyRunning);
    }
}
