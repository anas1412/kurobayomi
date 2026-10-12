//! Clock and local calendar helpers (T-05a-030, T-05a-020). The clock is passed in, so every re-query can
//! recompute the cut-off from the current time. Day arithmetic uses the zone rules at each instant, so a DST
//! change is handled; the offset is not cached, because one cached offset would be wrong after the change.

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Duration, Local, LocalResult, NaiveDate, NaiveDateTime, TimeZone};

/// A source of the current instant, in epoch milliseconds. Tests pass a fixed one.
pub trait Clock: Send + Sync {
    fn now_millis(&self) -> i64;
}

/// A closure that reads the instant is a clock too (the legacy repos still take one).
impl<F: Fn() -> i64 + Send + Sync> Clock for F {
    fn now_millis(&self) -> i64 {
        self()
    }
}

/// The system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> i64 {
        now_millis()
    }
}

/// Epoch milliseconds now. A clock set before 1970 reads as 0.
pub fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Epoch seconds now (the `*_secs` columns of the schema; `date_upload` and friends are milliseconds).
pub fn now_secs() -> i64 {
    now_millis() / 1000
}

/// Epoch milliseconds of local midnight that starts the day containing `now_ms`.
///
/// When midnight does not exist (a DST gap at 00:00), the first existing instant after it is used,
/// as `atStartOfDay` does. An ambiguous midnight takes the earlier instant.
pub fn start_of_local_day_millis(now_ms: i64) -> i64 {
    start_of_day_in(now_ms, &Local)
}

/// [`start_of_local_day_millis`] for an arbitrary zone. Tests use fixed offsets; the app uses the local zone.
pub fn start_of_day_in<Tz: TimeZone>(now_ms: i64, zone: &Tz) -> i64 {
    let Some(now) = zone.timestamp_millis_opt(now_ms).single() else {
        return now_ms;
    };
    midnight_millis(now.date_naive(), zone)
}

/// Epoch milliseconds of the local midnight that starts the day after the one containing `now_ms`.
pub fn next_local_midnight_millis(now_ms: i64) -> i64 {
    let Some(now) = Local.timestamp_millis_opt(now_ms).single() else {
        return now_ms;
    };
    match now.date_naive().succ_opt() {
        Some(tomorrow) => midnight_millis(tomorrow, &Local),
        None => now_ms,
    }
}

/// How long a clock-driven query waits before it runs again at the next local midnight (EC-03). At least
/// one second, so a clock that cannot advance never makes the query spin.
pub fn until_next_local_midnight(now_ms: i64) -> std::time::Duration {
    let wait_ms = (next_local_midnight_millis(now_ms) - now_ms).max(1_000);
    std::time::Duration::from_millis(wait_ms as u64)
}

fn midnight_millis<Tz: TimeZone>(date: NaiveDate, zone: &Tz) -> i64 {
    let midnight: NaiveDateTime = date.and_hms_opt(0, 0, 0).expect("00:00:00 is a valid time of day");
    (0..=3)
        .find_map(|hours| match zone.from_local_datetime(&(midnight + Duration::hours(hours))) {
            LocalResult::Single(t) => Some(t.timestamp_millis()),
            LocalResult::Ambiguous(earliest, _) => Some(earliest.timestamp_millis()),
            LocalResult::None => None,
        })
        .unwrap_or_else(|| midnight.and_utc().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, Timelike};

    use super::*;

    #[test]
    fn next_midnight_is_local_midnight_of_the_following_day() {
        let now = Local::now();
        let next_ms = next_local_midnight_millis(now.timestamp_millis());
        let next = Local.timestamp_millis_opt(next_ms).single().unwrap();
        assert!(next > now);
        assert_eq!(next.date_naive(), now.date_naive().succ_opt().unwrap());
        assert_eq!((next.hour(), next.minute(), next.second()), (0, 0, 0));
        let wait = until_next_local_midnight(now.timestamp_millis());
        assert!(wait >= std::time::Duration::from_secs(1) && wait <= std::time::Duration::from_secs(25 * 3600));
    }

    #[test]
    fn start_of_day_is_local_midnight_of_the_same_day() {
        let now = Local::now();
        let start_ms = start_of_local_day_millis(now.timestamp_millis());
        let start = Local.timestamp_millis_opt(start_ms).single().unwrap();
        assert!(start <= now);
        assert_eq!(start.date_naive(), now.date_naive());
        assert_eq!((start.hour(), start.minute(), start.second()), (0, 0, 0));
    }

    #[test]
    fn any_instant_of_a_day_maps_to_the_same_start() {
        let morning = Local.with_ymd_and_hms(2026, 3, 1, 0, 0, 1).single();
        let evening = Local.with_ymd_and_hms(2026, 3, 1, 23, 59, 59).single();
        if let (Some(a), Some(b)) = (morning, evening) {
            assert_eq!(
                start_of_local_day_millis(a.timestamp_millis()),
                start_of_local_day_millis(b.timestamp_millis())
            );
        }
    }

    #[test]
    fn start_of_day_matches_midnight_in_two_fixed_zones() {
        // 2026-03-01T20:30:00Z is 2026-03-02 05:30 in +09:00 and 2026-03-01 15:30 in -05:00.
        let instant = utc_ms(2026, 3, 1, 20, 30);
        let tokyo = FixedOffset::east_opt(9 * 3600).unwrap();
        let new_york = FixedOffset::west_opt(5 * 3600).unwrap();
        assert_eq!(start_of_day_in(instant, &tokyo), utc_ms(2026, 3, 1, 15, 0));
        assert_eq!(start_of_day_in(instant, &new_york), utc_ms(2026, 3, 1, 5, 0));
    }

    #[test]
    fn a_clock_reads_the_instant_it_was_given() {
        let clock = crate::support::FixedClock(1_700_000_000_123);
        assert_eq!(clock.now_millis(), 1_700_000_000_123);
        assert!(now_millis() > 1_700_000_000_000);
        assert_eq!(now_secs(), now_millis() / 1000);
    }

    fn utc_ms(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
        chrono::Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap().timestamp_millis()
    }
}
