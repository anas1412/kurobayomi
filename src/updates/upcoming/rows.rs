//! The text of the Upcoming rows (T-42b-030). Pure: the page passes today, the date preferences and the locale.

use chrono::NaiveDate;
use kuro_i18n::date::DateFormat;
use kuro_i18n::time::{DateStyle, relative_date};

/// The text of a date heading: Today, Tomorrow and "In N days" up to a week ahead, "N days ago" for a day that has gone,
/// else the absolute date (`relativeDateText`), or only the absolute date while `relative` is off.
pub fn header_text(date: NaiveDate, today: NaiveDate, relative: bool, pref: &str, locale: &str) -> String {
    relative_date(date, today, DateStyle { relative, format: DateFormat::from_pref(pref), locale })
}

/// The text of the count badge of a heading, in words: `1 entry`, `3 entries`.
pub fn badge_text(count: u32) -> String {
    if count == 1 { "1 entry".to_owned() } else { format!("{count} entries") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).expect("a date")
    }

    #[test]
    fn rows_heading_uses_relative_words_for_the_coming_week() {
        assert_eq!(header_text(d(9), d(9), true, "", "en"), "Today");
        assert_eq!(header_text(d(10), d(9), true, "", "en"), "Tomorrow");
        assert_eq!(header_text(d(12), d(9), true, "", "en"), "In 3 days");
    }

    #[test]
    fn rows_heading_is_absolute_past_a_week_or_with_relative_off() {
        assert_eq!(header_text(d(25), d(9), true, "yyyy-MM-dd", "en"), "2026-10-25");
        assert_eq!(header_text(d(9), d(9), false, "yyyy-MM-dd", "en"), "2026-10-09");
    }

    #[test]
    fn rows_badge_is_the_count() {
        assert_eq!(badge_text(1), "1 entry");
        assert_eq!(badge_text(3), "3 entries");
    }
}
