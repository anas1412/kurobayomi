//! Date display for `app_date_format` and the relative-day rule of `relative_time_v2`.
//!
//! Patterns are java.time patterns (the strings Aniyomi stores). They are translated to chrono
//! strftime items. The translator knows `yyyy yy MMM MM dd`; any other letter or a quote makes the
//! pattern unsupported, and an unsupported pattern renders as the short date, like an empty one.

use std::iter::Peekable;
use std::str::Chars;

use chrono::NaiveDate;

/// The short date (`03/09/24` for 2024-03-09). Used for an empty pattern and as the fallback for
/// a pattern this module cannot translate.
///
/// Decision: chrono's `%x` is the fixed en-US short date, so this is en-US in every locale until
/// kuro-i18n supplies the locale short-date pattern (plan p1-behavior 5.3 proposes glib `%x`).
const SHORT: &str = "%x";

/// Renders `date` with an `app_date_format` pattern. An empty or unsupported pattern gives the
/// short date.
pub fn format(pattern: &str, date: NaiveDate) -> String {
    let strftime = if pattern.is_empty() {
        None
    } else {
        translate(pattern)
    };
    date.format(strftime.as_deref().unwrap_or(SHORT))
        .to_string()
}

/// Translates a java.time pattern to a strftime string, or `None` when it uses an unsupported
/// letter or a quote. A literal `%` is escaped.
fn translate(pattern: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars: Peekable<Chars<'_>> = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if !matches!(c, 'y' | 'M' | 'd') {
            match c {
                '\'' => return None,
                c if c.is_ascii_alphabetic() => return None,
                '%' => out.push_str("%%"),
                other => out.push(other),
            }
            continue;
        }
        let mut run = 1;
        while chars.next_if_eq(&c).is_some() {
            run += 1;
        }
        out.push_str(match (c, run) {
            ('y', 4) => "%Y",
            ('y', 2) => "%y",
            ('M', 3) => "%b",
            ('M', 2) => "%m",
            ('d', 2) => "%d",
            _ => return None,
        });
    }
    Some(out)
}

/// Where a date falls relative to today, for relative timestamps (Kotlin `toRelativeString`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeDay {
    /// More than seven days ahead, or more than seven days ago: show the formatted date.
    Formatted,
    /// Ahead by this many days (1 to 7).
    Upcoming(u32),
    /// The same day.
    Today,
    /// Behind by this many days (1 to 6).
    Past(u32),
}

/// The relative rule for `date` seen from `today`. The difference is in whole days, positive
/// when `date` is in the past, as Kotlin's `ChronoUnit.DAYS.between(date, today)`.
pub fn relative_day(date: NaiveDate, today: NaiveDate) -> RelativeDay {
    let diff = (today - date).num_days();
    match diff {
        d if d < -7 => RelativeDay::Formatted,
        d if d < 0 => RelativeDay::Upcoming(d.unsigned_abs() as u32),
        0 => RelativeDay::Today,
        d if d < 7 => RelativeDay::Past(d as u32),
        _ => RelativeDay::Formatted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn march_9() -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 3, 9).unwrap()
    }

    #[test]
    fn the_five_listed_patterns_render_the_documented_strings() {
        let d = march_9();
        assert_eq!(format("MM/dd/yy", d), "03/09/24");
        assert_eq!(format("dd/MM/yy", d), "09/03/24");
        assert_eq!(format("yyyy-MM-dd", d), "2024-03-09");
        assert_eq!(format("dd MMM yyyy", d), "09 Mar 2024");
        assert_eq!(format("MMM dd, yyyy", d), "Mar 09, 2024");
    }

    #[test]
    fn an_empty_pattern_gives_the_en_us_short_date() {
        assert_eq!(format("", march_9()), "03/09/24");
    }

    #[test]
    fn an_unsupported_pattern_falls_back_to_the_en_us_short_date() {
        let d = march_9();
        assert_eq!(format("EEEE", d), "03/09/24");
        assert_eq!(format("yyyy-MM-dd'T", d), "03/09/24");
        assert_eq!(format("d/M/yy", d), "03/09/24");
    }

    #[test]
    fn literal_text_and_percent_signs_pass_through() {
        assert_eq!(format("yyyy%MM", march_9()), "2024%03");
        assert_eq!(format("dd.MM.yyyy", march_9()), "09.03.2024");
    }

    #[test]
    fn relative_day_follows_the_day_difference_table() {
        let today = march_9();
        let ago = |n: i64| today - chrono::TimeDelta::days(n);
        let ahead = |n: i64| today + chrono::TimeDelta::days(n);
        assert_eq!(relative_day(today, today), RelativeDay::Today);
        assert_eq!(relative_day(ago(1), today), RelativeDay::Past(1));
        assert_eq!(relative_day(ago(6), today), RelativeDay::Past(6));
        assert_eq!(relative_day(ago(7), today), RelativeDay::Formatted);
        assert_eq!(relative_day(ahead(1), today), RelativeDay::Upcoming(1));
        assert_eq!(relative_day(ahead(7), today), RelativeDay::Upcoming(7));
        assert_eq!(relative_day(ahead(8), today), RelativeDay::Formatted);
    }
}
