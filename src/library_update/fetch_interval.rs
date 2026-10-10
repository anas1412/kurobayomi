//! Fetch window of a pass. The fetch interval itself (`calculate_interval`, `increase_interval`, next update) has one
//! implementation for manga and anime, `kuro_domain::entries::fetch_interval`, which sync calls; this module only
//! keeps the `Window` shape the pass hands around.

use chrono::{DateTime, TimeZone};
use kuro_domain::entries::fetch_interval::get_window;

/// The fetch window. `upper_ms` is inclusive: tomorrow 00:00 minus one millisecond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub lower_ms: i64,
    pub upper_ms: i64,
}

/// The window of the pass: yesterday 00:00 through tomorrow 00:00 minus one millisecond, in `now`'s zone.
pub fn window_for<Tz: TimeZone>(now: &DateTime<Tz>) -> Window {
    let (lower_ms, upper_ms) = get_window(now);
    Window { lower_ms, upper_ms }
}

#[cfg(test)]
mod tests {
    //! FI-01..FI-23 against the one implementation in `kuro_domain::entries::fetch_interval`.
    use super::*;
    use chrono::{Duration, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Utc};
    use kuro_domain::entries::fetch_interval::{
        ItemDates, Stored, calculate_interval, calculate_next_update, increase_interval, to_update,
    };

    fn at(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap().timestamp_millis()
    }

    /// The base time of the upstream tests: 2020-01-01T00:00Z, `day(n)` is `n` days later.
    fn day(n: i64) -> i64 {
        at(2020, 1, 1, 0) + n * 86_400_000
    }

    fn uploads(stamps: &[i64]) -> Vec<ItemDates> {
        stamps.iter().map(|&u| ItemDates { date_upload: u, date_fetch: 0 }).collect()
    }

    fn pairs(v: &[(i64, i64)]) -> Vec<ItemDates> {
        v.iter().map(|&(date_upload, date_fetch)| ItemDates { date_upload, date_fetch }).collect()
    }

    fn interval<Tz: TimeZone>(items: &[ItemDates], tz: &Tz) -> i32 {
        calculate_interval(items, tz)
    }

    fn stored(fetch_interval: i32, next_update: i64, last_update: i64) -> Stored {
        Stored { fetch_interval, next_update, last_update }
    }

    fn noon() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2020, 1, 10, 12, 0, 0).unwrap()
    }

    /// Next due time of an entry, the clock being `now`.
    fn next_ms<Tz: TimeZone>(s: Stored, step: i32, now: &DateTime<Tz>) -> i64 {
        calculate_next_update(s, step, now, get_window(now), now.timestamp_millis())
    }

    #[test]
    fn fi01_default_interval_without_enough_distinct_days() {
        let fifty_same_day = vec![ItemDates { date_upload: day(1), date_fetch: 0 }; 50];
        assert_eq!(interval(&fifty_same_day, &Utc), 7);
        let no_upload = vec![ItemDates { date_upload: 0, date_fetch: day(1) }; 50];
        assert_eq!(interval(&no_upload, &Utc), 7);
    }

    #[test]
    fn fi02_uses_the_ten_most_recent_distinct_days() {
        let mut items: Vec<i64> = [7, 14, 21, 28, 35].iter().map(|&n| day(n)).collect();
        items.extend((36..=45).map(day));
        assert_eq!(interval(&uploads(&items), &Utc), 1);
    }

    #[test]
    fn fi03_uses_a_smaller_subset_when_very_few_chapters() {
        let items = [day(7), day(14), day(21), day(393), day(400), day(407)];
        assert_eq!(interval(&uploads(&items), &Utc), 7);
    }

    #[test]
    fn fi04_many_chapters_in_one_day_give_the_default() {
        let items: Vec<i64> = (0..10).map(|_| day(0) + 10 * 3_600_000).collect();
        assert_eq!(interval(&uploads(&items), &Utc), 7);
    }

    #[test]
    fn fi05_two_days_give_the_default() {
        let mut items = vec![day(1); 2];
        items.extend(vec![day(2); 5]);
        assert_eq!(interval(&uploads(&items), &Utc), 7);
    }

    #[test]
    fn fi06_daily_release_is_one_day() {
        let items: Vec<i64> = (1..=20).map(day).collect();
        assert_eq!(interval(&uploads(&items), &Utc), 1);
    }

    #[test]
    fn fi07_sub_day_gaps_are_one_day() {
        let items: Vec<i64> = (1..=20).map(|i| day(0) + i * 15 * 3_600_000).collect();
        assert_eq!(interval(&uploads(&items), &Utc), 1);
    }

    #[test]
    fn fi08_two_day_release_is_two_days() {
        let items: Vec<i64> = (1..=20).map(|i| day(2 * i)).collect();
        assert_eq!(interval(&uploads(&items), &Utc), 2);
    }

    #[test]
    fn fi09_decimal_interval_is_floored() {
        let items: Vec<i64> = (1..=5).map(|i| day(0) + i * 25 * 3_600_000).collect();
        assert_eq!(interval(&uploads(&items), &Utc), 1);
        let fetch_only: Vec<(i64, i64)> = items.iter().map(|&f| (0, f)).collect();
        assert_eq!(interval(&pairs(&fetch_only), &Utc), 1);
    }

    #[test]
    fn fi10_just_under_two_days_is_one_day() {
        let items: Vec<i64> = (1..=20).map(|i| day(0) + i * 43 * 3_600_000).collect();
        assert_eq!(interval(&uploads(&items), &Utc), 1);
    }

    #[test]
    fn fi11_empty_slice_gives_the_default() {
        assert_eq!(interval(&[], &Utc), 7);
    }

    #[test]
    fn fi12_falls_back_to_fetch_dates_when_uploads_are_zero() {
        let items: Vec<(i64, i64)> = (1..=5).map(|i| (0, day(i))).collect();
        assert_eq!(interval(&pairs(&items), &Utc), 1);
    }

    #[test]
    fn fi13_uses_fetch_dates_when_uploads_have_fewer_than_three_days() {
        let items = [(day(1), day(2)), (day(1), day(4)), (day(2), day(6)), (day(2), day(8)), (day(2), day(10))];
        assert_eq!(interval(&pairs(&items), &Utc), 2);
    }

    #[test]
    fn fi17_increase_interval_table() {
        assert_eq!(increase_interval(7, 0), 7);
        assert_eq!(increase_interval(7, 70), 14);
        assert_eq!(increase_interval(1, 9), 1);
        assert_eq!(increase_interval(1, 10), 2);
        assert_eq!(increase_interval(16, 200), 28);
    }

    #[test]
    fn the_pass_window_is_yesterday_to_tomorrow_minus_one_ms() {
        let w = window_for(&noon());
        assert_eq!(w.lower_ms, at(2020, 1, 9, 0));
        assert_eq!(w.upper_ms, at(2020, 1, 11, 0) - 1);
    }

    #[test]
    fn fi18_to_fi20b_window_boundaries_are_inclusive_at_both_ends() {
        let now = noon();
        let kept = |ms| next_ms(stored(7, ms, 0), 7, &now) == ms;
        assert!(kept(at(2020, 1, 10, 6))); // FI-18: inside
        assert!(kept(at(2020, 1, 11, 0))); // FI-19: tomorrow 00:00 exactly
        assert!(!kept(at(2020, 1, 11, 0) + 1)); // FI-20: one ms later is recomputed
        assert!(!kept(at(2020, 1, 9, 0) - 1)); // FI-20b: one ms before yesterday is recomputed
        assert!(kept(at(2020, 1, 9, 0))); // yesterday 00:00 is in range
    }

    #[test]
    fn fi14_zero_window_equals_the_window_of_now() {
        let now = noon();
        let items = pairs(&[(0, 0); 2]);
        let s = stored(7, at(2019, 6, 1, 0), day(0));
        let w = window_for(&now);
        let explicit = to_update(s, &items, &now, (w.lower_ms, w.upper_ms), now.timestamp_millis());
        let implicit = to_update(s, &items, &now, (0, 0), now.timestamp_millis());
        assert_eq!(explicit, implicit);
    }

    #[test]
    fn fi15_negative_override_steps_by_its_absolute_value_and_is_not_recalculated() {
        let now = noon();
        let items: Vec<i64> = (1..=20).map(day).collect();
        let out = to_update(stored(-3, 0, day(0)), &uploads(&items), &now, (0, 0), now.timestamp_millis());
        assert_eq!(out.fetch_interval, -3);
        // 9 whole days since 2020-01-01: cycle = 9 div 3 = 3, next = 2020-01-01 + 4 * 3 days.
        assert_eq!(out.next_update, at(2020, 1, 13, 0));
    }

    #[test]
    fn fi16_zero_stored_interval_is_recalculated() {
        let now = noon();
        let items: Vec<i64> = (1..=20).map(day).collect();
        let out = to_update(stored(0, 0, 0), &uploads(&items), &now, (0, 0), now.timestamp_millis());
        assert_eq!(out.fetch_interval, 1);
    }

    #[test]
    fn fi21_zero_last_update_counts_from_now() {
        // latest = start of today, 0 days since, cycle 0, next = today + 7 days.
        assert_eq!(next_ms(stored(7, 0, 0), 7, &noon()), at(2020, 1, 17, 0));
    }

    #[test]
    fn a_quiet_entry_steps_by_the_doubled_interval() {
        let now = noon();
        // Last change 70 days ago, interval 7: effective 14, cycle = 70 div 14 = 5, next = latest + 6 * 7 days.
        let last = now - Duration::days(70);
        let next = next_ms(stored(7, 0, last.timestamp_millis()), 7, &now);
        let latest = last.date_naive().and_hms_opt(0, 0, 0).unwrap();
        assert_eq!(next, (latest + Duration::days(42)).and_utc().timestamp_millis());
    }

    /// A zone with one spring-forward change (CET, +01:00, until 2026-03-29 02:00 local; CEST, +02:00, after). It
    /// stands in for Europe/Paris, which the crate does not link.
    #[derive(Clone, Copy, Debug)]
    struct SpringZone;

    fn cet() -> FixedOffset {
        FixedOffset::east_opt(3_600).unwrap()
    }

    fn cest() -> FixedOffset {
        FixedOffset::east_opt(7_200).unwrap()
    }

    fn transition_utc() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 3, 29).unwrap().and_hms_opt(1, 0, 0).unwrap()
    }

    impl TimeZone for SpringZone {
        type Offset = FixedOffset;

        fn from_offset(_: &FixedOffset) -> Self {
            SpringZone
        }

        fn offset_from_local_date(&self, local: &NaiveDate) -> LocalResult<FixedOffset> {
            self.offset_from_local_datetime(&local.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap()))
        }

        fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<FixedOffset> {
            let gap_start = NaiveDate::from_ymd_opt(2026, 3, 29).unwrap().and_hms_opt(2, 0, 0).unwrap();
            let gap_end = NaiveDate::from_ymd_opt(2026, 3, 29).unwrap().and_hms_opt(3, 0, 0).unwrap();
            if *local < gap_start {
                LocalResult::Single(cet())
            } else if *local < gap_end {
                LocalResult::None
            } else {
                LocalResult::Single(cest())
            }
        }

        fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
            self.offset_from_utc_datetime(&utc.and_time(NaiveTime::MIN))
        }

        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            if *utc < transition_utc() { cet() } else { cest() }
        }
    }

    #[test]
    fn fi22_day_counts_are_calendar_days_across_a_dst_change() {
        // Last change 2026-03-27 00:00 CET; now 2026-03-29 12:00 CEST. The count is calendar days: 2.
        let now = SpringZone.with_ymd_and_hms(2026, 3, 29, 12, 0, 0).single().unwrap();
        let last = SpringZone.with_ymd_and_hms(2026, 3, 27, 0, 0, 0).single().unwrap().timestamp_millis();
        let next = next_ms(stored(7, 0, last), 7, &now);
        // cycle = 2 div 7 = 0, next = 2026-03-27 + 7 days = 2026-04-03 00:00 at the offset of now (CEST) = 22:00 UTC.
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 4, 2, 22, 0, 0).unwrap().timestamp_millis());
    }

    #[test]
    fn fi23_a_last_update_in_the_future_gives_that_day_and_never_an_earlier_one() {
        let now = noon();
        let last = now + Duration::days(3);
        assert_eq!(next_ms(stored(7, 0, last.timestamp_millis()), 7, &now), at(2020, 1, 13, 0));
    }
}
