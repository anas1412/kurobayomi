//! Process-level foreground and background events (`ProcessLifecycle`).
//!
//! The GUI feeds one `watch` value: `true` while any top-level window is active. `Started` is sent when
//! that value first becomes `true`. `Stopped` is sent once the value has stayed `false` for
//! [`STOP_DEBOUNCE`], so a dialog that briefly deactivates the main window does not count as leaving.

use std::time::Duration;

use tokio::sync::{broadcast, watch};

/// How long the app must stay inactive before `Stopped` is sent (`TIMEOUT_MS = 700` in AndroidX).
pub const STOP_DEBOUNCE: Duration = Duration::from_millis(700);

/// Capacity of the broadcast channel. Events are rare, so a small buffer is enough.
const CHANNEL_CAPACITY: usize = 16;

/// The foreground and background events of the whole process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    Started,
    Stopped,
}

/// Starts the lifecycle task on the current tokio runtime and returns the sender. Subscribers call
/// `subscribe()` on it and may receive on any thread. The task ends when `active` has no senders left.
pub fn spawn(active: watch::Receiver<bool>) -> broadcast::Sender<LifecycleEvent> {
    let (events, _) = broadcast::channel(CHANNEL_CAPACITY);
    tokio::spawn(run(active, events.clone()));
    events
}

/// The state machine behind [`spawn`]. Exposed so a caller can drive it on its own task.
pub async fn run(mut active: watch::Receiver<bool>, events: broadcast::Sender<LifecycleEvent>) {
    let mut running = false;
    let mut is_active = *active.borrow_and_update();
    loop {
        if is_active && !running {
            running = true;
            let _ = events.send(LifecycleEvent::Started);
        }
        if !is_active && running {
            tokio::select! {
                changed = active.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    is_active = *active.borrow_and_update();
                }
                () = tokio::time::sleep(STOP_DEBOUNCE) => {
                    // Re-read the value: a reactivation that raced the timer must not produce `Stopped`.
                    if !*active.borrow() {
                        running = false;
                        let _ = events.send(LifecycleEvent::Stopped);
                    }
                }
            }
        } else {
            if active.changed().await.is_err() {
                return;
            }
            is_active = *active.borrow_and_update();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn settle(ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    fn drain(rx: &mut broadcast::Receiver<LifecycleEvent>) -> Vec<LifecycleEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            out.push(ev);
        }
        out
    }

    #[tokio::test(start_paused = true)]
    async fn started_on_first_activation_and_no_stop_for_a_short_gap() {
        let (tx, rx) = watch::channel(false);
        let events = spawn(rx);
        let mut sub = events.subscribe();

        tx.send(true).unwrap();
        settle(10).await;
        tx.send(false).unwrap();
        settle(300).await;
        tx.send(true).unwrap();
        settle(1_000).await;

        assert_eq!(drain(&mut sub), [LifecycleEvent::Started]);
    }

    #[tokio::test(start_paused = true)]
    async fn one_stop_after_the_debounce_when_inactive_for_long_enough() {
        let (tx, rx) = watch::channel(true);
        let events = spawn(rx);
        let mut sub = events.subscribe();
        settle(10).await;
        assert_eq!(drain(&mut sub), [LifecycleEvent::Started], "already active at start counts as the first activation");

        tx.send(false).unwrap();
        settle(800).await;
        assert_eq!(drain(&mut sub), [LifecycleEvent::Stopped]);

        settle(5_000).await;
        assert_eq!(drain(&mut sub), [], "no second Stopped while still inactive");
    }

    #[tokio::test(start_paused = true)]
    async fn stop_then_start_again_gives_two_pairs() {
        let (tx, rx) = watch::channel(false);
        let events = spawn(rx);
        let mut sub = events.subscribe();
        tx.send(true).unwrap();
        settle(10).await;
        tx.send(false).unwrap();
        settle(1_000).await;
        tx.send(true).unwrap();
        settle(10).await;
        assert_eq!(
            drain(&mut sub),
            [LifecycleEvent::Started, LifecycleEvent::Stopped, LifecycleEvent::Started]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn nothing_is_sent_while_never_active() {
        let (_tx, rx) = watch::channel(false);
        let events = spawn(rx);
        let mut sub = events.subscribe();
        settle(5_000).await;
        assert_eq!(drain(&mut sub), []);
    }
}
