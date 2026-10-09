//! Generation-guarded events and task cancellation for a browse page (T-40b-004). A page owns one event channel and the
//! tasks that feed it. Events carry the generation they were started under, and events from an older generation are
//! dropped. Dropping the page aborts its tasks, so none outlives it.

use std::future::Future;

use tokio::task::{AbortHandle, JoinSet};

/// The current generation of a page. Restarting a listing (a new search, a refresh) bumps it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Generation(u64);

impl Generation {
    pub fn current(self) -> u64 {
        self.0
    }

    /// Starts a new generation and returns it.
    pub fn bump(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }

    /// True when an event started under `generation` still belongs to the page.
    pub fn accepts(self, generation: u64) -> bool {
        self.0 == generation
    }
}

/// An event tagged with the generation it was started under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tagged<E> {
    pub generation: u64,
    pub event: E,
}

/// The channel and loader tasks of one page.
pub struct PageTasks<E> {
    generation: Generation,
    tx: async_channel::Sender<Tagged<E>>,
    rx: async_channel::Receiver<Tagged<E>>,
    tasks: JoinSet<()>,
}

impl<E: Send + 'static> PageTasks<E> {
    pub fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();
        Self { generation: Generation::default(), tx, rx, tasks: JoinSet::new() }
    }

    /// The receiver the main-thread loop reads. Stale events are filtered with [`PageTasks::accepts`].
    pub fn events(&self) -> async_channel::Receiver<Tagged<E>> {
        self.rx.clone()
    }

    /// A sender for a task started now, and the generation it must tag its events with.
    pub fn sender(&self) -> (u64, async_channel::Sender<Tagged<E>>) {
        (self.generation.current(), self.tx.clone())
    }

    /// Runs a loader on the runtime. The loader is aborted when the page restarts or drops.
    pub fn spawn<F>(&mut self, loader: F) -> AbortHandle
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.spawn(loader)
    }

    /// Aborts every loader and starts a new generation. Events still queued from before are dropped by the loop.
    pub fn restart(&mut self) -> u64 {
        self.abort_all();
        self.generation.bump()
    }

    pub fn abort_all(&mut self) {
        self.tasks.abort_all();
    }

    pub fn accepts(&self, event: &Tagged<E>) -> bool {
        self.generation.accepts(event.generation)
    }

    /// Loaders not yet finished or aborted.
    pub fn live_tasks(&self) -> usize {
        self.tasks.len()
    }
}

impl<E: Send + 'static> Default for PageTasks<E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::time::Duration;

    /// Counts how many loaders are alive: incremented when built, decremented when its future is dropped.
    struct Live(Arc<AtomicUsize>);

    impl Live {
        fn new(counter: &Arc<AtomicUsize>) -> Self {
            counter.fetch_add(1, AtomicOrdering::SeqCst);
            Self(Arc::clone(counter))
        }
    }

    impl Drop for Live {
        fn drop(&mut self) {
            self.0.fetch_sub(1, AtomicOrdering::SeqCst);
        }
    }

    #[test]
    fn generations_accept_only_their_own_events() {
        let mut generation = Generation::default();
        let first = generation.bump();
        assert!(generation.accepts(first));
        let second = generation.bump();
        assert!(!generation.accepts(first), "an event from the old generation is dropped");
        assert!(generation.accepts(second));
    }

    #[tokio::test]
    async fn events_from_a_restarted_page_are_filtered_out() {
        let mut page: PageTasks<&'static str> = PageTasks::new();
        let (old_gen, tx) = page.sender();
        tx.send(Tagged { generation: old_gen, event: "late" }).await.expect("send");
        page.restart();
        let (new_gen, tx) = page.sender();
        tx.send(Tagged { generation: new_gen, event: "fresh" }).await.expect("send");
        let rx = page.events();
        let mut accepted = Vec::new();
        for _ in 0..2 {
            let event = rx.recv().await.expect("recv");
            if page.accepts(&event) {
                accepted.push(event.event);
            }
        }
        assert_eq!(accepted, vec!["fresh"]);
    }

    #[tokio::test]
    async fn dropping_a_page_aborts_every_loader() {
        let live = Arc::new(AtomicUsize::new(0));
        {
            let mut page: PageTasks<()> = PageTasks::new();
            for _ in 0..3 {
                let guard = Live::new(&live);
                page.spawn(async move {
                    let _guard = guard;
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                });
            }
            assert_eq!(page.live_tasks(), 3);
            assert_eq!(live.load(AtomicOrdering::SeqCst), 3);
        }
        for _ in 0..200 {
            if live.load(AtomicOrdering::SeqCst) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(live.load(AtomicOrdering::SeqCst), 0, "no loader outlives its page");
    }

    #[tokio::test]
    async fn restarting_aborts_the_loaders_of_the_old_generation() {
        let live = Arc::new(AtomicUsize::new(0));
        let mut page: PageTasks<()> = PageTasks::new();
        let guard = Live::new(&live);
        page.spawn(async move {
            let _guard = guard;
            tokio::time::sleep(Duration::from_secs(3600)).await;
        });
        page.restart();
        for _ in 0..200 {
            if live.load(AtomicOrdering::SeqCst) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(live.load(AtomicOrdering::SeqCst), 0);
    }
}
