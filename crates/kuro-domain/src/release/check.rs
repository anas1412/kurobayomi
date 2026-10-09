//! The release check flow (`GetApplicationRelease.kt`): the three-day throttle, the lookup, and the decision of
//! whether `last_app_check` moves. The stored time itself is the caller's: [`Check::checked_at`] says what to write.

use super::version::{is_update, Current};
use super::{Release, ReleaseError, ReleaseService};

/// Minimum time between two lookups, in milliseconds (three days).
pub const CHECK_INTERVAL_MS: i64 = 3 * 24 * 60 * 60 * 1000;

/// What the check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    NoNewUpdate,
    NewUpdate(Release),
}

/// The result of one check. `checked_at` is the time to store as `last_app_check`, or `None` to keep the stored one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub outcome: Outcome,
    pub checked_at: Option<i64>,
}

/// True when a lookup at `now` is too soon after one at `last_checked`: fewer than three days have passed.
pub fn is_throttled(now: i64, last_checked: i64) -> bool {
    now < last_checked + CHECK_INTERVAL_MS
}

/// Runs the check. `repository` empty disables it (`UPDATE_REPOSITORY`), as does a throttled call unless `force` is set;
/// neither calls the service. A lookup that returns no release (no asset for this host) leaves the stored time alone,
/// as does an error, which is returned. A release that comes back moves the time to `now` and is compared with
/// `current` (a release version, or a preview build's commit count).
pub fn check_for_update(
    service: &dyn ReleaseService,
    repository: &str,
    current: &Current,
    last_checked: i64,
    now: i64,
    force: bool,
) -> Result<Check, ReleaseError> {
    let nothing = Check { outcome: Outcome::NoNewUpdate, checked_at: None };
    if repository.is_empty() || (!force && is_throttled(now, last_checked)) {
        return Ok(nothing);
    }
    let Some(release) = service.latest(repository)? else {
        return Ok(nothing);
    };
    let outcome = if is_update(&release.version, current) { Outcome::NewUpdate(release) } else { Outcome::NoNewUpdate };
    Ok(Check { outcome, checked_at: Some(now) })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    const DAY: i64 = 24 * 60 * 60 * 1000;
    const NOW: i64 = 100 * DAY;

    /// Counts the lookups and answers with `reply` each time.
    struct Counting {
        calls: Cell<u32>,
        reply: Result<Option<Release>, ReleaseError>,
    }

    impl Counting {
        fn new(reply: Result<Option<Release>, ReleaseError>) -> Self {
            Self { calls: Cell::new(0), reply }
        }
    }

    impl ReleaseService for Counting {
        fn latest(&self, _repository: &str) -> Result<Option<Release>, ReleaseError> {
            self.calls.set(self.calls.get() + 1);
            self.reply.clone()
        }
    }

    fn release(version: &str) -> Release {
        Release {
            version: version.into(),
            info: String::new(),
            release_link: "https://r".into(),
            download_link: "https://d".into(),
        }
    }

    fn check(svc: &Counting, last: i64, force: bool) -> Result<Check, ReleaseError> {
        check_for_update(svc, "o/r", &Current::Release("1.0.0".into()), last, NOW, force)
    }

    #[test]
    fn a_newer_release_is_an_update_and_moves_the_stored_time() {
        let svc = Counting::new(Ok(Some(release("v2.0.0"))));
        let got = check(&svc, 0, false).unwrap();
        assert_eq!(got, Check { outcome: Outcome::NewUpdate(release("v2.0.0")), checked_at: Some(NOW) });
    }

    #[test]
    fn an_older_or_equal_release_is_no_update_but_still_moves_the_time() {
        let svc = Counting::new(Ok(Some(release("v1.0.0"))));
        assert_eq!(check(&svc, 0, false).unwrap(), Check { outcome: Outcome::NoNewUpdate, checked_at: Some(NOW) });
    }

    #[test]
    fn inside_three_days_nothing_is_asked_for_and_nothing_moves() {
        let svc = Counting::new(Ok(Some(release("v9.0.0"))));
        let got = check(&svc, NOW - DAY, false).unwrap();
        assert_eq!(got, Check { outcome: Outcome::NoNewUpdate, checked_at: None });
        assert_eq!(svc.calls.get(), 0, "the service is never called");
    }

    #[test]
    fn exactly_three_days_is_not_throttled_and_one_millisecond_less_is() {
        let svc = Counting::new(Ok(Some(release("v9.0.0"))));
        assert!(check(&svc, NOW - CHECK_INTERVAL_MS, false).unwrap().checked_at.is_some());
        assert_eq!(svc.calls.get(), 1);
        let throttled = Counting::new(Ok(Some(release("v9.0.0"))));
        assert_eq!(check(&throttled, NOW - CHECK_INTERVAL_MS + 1, false).unwrap().checked_at, None);
        assert_eq!(throttled.calls.get(), 0);
    }

    #[test]
    fn force_bypasses_the_throttle() {
        let svc = Counting::new(Ok(Some(release("v9.0.0"))));
        assert!(check(&svc, NOW - DAY, true).unwrap().checked_at.is_some());
        assert_eq!(svc.calls.get(), 1);
    }

    #[test]
    fn no_asset_or_an_error_leaves_the_stored_time_alone() {
        let none = Counting::new(Ok(None));
        assert_eq!(check(&none, 0, false).unwrap(), Check { outcome: Outcome::NoNewUpdate, checked_at: None });
        let failing = Counting::new(Err(ReleaseError::Http(403)));
        assert_eq!(check(&failing, 0, false), Err(ReleaseError::Http(403)));
    }

    #[test]
    fn an_empty_repository_disables_the_check() {
        let svc = Counting::new(Ok(Some(release("v9.0.0"))));
        let got = check_for_update(&svc, "", &Current::Release("1.0.0".into()), 0, NOW, true).unwrap();
        assert_eq!(got.checked_at, None);
        assert_eq!(svc.calls.get(), 0);
    }

    #[test]
    fn a_nightly_tag_is_never_an_update() {
        let svc = Counting::new(Ok(Some(release("nightly"))));
        assert_eq!(check(&svc, 0, false).unwrap().outcome, Outcome::NoNewUpdate);
    }

    #[test]
    fn a_preview_build_is_offered_a_higher_numbered_tag() {
        let svc = Counting::new(Ok(Some(release("r2000"))));
        let current = Current::Preview { commit_count: 1000 };
        let got = check_for_update(&svc, "o/r", &current, 0, NOW, false).unwrap();
        assert_eq!(got, Check { outcome: Outcome::NewUpdate(release("r2000")), checked_at: Some(NOW) });
    }
}
