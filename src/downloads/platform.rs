//! The network feed of the download queue: the platform's power and network reading (`kuro_platform::power_network`,
//! NetworkManager with the desktop network monitor as the fallback) becomes the queue's `Connectivity` watch. Offline for
//! longer than the gate's debounce stops both lanes, and the return resumes only the lanes the network stopped.

use std::sync::Arc;

use kuro_downloads::next::gate::Connectivity;
use kuro_platform::power_network::{Conditions, POLL_INTERVAL, PowerNetworkState};
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// Keeps the feed running. Dropping it stops the reader and the forwarder.
pub struct NetworkFeed {
    tasks: [JoinHandle<()>; 2],
}

impl NetworkFeed {
    /// Starts reading the system and feeding `tx`. Needs a Tokio runtime. The queue starts as online and the first
    /// reading follows at once.
    pub fn start(tx: watch::Sender<Connectivity>) -> Self {
        let state = PowerNetworkState::new(Conditions::DESKTOP);
        let monitor = state.spawn_monitor(POLL_INTERVAL);
        Self { tasks: [monitor, forward(state, tx)] }
    }
}

impl Drop for NetworkFeed {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

fn connectivity(conditions: Conditions) -> Connectivity {
    Connectivity { online: conditions.connected, metered: conditions.metered }
}

/// Sends the state's conditions to `tx` now and after each change. The subscription is taken before the task starts, so
/// no change between the two is lost.
fn forward(state: Arc<PowerNetworkState>, tx: watch::Sender<Connectivity>) -> JoinHandle<()> {
    let mut changes = state.subscribe();
    tokio::spawn(async move {
        loop {
            tx.send_replace(connectivity(state.conditions()));
            if changes.changed().await.is_err() {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFLINE: Conditions = Conditions { connected: false, metered: true, on_ac: true, battery_low: false };

    async fn seen(rx: &mut watch::Receiver<Connectivity>, online: bool) {
        let wait = rx.wait_for(|c| c.online == online);
        tokio::time::timeout(std::time::Duration::from_secs(5), wait).await.expect("the feed answered").unwrap();
    }

    #[tokio::test]
    async fn the_queue_sees_the_network_go_and_come_back() {
        let state = PowerNetworkState::new(Conditions::DESKTOP);
        let (tx, mut rx) = watch::channel(Connectivity { online: false, metered: false });
        let task = forward(state.clone(), tx);
        seen(&mut rx, true).await;
        state.apply(OFFLINE);
        seen(&mut rx, false).await;
        state.apply(Conditions::DESKTOP);
        seen(&mut rx, true).await;
        task.abort();
    }
}
