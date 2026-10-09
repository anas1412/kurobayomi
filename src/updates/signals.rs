//! Download-cache signals (T-42a-009). `changed` is a broadcast channel, so every subscriber sees every change (the
//! Kotlin single-consumer `Channel` lost them, EC-42-T-01). `initializing` is a watch channel, so a late subscriber
//! reads the current value at once and does not wait for the next change.

use tokio::sync::{broadcast, watch};

/// Capacity of the change channel: a slow subscriber lags past this many changes and then sees the newest.
const CHANGE_CAPACITY: usize = 64;

pub struct CacheSignals {
    changed: broadcast::Sender<()>,
    initializing: watch::Sender<bool>,
}

impl Default for CacheSignals {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheSignals {
    /// A new cache that is still initializing.
    pub fn new() -> Self {
        let (changed, _) = broadcast::channel(CHANGE_CAPACITY);
        let (initializing, _) = watch::channel(true);
        Self { changed, initializing }
    }

    /// Every change is delivered to every subscriber that exists when it is sent.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.changed.subscribe()
    }

    /// Announces a change. With no subscriber the change is dropped, which is correct: nobody is showing the cache.
    pub fn notify_changed(&self) {
        let _ = self.changed.send(());
    }

    /// The watch receiver: `borrow()` gives the latest value without waiting.
    pub fn initializing(&self) -> watch::Receiver<bool> {
        self.initializing.subscribe()
    }

    pub fn set_initializing(&self, initializing: bool) {
        self.initializing.send_replace(initializing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_subscribers_both_receive_one_change() {
        let signals = CacheSignals::new();
        let mut a = signals.subscribe();
        let mut b = signals.subscribe();
        signals.notify_changed();
        assert!(a.try_recv().is_ok());
        assert!(b.try_recv().is_ok());
        assert!(a.try_recv().is_err());
        assert!(b.try_recv().is_err());
    }

    #[test]
    fn a_change_with_no_subscriber_is_not_an_error() {
        let signals = CacheSignals::new();
        signals.notify_changed();
        let mut late = signals.subscribe();
        assert!(late.try_recv().is_err());
    }

    #[test]
    fn late_subscriber_reads_initializing_without_a_new_value() {
        let signals = CacheSignals::new();
        assert!(*signals.initializing().borrow());
        signals.set_initializing(false);
        let late = signals.initializing();
        assert!(!*late.borrow());
    }
}
