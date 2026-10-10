//! The fetch interval dialog rules (T-38a-028, T-38b-022). Upstream: `ItemsDialogs.kt` (the "Smart update" dialog) and
//! `MangaScreenModel.setFetchInterval`. The domain stores the wheel value as `-wheel` (`stored_for_wheel`); this file
//! decides what the dialog shows and what it writes. The widget is `fetch_interval_dialog`.

use kuro_domain::entries::fetch_interval::{
    MAX_INTERVAL, expected_update_uses_prediction, next_update_days,
};

/// The text under the title: the expected update when a date is known and the interval is automatic (`>= 0`), else the
/// null text (E-46 neighbourhood, behaviour 8.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    /// The expected next update, in whole days.
    Expected(i64),
    /// No predicted date: the null text.
    Unknown,
}

pub fn body(interval: i32, expected_next_update_ms: Option<i64>, now_ms: i64) -> Body {
    if !expected_update_uses_prediction(interval, expected_next_update_ms) {
        return Body::Unknown;
    }
    match next_update_days(expected_next_update_ms, now_ms) {
        Some(days) => Body::Expected(days),
        None => Body::Unknown,
    }
}

/// The wheel shows whenever the entry allows interval changes, in every build (fix 23.8). The gate is the
/// `isUpdateIntervalEnabled` setting, which defaults on (E-46, E-47).
pub fn wheel_visible(interval_changes_allowed: bool) -> bool {
    interval_changes_allowed
}

/// The starting wheel index: `|interval|` for a user interval (`< 0`), else 0 (E-45). Clamped to the wheel's range.
pub fn initial_wheel(interval: i32) -> i32 {
    if interval < 0 { (-interval).min(MAX_INTERVAL) } else { 0 }
}

/// The explicit reset of the dialog: back to automatic, which stores 0 (E-INT-1). The page stores `-wheel` (domain `stored_for_wheel`).
pub const RESET_WHEEL: i32 = 0;

#[cfg(test)]
mod tests {
    use kuro_domain::entries::fetch_interval::stored_for_wheel;

    use super::*;

    #[test]
    fn the_body_predicts_only_for_an_automatic_interval_with_a_known_date() {
        const DAY: i64 = 86_400_000;
        assert_eq!(body(0, Some(3 * DAY), 0), Body::Expected(3));
        assert_eq!(body(-7, Some(3 * DAY), 0), Body::Unknown, "a user interval has no prediction");
        assert_eq!(body(0, None, 0), Body::Unknown);
    }

    #[test]
    fn the_wheel_follows_the_allowed_setting_in_every_build() {
        assert!(wheel_visible(true));
        assert!(!wheel_visible(false));
    }

    #[test]
    fn the_wheel_starts_at_the_absolute_interval_of_a_user_setting() {
        assert_eq!(initial_wheel(-7), 7);
        assert_eq!(initial_wheel(0), 0);
        assert_eq!(initial_wheel(5), 0);
        assert_eq!(initial_wheel(-99), MAX_INTERVAL);
    }

    #[test]
    fn ok_stores_the_negative_of_the_wheel_and_zero_is_automatic() {
        assert_eq!(stored_for_wheel(7), -7);
        assert_eq!(stored_for_wheel(0), 0);
    }

    #[test]
    fn reset_stores_automatic() {
        assert_eq!(stored_for_wheel(RESET_WHEEL), 0);
    }
}
