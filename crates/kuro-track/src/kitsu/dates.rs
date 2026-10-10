//! Kitsu date wire format (T-30a-016, `KitsuDateHelper.kt`). Timestamps are UTC with milliseconds, for example
//! `2026-10-10T08:30:00.000Z`. Stored dates are epoch milliseconds, and 0 means unset.

use chrono::{DateTime, NaiveDateTime, Utc};

use super::error::KitsuError;

/// `%Y-%m-%dT%H:%M:%S%.3fZ`, written in UTC.
pub const WIRE: &str = "%Y-%m-%dT%H:%M:%S%.3fZ";

/// The wire string of epoch milliseconds. 0 is unset and gives `None` (JSON null).
pub fn to_wire(ms: i64) -> Option<String> {
    if ms == 0 {
        return None;
    }
    let at = DateTime::<Utc>::from_timestamp_millis(ms)?;
    Some(at.format(WIRE).to_string())
}

/// Epoch milliseconds of a wire string. A missing value is 0 (unset). The fraction of seconds is optional, and an
/// empty string or a date without a time is `BadDate`.
pub fn from_wire(value: Option<&str>) -> Result<i64, KitsuError> {
    let Some(text) = value else {
        return Ok(0);
    };
    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.fZ")
        .map(|at| at.and_utc().timestamp_millis())
        .map_err(|_| KitsuError::BadDate(text.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_30a_016_zero_is_null_on_write() {
        assert_eq!(to_wire(0), None);
    }

    #[test]
    fn t_30a_016_write_uses_the_utc_wire_format() {
        // 2026-10-10T08:30:00.123Z
        let ms = 1_791_621_000_123_i64;
        assert_eq!(to_wire(ms).as_deref(), Some("2026-10-10T08:30:00.123Z"));
    }

    #[test]
    fn t_30a_016_round_trip_keeps_milliseconds() {
        let ms = 1_791_621_000_123_i64;
        let wire = to_wire(ms).unwrap();
        assert_eq!(from_wire(Some(&wire)).unwrap(), ms);
    }

    #[test]
    fn t_30a_016_empty_and_date_only_are_bad_dates() {
        assert!(matches!(from_wire(Some("")), Err(KitsuError::BadDate(_))));
        assert!(matches!(
            from_wire(Some("2026-10-10")),
            Err(KitsuError::BadDate(_))
        ));
        assert_eq!(from_wire(None).unwrap(), 0);
    }

    #[test]
    fn t_30a_016_parse_accepts_with_or_without_milliseconds() {
        assert_eq!(
            from_wire(Some("2026-10-10T08:30:00Z")).unwrap(),
            1_791_621_000_000
        );
        assert_eq!(
            from_wire(Some("2026-10-10T08:30:00.000Z")).unwrap(),
            1_791_621_000_000
        );
    }
}
