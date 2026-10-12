//! The entry points of the tracking button and the sheet (T-38a-031, T-38a-032, T-38b-029; behaviour 11.4, defect
//! D13). The button text and the tap are the rules of `entry_core::action_row`. The sheet opens after a category
//! change only when the change came from this flow, and the auto-open follows `kuro_track::policy`.

use std::collections::HashSet;

use kuro_track::error::TrackError;
use kuro_track::messages::MsgKey;
use kuro_track::policy::auto_open_track_sheet;

/// The sheet opens after the category dialog closed only when the change came from this flow, auto-open is on and the
/// entry already has a supported binding (E-74, D13).
pub fn sheet_after_category_change(from_flow: bool, track_on_adding: bool, tracking_count: usize) -> bool {
    from_flow && auto_open_track_sheet(track_on_adding, tracking_count)
}

/// The track sheet opens after an add to the library only when the `track_on_adding_to_library` preference is on and at
/// least one tracker is logged in (T-39b-048, `T-LIB-5`).
pub fn open_track_sheet_on_add(pref_on: bool, logged_in: bool) -> bool {
    pref_on && logged_in
}

/// The refresh failure of a tracker. The service name is the one captured when the refresh failed (E-TRK-6).
pub fn refresh_failure(service: &str, error: &TrackError) -> MsgKey {
    error.toast(service)
}

/// One toast per failing tracker, in the order the failures came (T-39b-049, `MR track_error`). Each failure is its own
/// toast, so one failing tracker does not hide the others (E-TRK-14).
pub fn refresh_toasts(failures: &[(String, TrackError)]) -> Vec<MsgKey> {
    failures.iter().map(|(service, error)| refresh_failure(service, error)).collect()
}

/// Single-flight refresh per track id (RC-10): a refresh of a binding that is already running is not started again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefreshFlight {
    running: HashSet<i64>,
}

impl RefreshFlight {
    /// `true` when the caller may start the refresh of `track_id`; `false` while one is still running.
    pub fn begin(&mut self, track_id: i64) -> bool {
        self.running.insert(track_id)
    }

    /// Marks the refresh of `track_id` as finished, success or failure.
    pub fn end(&mut self, track_id: i64) {
        self.running.remove(&track_id);
    }
}

/// The refresh on open runs once per opening of the sheet, and reports its failures as banners (behaviour 5.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefreshOnOpen {
    started: bool,
}

impl RefreshOnOpen {
    /// `true` the first time it is asked in one opening of the sheet, `false` after that.
    pub fn begin(&mut self) -> bool {
        !std::mem::replace(&mut self.started, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_core::action_row::{TrackingLabel, tracking_label};

    #[test]
    fn the_sheet_opens_after_a_category_change_only_from_this_flow() {
        assert!(sheet_after_category_change(true, true, 1));
        assert!(!sheet_after_category_change(false, true, 1), "a change from elsewhere does not open it");
        assert!(!sheet_after_category_change(true, false, 1), "auto-open off");
        assert!(!sheet_after_category_change(true, true, 0), "no supported binding");
    }

    #[test]
    fn the_button_reads_tracking_with_no_tracker_and_the_count_otherwise() {
        assert_eq!(tracking_label(0), TrackingLabel::Tracking);
        assert_eq!(tracking_label(2), TrackingLabel::Trackers(2));
    }

    #[test]
    fn refresh_on_open_runs_once_per_opening() {
        let mut gate = RefreshOnOpen::default();
        assert!(gate.begin());
        assert!(!gate.begin(), "a second ask in the same opening does nothing");
    }

    #[test]
    fn a_refresh_failure_names_the_service_it_was_captured_for() {
        assert_eq!(refresh_failure("AniList", &TrackError::UnknownTracker(2)), TrackError::UnknownTracker(2).toast("AniList"));
    }

    #[test]
    fn the_track_sheet_opens_on_add_only_with_the_pref_and_a_logged_in_tracker() {
        assert!(open_track_sheet_on_add(true, true));
        assert!(!open_track_sheet_on_add(false, true), "preference off");
        assert!(!open_track_sheet_on_add(true, false), "no logged-in tracker");
    }

    #[test]
    fn each_failing_tracker_gets_its_own_toast_and_the_others_still_show() {
        let failures = vec![
            ("AniList".to_owned(), TrackError::Message("timeout".to_owned())),
            ("MyAnimeList".to_owned(), TrackError::UnknownTracker(3)),
        ];
        let toasts = refresh_toasts(&failures);
        assert_eq!(toasts.len(), 2, "one failure does not hide the other");
        assert_eq!(toasts[0], TrackError::Message("timeout".to_owned()).toast("AniList"));
        assert_eq!(toasts[1], TrackError::UnknownTracker(3).toast("MyAnimeList"));
        assert!(refresh_toasts(&[]).is_empty());
    }

    #[test]
    fn a_track_refreshes_once_at_a_time_and_again_after_it_ends() {
        let mut flight = RefreshFlight::default();
        assert!(flight.begin(7));
        assert!(!flight.begin(7), "already running");
        assert!(flight.begin(8), "another binding is its own flight");
        flight.end(7);
        assert!(flight.begin(7), "free again once it ended");
    }
}
