//! The restore job runner (area 22, T-22a-005 and T-22a-040). One restore runs at a time on its own thread. The GTK side
//! gets every event over an unbounded channel and clears its running state on `Finished`, `Canceled` or `Failed`. The
//! slot is released before the last event is sent, so `is_running` is already false when that event arrives.

use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

use crate::error::BackupError;
use crate::job::JobSlot;

use super::job::{RestoreEnd, Summary};
use super::options::RestoreOptions;
use super::progress::ProgressEvent;
use super::screen::StopFlag;

/// What the restore job sends to the GTK side.
#[derive(Debug)]
pub enum RestoreEvent {
    Started,
    Progress(ProgressEvent),
    Finished(Summary),
    Canceled { error_log: Option<PathBuf> },
    Failed(BackupError),
}

/// Why a restore was not started.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("Restore is already in progress")]
    AlreadyRunning,
    /// Shown when every option is off. The UI disables the action first, so this is the backstop.
    #[error("Select at least one item to restore")]
    NothingSelected,
    #[error("the restore could not start: {0}")]
    Spawn(#[from] io::Error),
}

/// Whether a restore is active on this slot (T-22a-040). It is true only while the restore thread holds the slot.
pub fn is_running(slot: &JobSlot) -> bool {
    slot.is_held()
}

/// Starts a restore on its own thread. `work` runs the restore and gets the stop flag and an emitter for progress
/// events. Refused with `AlreadyRunning` while another restore holds the slot, and with `NothingSelected` when no option
/// is on. The stop flag is only read by `work`; this never blocks on it.
pub fn start_restore<F>(
    slot: &JobSlot,
    opts: RestoreOptions,
    stop: StopFlag,
    events: async_channel::Sender<RestoreEvent>,
    work: F,
) -> Result<(), StartError>
where
    F: FnOnce(&StopFlag, &mut dyn FnMut(ProgressEvent)) -> Result<RestoreEnd, BackupError> + Send + 'static,
{
    if !opts.can_restore() {
        return Err(StartError::NothingSelected);
    }
    let guard = slot.try_acquire().ok_or(StartError::AlreadyRunning)?;
    std::thread::Builder::new().name("kuro-backup-restore".into()).spawn(move || {
        let _ = events.send_blocking(RestoreEvent::Started);
        let mut emit = |event: ProgressEvent| {
            let _ = events.send_blocking(RestoreEvent::Progress(event));
        };
        let result = panic::catch_unwind(AssertUnwindSafe(|| work(&stop, &mut emit)))
            .unwrap_or_else(|_| Err(BackupError::Io(io::Error::other("restore worker panicked"))));
        drop(guard);
        let last = match result {
            Ok(RestoreEnd::Completed(summary)) => RestoreEvent::Finished(summary),
            Ok(RestoreEnd::Canceled { error_log }) => RestoreEvent::Canceled { error_log },
            Err(error) => RestoreEvent::Failed(error),
        };
        let _ = events.send_blocking(last);
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn all_on() -> RestoreOptions {
        RestoreOptions::from_bits([true; 7])
    }

    fn finished() -> Result<RestoreEnd, BackupError> {
        Ok(RestoreEnd::Completed(Summary { elapsed_ms: 1, errors: 0, error_log: None, skipped_settings: 0, skipped_extensions: 0 }))
    }

    /// Reads events until the last one of a run arrives.
    fn collect(rx: &async_channel::Receiver<RestoreEvent>) -> Vec<RestoreEvent> {
        let mut out = Vec::new();
        loop {
            let event = rx.recv_blocking().expect("the job sends a last event");
            let last = matches!(event, RestoreEvent::Finished(_) | RestoreEvent::Canceled { .. } | RestoreEvent::Failed(_));
            out.push(event);
            if last {
                return out;
            }
        }
    }

    #[test]
    fn all_false_options_are_refused_without_starting_a_thread() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        let err = start_restore(&slot, RestoreOptions::from_bits([false; 7]), StopFlag::default(), tx, |_, _| finished());
        assert!(matches!(err, Err(StartError::NothingSelected)));
        assert!(rx.try_recv().is_err());
        assert!(!is_running(&slot));
    }

    #[test]
    fn a_second_start_while_one_runs_is_refused_with_the_running_text() {
        let slot = JobSlot::new();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (tx, rx) = async_channel::unbounded();
        start_restore(&slot, all_on(), StopFlag::default(), tx.clone(), move |_, _| {
            release_rx.recv().ok();
            finished()
        })
        .unwrap();
        assert!(is_running(&slot));
        let second = start_restore(&slot, all_on(), StopFlag::default(), tx, |_, _| finished());
        assert!(matches!(second, Err(StartError::AlreadyRunning)));
        assert_eq!(StartError::AlreadyRunning.to_string(), "Restore is already in progress");
        release_tx.send(()).unwrap();
        collect(&rx);
    }

    #[test]
    fn the_slot_is_free_before_the_last_event_arrives() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        start_restore(&slot, all_on(), StopFlag::default(), tx, |_, _| finished()).unwrap();
        let events = collect(&rx);
        assert!(matches!(events.last(), Some(RestoreEvent::Finished(_))));
        assert!(!is_running(&slot));
    }

    #[test]
    fn progress_is_sent_in_order_and_a_cancel_ends_with_canceled() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        start_restore(&slot, all_on(), StopFlag::default(), tx, |_, emit| {
            emit(ProgressEvent { title: "a".into(), done: 1, total: 2 });
            Ok(RestoreEnd::Canceled { error_log: None })
        })
        .unwrap();
        let events = collect(&rx);
        assert!(matches!(events[0], RestoreEvent::Started));
        assert!(matches!(&events[1], RestoreEvent::Progress(p) if p.title == "a"));
        assert!(matches!(events.last(), Some(RestoreEvent::Canceled { error_log: None })));
    }

    #[test]
    fn a_failing_run_sends_failed_and_frees_the_slot() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        start_restore(&slot, all_on(), StopFlag::default(), tx, |_, _| Err(BackupError::Io(io::Error::other("disk"))))
            .unwrap();
        assert!(matches!(collect(&rx).last(), Some(RestoreEvent::Failed(_))));
        assert!(!is_running(&slot));
    }

    #[test]
    fn a_panicking_run_is_failed_not_propagated() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        start_restore(&slot, all_on(), StopFlag::default(), tx, |_, _| panic!("boom")).unwrap();
        assert!(matches!(collect(&rx).last(), Some(RestoreEvent::Failed(_))));
        assert!(!is_running(&slot));
    }

    #[test]
    fn the_stop_flag_reaches_the_run_and_a_requested_stop_ends_canceled() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        let stop = StopFlag::default();
        stop.request();
        start_restore(&slot, all_on(), stop, tx, |stop, _| {
            if stop.requested() {
                Ok(RestoreEnd::Canceled { error_log: None })
            } else {
                finished()
            }
        })
        .unwrap();
        assert!(matches!(collect(&rx).last(), Some(RestoreEvent::Canceled { .. })));
    }

    #[test]
    fn the_run_is_on_the_restore_thread_not_the_caller() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        let caller = std::thread::current().id();
        start_restore(&slot, all_on(), StopFlag::default(), tx, move |_, _| {
            assert_ne!(std::thread::current().id(), caller);
            assert_eq!(std::thread::current().name(), Some("kuro-backup-restore"));
            finished()
        })
        .unwrap();
        assert!(matches!(collect(&rx).last(), Some(RestoreEvent::Finished(_))));
    }
}
