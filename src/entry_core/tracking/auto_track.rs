//! Auto-track after marking read (T-38b-040 for manga, T-39b-047 for anime; behaviour 11.2 and 12). The decision is
//! `kuro_track::policy::mark_read_plan`; this file gathers its inputs: the largest number of the items marked, the
//! "changed" check of a re-mark, and the success toast rule.

use kuro_track::model::Kind;
use kuro_track::policy::{AutoTrackState, MarkReadPlan, mark_read_plan};

/// Everything the mark-read flow knows about one marking.
#[derive(Clone, Copy, Debug)]
pub struct Marking<'a> {
    pub kind: Kind,
    /// The new read state of the items.
    pub read: bool,
    pub has_logged_in: bool,
    /// The chapter or episode numbers of the items marked. `-1` is unrecognised and skipped (E-CHG-2).
    pub numbers: &'a [f64],
    /// Whether at least one item changed state (a re-mark of read items changes nothing).
    pub changed: bool,
    /// The progress of each binding of the entry.
    pub tracker_progress: &'a [f64],
}

/// The largest recognised number of the items. `None` when no number is recognised.
pub fn max_number(numbers: &[f64]) -> Option<f64> {
    numbers.iter().copied().filter(|n| *n != -1.0).reduce(f64::max)
}

/// The decision for one marking. `prompt_only_on_change` is decision D-8: when set, re-marking items that were already
/// read asks nothing. The default is `false`, which keeps the upstream behaviour (flagged to the user).
pub fn plan(m: &Marking, state: AutoTrackState, prompt_only_on_change: bool) -> MarkReadPlan {
    if prompt_only_on_change && !m.changed {
        return MarkReadPlan::Nothing;
    }
    match max_number(m.numbers) {
        None => MarkReadPlan::Nothing,
        Some(max) => mark_read_plan(m.kind, m.read, m.has_logged_in, max, m.tracker_progress, state),
    }
}

/// The "Trackers updated" toast shows only when at least one tracker accepted the update (T-MRK-6, fix 23.12).
pub fn success_toast_shown(accepted: usize) -> bool {
    accepted > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marking<'a>(numbers: &'a [f64], progress: &'a [f64], changed: bool) -> Marking<'a> {
        Marking { kind: Kind::Manga, read: true, has_logged_in: true, numbers, changed, tracker_progress: progress }
    }

    #[test]
    fn unrecognised_numbers_are_skipped_when_taking_the_maximum() {
        assert_eq!(max_number(&[3.0, -1.0, 7.0]), Some(7.0));
        assert_eq!(max_number(&[-1.0]), None);
        assert_eq!(max_number(&[]), None);
    }

    #[test]
    fn always_applies_and_ask_prompts_with_the_largest_number() {
        let m = marking(&[4.0, 9.0], &[2.0], true);
        assert_eq!(plan(&m, AutoTrackState::ALWAYS, false), MarkReadPlan::Apply {
            update_to: 9.0,
            toast: kuro_track::messages::MsgKey::TrackersUpdatedManga(9),
        });
        assert_eq!(plan(&m, AutoTrackState::ASK, false), MarkReadPlan::Prompt {
            update_to: 9.0,
            banner: kuro_track::messages::MsgKey::ConfirmUpdateManga(9),
        });
    }

    #[test]
    fn never_and_unread_and_no_tracker_and_no_track_behind_do_nothing() {
        let behind = [2.0];
        assert_eq!(plan(&marking(&[5.0], &behind, true), AutoTrackState::NEVER, false), MarkReadPlan::Nothing);
        let unread = Marking { read: false, ..marking(&[5.0], &behind, true) };
        assert_eq!(plan(&unread, AutoTrackState::ALWAYS, false), MarkReadPlan::Nothing);
        let logged_out = Marking { has_logged_in: false, ..marking(&[5.0], &behind, true) };
        assert_eq!(plan(&logged_out, AutoTrackState::ALWAYS, false), MarkReadPlan::Nothing);
        assert_eq!(plan(&marking(&[1.0], &[2.0], true), AutoTrackState::ASK, false), MarkReadPlan::Nothing, "no track behind");
        assert_eq!(plan(&marking(&[-1.0], &behind, true), AutoTrackState::ALWAYS, false), MarkReadPlan::Nothing);
    }

    #[test]
    fn a_remark_prompts_only_when_something_changed_under_decision_d8() {
        let remark = marking(&[5.0], &[2.0], false);
        assert_eq!(plan(&remark, AutoTrackState::ASK, false), MarkReadPlan::Prompt {
            update_to: 5.0,
            banner: kuro_track::messages::MsgKey::ConfirmUpdateManga(5),
        }, "the default keeps the upstream prompt");
        assert_eq!(plan(&remark, AutoTrackState::ASK, true), MarkReadPlan::Nothing, "D-8 accepted: no change, no prompt");
    }

    #[test]
    fn the_success_toast_needs_one_accepting_tracker() {
        assert!(!success_toast_shown(0));
        assert!(success_toast_shown(1));
    }
}
