//! The recent window (T-50a-004): now minus three calendar months in the local zone, recomputed on every call.

use chrono::{Local, Months, TimeZone};

use super::grid::WINDOW_MONTHS;

/// Epoch milliseconds three calendar months before `now_ms`. A day that does not exist in the earlier month clamps
/// to its last day (31 May gives 28 Feb). A row is in the window only when its upload time is strictly greater.
pub fn window_start_ms(now_ms: i64) -> i64 {
    let Some(now) = Local.timestamp_millis_opt(now_ms).single() else { return now_ms };
    now.checked_sub_months(Months::new(WINDOW_MONTHS)).map_or(now_ms, |t| t.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_ms(y: i32, mo: u32, d: u32, h: u32) -> i64 {
        Local.with_ymd_and_hms(y, mo, d, h, 0, 0).single().unwrap().timestamp_millis()
    }

    #[test]
    fn three_calendar_months_back() {
        assert_eq!(window_start_ms(local_ms(2026, 10, 8, 12)), local_ms(2026, 7, 8, 12));
    }

    #[test]
    fn day_clamps_to_the_end_of_a_short_month() {
        assert_eq!(window_start_ms(local_ms(2026, 5, 31, 0)), local_ms(2026, 2, 28, 0));
        assert_eq!(window_start_ms(local_ms(2028, 5, 31, 0)), local_ms(2028, 2, 29, 0));
    }
}
