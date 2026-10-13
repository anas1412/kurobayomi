//! Start and finish date rules of the track sheet (T-38b-032, T-39b-043; behaviour 8.5, topic track 6.1 and 6.3). The
//! picker math is `kuro_track::picker::DatePicker`; this file names the rules the widgets ask for and fixes the zone
//! handling: a stored date is local midnight, and it reads back on the same day.

use chrono::{NaiveDate, TimeZone};
use kuro_track::picker::{DatePicker, from_utc_wall, picker_day, picker_today, utc_midnight_millis};

/// Whether `day` can be picked for the start (`is_start`) or finish date. Future days are never selectable. A start
/// may not be after the finish, and a finish may not be before the start (rules 1 to 4).
pub fn date_selectable(is_start: bool, day: NaiveDate, other: Option<NaiveDate>, today: NaiveDate) -> bool {
    DatePicker::new(is_start, day, other, today).is_selectable(day)
}

/// The picker opens on the stored date, or on today when none is stored (`initial_date_ms`).
pub fn initial_day(stored_ms: i64, now_ms: i64, zone: &impl TimeZone) -> NaiveDate {
    picker_day(stored_ms, zone).unwrap_or_else(|| picker_today(now_ms, zone))
}

/// The stored value of `day`: its local midnight in `zone`. In a DST gap this is the earliest valid instant (E-DATE-1).
pub fn stored_ms(day: NaiveDate, zone: &impl TimeZone) -> i64 {
    from_utc_wall(utc_midnight_millis(day), zone)
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, Utc};

    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    const TODAY: fn() -> NaiveDate = || day(2026, 10, 10);

    #[test]
    fn start_date_not_after_finish() {
        let finish = Some(day(2026, 3, 15));
        assert!(date_selectable(true, day(2026, 3, 15), finish, TODAY()), "the start may equal the finish");
        assert!(!date_selectable(true, day(2026, 3, 16), finish, TODAY()), "a start after the finish is refused");
        assert!(date_selectable(true, day(2026, 3, 1), None, TODAY()), "no finish, no limit");
    }

    #[test]
    fn finish_date_not_before_start() {
        let start = Some(day(2026, 3, 15));
        assert!(date_selectable(false, day(2026, 3, 15), start, TODAY()));
        assert!(!date_selectable(false, day(2026, 3, 14), start, TODAY()), "a finish before the start is refused");
    }

    #[test]
    fn local_midnight_roundtrip() {
        for zone in [FixedOffset::east_opt(5 * 3600 + 1800).unwrap(), FixedOffset::west_opt(7 * 3600).unwrap()] {
            let d = day(2026, 1, 31);
            let stored = stored_ms(d, &zone);
            assert_eq!(initial_day(stored, 0, &zone), d, "the stored midnight reads back on the same day");
        }
        assert_eq!(stored_ms(day(1970, 1, 1), &Utc), 0, "UTC midnight of the epoch is zero");
    }

    #[test]
    fn an_unset_date_opens_on_today() {
        let zone = Utc;
        let now = utc_midnight_millis(day(2026, 10, 10)) + 3_600_000;
        assert_eq!(initial_day(0, now, &zone), day(2026, 10, 10));
    }
}
