//! Single-flight slot shared by manual and automatic backup runs (area 21, T-21a-027). The slot is an atomic flag:
//! a second acquire fails at once, and the guard releases the flag on drop, including when the run panics.

use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kuro_platform::notifier::Notifier;

use crate::error::BackupError;
use crate::notify;

#[derive(Clone, Default)]
pub struct JobSlot(Arc<AtomicBool>);

/// Holding this guard means the job runs. Dropping it frees the slot.
pub struct JobGuard(Arc<AtomicBool>);

impl JobSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// `None` when another run holds the slot.
    pub fn try_acquire(&self) -> Option<JobGuard> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| JobGuard(Arc::clone(&self.0)))
    }

    pub fn is_held(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// What the GTK side receives. Use an unbounded channel so the worker never waits on the UI.
#[derive(Debug)]
pub enum JobEvent {
    Started,
    Finished(Result<PathBuf, BackupError>),
}

/// Starts a manual run on its own thread. Refused with `InProgress` while the slot is held (B-03, E-24). "Creating
/// backup" shows before the work starts and is closed on completion or failure, which then shows the result notice.
pub fn start_manual<F>(
    slot: &JobSlot,
    notifier: Arc<dyn Notifier>,
    events: async_channel::Sender<JobEvent>,
    work: F,
) -> Result<(), BackupError>
where
    F: FnOnce() -> Result<PathBuf, BackupError> + Send + 'static,
{
    let guard = slot.try_acquire().ok_or(BackupError::InProgress)?;
    std::thread::Builder::new()
        .name("kuro-backup-manual".into())
        .spawn(move || {
            notify::create_progress().show(notifier.as_ref());
            let _ = events.send_blocking(JobEvent::Started);
            let result = panic::catch_unwind(AssertUnwindSafe(work))
                .unwrap_or_else(|_| Err(BackupError::Io(io::Error::other("backup worker panicked"))));
            drop(guard);
            notify::manual_finished(notifier.as_ref(), &result);
            let _ = events.send_blocking(JobEvent::Finished(result));
        })
        .map_err(BackupError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn quiet() -> Arc<dyn Notifier> {
        Arc::new(crate::notify::recorder::Recorder::default())
    }

    fn ok_path() -> Result<PathBuf, BackupError> {
        Ok(PathBuf::from("/tmp/x.tachibk"))
    }

    fn finish(rx: &async_channel::Receiver<JobEvent>) -> Result<PathBuf, BackupError> {
        loop {
            match rx.recv_blocking().expect("channel open") {
                JobEvent::Started => continue,
                JobEvent::Finished(r) => return r,
            }
        }
    }

    #[test]
    fn slot_is_exclusive_and_frees_on_drop() {
        let slot = JobSlot::new();
        let g = slot.try_acquire().expect("first acquire");
        assert!(slot.is_held());
        assert!(slot.try_acquire().is_none());
        drop(g);
        assert!(!slot.is_held());
        assert!(slot.try_acquire().is_some());
    }

    #[test]
    fn second_manual_refused() {
        let slot = JobSlot::new();
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let (tx, rx) = async_channel::unbounded();
        start_manual(&slot, quiet(), tx.clone(), move || {
            gate_rx.recv().ok();
            ok_path()
        })
        .unwrap();
        let second = start_manual(&slot, quiet(), tx, ok_path);
        assert!(matches!(second, Err(BackupError::InProgress)));
        gate_tx.send(()).unwrap();
        assert!(finish(&rx).is_ok());
    }

    #[test]
    fn auto_tick_skipped_during_manual() {
        let slot = JobSlot::new();
        let manual = slot.try_acquire().unwrap();
        let skipped = slot.try_acquire().is_none();
        assert!(skipped, "an automatic tick cannot take the slot");
        drop(manual);
        assert!(slot.try_acquire().is_some());
    }

    #[test]
    fn slot_released_on_success() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        start_manual(&slot, quiet(), tx, ok_path).unwrap();
        assert!(finish(&rx).is_ok());
        assert!(!slot.is_held());
    }

    #[test]
    fn slot_released_on_error() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        start_manual(&slot, quiet(), tx, || Err(BackupError::CreateFile)).unwrap();
        assert!(matches!(finish(&rx), Err(BackupError::CreateFile)));
        assert!(!slot.is_held());
    }

    #[test]
    fn slot_released_on_panic() {
        let slot = JobSlot::new();
        let (tx, rx) = async_channel::unbounded();
        start_manual(&slot, quiet(), tx, || panic!("boom")).unwrap();
        assert!(matches!(finish(&rx), Err(BackupError::Io(_))));
        assert!(!slot.is_held());
    }

    #[test]
    fn progress_shows_first_then_closes_with_the_result() {
        use crate::notify::{ID_BACKUP_PROGRESS, ID_BACKUP_RESULT, recorder::Recorder};
        for (work, title) in [(ok_path as fn() -> Result<PathBuf, BackupError>, "Backup created"), (|| Err(BackupError::CreateFile), "Backup failed")] {
            let rec = Arc::new(Recorder::default());
            let slot = JobSlot::new();
            let (tx, rx) = async_channel::unbounded();
            start_manual(&slot, rec.clone(), tx, work).unwrap();
            finish(&rx).ok();
            // Finished is sent after the notices, so every call is recorded by now.
            assert_eq!(
                *rec.calls.lock().unwrap(),
                vec![format!("show:{ID_BACKUP_PROGRESS}"), format!("withdraw:{ID_BACKUP_PROGRESS}"), format!("show:{ID_BACKUP_RESULT}")]
            );
            assert_eq!(rec.shown.lock().unwrap()[0].1, "Creating backup");
            assert_eq!(rec.shown.lock().unwrap()[1].1, title);
        }
    }
}
