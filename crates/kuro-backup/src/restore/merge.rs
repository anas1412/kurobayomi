//! Rules shared by the manga and anime restorers (area 22, T-22a-011, T-22a-015, T-22a-016): the sort by new, the run
//! clock, the read and position rule of chapters and episodes, and the float rounding of chapter numbers.

use std::cmp::Reverse;
use std::collections::HashSet;

use chrono::{DateTime, Local};
use kuro_domain::entries::fetch_interval::get_window;

/// The clock of one restore run, captured once (E-47): `now`, the fetch window and `now` in epoch ms.
#[derive(Debug, Clone)]
pub struct RunClock {
    pub now: DateTime<Local>,
    pub window: (i64, i64),
    pub clock_now_ms: i64,
}

impl RunClock {
    pub fn new(now: DateTime<Local>) -> Self {
        let window = get_window(&now);
        let clock_now_ms = now.timestamp_millis();
        Self { now, window, clock_now_ms }
    }
}

/// Sorts backup entries with rows not yet in the database first, then by `lastModifiedAt` descending. Ties keep the
/// backup order (stable). `existing` is the `(source, url)` snapshot of the table, read once per section (T-22a-011).
/// `key` returns `(source, url, lastModifiedAt)`.
pub fn sort_new_first<T>(
    items: Vec<T>,
    key: impl Fn(&T) -> (i64, &str, i64),
    existing: &HashSet<(i64, String)>,
) -> Vec<T> {
    let mut keyed: Vec<((bool, Reverse<i64>), T)> = items
        .into_iter()
        .map(|t| {
            let (source, url, modified) = key(&t);
            let in_db = existing.contains(&(source, url.to_owned()));
            ((in_db, Reverse(modified)), t)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    keyed.into_iter().map(|(_, t)| t).collect()
}

/// Chapter numbers are stored with six decimals (E-27, X-26): `1.1` read from the `f32` stores as `1.1`.
pub fn round6(v: f32) -> f64 {
    let x = f64::from(v);
    (x * 1_000_000.0).round() / 1_000_000.0
}

/// The read and position rule of an existing chapter or episode (T-22a-015, topic merge rules section 2).
/// `b_done`/`b_pos` are the backup values, `d_done`/`d_pos` the database values. The done flag never reverts.
/// A position that is zero in the backup takes the database value. A smaller backup position still wins (D8 parity).
pub fn merge_progress(b_done: bool, b_pos: i64, d_done: bool, d_pos: i64) -> (bool, i64) {
    if d_done && !b_done {
        (true, d_pos)
    } else if b_pos == 0 && d_pos != 0 {
        (b_done, d_pos)
    } else {
        (b_done, b_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn new_rows_sort_before_existing_then_newest_first_and_ties_keep_order() {
        let existing: HashSet<(i64, String)> = [(1, "old".to_string())].into_iter().collect();
        // (source, url, lastModifiedAt, tag)
        let items = vec![
            (1, "old", 900, "a"),
            (1, "new1", 100, "b"),
            (1, "new2", 300, "c"),
            (1, "new3", 300, "d"),
            (2, "old", 500, "e"),
        ];
        let sorted = sort_new_first(items, |t| (t.0, t.1, t.2), &existing);
        let tags: Vec<&str> = sorted.iter().map(|t| t.3).collect();
        assert_eq!(tags, vec!["e", "c", "d", "b", "a"]);
    }

    #[test]
    fn chapter_numbers_round_to_six_decimals() {
        assert_eq!(round6(1.1), 1.1);
        assert_eq!(round6(10.5), 10.5);
        assert_eq!(round6(0.333_333_34), 0.333333);
    }

    #[test]
    fn read_flag_never_reverts_and_position_follows_the_table() {
        // D.read=true, B.read=false: read stays true, position from D
        assert_eq!(merge_progress(false, 0, true, 9), (true, 9));
        // both read: position from B, even when smaller (D8 parity)
        assert_eq!(merge_progress(true, 2, true, 9), (true, 2));
        // unread in both, B position zero: take D
        assert_eq!(merge_progress(true, 0, false, 7), (true, 7));
        // unread in both, B has position: keep B
        assert_eq!(merge_progress(false, 3, false, 0), (false, 3));
        // D unread, B read at 5 vs D 7: regress to 5
        assert_eq!(merge_progress(false, 5, false, 7), (false, 5));
    }

    #[test]
    fn run_clock_window_covers_yesterday_to_tomorrow() {
        let now = Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).single().unwrap();
        let clock = RunClock::new(now);
        let (lower, upper) = clock.window;
        let start_today = Local.with_ymd_and_hms(2026, 10, 9, 0, 0, 0).single().unwrap().timestamp_millis();
        let start_tomorrow = Local.with_ymd_and_hms(2026, 10, 10, 0, 0, 0).single().unwrap().timestamp_millis();
        assert_eq!(upper, start_tomorrow - 1);
        assert!(lower < start_today && lower >= start_today - 2 * 86_400_000);
        assert_eq!(clock.clock_now_ms, now.timestamp_millis());
    }
}
