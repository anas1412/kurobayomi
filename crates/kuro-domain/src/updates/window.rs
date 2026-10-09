//! The updates window: "three months ago" from a fixed instant, with the day clamped and the gap of a DST change
//! resolved forward (`ZonedDateTime.minusMonths(3)`).

use chrono::{DateTime, Duration, LocalResult, Months, TimeZone};

/// Epoch ms of the instant three months before `now`, on the same local wall clock. A day that does not exist in the
/// target month is clamped to its last day (31 May gives 28 Feb). A local time that falls in a DST gap moves forward
/// by the gap, one hour at a time.
pub fn three_months_ago<Tz: TimeZone>(now: &DateTime<Tz>) -> i64 {
    let zone = now.timezone();
    let mut local = now.naive_local().checked_sub_months(Months::new(3)).unwrap_or(chrono::NaiveDateTime::MIN);
    loop {
        match zone.from_local_datetime(&local) {
            LocalResult::Single(t) => return t.timestamp_millis(),
            LocalResult::Ambiguous(earliest, _) => return earliest.timestamp_millis(),
            LocalResult::None => local += Duration::hours(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, NaiveDate, NaiveDateTime, Utc};

    fn at<Tz: TimeZone>(zone: &Tz, y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Tz> {
        let naive = NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, min, 0).unwrap();
        zone.from_local_datetime(&naive).single().unwrap()
    }

    fn naive_of(ms: i64) -> NaiveDateTime {
        DateTime::from_timestamp_millis(ms).unwrap().naive_utc()
    }

    #[test]
    fn day_of_month_is_clamped_to_the_target_month() {
        let utc = Utc;
        let a = naive_of(three_months_ago(&at(&utc, 2025, 5, 31, 10, 0)));
        assert_eq!(a, NaiveDate::from_ymd_opt(2025, 2, 28).unwrap().and_hms_opt(10, 0, 0).unwrap());
        let b = naive_of(three_months_ago(&at(&utc, 2024, 5, 31, 10, 0)));
        assert_eq!(b, NaiveDate::from_ymd_opt(2024, 2, 29).unwrap().and_hms_opt(10, 0, 0).unwrap());
        let c = naive_of(three_months_ago(&at(&utc, 2025, 1, 15, 0, 0)));
        assert_eq!(c, NaiveDate::from_ymd_opt(2024, 10, 15).unwrap().and_hms_opt(0, 0, 0).unwrap());
    }

    #[test]
    fn the_wall_clock_of_a_fixed_offset_zone_is_kept() {
        let zone = FixedOffset::east_opt(3600).unwrap();
        let now = at(&zone, 2025, 5, 31, 10, 0);
        let back = DateTime::from_timestamp_millis(three_months_ago(&now)).unwrap().with_timezone(&zone);
        assert_eq!(back.naive_local(), NaiveDate::from_ymd_opt(2025, 2, 28).unwrap().and_hms_opt(10, 0, 0).unwrap());
    }
}
