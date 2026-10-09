//! Source-ready gate (T-17a-011, area 17 section 6). A read-only view of the source registry's readiness flag. The
//! registry's first extension snapshot is the only writer, so there is one source of truth. A view that needs sources
//! waits on [`SourceGate::ready_watch`]: it reads `false` until the first snapshot, then `true` for good.

use tokio::sync::watch;

/// A read-only view of the source-ready flag.
pub struct SourceGate {
    ready: watch::Receiver<bool>,
}

impl SourceGate {
    /// Wraps the readiness receiver of the source registry (`SourceRegistry::subscribe_initialized`).
    pub fn new(ready: watch::Receiver<bool>) -> Self {
        Self { ready }
    }

    /// A receiver that reads the current state and is woken when it changes.
    pub fn ready_watch(&self) -> watch::Receiver<bool> {
        self.ready.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_starts_closed_and_a_receiver_made_later_reads_the_flip() {
        let (tx, rx) = watch::channel(false);
        let gate = SourceGate::new(rx);
        assert!(!*gate.ready_watch().borrow());
        tx.send_replace(true);
        assert!(*gate.ready_watch().borrow());
    }

    #[test]
    fn an_existing_receiver_is_woken_by_the_first_snapshot() {
        let (tx, rx) = watch::channel(false);
        let gate = SourceGate::new(rx);
        let mut waiting = gate.ready_watch();
        tx.send_replace(true);
        assert!(waiting.has_changed().unwrap());
        assert!(*waiting.borrow_and_update());
    }
}
