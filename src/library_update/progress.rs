//! Carries the events of a pass from its worker task to the GTK main loop (T-26a-130). The pass sends from a tokio
//! worker; the consumer runs on the main context, so no widget is touched off the main thread. The channel is
//! unbounded, so a send never waits on the UI and a pass never stalls on a slow repaint.

use std::sync::Mutex;

use async_channel::{Receiver, Sender};
use kuro_platform::kind::Kind;

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

/// A live event, tagged with the kind of the pass that sent it (the two kinds run side by side).
pub type LiveEvent = (Kind, PassEvent);

/// Fans the live events of the passes out to every subscriber (the library header bars). A subscriber that went away is
/// dropped on the next publish.
pub struct Feed {
    subscribers: Mutex<Vec<Sender<LiveEvent>>>,
}

impl Feed {
    pub const fn new() -> Self {
        Self { subscribers: Mutex::new(Vec::new()) }
    }

    pub fn subscribe(&self) -> Receiver<LiveEvent> {
        let (tx, rx) = async_channel::unbounded();
        self.subscribers.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
        rx
    }

    pub fn publish(&self, kind: Kind, event: PassEvent) {
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        subscribers.retain(|tx| tx.try_send((kind, event.clone())).is_ok());
    }
}

/// The feed of the app's passes: `PassRunner` publishes to it as a pass runs.
static LIVE: Feed = Feed::new();

/// Subscribes to the live events of every pass.
pub fn subscribe() -> Receiver<LiveEvent> {
    LIVE.subscribe()
}

/// Called by the pass at its start, after each entry and at its end (T-26a-111).
pub fn publish(kind: Kind, event: PassEvent) {
    LIVE.publish(kind, event);
}

fn show_fraction(bar: &gtk::ProgressBar, fraction: Option<f64>) {
    use gtk::prelude::*;
    match fraction {
        Some(fraction) => {
            bar.set_fraction(fraction);
            bar.set_visible(true);
        }
        None => bar.set_visible(false),
    }
}

/// The determinate bar for the library header of `kind`: hidden while idle, filled while a pass of that kind runs
/// (T-26a-111). The library header appends it once; it follows the live events on the main context.
pub fn header_bar(kind: Kind) -> gtk::ProgressBar {
    header_bar_from(subscribe(), kind)
}

fn header_bar_from(rx: Receiver<LiveEvent>, kind: Kind) -> gtk::ProgressBar {
    use gtk::prelude::*;
    let bar = gtk::ProgressBar::new();
    bar.set_visible(false);
    let weak = bar.downgrade();
    gtk::glib::spawn_future_local(async move {
        let mut state = HeaderProgress::default();
        while let Ok((sent_by, event)) = rx.recv().await {
            // the window closed: stop listening, the feed drops the subscription
            let Some(bar) = weak.upgrade() else { break };
            if sent_by == kind {
                state.apply(&event);
                show_fraction(&bar, state.fraction());
            }
        }
    });
    bar
}

/// The library header's progress bar (T-26a-111), folded from the pass events. The bar is determinate while a pass runs
/// and hidden when the pass is idle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeaderProgress {
    running: bool,
    done: usize,
    total: usize,
}

impl HeaderProgress {
    pub fn apply(&mut self, event: &PassEvent) {
        match event {
            PassEvent::Started { total } => *self = Self { running: true, done: 0, total: *total },
            PassEvent::EntryStarted { .. } => {}
            PassEvent::EntryFinished { done, .. } => self.done = *done,
            PassEvent::Finished { .. } => self.running = false,
        }
    }

    /// The fraction to show, or `None` to hide the bar (idle, or nothing to count).
    pub fn fraction(&self) -> Option<f64> {
        (self.running && self.total > 0).then(|| self.done.min(self.total) as f64 / self.total as f64)
    }
}

#[cfg(test)]
mod header_tests {
    use super::*;
    use crate::library_update::pass::PassStatus;

    #[test]
    fn the_bar_is_hidden_when_idle_and_determinate_while_a_pass_runs() {
        // T-26a-111
        let mut header = HeaderProgress::default();
        assert_eq!(header.fraction(), None);
        header.apply(&PassEvent::Started { total: 4 });
        assert_eq!(header.fraction(), Some(0.0));
        header.apply(&PassEvent::EntryFinished { entry_id: 1, done: 2 });
        assert_eq!(header.fraction(), Some(0.5));
        header.apply(&PassEvent::Finished { status: PassStatus::Success });
        assert_eq!(header.fraction(), None);
    }

    #[test]
    fn a_pass_with_nothing_to_count_shows_no_bar() {
        let mut header = HeaderProgress::default();
        header.apply(&PassEvent::Started { total: 0 });
        assert_eq!(header.fraction(), None);
    }
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

#[cfg(test)]
mod bar_tests {
    use super::*;
    use crate::library_update::pass::PassStatus;
    use gtk::prelude::*;

    fn settle() {
        let context = gtk::glib::MainContext::default();
        for _ in 0..50 {
            while context.iteration(false) {}
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    #[test]
    fn the_header_bar_is_hidden_when_idle_and_follows_a_pass_of_its_kind_only() {
        // T-26a-111
        crate::testing::with_display(|| {
            let feed = Feed::new();
            let bar = header_bar_from(feed.subscribe(), Kind::Manga);
            assert!(!bar.is_visible());
            feed.publish(Kind::Manga, PassEvent::Started { total: 4 });
            feed.publish(Kind::Anime, PassEvent::EntryFinished { entry_id: 9, done: 4 });
            settle();
            assert!(bar.is_visible());
            assert_eq!(bar.fraction(), 0.0);
            feed.publish(Kind::Manga, PassEvent::EntryFinished { entry_id: 1, done: 2 });
            settle();
            assert_eq!(bar.fraction(), 0.5);
            feed.publish(Kind::Manga, PassEvent::Finished { status: PassStatus::Success });
            settle();
            assert!(!bar.is_visible());
        });
    }

    #[test]
    fn a_feed_drops_a_subscriber_that_went_away() {
        let feed = Feed::new();
        let kept = feed.subscribe();
        drop(feed.subscribe());
        feed.publish(Kind::Anime, PassEvent::Started { total: 1 });
        assert_eq!(feed.subscribers.lock().unwrap().len(), 1);
        assert_eq!(kept.try_recv().ok(), Some((Kind::Anime, PassEvent::Started { total: 1 })));
    }
}
