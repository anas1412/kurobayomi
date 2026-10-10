//! The next update label of an anime (T-39a-027; E-8). Null, or a completed entry, reads "N/A". Due today or already
//! past reads "Soon". Otherwise "1 day" or "N days", from the whole days left (`action_row::next_update`).

use kuro_i18n::{MsgKey, tr, tr_plural};

use crate::entry_core::action_row::{NextUpdate, next_update};

/// The label of the next update at `now_ms`.
pub fn label(next_ms: Option<i64>, now_ms: i64, completed: bool) -> String {
    match next_update(next_ms, now_ms, completed) {
        NextUpdate::NotApplicable => tr(MsgKey::not_applicable),
        NextUpdate::Soon => tr(MsgKey::manga_interval_expected_update_soon),
        NextUpdate::Days(days) => tr_plural(MsgKey::day, days, &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400_000;

    #[test]
    fn null_or_completed_reads_not_applicable() {
        assert_eq!(label(None, 0, false), tr(MsgKey::not_applicable));
        assert_eq!(label(Some(DAY), 0, true), tr(MsgKey::not_applicable));
    }

    #[test]
    fn today_or_past_reads_soon_and_days_left_read_as_days() {
        assert_eq!(label(Some(DAY / 2), 0, false), tr(MsgKey::manga_interval_expected_update_soon));
        assert_eq!(label(Some(-DAY), 0, false), tr(MsgKey::manga_interval_expected_update_soon));
        assert_eq!(label(Some(DAY + DAY / 2), 0, false), tr_plural(MsgKey::day, 1, &[]));
        assert_eq!(label(Some(3 * DAY), 0, false), tr_plural(MsgKey::day, 3, &[]));
    }
}
