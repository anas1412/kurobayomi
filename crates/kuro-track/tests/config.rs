//! T-29: an empty client-secret config disables the browser login of the trackers that need a secret (E-63).
//! Shikimori and Bangumi are gated by their tracker's own client instead (T-31a-039).

use std::sync::Arc;

use kuro_testkit::FakeTracker;
use kuro_track::manager::{REGISTRY_ORDER, TrackerManager};
use kuro_track::messages::MsgKey;
use kuro_track::model::TrackerId;
use kuro_track::oauth::{ClientSecrets, PendingLogins};
use kuro_track::tracker::Tracker;

#[tokio::test]
async fn t_29_client_secret_missing_disables_login_with_a_toast() {
    let trackers = TrackerManager::new(
        REGISTRY_ORDER
            .iter()
            .map(|id| Arc::new(FakeTracker::new(*id, "x")) as Arc<dyn Tracker>)
            .collect(),
    )
    .unwrap();
    let empty = ClientSecrets::default();
    let mut pending = PendingLogins::default();
    assert!(!empty.allows_login(TrackerId::SIMKL));
    assert_eq!(
        pending.begin_configured(TrackerId::SIMKL, "Simkl", &empty, &trackers, 0),
        Err(MsgKey::TrackerNotConfigured("Simkl".to_owned()))
    );
    for (tracker, name) in [
        (TrackerId::SHIKIMORI, "Shikimori"),
        (TrackerId::BANGUMI, "Bangumi"),
    ] {
        assert_eq!(
            pending.begin_configured(tracker, name, &empty, &trackers, 0),
            Err(MsgKey::TrackerNotConfigured(name.to_owned())),
            "a tracker without a client gives no URL"
        );
    }
    assert!(empty.allows_login(TrackerId::ANILIST));
}
