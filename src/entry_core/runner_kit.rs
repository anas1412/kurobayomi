//! The plumbing both store runners share (manga `runner`, anime `anime::runner`): the page's handle on a runner, and the
//! live subscription that publishes the entry row and its item list together. Nothing here imports GTK.

use std::sync::{Arc, Mutex, PoisonError};

use async_channel::{Receiver, Sender};
use kuro_db::subscribe::Subscription;
use tokio::task::AbortHandle;
use tokio_util::sync::CancellationToken;

/// The page's handle on a runner. Dropping it cancels the page's token (a running refresh ends) and sends the
/// runner's shutdown command (the subscriptions stop).
pub struct RunnerHandle<C, E> {
    cmds: Sender<C>,
    events: Receiver<E>,
    cancel: CancellationToken,
    shutdown: fn() -> C,
}

impl<C, E> RunnerHandle<C, E> {
    /// A handle and the runner's ends of it: the command queue, the event sender and the page's token. `shutdown` makes
    /// the command a drop sends.
    pub fn new(shutdown: fn() -> C) -> (Self, Receiver<C>, Sender<E>, CancellationToken) {
        let (cmd_tx, cmd_rx) = async_channel::unbounded();
        let (event_tx, event_rx) = async_channel::unbounded();
        let cancel = CancellationToken::new();
        (Self { cmds: cmd_tx, events: event_rx, cancel: cancel.clone(), shutdown }, cmd_rx, event_tx, cancel)
    }

    /// Queues a command. The queue is unbounded, so this never blocks the GTK thread.
    pub fn send(&self, cmd: C) {
        let _ = self.cmds.try_send(cmd);
    }

    pub fn events(&self) -> Receiver<E> {
        self.events.clone()
    }
}

impl<C, E> Drop for RunnerHandle<C, E> {
    fn drop(&mut self) {
        self.cancel.cancel();
        let _ = self.cmds.try_send((self.shutdown)());
    }
}

/// Aborts every subscription task of a runner.
pub fn abort_all(subscriptions: &Mutex<Vec<AbortHandle>>) {
    for handle in subscriptions.lock().unwrap_or_else(PoisonError::into_inner).drain(..) {
        handle.abort();
    }
}

/// Two tasks, one per table: each keeps the newest value of its side and calls `publish` with both once both have
/// arrived, whenever either side changes. A row that is gone (`None`) or a failed query publishes nothing.
pub fn watch_pair<A, B, F>(mut entry: Subscription<Option<A>>, mut items: Subscription<Vec<B>>, publish: F) -> [AbortHandle; 2]
where
    A: Clone + Send + Sync + 'static,
    B: Clone + Send + Sync + 'static,
    F: Fn(&A, &[B]) + Send + Sync + 'static,
{
    let latest: Arc<Mutex<(Option<A>, Option<Vec<B>>)>> = Arc::new(Mutex::new((None, None)));
    let publish = Arc::new(publish);
    let emit = {
        let latest = Arc::clone(&latest);
        move || {
            let guard = latest.lock().unwrap_or_else(PoisonError::into_inner);
            if let (Some(a), Some(b)) = (&guard.0, &guard.1) {
                (*publish)(a, b.as_slice());
            }
        }
    };
    let entry_task = tokio::spawn({
        let (latest, emit) = (Arc::clone(&latest), emit.clone());
        async move {
            while let Some(published) = entry.changed().await {
                if let Ok(Some(value)) = published.map(|p| p.as_ref().clone()) {
                    latest.lock().unwrap_or_else(PoisonError::into_inner).0 = Some(value);
                    emit();
                }
            }
        }
    });
    let items_task = tokio::spawn(async move {
        while let Some(published) = items.changed().await {
            if let Ok(list) = published {
                latest.lock().unwrap_or_else(PoisonError::into_inner).1 = Some(list.as_ref().clone());
                emit();
            }
        }
    });
    [entry_task.abort_handle(), items_task.abort_handle()]
}
