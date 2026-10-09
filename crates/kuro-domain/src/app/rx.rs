//! Reactive helpers (T-10a-002), the replacements for the Kotlin `combine`, `changes()` and `distinctUntilChanged`
//! operators. Streams are `tokio::sync::watch` receivers: a receiver holds the latest value, so a late reader gets the
//! current state first. The helpers spawn tasks, so they must run inside a tokio runtime.

use std::future::Future;

use kuro_prefs::pref::Pref;
use kuro_prefs::values::PrefType;
use tokio::sync::watch;

/// Publishes the tuple of the three inputs. The first tuple is published at once, from the values the inputs hold, and
/// then one tuple after each change of any input. A `watch` receiver always holds a value, so "wait until every input has
/// a value" is already true when the inputs are created; the first publication is therefore immediate. Input changes that
/// arrive together are published once.
pub fn combine3<A, B, C>(
    mut a: watch::Receiver<A>,
    mut b: watch::Receiver<B>,
    mut c: watch::Receiver<C>,
) -> watch::Receiver<(A, B, C)>
where
    A: Clone + Send + Sync + 'static,
    B: Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
{
    let first = (
        a.borrow_and_update().clone(),
        b.borrow_and_update().clone(),
        c.borrow_and_update().clone(),
    );
    let (tx, rx) = watch::channel(first);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                changed = a.changed() => if changed.is_err() { break },
                changed = b.changed() => if changed.is_err() { break },
                changed = c.changed() => if changed.is_err() { break },
            }
            let value = (
                a.borrow_and_update().clone(),
                b.borrow_and_update().clone(),
                c.borrow_and_update().clone(),
            );
            if tx.send(value).is_err() {
                break;
            }
        }
    });
    rx
}

/// Publishes the pair of the two inputs, the same way as [`combine3`]: the first pair at once, then one pair after each
/// change of either input.
pub fn combine2<A, B>(mut a: watch::Receiver<A>, mut b: watch::Receiver<B>) -> watch::Receiver<(A, B)>
where
    A: Clone + Send + Sync + 'static,
    B: Clone + Send + Sync + 'static,
{
    let first = (a.borrow_and_update().clone(), b.borrow_and_update().clone());
    let (tx, rx) = watch::channel(first);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                changed = a.changed() => if changed.is_err() { break },
                changed = b.changed() => if changed.is_err() { break },
            }
            let value = (a.borrow_and_update().clone(), b.borrow_and_update().clone());
            if tx.send(value).is_err() {
                break;
            }
        }
    });
    rx
}

/// The value of `pref` first, then each change of it (`pref.changes()`). The store is watched before the value is read,
/// so a change in between is not lost. A bridge thread waits on the store; it ends when the store is dropped, or at the
/// next change after the receiver is dropped. The store only reports real changes, so no duplicate is published.
pub fn pref_stream<T>(pref: &Pref<T>) -> watch::Receiver<T>
where
    T: PrefType + Send + Sync,
{
    let changes = pref.watch();
    let (tx, rx) = watch::channel(pref.get());
    std::thread::spawn(move || {
        while let Some(value) = changes.changed() {
            if tx.send(value).is_err() {
                break;
            }
        }
    });
    rx
}

/// Drops every item equal to the one before it (`distinctUntilChanged`). The first item is always kept.
pub fn distinct_until_changed<T: PartialEq>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut out: Vec<T> = Vec::new();
    for item in items {
        if out.last() != Some(&item) {
            out.push(item);
        }
    }
    out
}

/// Runs `fut` to its end even when the caller stops waiting for it (`withNonCancellableContext`, EC-29). The future runs
/// as its own task. A panic in it is resumed in the caller, as it would be inline.
pub async fn non_cancellable<F>(fut: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match tokio::spawn(fut).await {
        Ok(value) => value,
        Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
        Err(_) => panic!("a non-cancellable task was cancelled by runtime shutdown"),
    }
}

/// How long a test waits for a published value before it fails.
#[cfg(test)]
pub(crate) const TEST_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use kuro_prefs::store::PrefStore;

    use super::*;

    #[tokio::test]
    async fn combine3_publishes_once_per_later_change_of_any_input() {
        let (ta, ra) = watch::channel(1);
        let (tb, rb) = watch::channel("b");
        let (tc, rc) = watch::channel(0.5);
        let mut out = combine3(ra, rb, rc);
        assert_eq!(*out.borrow_and_update(), (1, "b", 0.5), "the first tuple is published at once");

        ta.send(2).expect("a");
        tokio::time::timeout(TEST_WAIT, out.changed()).await.expect("waited").expect("open");
        assert_eq!(*out.borrow_and_update(), (2, "b", 0.5));

        tb.send("c").expect("b");
        tokio::time::timeout(TEST_WAIT, out.changed()).await.expect("waited").expect("open");
        assert_eq!(*out.borrow_and_update(), (2, "c", 0.5));

        tc.send(1.5).expect("c");
        tokio::time::timeout(TEST_WAIT, out.changed()).await.expect("waited").expect("open");
        assert_eq!(*out.borrow_and_update(), (2, "c", 1.5));
        assert!(!out.has_changed().expect("open"), "exactly one publication per change");
    }

    #[tokio::test]
    async fn combine3_ends_when_an_input_is_dropped() {
        let (ta, ra) = watch::channel(1);
        let (_tb, rb) = watch::channel(2);
        let (_tc, rc) = watch::channel(3);
        let mut out = combine3(ra, rb, rc);
        drop(ta);
        let ended = tokio::time::timeout(TEST_WAIT, out.changed()).await.expect("waited");
        assert!(ended.is_err(), "the output closes with its input");
    }

    #[test]
    fn distinct_until_changed_drops_equal_neighbours_only() {
        assert_eq!(distinct_until_changed([1, 1, 2, 2, 2, 1, 1]), vec![1, 2, 1]);
        assert_eq!(distinct_until_changed(Vec::<i32>::new()), Vec::<i32>::new());
    }

    #[test]
    fn pref_stream_yields_the_current_value_first_then_each_change() {
        let store = PrefStore::in_memory();
        let pref = store.pref("n", 1i32);
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("runtime");
        rt.block_on(async {
            pref.set(5);
            let mut stream = pref_stream(&pref);
            assert_eq!(*stream.borrow_and_update(), 5, "the current value comes first");
            pref.set(6);
            tokio::time::timeout(TEST_WAIT, stream.changed()).await.expect("waited").expect("open");
            assert_eq!(*stream.borrow_and_update(), 6);
            pref.set(6);
            pref.set(7);
            tokio::time::timeout(TEST_WAIT, stream.changed()).await.expect("waited").expect("open");
            assert_eq!(*stream.borrow_and_update(), 7);
        });
    }

    #[tokio::test]
    async fn non_cancellable_keeps_running_after_the_caller_is_dropped() {
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        let work = non_cancellable(async move {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            flag.store(true, Ordering::SeqCst);
        });
        let dropped = tokio::time::timeout(std::time::Duration::from_millis(1), work).await;
        assert!(dropped.is_err(), "the caller gave up first");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(done.load(Ordering::SeqCst), "the work finished without its caller");
    }

    #[tokio::test]
    async fn non_cancellable_returns_the_value_of_the_work() {
        assert_eq!(non_cancellable(async { 41 + 1 }).await, 42);
    }
}
