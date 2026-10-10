//! T-29: an empty client-secret config disables the browser login of the trackers that need a secret (E-63).

use kuro_track::messages::MsgKey;
use kuro_track::model::TrackerId;
use kuro_track::oauth::{ClientSecrets, PendingLogins};

#[test]
fn t_29_client_secret_missing_disables_login_with_a_toast() {
    let empty = ClientSecrets::default();
    let mut pending = PendingLogins::default();
    for (tracker, name) in [
        (TrackerId::SHIKIMORI, "Shikimori"),
        (TrackerId::BANGUMI, "Bangumi"),
        (TrackerId::SIMKL, "Simkl"),
    ] {
        assert!(!empty.allows_login(tracker));
        assert_eq!(
            pending.begin_configured(tracker, name, &empty, 0),
            Err(MsgKey::TrackerNotConfigured(name.to_owned()))
        );
    }
    assert!(empty.allows_login(TrackerId::ANILIST));
}
