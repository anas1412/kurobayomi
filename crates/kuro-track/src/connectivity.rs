//! Network reachability (`NetworkExtensions.kt`, `Context.isOnline()`). The Linux adapter over
//! `gio::NetworkMonitor::is_network_available()` is wired by the composition root, because this crate keeps
//! GIO out of its dependencies (`lib.rs`). GIO reports link availability, not internet reachability.

use std::time::Duration;

use async_trait::async_trait;

/// How often `wait_online` looks again. One second is the bound of T-27a-042.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[async_trait]
pub trait Connectivity: Send + Sync {
    /// True when the OS reports a usable network.
    fn is_online(&self) -> bool;

    /// Resolves once `is_online` is true. The default polls every [`POLL_INTERVAL`]. An adapter with a change
    /// signal may override it.
    async fn wait_online(&self) {
        while !self.is_online() {
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    /// Fake monitor: the test flips the flag.
    struct FakeNet(AtomicBool);

    impl Connectivity for FakeNet {
        fn is_online(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    #[tokio::test(start_paused = true)]
    async fn tr_27_042_wait_online_resolves_within_one_second_of_the_network_returning() {
        let net = Arc::new(FakeNet(AtomicBool::new(false)));
        let flip = Arc::clone(&net);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(2300)).await;
            flip.0.store(true, Ordering::SeqCst);
        });
        let start = tokio::time::Instant::now();
        net.wait_online().await;
        let waited = start.elapsed();
        assert!(waited >= Duration::from_millis(2300));
        assert!(waited <= Duration::from_millis(2300) + Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn tr_27_042_is_online_reads_the_monitor_without_waiting() {
        let net = FakeNet(AtomicBool::new(true));
        assert!(net.is_online());
        let start = tokio::time::Instant::now();
        net.wait_online().await;
        assert_eq!(start.elapsed(), Duration::ZERO);
    }
}
