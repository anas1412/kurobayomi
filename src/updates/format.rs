//! Formatting helpers of area 42: duration text, progress clock, relative dates and times, and byte sizes.
//! Pure; the caller passes the clock and the absolute date text (T-42a-002, T-42a-003, T-42a-004).

use chrono::NaiveDate;

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;

/// The text of a date that is shown relatively, or the absolute date when the rule does not apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelativeDate {
    Today,
    /// Past days, 1 to 6.
    Ago(i64),
    /// Upcoming days, 1 to 7 (the absolute `difference` is kept positive).
    InDays(i64),
    Absolute(String),
}

impl RelativeDate {
    /// English text of the result (`relative_time_today`, `relative_time`, `upcoming_relative_time`).
    pub fn text(&self) -> String {
        match self {
            RelativeDate::Today => "Today".to_owned(),
            RelativeDate::Ago(n) => format!("{} ago", plural(*n, "day")),
            RelativeDate::InDays(n) => format!("in {}", plural(*n, "day")),
            RelativeDate::Absolute(text) => text.clone(),
        }
    }
}

/// `LocalDate.toRelativeString`: `difference` is today minus the date in days. Beyond 7 days either way, or with
/// `relative` off, the absolute text is used.
pub fn relative_date(date: NaiveDate, today: NaiveDate, relative: bool, absolute: &str) -> RelativeDate {
    if !relative {
        return RelativeDate::Absolute(absolute.to_owned());
    }
    let difference = (today - date).num_days();
    if difference < -7 || difference >= 7 {
        RelativeDate::Absolute(absolute.to_owned())
    } else if difference < 0 {
        RelativeDate::InDays(-difference)
    } else if difference < 1 {
        RelativeDate::Today
    } else {
        RelativeDate::Ago(difference)
    }
}

/// `Duration.toDurationString` (behaviour 5.4): the four `if` rules, joined with a space; blank gives `none`.
pub fn duration_string(ms: i64, none: &str) -> String {
    let total_seconds = ms / 1000;
    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    let mut parts = Vec::new();
    if days != 0 {
        parts.push(format!("{days}d"));
    }
    if hours != 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes != 0 && (days == 0 || hours == 0) {
        parts.push(format!("{minutes}m"));
    }
    if seconds != 0 && days == 0 && hours == 0 {
        parts.push(format!("{seconds}s"));
    }
    let joined = parts.join(" ");
    if joined.trim().is_empty() { none.to_owned() } else { joined }
}

/// `formatProgress` (EC-42-U-14): `H:MM:SS` only when `ms` is strictly more than one hour, else `M:SS`.
pub fn progress_mmss(ms: i64) -> String {
    let minutes_total = ms / MINUTE_MS;
    let seconds = ms / 1000 - minutes_total * 60;
    if ms > HOUR_MS {
        let hours = ms / HOUR_MS;
        let minutes = minutes_total - hours * 60;
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes_total}:{seconds:02}")
    }
}

/// A timestamp shown as elapsed time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelativeTime {
    Never,
    JustNow,
    Ago(String),
}

impl RelativeTime {
    /// English text of the result: "Never", "Just now" or the elapsed text.
    pub fn text(&self) -> String {
        match self {
            RelativeTime::Never => "Never".to_owned(),
            RelativeTime::JustNow => "Just now".to_owned(),
            RelativeTime::Ago(text) => text.clone(),
        }
    }
}

/// The "Library last updated: ..." line of the Updates feed (T-42b-016, E-U41, E-U42).
pub fn last_updated_line(last_updated_ms: i64, now_ms: i64) -> String {
    format!("{}{}", crate::updates::copy::LAST_UPDATED_PREFIX, relative_time_span(last_updated_ms, now_ms).text())
}

/// `relative_time_span`: `Never` for an unset timestamp, `JustNow` under a minute, else minutes, hours or days.
pub fn relative_time_span(epoch_ms: i64, now_ms: i64) -> RelativeTime {
    if epoch_ms <= 0 {
        return RelativeTime::Never;
    }
    let elapsed = now_ms - epoch_ms;
    if elapsed < MINUTE_MS {
        RelativeTime::JustNow
    } else if elapsed < HOUR_MS {
        RelativeTime::Ago(format!("{} ago", plural(elapsed / MINUTE_MS, "minute")))
    } else if elapsed < 24 * HOUR_MS {
        RelativeTime::Ago(format!("{} ago", plural(elapsed / HOUR_MS, "hour")))
    } else {
        RelativeTime::Ago(format!("{} ago", plural(elapsed / (24 * HOUR_MS), "day")))
    }
}

/// `Long.toSize()` (EC-42-T-10): SI units with 1000 steps and `f32` division. `999_999` gives `1000.00 KB`.
pub fn format_size(bytes: i64) -> String {
    const KB: i64 = 1_000;
    const MB: i64 = 1_000_000;
    const GB: i64 = 1_000_000_000;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f32 / GB as f32)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f32 / MB as f32)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f32 / KB as f32)
    } else {
        format!("{bytes} B")
    }
}

/// "1 day" or "5 days".
fn plural(n: i64, unit: &str) -> String {
    if n == 1 { format!("1 {unit}") } else { format!("{n} {unit}s") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    #[test]
    fn format_relative_date_thresholds() {
        let today = d(9);
        assert_eq!(relative_date(d(9), today, true, "abs"), RelativeDate::Today);
        assert_eq!(relative_date(d(8), today, true, "abs"), RelativeDate::Ago(1));
        assert_eq!(relative_date(d(3), today, true, "abs"), RelativeDate::Ago(6));
        assert_eq!(relative_date(d(2), today, true, "abs"), RelativeDate::Absolute("abs".into()));
        assert_eq!(relative_date(d(10), today, true, "abs"), RelativeDate::InDays(1));
        assert_eq!(relative_date(d(16), today, true, "abs"), RelativeDate::InDays(7));
        assert_eq!(relative_date(d(17), today, true, "abs"), RelativeDate::Absolute("abs".into()));
        assert_eq!(relative_date(d(9), today, false, "abs"), RelativeDate::Absolute("abs".into()));
        assert_eq!(RelativeDate::Ago(1).text(), "1 day ago");
        assert_eq!(RelativeDate::Ago(6).text(), "6 days ago");
        assert_eq!(RelativeDate::InDays(3).text(), "in 3 days");
    }

    #[test]
    fn format_duration_string_rules() {
        assert_eq!(duration_string(0, "None"), "None");
        assert_eq!(duration_string(5_000, "None"), "5s");
        assert_eq!(duration_string(61 * 60_000, "None"), "1h 1m");
        assert_eq!(duration_string(25 * 3_600_000 + 3 * 60_000, "None"), "1d 1h");
        assert_eq!(duration_string(86_400_000 + 5 * 60_000, "None"), "1d 5m");
        assert_eq!(duration_string(3_600_000 + 5_000, "None"), "1h");
    }

    #[test]
    fn format_progress_mmss_boundaries() {
        assert_eq!(progress_mmss(0), "0:00");
        assert_eq!(progress_mmss(59_999), "0:59");
        assert_eq!(progress_mmss(3_600_000), "60:00");
        assert_eq!(progress_mmss(3_600_001), "1:00:00");
        assert_eq!(progress_mmss(3_661_000), "1:01:01");
    }

    #[test]
    fn format_relative_time_span_never_and_just_now() {
        let now = 10_000_000_000;
        assert_eq!(relative_time_span(0, now), RelativeTime::Never);
        assert_eq!(relative_time_span(now - 59_999, now), RelativeTime::JustNow);
        assert_eq!(relative_time_span(now - 60_000, now), RelativeTime::Ago("1 minute ago".into()));
        assert_eq!(relative_time_span(now - 5 * HOUR_MS, now), RelativeTime::Ago("5 hours ago".into()));
    }

    #[test]
    fn format_size_vectors() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(999), "999 B");
        assert_eq!(format_size(1_000), "1.00 KB");
        assert_eq!(format_size(999_999), "1000.00 KB");
        assert_eq!(format_size(1_000_000), "1.00 MB");
        assert_eq!(format_size(1_500_000_000), "1.50 GB");
    }

    #[test]
    fn updates_last_updated_line_values() {
        let now = 1_700_000_000_000;
        assert_eq!(last_updated_line(0, now), "Library last updated: Never");
        assert_eq!(last_updated_line(now - 30_000, now), "Library last updated: Just now");
        assert_eq!(last_updated_line(now - 5 * 60_000, now), "Library last updated: 5 minutes ago");
        assert_eq!(last_updated_line(now - 3 * 3_600_000, now), "Library last updated: 3 hours ago");
    }
}
