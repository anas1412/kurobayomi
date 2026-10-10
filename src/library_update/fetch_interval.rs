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

    /// A zone with one clock change: `BEFORE` seconds east of UTC until the instant `AT` (unix seconds), `AFTER` from
    /// then on. Spring-forward and fall-back changes are both this shape, so the four zones below need no zone
    /// database. A local time the change skips has no instant; one it repeats has two.
    #[derive(Clone, Copy, Debug)]
    struct Change<const BEFORE: i32, const AFTER: i32, const AT: i64>;

    impl<const BEFORE: i32, const AFTER: i32, const AT: i64> Change<BEFORE, AFTER, AT> {
        fn offset(secs: i32) -> FixedOffset {
            FixedOffset::east_opt(secs).unwrap()
        }

        fn at() -> NaiveDateTime {
            DateTime::from_timestamp(AT, 0).unwrap().naive_utc()
        }
    }

    impl<const BEFORE: i32, const AFTER: i32, const AT: i64> TimeZone for Change<BEFORE, AFTER, AT> {
        type Offset = FixedOffset;

        fn from_offset(_: &FixedOffset) -> Self {
            Self
        }

        fn offset_from_local_date(&self, local: &NaiveDate) -> LocalResult<FixedOffset> {
            self.offset_from_local_datetime(&local.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap()))
        }

        fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<FixedOffset> {
            let before_holds = *local - Duration::seconds(i64::from(BEFORE)) < Self::at();
            let after_holds = *local - Duration::seconds(i64::from(AFTER)) >= Self::at();
            match (before_holds, after_holds) {
                (true, true) => LocalResult::Ambiguous(Self::offset(BEFORE), Self::offset(AFTER)),
                (true, false) => LocalResult::Single(Self::offset(BEFORE)),
                (false, true) => LocalResult::Single(Self::offset(AFTER)),
                (false, false) => LocalResult::None,
            }
        }

        fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
            self.offset_from_utc_datetime(&utc.and_time(NaiveTime::MIN))
        }

        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            Self::offset(if *utc < Self::at() { BEFORE } else { AFTER })
        }
    }

    /// Europe/Paris 2026-03-29: 02:00 CET (+01:00) becomes 03:00 CEST (+02:00). The day has 23 hours.
    type ParisSpring = Change<3_600, 7_200, 1_774_746_000>;
    /// Europe/Paris 2026-10-25: 03:00 CEST becomes 02:00 CET. The day has 25 hours.
    type ParisAutumn = Change<7_200, 3_600, 1_792_890_000>;
    /// America/New_York 2026-03-08: 02:00 EST (-05:00) becomes 03:00 EDT (-04:00). The day has 23 hours.
    type NewYorkSpring = Change<{ -18_000 }, { -14_400 }, 1_772_953_200>;
    /// America/New_York 2026-11-01: 02:00 EDT becomes 01:00 EST. The day has 25 hours.
    type NewYorkAutumn = Change<{ -14_400 }, { -18_000 }, 1_793_512_800>;

    const PARIS_SPRING: ParisSpring = Change;
    const PARIS_AUTUMN: ParisAutumn = Change;
    const NEW_YORK_SPRING: NewYorkSpring = Change;
    const NEW_YORK_AUTUMN: NewYorkAutumn = Change;

    /// Local wall-clock instant in `zone`, in epoch milliseconds (never a skipped or repeated time).
    fn local<Tz: TimeZone>(zone: &Tz, y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Tz> {
        zone.with_ymd_and_hms(y, m, d, h, min, 0).single().expect("an unambiguous local time")
    }

    fn utc_ms(y: i32, m: u32, d: u32, h: u32) -> i64 {
        at(y, m, d, h)
    }

    #[test]
    fn fi22_day_counts_are_calendar_days_across_a_dst_change() {
        // Last change 2026-03-27 00:00 CET; now 2026-03-29 12:00 CEST. The count is calendar days: 2.
        let now = local(&PARIS_SPRING, 2026, 3, 29, 12, 0);
        let last = local(&PARIS_SPRING, 2026, 3, 27, 0, 0).timestamp_millis();
        let next = next_ms(stored(7, 0, last), 7, &now);
        // cycle = 2 div 7 = 0, next = 2026-03-27 + 7 days = 2026-04-03 00:00 at the offset of now (CEST) = 22:00 UTC.
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 4, 2, 22, 0, 0).unwrap().timestamp_millis());
    }

    #[test]
    fn fi22_europe_paris_window_uses_local_midnights_on_a_23_and_a_25_hour_day() {
        // Spring: yesterday 00:00 CET to tomorrow 00:00 CEST is 47 hours, not 48.
        let now = local(&PARIS_SPRING, 2026, 3, 29, 12, 0);
        let window = window_for(&now);
        assert_eq!(window.lower_ms, utc_ms(2026, 3, 27, 23));
        assert_eq!(window.upper_ms, utc_ms(2026, 3, 29, 22) - 1);
        // Autumn: yesterday 00:00 CEST to tomorrow 00:00 CET is 49 hours.
        let now = local(&PARIS_AUTUMN, 2026, 10, 25, 12, 0);
        let window = window_for(&now);
        assert_eq!(window.lower_ms, utc_ms(2026, 10, 23, 22));
        assert_eq!(window.upper_ms, utc_ms(2026, 10, 25, 23) - 1);
    }

    #[test]
    fn fi22_europe_paris_next_update_counts_calendar_days_not_hours() {
        // Spring: 2026-03-28 00:00 to 2026-03-30 00:30 is 2 calendar days but 47.5 hours.
        let now = local(&PARIS_SPRING, 2026, 3, 30, 0, 30);
        let last = local(&PARIS_SPRING, 2026, 3, 28, 0, 0).timestamp_millis();
        // 2 days since the last change at a 1-day interval: cycle 2, next = last + 3 days = 03-31 00:00 CEST.
        assert_eq!(next_ms(stored(1, 0, last), 1, &now), utc_ms(2026, 3, 30, 22));
        // Autumn: 2026-10-24 00:00 to 2026-10-25 23:30 is 1 calendar day but 48.5 hours.
        let now = local(&PARIS_AUTUMN, 2026, 10, 25, 23, 30);
        let last = local(&PARIS_AUTUMN, 2026, 10, 24, 0, 0).timestamp_millis();
        // 1 day since the last change: cycle 1, next = last + 2 days = 10-26 00:00 CET.
        assert_eq!(next_ms(stored(1, 0, last), 1, &now), utc_ms(2026, 10, 25, 23));
    }

    #[test]
    fn fi22_america_new_york_window_uses_local_midnights_on_a_23_and_a_25_hour_day() {
        let now = local(&NEW_YORK_SPRING, 2026, 3, 8, 12, 0);
        let window = window_for(&now);
        assert_eq!(window.lower_ms, utc_ms(2026, 3, 7, 5), "03-07 00:00 EST");
        assert_eq!(window.upper_ms, utc_ms(2026, 3, 9, 4) - 1, "03-09 00:00 EDT, 47 hours later");
        let now = local(&NEW_YORK_AUTUMN, 2026, 11, 1, 12, 0);
        let window = window_for(&now);
        assert_eq!(window.lower_ms, utc_ms(2026, 10, 31, 4), "10-31 00:00 EDT");
        assert_eq!(window.upper_ms, utc_ms(2026, 11, 2, 5) - 1, "11-02 00:00 EST, 49 hours later");
    }

    #[test]
    fn fi22_america_new_york_next_update_counts_calendar_days_not_hours() {
        // Spring: 2026-03-07 00:00 to 2026-03-09 00:30 is 2 calendar days but 47.5 hours.
        let now = local(&NEW_YORK_SPRING, 2026, 3, 9, 0, 30);
        let last = local(&NEW_YORK_SPRING, 2026, 3, 7, 0, 0).timestamp_millis();
        assert_eq!(next_ms(stored(1, 0, last), 1, &now), utc_ms(2026, 3, 10, 4), "03-10 00:00 EDT");
        // Autumn: 2026-10-31 00:00 to 2026-11-01 23:30 is 1 calendar day but 48.5 hours.
        let now = local(&NEW_YORK_AUTUMN, 2026, 11, 1, 23, 30);
        let last = local(&NEW_YORK_AUTUMN, 2026, 10, 31, 0, 0).timestamp_millis();
        assert_eq!(next_ms(stored(1, 0, last), 1, &now), utc_ms(2026, 11, 2, 5), "11-02 00:00 EST");
    }

    #[test]
    fn the_change_zones_skip_and_repeat_the_local_hours_a_clock_change_does() {
        let spring = NaiveDate::from_ymd_opt(2026, 3, 29).unwrap();
        assert!(matches!(PARIS_SPRING.offset_from_local_datetime(&spring.and_hms_opt(2, 30, 0).unwrap()), LocalResult::None));
        let autumn = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        assert!(matches!(PARIS_AUTUMN.offset_from_local_datetime(&autumn.and_hms_opt(2, 30, 0).unwrap()), LocalResult::Ambiguous(..)));
    }

    #[test]
    fn fi23_a_last_update_in_the_future_gives_that_day_and_never_an_earlier_one() {
        let now = noon();
        let last = now + Duration::days(3);
        assert_eq!(next_ms(stored(7, 0, last.timestamp_millis()), 7, &now), at(2020, 1, 13, 0));
    }
}
