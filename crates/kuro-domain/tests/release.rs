//! Port of `GetApplicationReleaseTest` (T-09a-057): the release-check cases (the preview case is dropped with the preview channel, T-06a-073), through the public API. The fake
//! release source counts its calls, and the clock is passed in as `now`.

use std::cell::Cell;

use kuro_domain::release::check::{CHECK_INTERVAL_MS, Check, Outcome, check_for_update};
use kuro_domain::release::{Release, ReleaseError, ReleaseService};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const NOW: i64 = 100 * DAY_MS;
const REPOSITORY: &str = "owner/repo";

/// Answers every lookup with one fixed reply and counts the lookups.
struct FakeReleaseSource {
    reply: Option<Release>,
    calls: Cell<u32>,
}

impl FakeReleaseSource {
    fn answering(version: &str) -> Self {
        let reply = Release {
            version: version.to_owned(),
            info: String::new(),
            release_link: "https://example.invalid/release".to_owned(),
            download_link: "https://example.invalid/asset".to_owned(),
        };
        Self { reply: Some(reply), calls: Cell::new(0) }
    }
}

impl ReleaseService for FakeReleaseSource {
    fn latest(&self, _repository: &str) -> Result<Option<Release>, ReleaseError> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.reply.clone())
    }
}

#[test]
fn tc_rel_p2_when_has_update_expect_new_update() {
    let source = FakeReleaseSource::answering("v2.0.0");
    let current = "1.0.0";
    let got = check_for_update(&source, REPOSITORY, current, 0, NOW, false).expect("lookup succeeds");
    assert!(matches!(got.outcome, Outcome::NewUpdate(ref release) if release.version == "v2.0.0"));
    assert_eq!(got.checked_at, Some(NOW));
}

#[test]
fn tc_rel_p3_when_has_no_update_expect_no_new_update() {
    let source = FakeReleaseSource::answering("v1.0.0");
    let current = "1.0.0";
    let got = check_for_update(&source, REPOSITORY, current, 0, NOW, false).expect("lookup succeeds");
    assert_eq!(got, Check { outcome: Outcome::NoNewUpdate, checked_at: Some(NOW) });
}

#[test]
fn tc_rel_p4_when_now_is_before_three_days_expect_no_new_update_and_no_service_call() {
    let source = FakeReleaseSource::answering("v9.0.0");
    let current = "1.0.0";
    let last_checked = NOW - DAY_MS;
    assert!(NOW < last_checked + CHECK_INTERVAL_MS, "the stored check is inside the throttle window");
    let got = check_for_update(&source, REPOSITORY, current, last_checked, NOW, false).expect("no lookup, no error");
    assert_eq!(got, Check { outcome: Outcome::NoNewUpdate, checked_at: None });
    assert_eq!(source.calls.get(), 0, "the service is not called inside the throttle window");
}
