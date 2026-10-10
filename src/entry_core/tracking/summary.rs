//! The entry points of the tracking button and the sheet (T-38a-031, T-38a-032, T-38b-029; behaviour 11.4, defect
//! D13). The button text and the tap are the rules of `entry_core::action_row`. The sheet opens after a category
//! change only when the change came from this flow, and the auto-open follows `kuro_track::policy`.

use kuro_track::error::TrackError;
use kuro_track::messages::MsgKey;
use kuro_track::policy::auto_open_track_sheet;

/// The sheet opens after the category dialog closed only when the change came from this flow, auto-open is on and the
/// entry already has a supported binding (E-74, D13).
pub fn sheet_after_category_change(from_flow: bool, track_on_adding: bool, tracking_count: usize) -> bool {
    from_flow && auto_open_track_sheet(track_on_adding, tracking_count)
}

/// The refresh failure of a tracker. The service name is the one captured when the refresh failed (E-TRK-6).
pub fn refresh_failure(service: &str, error: &TrackError) -> MsgKey {
    error.toast(service)
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
    use crate::entry_core::action_row::{TrackingClick, TrackingLabel, tracking_click, tracking_label};

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
    fn a_tap_opens_the_settings_without_a_logged_in_tracker() {
        assert_eq!(tracking_click(false), TrackingClick::Settings);
        assert_eq!(tracking_click(true), TrackingClick::Sheet);
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
}
