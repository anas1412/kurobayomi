//! Power and network state for scheduled jobs (T-11a-007). A job that needs a connection, an unmetered network or
//! external power asks [`PowerNetworkState`]; a job whose condition is unmet waits for the watch value to change and
//! checks again. The state is a plain snapshot behind a lock, so it never blocks and never touches a UI thread.
//!
//! Linux fills the snapshot from NetworkManager and UPower over the system bus. When NetworkManager is not on the bus
//! the network part comes from the GIO network monitor instead. Other platforms report a connected, unmetered
//! machine on external power until they get their own reader.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

/// How often [`PowerNetworkState::spawn_monitor`] reads the system again.
pub const POLL_INTERVAL: Duration = Duration::from_secs(10);

/// The conditions at one moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conditions {
    /// The machine has a connection that reaches the network.
    pub connected: bool,
    /// The connection is metered, or its metering is not known.
    pub metered: bool,
    /// The machine runs on external power. True on a machine without a battery.
    pub on_ac: bool,
    /// The battery has reached its low warning level. False on a machine without a battery.
    pub battery_low: bool,
}

impl Conditions {
    /// What a desktop on a wired connection with mains power reports.
    pub const DESKTOP: Self = Self { connected: true, metered: false, on_ac: true, battery_low: false };
}

/// The shared snapshot of [`Conditions`] and the channel that announces changes.
pub struct PowerNetworkState {
    snapshot: Mutex<Conditions>,
    changes: watch::Sender<u64>,
}

impl PowerNetworkState {
    /// A state that starts at `initial`. Nothing updates it until [`PowerNetworkState::apply`] or
    /// [`PowerNetworkState::spawn_monitor`] runs.
    pub fn new(initial: Conditions) -> Arc<Self> {
        Arc::new(Self { snapshot: Mutex::new(initial), changes: watch::channel(0).0 })
    }

    pub fn conditions(&self) -> Conditions {
        *self.snapshot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn is_metered(&self) -> bool {
        self.conditions().metered
    }

    pub fn is_connected(&self) -> bool {
        self.conditions().connected
    }

    pub fn on_ac_power(&self) -> bool {
        self.conditions().on_ac
    }

    pub fn is_battery_low(&self) -> bool {
        self.conditions().battery_low
    }

    /// A receiver whose value goes up by one each time the conditions change. Its first value is the current count.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    /// Stores `next`. The watch value moves only when something differs. Returns whether it did.
    pub fn apply(&self, next: Conditions) -> bool {
        let changed = {
            let mut snapshot = self.snapshot.lock().unwrap_or_else(PoisonError::into_inner);
            let changed = *snapshot != next;
            *snapshot = next;
            changed
        };
        if changed {
            self.changes.send_modify(|count| *count += 1);
        }
        changed
    }

    /// Reads the system once now and then every `interval`, applying each reading. Needs a tokio runtime. Dropping the
    /// handle does not stop the task; abort it to stop.
    pub fn spawn_monitor(self: &Arc<Self>, interval: Duration) -> JoinHandle<()> {
        let state = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                state.apply(read_conditions().await);
                tokio::time::sleep(interval).await;
            }
        })
    }
}

/// Reads the conditions from the system. Never fails: a part that cannot be read counts as not met for the network
/// (not connected, metered) and as met for power (on external power, battery not low), so a missing reader makes a job
/// wait for the network but not for a battery it cannot see.
pub async fn read_conditions() -> Conditions {
    #[cfg(all(target_os = "linux", feature = "platform-linux"))]
    {
        crate::linux::power_network::read().await
    }
    #[cfg(not(all(target_os = "linux", feature = "platform-linux")))]
    {
        Conditions::DESKTOP
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFLINE: Conditions = Conditions { connected: false, metered: true, on_ac: false, battery_low: false };

    #[test]
    fn the_accessors_read_the_snapshot() {
        let state = PowerNetworkState::new(OFFLINE);
        assert!(!state.is_connected());
        assert!(state.is_metered());
        assert!(!state.on_ac_power());
        assert!(!state.is_battery_low());
        state.apply(Conditions::DESKTOP);
        assert!(state.is_connected() && !state.is_metered() && state.on_ac_power());
    }

    #[test]
    fn the_watch_value_moves_once_per_real_change() {
        let state = PowerNetworkState::new(OFFLINE);
        let mut rx = state.subscribe();
        assert_eq!(*rx.borrow_and_update(), 0);
        assert!(!state.apply(OFFLINE), "the same reading is not a change");
        assert!(!rx.has_changed().unwrap());
        assert!(state.apply(Conditions::DESKTOP));
        assert!(rx.has_changed().unwrap());
        assert_eq!(*rx.borrow_and_update(), 1);
        assert!(state.apply(OFFLINE));
        assert_eq!(*rx.borrow_and_update(), 2);
    }

    #[tokio::test]
    async fn a_waiting_subscriber_wakes_when_the_network_comes_back() {
        let state = PowerNetworkState::new(OFFLINE);
        let mut rx = state.subscribe();
        let waiter = tokio::spawn(async move {
            rx.changed().await.unwrap();
        });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        state.apply(Conditions::DESKTOP);
        tokio::time::timeout(Duration::from_secs(5), waiter).await.expect("woken").unwrap();
    }

    #[tokio::test]
    async fn reading_the_system_never_fails() {
        // Whatever the machine has, a reading comes back; the monitor applies it and stays alive.
        let state = PowerNetworkState::new(OFFLINE);
        let monitor = state.spawn_monitor(Duration::from_millis(5));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!monitor.is_finished());
        monitor.abort();
    }
}
