//! Fetch window and fetch interval (behaviour 4, `MangaFetchInterval` and `AnimeFetchInterval`, one copy for both
//! kinds). Pure: the caller passes the zone, the clock and the stored values.

use chrono::{DateTime, Days, NaiveDate, TimeDelta, TimeZone};

pub const MAX_INTERVAL: i32 = 28;
pub const DEFAULT_INTERVAL: i32 = 7;
/// Days either side of today that count as "due" (yesterday 00:00 to tomorrow 00:00).
pub const GRACE_PERIOD_DAYS: u64 = 1;
/// Missed cycles after which the interval doubles.
pub const INCREASE_WHEN_OVER: i64 = 10;

/// The fetch window. `upper_ms` is inclusive: tomorrow 00:00 minus one millisecond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub lower_ms: i64,
    pub upper_ms: i64,
}

/// Start of `date` in `zone` in epoch milliseconds. A midnight that does not exist (a DST gap) takes the first
/// valid hour, like Java's `atStartOfDay`.
pub fn start_of_day<Tz: TimeZone>(date: NaiveDate, zone: &Tz) -> DateTime<Tz> {
    (0..24)
        .find_map(|hour| date.and_hms_opt(hour, 0, 0)?.and_local_timezone(zone.clone()).earliest())
        .expect("every local date has a valid start in some hour")
}

fn local_date<Tz: TimeZone>(ms: i64, zone: &Tz) -> Option<NaiveDate> {
    zone.timestamp_millis_opt(ms).earliest().map(|t| t.date_naive())
}

/// The window of the pass: yesterday 00:00 through tomorrow 00:00 minus one millisecond, in `now`'s zone.
pub fn window_for<Tz: TimeZone>(now: &DateTime<Tz>) -> Window {
    let zone = now.timezone();
    let today = now.date_naive();
    let lower = today - Days::new(GRACE_PERIOD_DAYS);
    let upper = today + Days::new(GRACE_PERIOD_DAYS);
    Window {
        lower_ms: start_of_day(lower, &zone).timestamp_millis(),
        upper_ms: start_of_day(upper, &zone).timestamp_millis() - 1,
    }
}

/// Distinct local days of `stamps`, newest first, at most `take` of them.
fn recent_days<Tz: TimeZone>(mut stamps: Vec<i64>, take: usize, zone: &Tz) -> Vec<NaiveDate> {
    stamps.sort_unstable_by(|a, b| b.cmp(a));
    let mut days: Vec<NaiveDate> = Vec::with_capacity(take);
    for ms in stamps {
        if days.len() == take {
            break;
        }
        if let Some(day) = local_date(ms, zone) {
            if !days.contains(&day) {
                days.push(day);
            }
        }
    }
    days
}

/// Average gap in days between the newest and oldest of `days` (newest first). `None` with fewer than three days.
fn average_gap(days: &[NaiveDate]) -> Option<i64> {
    if days.len() < 3 {
        return None;
    }
    let delta = (days[0] - days[days.len() - 1]).num_days();
    Some(delta.div_euclid((days.len() - 1) as i64))
}

/// Interval in days, `1..=28`, from `(date_upload_ms, date_fetch_ms)` per chapter or episode.
pub fn calculate_interval<Tz: TimeZone>(items: &[(i64, i64)], zone: &Tz) -> i32 {
    let take = if items.len() <= 8 { 3 } else { 10 };
    let uploads: Vec<i64> = items.iter().filter(|(upload, _)| *upload > 0).map(|(upload, _)| *upload).collect();
    let fetches: Vec<i64> = items.iter().map(|(_, fetch)| *fetch).collect();
    let raw = average_gap(&recent_days(uploads, take, zone))
        .or_else(|| average_gap(&recent_days(fetches, take, zone)))
        .unwrap_or(i64::from(DEFAULT_INTERVAL));
    raw.clamp(1, i64::from(MAX_INTERVAL)) as i32
}

/// The interval after `delta` has gone unchanged for `days_since_latest` days: doubles while the entry has been
/// missed for more than `increase_when_over` cycles, capped at 28.
pub fn increase_interval(delta: i32, days_since_latest: i64, increase_when_over: i64) -> i32 {
    let mut delta = delta;
    loop {
        if delta >= MAX_INTERVAL {
            return MAX_INTERVAL;
        }
        debug_assert!(delta >= 1, "a fetch interval is at least one day");
        let cycle = days_since_latest.div_euclid(i64::from(delta)) + 1;
        if cycle > increase_when_over {
            delta *= 2;
        } else {
            return delta;
        }
    }
}

/// The next due time in epoch milliseconds (behaviour 4.4).
pub fn next_update_ms<Tz: TimeZone>(
    stored_next_ms: i64,
    interval: i32,
    last_update_ms: i64,
    now: &DateTime<Tz>,
    window: Window,
) -> i64 {
    // Inclusive on both ends: tomorrow 00:00 exactly is `upper_ms + 1`.
    if (window.lower_ms..=window.upper_ms + 1).contains(&stored_next_ms) {
        return stored_next_ms;
    }
    let zone = now.timezone();
    let latest_ms = if last_update_ms > 0 { last_update_ms } else { now.timestamp_millis() };
    let latest_date = local_date(latest_ms, &zone).unwrap_or_else(|| now.date_naive());
    let latest_start = start_of_day(latest_date, &zone);
    let days_since = (now.naive_local() - latest_start.naive_local()).num_days();
    // A negative stored interval is a fixed override; `calculate_interval` never returns 0, so the divisor is >= 1.
    let effective = if interval < 0 {
        i64::from(-interval)
    } else {
        i64::from(increase_interval(interval, days_since, INCREASE_WHEN_OVER))
    };
    let cycle = days_since.div_euclid(effective);
    let step = (cycle + 1) * i64::from(interval.abs());
    let next_date = latest_date
        .checked_add_signed(TimeDelta::days(step))
        .unwrap_or(latest_date);
    start_of_day(next_date, &zone).timestamp_millis()
}

/// `(next_update_ms, fetch_interval)` for one entry (behaviour 4.1). A negative stored interval is kept as is. A
/// `window` of `None` means the window of `now` (the Kotlin `(0, 0)` argument).
pub fn next_update<Tz: TimeZone>(
    stored_interval: i32,
    items: &[(i64, i64)],
    stored_next_ms: i64,
    stored_last_update_ms: i64,
    now: &DateTime<Tz>,
    window: Option<Window>,
) -> (i64, i32) {
    let interval = if stored_interval < 0 {
        stored_interval
    } else {
        calculate_interval(items, &now.timezone())
    };
    let window = window.unwrap_or_else(|| window_for(now));
    (next_update_ms(stored_next_ms, interval, stored_last_update_ms, now, window), interval)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    /// Epoch ms of a UTC wall time.
    fn at(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap().timestamp_millis()
    }

    /// The base time of the upstream tests: 2020-01-01T00:00Z, `day(n)` is `n` days later.
    fn day(n: i64) -> i64 {
        at(2020, 1, 1, 0) + n * 86_400_000
    }

    fn uploads(stamps: &[i64]) -> Vec<(i64, i64)> {
        stamps.iter().map(|&u| (u, 0)).collect()
    }

    #[test]
    fn fi01_default_interval_without_enough_distinct_days() {
        let fifty_same_day = vec![(day(1), 0); 50];
        assert_eq!(calculate_interval(&fifty_same_day, &Utc), 7);
        let no_upload: Vec<(i64, i64)> = vec![(0, day(1)); 50];
        assert_eq!(calculate_interval(&no_upload, &Utc), 7);
    }

    #[test]
    fn fi02_uses_the_ten_most_recent_distinct_days() {
        let mut items: Vec<i64> = [7, 14, 21, 28, 35].iter().map(|&n| day(n)).collect();
        items.extend((36..=45).map(day));
        assert_eq!(calculate_interval(&uploads(&items), &Utc), 1);
    }

    #[test]
    fn fi03_uses_a_smaller_subset_when_very_few_chapters() {
        let items = [day(7), day(14), day(21), day(393), day(400), day(407)];
        assert_eq!(calculate_interval(&uploads(&items), &Utc), 7);
    }

    #[test]
    fn fi04_many_chapters_in_one_day_give_the_default() {
        let items: Vec<i64> = (0..10).map(|_| day(0) + 10 * 3_600_000).collect();
        assert_eq!(calculate_interval(&uploads(&items), &Utc), 7);
    }

    #[test]
    fn fi05_two_days_give_the_default() {
        let mut items = vec![day(1); 2];
        items.extend(vec![day(2); 5]);
        assert_eq!(calculate_interval(&uploads(&items), &Utc), 7);
    }

    #[test]
    fn fi06_daily_release_is_one_day() {
        let items: Vec<i64> = (1..=20).map(day).collect();
        assert_eq!(calculate_interval(&uploads(&items), &Utc), 1);
    }

    #[test]
    fn fi07_sub_day_gaps_are_one_day() {
        let items: Vec<i64> = (1..=20).map(|i| day(0) + i * 15 * 3_600_000).collect();
        assert_eq!(calculate_interval(&uploads(&items), &Utc), 1);
    }

    #[test]
    fn fi08_two_day_release_is_two_days() {
        let items: Vec<i64> = (1..=20).map(|i| day(2 * i)).collect();
        assert_eq!(calculate_interval(&uploads(&items), &Utc), 2);
    }

    #[test]
    fn fi09_decimal_interval_is_floored() {
        let items: Vec<i64> = (1..=5).map(|i| day(0) + i * 25 * 3_600_000).collect();
        assert_eq!(calculate_interval(&uploads(&items), &Utc), 1);
        let fetch_only: Vec<(i64, i64)> = items.iter().map(|&f| (0, f)).collect();
        assert_eq!(calculate_interval(&fetch_only, &Utc), 1);
    }

    #[test]
    fn fi10_just_under_two_days_is_one_day() {
        let items: Vec<i64> = (1..=20).map(|i| day(0) + i * 43 * 3_600_000).collect();
        assert_eq!(calculate_interval(&uploads(&items), &Utc), 1);
    }

    #[test]
    fn fi11_empty_slice_gives_the_default() {
        assert_eq!(calculate_interval(&[], &Utc), 7);
    }

    #[test]
    fn fi12_falls_back_to_fetch_dates_when_uploads_are_zero() {
        let items: Vec<(i64, i64)> = (1..=5).map(|i| (0, day(i))).collect();
        assert_eq!(calculate_interval(&items, &Utc), 1);
    }

    #[test]
    fn fi13_uses_fetch_dates_when_uploads_have_fewer_than_three_days() {
        // Uploads on two days only; fetch dates on five days with a 2-day gap.
        let items: Vec<(i64, i64)> = vec![
            (day(1), day(2)),
            (day(1), day(4)),
            (day(2), day(6)),
            (day(2), day(8)),
            (day(2), day(10)),
        ];
        assert_eq!(calculate_interval(&items, &Utc), 2);
    }

    #[test]
    fn fi17_increase_interval_table() {
        assert_eq!(increase_interval(7, 0, INCREASE_WHEN_OVER), 7);
        assert_eq!(increase_interval(7, 70, INCREASE_WHEN_OVER), 14);
        assert_eq!(increase_interval(1, 9, INCREASE_WHEN_OVER), 1);
        assert_eq!(increase_interval(1, 10, INCREASE_WHEN_OVER), 2);
        assert_eq!(increase_interval(16, 200, INCREASE_WHEN_OVER), 28);
    }

    #[test]
    fn fi_window_is_yesterday_to_tomorrow_minus_one_ms() {
        let now = Utc.with_ymd_and_hms(2020, 1, 10, 12, 0, 0).unwrap();
        let w = window_for(&now);
        assert_eq!(w.lower_ms, at(2020, 1, 9, 0));
        assert_eq!(w.upper_ms, at(2020, 1, 11, 0) - 1);
    }

    #[test]
    fn fi18_to_fi20b_window_boundaries_are_inclusive_at_both_ends() {
        let now = Utc.with_ymd_and_hms(2020, 1, 10, 12, 0, 0).unwrap();
        let w = window_for(&now);
        let kept = |ms| next_update_ms(ms, 7, 0, &now, w) == ms;
        assert!(kept(at(2020, 1, 10, 6))); // FI-18: inside
        assert!(kept(at(2020, 1, 11, 0))); // FI-19: tomorrow 00:00 exactly
        assert!(!kept(at(2020, 1, 11, 0) + 1)); // FI-20: one ms later is recomputed
        assert!(!kept(at(2020, 1, 9, 0) - 1)); // FI-20b: one ms before yesterday is recomputed
        assert!(kept(at(2020, 1, 9, 0))); // yesterday 00:00 is in range
    }

    #[test]
    fn fi14_none_window_equals_window_of_now() {
        let now = Utc.with_ymd_and_hms(2020, 1, 10, 12, 0, 0).unwrap();
        let items: Vec<(i64, i64)> = vec![(0, 0); 2];
        let explicit = next_update(7, &items, at(2019, 6, 1, 0), day(0), &now, Some(window_for(&now)));
        let implicit = next_update(7, &items, at(2019, 6, 1, 0), day(0), &now, None);
        assert_eq!(explicit, implicit);
    }

    #[test]
    fn fi15_negative_override_steps_by_its_absolute_value_and_is_not_recalculated() {
        let now = Utc.with_ymd_and_hms(2020, 1, 10, 12, 0, 0).unwrap();
        // Items would give 1 day, but the stored -3 wins.
        let items: Vec<i64> = (1..=20).map(day).collect();
        let (next, interval) = next_update(-3, &uploads(&items), 0, day(0), &now, None);
        assert_eq!(interval, -3);
        // 9 whole days since 2020-01-01: cycle = 9 div 3 = 3, next = 2020-01-01 + 4 * 3 days.
        assert_eq!(next, at(2020, 1, 13, 0));
    }

    #[test]
    fn fi16_zero_stored_interval_is_recalculated() {
        let now = Utc.with_ymd_and_hms(2020, 1, 10, 12, 0, 0).unwrap();
        let items: Vec<i64> = (1..=20).map(day).collect();
        let (_, interval) = next_update(0, &uploads(&items), 0, 0, &now, None);
        assert_eq!(interval, 1);
    }

    #[test]
    fn fi21_zero_last_update_counts_from_now() {
        let now = Utc.with_ymd_and_hms(2020, 1, 10, 12, 0, 0).unwrap();
        let w = window_for(&now);
        // latest = start of today (2020-01-10), 0 days since, cycle 0, next = today + 1 * 7 days.
        let next = next_update_ms(0, 7, 0, &now, w);
        assert_eq!(next, at(2020, 1, 17, 0));
    }

    #[test]
    fn the_increase_rule_applies_when_the_entry_has_gone_quiet() {
        let now = Utc.with_ymd_and_hms(2020, 1, 10, 12, 0, 0).unwrap();
        // Last change 70 days ago, interval 7: effective 14, cycle = 70 div 14 = 5, next = latest + 6 * 7 days.
        let last = now - Duration::days(70);
        let next = next_update_ms(0, 7, last.timestamp_millis(), &now, window_for(&now));
        let latest = last.date_naive();
        let expected = start_of_day(latest.checked_add_signed(TimeDelta::days(42)).unwrap(), &Utc).timestamp_millis();
        assert_eq!(next, expected);
    }

    #[test]
    fn interval_never_exceeds_28_by_increase() {
        let now = Utc.with_ymd_and_hms(2021, 1, 1, 12, 0, 0).unwrap();
        let last = now - Duration::days(2000);
        let next = next_update_ms(0, 16, last.timestamp_millis(), &now, window_for(&now));
        let latest = last.date_naive();
        let days_since = (now.naive_local() - start_of_day(latest, &Utc).naive_local()).num_days();
        let effective = i64::from(increase_interval(16, days_since, INCREASE_WHEN_OVER));
        assert_eq!(effective, 28);
        let cycle = days_since.div_euclid(effective);
        let expected = start_of_day(latest.checked_add_signed(TimeDelta::days((cycle + 1) * 16)).unwrap(), &Utc);
        assert_eq!(next, expected.timestamp_millis());
    }
}
