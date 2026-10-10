//! Preference observation bridge (`collectAsState`, T-49b-035). A widget follows a preference: it
//! gets the current value at once, then each later change, and nothing runs once it is destroyed.
//!
//! The store signals changes on a plain channel. One thread per observation blocks on that channel
//! and pings the main context, so a change is delivered on the next main-loop iteration, with no
//! timer. Values are read on the main thread (`PrefType` is not `Send`), the thread only carries the ping.

use std::sync::mpsc::Receiver;

use async_channel::{Receiver as ValueReceiver, Sender};
use gtk::glib;
use gtk::prelude::*;
use kuro_prefs::pref::Pref;
use kuro_prefs::store::{Changed, PrefStore};
use kuro_prefs::values::PrefType;

/// Pings `tx` for each change of `key` (`None` means many keys changed). Ends when the store is gone
/// or the main side dropped its receiver.
fn forward(events: Receiver<Changed>, key: String, tx: Sender<()>) {
    while let Ok(Changed { key: changed }) = events.recv() {
        if changed.as_deref().is_none_or(|k| k == key) && tx.send_blocking(()).is_err() {
            break;
        }
    }
}

/// The current value, and a receiver of each later value. `store` is the store `pref` lives in. Must run on the main-context thread. Dropping
/// the receiver ends the observation.
pub fn observe<T: PrefType>(store: &PrefStore, pref: &Pref<T>) -> (T, ValueReceiver<T>) {
    let (ping_tx, ping_rx) = async_channel::unbounded::<()>();
    let (value_tx, value_rx) = async_channel::unbounded::<T>();
    let events = store.subscribe();
    let key = pref.key().to_owned();
    std::thread::spawn(move || forward(events, key, ping_tx));
    let current = pref.get();
    let pref = pref.clone();
    glib::spawn_future_local(async move {
        while ping_rx.recv().await.is_ok() {
            if value_tx.send(pref.get()).await.is_err() {
                break;
            }
        }
    });
    (current, value_rx)
}

/// Calls `on_change` with the current value now, then with each change while `widget` lives.
pub fn bind<T, F>(widget: &impl IsA<gtk::Widget>, store: &PrefStore, pref: &Pref<T>, on_change: F)
where
    T: PrefType,
    F: Fn(T) + 'static,
{
    let (value, rx) = observe(store, pref);
    on_change(value);
    let weak = widget.as_ref().downgrade();
    glib::spawn_future_local(async move {
        while let Ok(value) = rx.recv().await {
            if weak.upgrade().is_none() {
                break;
            }
            on_change(value);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;
    use std::time::Duration;

    fn fake() -> (PrefStore, Pref<i32>) {
        let store = PrefStore::in_memory();
        let pref = Pref::new(store.clone(), "observed", 5_i32);
        (store, pref)
    }

    /// Runs the main context of the GTK test thread until `done` holds, at most two seconds.
    fn pump(done: impl Fn() -> bool) {
        let ctx = glib::MainContext::default();
        let start = std::time::Instant::now();
        while !done() && start.elapsed() < Duration::from_secs(2) {
            ctx.iteration(false);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn the_current_value_comes_first_and_a_change_follows() {
        crate::testing::with_display(|| {
            let (store, pref) = fake();
            let (value, rx) = observe(&store, &pref);
            assert_eq!(value, 5);
            pref.set(7).unwrap();
            pump(|| !rx.is_empty());
            assert_eq!(rx.try_recv().ok(), Some(7));
        });
    }

    #[test]
    fn a_change_to_another_key_is_not_delivered() {
        crate::testing::with_display(|| {
            let (store, pref) = fake();
            let (_, rx) = observe(&store, &pref);
            Pref::new(store.clone(), "other", 0_i32).set(1).unwrap();
            pump(|| !rx.is_empty());
            assert!(rx.try_recv().is_err());
        });
    }

    #[test]
    fn the_thread_ends_when_the_receiver_is_dropped() {
        let store = PrefStore::in_memory();
        let events = store.subscribe();
        let (tx, rx) = async_channel::unbounded();
        drop(rx);
        let worker = std::thread::spawn(move || forward(events, "k".into(), tx));
        Pref::new(store, "k", 0_i32).set(1).unwrap();
        worker.join().expect("the forwarding thread ends");
    }
}
