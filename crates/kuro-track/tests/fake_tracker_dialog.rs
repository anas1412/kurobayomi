//! The login and logout dialog models over `kuro_testkit::FakeTracker` (T-01a-039). An integration test, so the
//! tracker implements the same `Tracker` trait that this crate exports.

use kuro_testkit::FakeTracker;
use kuro_track::dialog::{LoginForm, login_title, run_login, run_logout};
use kuro_track::messages::MsgKey;
use kuro_track::model::TrackerId;

#[test]
fn the_form_starts_with_the_fake_trackers_stored_values() {
    let tracker = FakeTracker::new(TrackerId::ANILIST, "AniList");
    let form = LoginForm::for_tracker(&tracker);
    assert_eq!(
        (form.username.as_str(), form.password.as_str()),
        ("username", "passw0rd")
    );
    assert!(form.can_submit());
    assert_eq!(login_title(tracker.name.as_str()), "Log in to AniList");
}

#[tokio::test]
async fn a_login_through_the_fake_succeeds_and_the_logout_reports() {
    let tracker = FakeTracker::new(TrackerId::ANILIST, "AniList");
    assert_eq!(
        run_login(&tracker, "user", "pass").await,
        Ok(MsgKey::LoginSuccess)
    );
    assert!(run_logout(&tracker).is_ok());
    assert!(
        tracker.client().is_err(),
        "the client is an error value, not a panic"
    );
}
