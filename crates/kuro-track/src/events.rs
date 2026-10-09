//! Change notifications of the tracking core (behaviour T-27b-040). Each event is sent after its write has committed,
//! so a subscriber that re-reads sees the new row or login state. The GUI forwards the receiver to its own thread.

use tokio::sync::broadcast;

use crate::model::TrackerId;

/// One committed change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackEvent {
    /// A track row of this entry was written (bind, progress, edit, remove).
    TrackChanged { entry_id: i64 },
    /// A tracker's login, username or auth-expired state was written (login, logout, expiry).
    LoginChanged { tracker_id: TrackerId },
}

/// Fan-out of [`TrackEvent`]. Sending with no subscriber is not an error.
#[derive(Debug, Clone)]
pub struct EventBus(broadcast::Sender<TrackEvent>);

impl Default for EventBus {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(64);
        Self(tx)
    }
}

impl EventBus {
    pub fn subscribe(&self) -> broadcast::Receiver<TrackEvent> {
        self.0.subscribe()
    }

    pub fn send(&self, event: TrackEvent) {
        let _ = self.0.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tr_27b_040_subscribers_receive_each_event_in_order() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        bus.send(TrackEvent::TrackChanged { entry_id: 4 });
        bus.send(TrackEvent::LoginChanged {
            tracker_id: TrackerId::ANILIST,
        });
        assert_eq!(
            rx.recv().await.unwrap(),
            TrackEvent::TrackChanged { entry_id: 4 }
        );
        assert_eq!(
            rx.recv().await.unwrap(),
            TrackEvent::LoginChanged {
                tracker_id: TrackerId::ANILIST
            }
        );
    }

    #[test]
    fn tr_27b_040_sending_without_a_subscriber_is_not_an_error() {
        EventBus::default().send(TrackEvent::TrackChanged { entry_id: 1 });
    }
}
