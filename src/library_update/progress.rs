//! Carries the events of a pass from its worker task to the GTK main loop (T-26a-130). The pass sends from a tokio
//! worker; the consumer runs on the main context, so no widget is touched off the main thread. The channel is
//! unbounded, so a send never waits on the UI and a pass never stalls on a slow repaint.

use async_channel::{Receiver, Sender};

use crate::library_update::pass::PassEvent;

pub type EventSender = Sender<PassEvent>;

pub fn channel() -> (EventSender, Receiver<PassEvent>) {
    async_channel::unbounded()
}

/// Sends one event. Returns `false` when the receiver is gone (the window closed); the pass carries on regardless.
pub fn send(tx: &EventSender, event: PassEvent) -> bool {
    tx.try_send(event).is_ok()
}

/// Hands each event to `apply`, in order, until every sender is dropped. Returns how many were applied.
pub async fn pump(rx: Receiver<PassEvent>, mut apply: impl FnMut(PassEvent)) -> usize {
    let mut applied = 0;
    while let Ok(event) = rx.recv().await {
        apply(event);
        applied += 1;
    }
    applied
}

/// Starts the consumer on the GTK main context. Call from the main thread; `apply` may touch widgets.
pub fn attach_to_main_loop(rx: Receiver<PassEvent>, apply: impl FnMut(PassEvent) + 'static) {
    gtk::glib::spawn_future_local(pump(rx, apply));
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::library_update::pass::PassStatus;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime")
    }

    #[test]
    fn events_arrive_in_the_order_they_were_sent_and_the_pump_ends_with_the_senders() {
        let (tx, rx) = channel();
        assert!(send(&tx, PassEvent::Started { total: 2 }));
        assert!(send(&tx, PassEvent::EntryStarted { entry_id: 1 }));
        assert!(send(&tx, PassEvent::EntryFinished { entry_id: 1, done: 1 }));
        assert!(send(&tx, PassEvent::Finished { status: PassStatus::Success }));
        drop(tx);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        let applied = runtime().block_on(pump(rx, move |e| sink.borrow_mut().push(e)));
        assert_eq!(applied, 4);
        assert_eq!(
            *seen.borrow(),
            vec![
                PassEvent::Started { total: 2 },
                PassEvent::EntryStarted { entry_id: 1 },
                PassEvent::EntryFinished { entry_id: 1, done: 1 },
                PassEvent::Finished { status: PassStatus::Success },
            ]
        );
    }

    #[test]
    fn dropping_the_receiver_does_not_panic_the_sender() {
        let (tx, rx) = channel();
        drop(rx);
        assert!(!send(&tx, PassEvent::Started { total: 1 }), "a closed channel reports the loss and nothing else");
    }

    #[test]
    fn a_send_never_blocks_however_many_events_queue_up() {
        let (tx, rx) = channel();
        for entry_id in 0..10_000 {
            assert!(send(&tx, PassEvent::EntryStarted { entry_id }));
        }
        drop(tx);
        let applied = runtime().block_on(pump(rx, |_| {}));
        assert_eq!(applied, 10_000);
    }
}
