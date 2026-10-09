//! Calendar helpers shared by history and updates: the local date of an epoch-ms timestamp, the date-header rule,
//! the `#.###` number format of "Ch. {n}" rows and the "Library last updated" phrase. Pure: every conversion takes
//! an explicit time zone, and the app passes `chrono::Local`.

use std::fmt::Write;

use chrono::{DateTime, NaiveDate, TimeZone};

/// One line of a date-grouped list: a header, or an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListRow<T> {
    Header(String),
    Item(T),
}

/// The local date of an epoch-ms timestamp, or `None` for a timestamp chrono cannot represent.
pub fn local_date<Tz: TimeZone>(ms: i64, tz: &Tz) -> Option<NaiveDate> {
    DateTime::from_timestamp_millis(ms).map(|d| d.with_timezone(tz).date_naive())
}

/// The local wall-clock time `HH:MM` of a timestamp.
pub fn short_time<Tz: TimeZone>(ms: i64, tz: &Tz) -> Option<String> {
    DateTime::from_timestamp_millis(ms).map(|d| d.with_timezone(tz).naive_local().format("%H:%M").to_string())
}

/// The text of a date header. With `relative` on, the day itself is "Today", the day before "Yesterday", and two
/// to six days back "N days ago". Anything else, and every date when `relative` is off, is the absolute date in
/// `format` (an strftime pattern, `%Y-%m-%d` when empty or invalid).
pub fn header_label(date: NaiveDate, today: NaiveDate, relative: bool, format: Option<&str>) -> String {
    let days = (today - date).num_days();
    if relative {
        match days {
            0 => return "Today".to_owned(),
            1 => return "Yesterday".to_owned(),
            2..=6 => return format!("{days} days ago"),
            _ => {}
        }
    }
    let pattern = format.filter(|f| !f.is_empty()).unwrap_or("%Y-%m-%d");
    let mut out = String::new();
    if write!(out, "{}", date.format(pattern)).is_err() {
        out = date.format("%Y-%m-%d").to_string();
    }
    out
}

/// `insertSeparators`: a header goes before the first item and before every item whose date differs from the one
/// before it. The list keeps its order.
pub fn with_date_headers<T>(items: Vec<(T, NaiveDate)>, label: impl Fn(NaiveDate) -> String) -> Vec<ListRow<T>> {
    let mut out = Vec::with_capacity(items.len() * 2);
    let mut previous: Option<NaiveDate> = None;
    for (item, date) in items {
        if previous != Some(date) {
            out.push(ListRow::Header(label(date)));
            previous = Some(date);
        }
        out.push(ListRow::Item(item));
    }
    out
}

/// `DecimalFormat("#.###")`: at most three decimals, no trailing zeros, and no leading zero before the point
/// (0.5 reads `.5`). The grouping separator is never used.
pub fn format_number(n: f64) -> String {
    let rounded = (n * 1000.0).round() / 1000.0;
    let mut body = format!("{:.3}", rounded.abs());
    if body.contains('.') {
        while body.ends_with('0') {
            body.pop();
        }
        if body.ends_with('.') {
            body.pop();
        }
    }
    if let Some(rest) = body.strip_prefix("0.") {
        body = format!(".{rest}");
    }
    if rounded < 0.0 && body != "0" { format!("-{body}") } else { body }
}

/// The row text of a chapter or episode: `Ch. 12 - 14:05`. A number not greater than -1 shows only the time.
pub fn item_row_text(prefix: &str, number: f64, time: &str) -> String {
    if number > -1.0 { format!("{prefix} {} - {time}", format_number(number)) } else { time.to_owned() }
}

/// "Library last updated" phrase: "Never" for 0, "Just now" under one minute, else minutes, hours or days ago.
pub fn last_updated_text(last_ms: i64, now_ms: i64) -> String {
    if last_ms == 0 {
        return "Library last updated: Never".to_owned();
    }
    let minutes = (now_ms - last_ms).max(0) / 60_000;
    let phrase = if minutes < 1 {
        "Just now".to_owned()
    } else if minutes < 60 {
        plural(minutes, "minute")
    } else if minutes < 60 * 24 {
        plural(minutes / 60, "hour")
    } else {
        plural(minutes / (60 * 24), "day")
    };
    if phrase == "Just now" {
        format!("Library last updated: {phrase}")
    } else {
        format!("Library last updated: {phrase} ago")
    }
}

fn plural(n: i64, unit: &str) -> String {
    if n == 1 { format!("1 {unit}") } else { format!("{n} {unit}s") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn number_format_matches_the_decimal_pattern() {
        assert_eq!(format_number(12.0), "12");
        assert_eq!(format_number(12.5), "12.5");
        assert_eq!(format_number(1.0015), "1.002", "rounds half-up at three decimals");
        assert_eq!(format_number(0.5), ".5");
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-2.25), "-2.25");
    }

    #[test]
    fn row_text_drops_the_number_when_it_is_not_greater_than_minus_one() {
        assert_eq!(item_row_text("Ch.", 3.0, "14:05"), "Ch. 3 - 14:05");
        assert_eq!(item_row_text("Ch.", -1.0, "14:05"), "14:05");
        assert_eq!(item_row_text("Ep.", -0.5, "09:00"), "Ep. -.5 - 09:00");
    }

    #[test]
    fn header_labels_use_relative_words_only_when_enabled() {
        let today = d(2025, 3, 10);
        assert_eq!(header_label(d(2025, 3, 10), today, true, None), "Today");
        assert_eq!(header_label(d(2025, 3, 9), today, true, None), "Yesterday");
        assert_eq!(header_label(d(2025, 3, 5), today, true, None), "5 days ago");
        assert_eq!(header_label(d(2025, 3, 3), today, true, None), "2025-03-03");
        assert_eq!(header_label(d(2025, 3, 10), today, false, None), "2025-03-10");
        assert_eq!(header_label(d(2025, 3, 3), today, false, Some("%d/%m/%Y")), "03/03/2025");
        assert_eq!(header_label(d(2025, 3, 3), today, false, Some("%Q")), "2025-03-03", "invalid pattern falls back");
    }

    #[test]
    fn a_header_goes_before_the_first_item_and_at_each_date_change() {
        let items = vec![("a", d(2025, 3, 2)), ("b", d(2025, 3, 2)), ("c", d(2025, 3, 1))];
        let rows = with_date_headers(items, |date| date.to_string());
        assert_eq!(
            rows,
            vec![
                ListRow::Header("2025-03-02".into()),
                ListRow::Item("a"),
                ListRow::Item("b"),
                ListRow::Header("2025-03-01".into()),
                ListRow::Item("c"),
            ]
        );
    }

    #[test]
    fn local_date_follows_the_zone_passed_in() {
        // 2025-03-02 23:59 at UTC+1 is 22:59 UTC on the same day; 2025-03-03 00:30 UTC+1 is 2025-03-02 23:30 UTC.
        let late = DateTime::parse_from_rfc3339("2025-03-02T23:59:00+01:00").unwrap().timestamp_millis();
        let zone = chrono::FixedOffset::east_opt(3600).unwrap();
        assert_eq!(local_date(late, &zone), Some(d(2025, 3, 2)));
        assert_eq!(local_date(late, &Utc), Some(d(2025, 3, 2)));
        assert_eq!(short_time(late, &zone).as_deref(), Some("23:59"));
    }

    #[test]
    fn last_updated_phrases() {
        let now = 10_000_000_000i64;
        assert_eq!(last_updated_text(0, now), "Library last updated: Never");
        assert_eq!(last_updated_text(now - 30_000, now), "Library last updated: Just now");
        assert_eq!(last_updated_text(now - 5 * 60_000, now), "Library last updated: 5 minutes ago");
        assert_eq!(last_updated_text(now - 60 * 60_000, now), "Library last updated: 1 hour ago");
        assert_eq!(last_updated_text(now - 3 * 24 * 60 * 60_000, now), "Library last updated: 3 days ago");
    }
}
