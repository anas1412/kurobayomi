//! The entry points of the tracking button and the sheet (T-38a-031, T-38a-032, T-38b-029; behaviour 11.4, defect
//! D13). The button text and the tap are the rules of `entry_core::action_row`. The sheet opens after a category
//! change only when the change came from this flow, and the auto-open follows `kuro_track::policy`.



/// The track sheet opens after an add to the library only when the `track_on_adding_to_library` preference is on and at
/// least one tracker is logged in (T-39b-048, `T-LIB-5`).
pub fn open_track_sheet_on_add(pref_on: bool, logged_in: bool) -> bool {
    pref_on && logged_in
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_core::action_row::{TrackingLabel, tracking_label};

    #[test]
    fn the_button_reads_tracking_with_no_tracker_and_the_count_otherwise() {
        assert_eq!(tracking_label(0), TrackingLabel::Tracking);
        assert_eq!(tracking_label(2), TrackingLabel::Trackers(2));
    }

    #[test]
    fn the_track_sheet_opens_on_add_only_with_the_pref_and_a_logged_in_tracker() {
        assert!(open_track_sheet_on_add(true, true));
        assert!(!open_track_sheet_on_add(false, true), "preference off");
        assert!(!open_track_sheet_on_add(true, false), "no logged-in tracker");
    }

}
