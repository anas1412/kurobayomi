//! T-27: pending attempts. The last attempt wins, and a callback with no pending attempt changes nothing.

use kuro_track::model::TrackerId;
use kuro_track::oauth::{PendingLogins, Route};

fn state_of(url: &str) -> String {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned()
}

#[test]
fn t_27_the_callback_uses_the_last_attempt() {
    let mut pending = PendingLogins::default();
    let first = state_of(&pending.begin(TrackerId::SIMKL, 0).unwrap());
    let second = state_of(&pending.begin(TrackerId::SIMKL, 1).unwrap());
    assert_ne!(first, second);
    assert_eq!(
        pending.route(&format!("aniyomi://simkl-auth?code=c&state={first}"), 2),
        Route::StateMismatch(TrackerId::SIMKL),
        "the first attempt was replaced"
    );
    let Route::Accepted(credential) =
        pending.route(&format!("aniyomi://simkl-auth?code=c&state={second}"), 3)
    else {
        panic!("the last attempt is accepted");
    };
    assert_eq!(credential.value, "c");
}

#[test]
fn t_27_a_callback_without_a_pending_attempt_is_not_allowed() {
    let mut pending = PendingLogins::default();
    assert_eq!(
        pending.route("aniyomi://simkl-auth?code=c", 0),
        Route::NotAllowed(TrackerId::SIMKL)
    );
}
