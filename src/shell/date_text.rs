//! Relative date text for a timestamp (Kotlin `relativeDateText` and `relativeDateTimeText`, T-36a-047).
//!
//! The day and time rules are in `kuro_i18n::time`. This module adds the `N/A` text for a timestamp of zero or less,
//! the preference reads (`relative_time_v2`, `app_date_format`) and the system time zone.

use chrono::{DateTime, Local, NaiveDateTime};
use kuro_i18n::date::DateFormat;
use kuro_i18n::time::{DateStyle, relative_date, relative_datetime};
use kuro_i18n::{MsgKey, tr};

/// The two preferences that shape the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs<'a> {
    /// `relative_time_v2`, default true.
    pub relative: bool,
    /// `app_date_format`: empty for the locale short date, otherwise a java.time pattern.
    pub date_format: &'a str,
}

/// The absolute format for a stored `app_date_format`. A non-empty value that is not one of the listed patterns
/// falls back to the locale short date and logs.
pub fn format_for(raw: &str) -> DateFormat {
    let format = DateFormat::from_pref(raw);
    if !raw.is_empty() && format.pref() != raw {
        tracing::warn!("app_date_format {raw:?} is not a known pattern, the locale short date is used");
    }
    format
}

/// The text for a day-only timestamp (`relativeDateText`). Zero or less renders `N/A`.
pub fn day_text(epoch_millis: i64, now_millis: i64, prefs: Prefs<'_>, locale: &str) -> String {
    if epoch_millis <= 0 {
        return tr(MsgKey::not_applicable);
    }
    let date = local_naive(epoch_millis).date();
    let today = local_naive(now_millis).date();
    relative_date(date, today, style(prefs, locale))
}

/// The text for a release time, with the hour and minute parts (`relativeDateTimeText`). Zero or less renders `N/A`.
pub fn datetime_text(epoch_millis: i64, now_millis: i64, prefs: Prefs<'_>, locale: &str) -> String {
    if epoch_millis <= 0 {
        return tr(MsgKey::not_applicable);
    }
    relative_datetime(local_naive(epoch_millis), local_naive(now_millis), style(prefs, locale))
}

fn style<'a>(prefs: Prefs<'_>, locale: &'a str) -> DateStyle<'a> {
    DateStyle { relative: prefs.relative, format: format_for(prefs.date_format), locale }
}

/// Kotlin `convertEpochMillisZone(from, to)`: the wall clock of the instant in zone `from`, read again as a wall clock
/// in zone `to`. Zones are IANA names (`UTC`, `Asia/Tokyo`); `None` for an unknown name or an out-of-range value.
pub fn convert_epoch_millis_zone(epoch_millis: i64, from: &str, to: &str) -> Option<i64> {
    let wall = jiff::Timestamp::from_millisecond(epoch_millis).ok()?.to_zoned(jiff::tz::TimeZone::get(from).ok()?).datetime();
    let zoned = wall.to_zoned(jiff::tz::TimeZone::get(to).ok()?).ok()?;
    Some(zoned.timestamp().as_millisecond())
}

fn icu_locale(locale: &str) -> icu_locale::Locale {
    locale.parse().unwrap_or_else(|_| icu_locale::Locale::try_from_str("en").expect("en is a valid locale"))
}

/// The locale short time, 12 or 24 hour by the locale (ICU short length). Falls back to `HH:mm`.
pub fn short_time(time: chrono::NaiveTime, locale: &str) -> String {
    use chrono::Timelike;
    let fallback = || time.format("%H:%M").to_string();
    let Ok(input) = icu_datetime::input::Time::try_new(time.hour() as u8, time.minute() as u8, 0, 0) else { return fallback() };
    match icu_datetime::NoCalendarFormatter::try_new(icu_locale(locale).into(), icu_datetime::fieldsets::T::short().with_time_precision(icu_datetime::options::TimePrecision::Minute)) {
        Ok(formatter) => formatter.format(&input).to_string(),
        Err(_) => fallback(),
    }
}

/// The locale short date and short time, `"{date} {time}"` (`toTimestampString`).
pub fn short_datetime(when: NaiveDateTime, locale: &str) -> String {
    format!("{} {}", DateFormat::LocaleShort.format(when.date(), locale), short_time(when.time(), locale))
}

/// The system-zone wall clock for epoch milliseconds.
fn local_naive(epoch_millis: i64) -> NaiveDateTime {
    DateTime::from_timestamp_millis(epoch_millis).map(|utc| utc.with_timezone(&Local).naive_local()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeDelta, TimeZone};

    /// Noon local time on `date`, as epoch milliseconds.
    fn noon(date: NaiveDate) -> i64 {
        Local.from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap()).unwrap().timestamp_millis()
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    const ABSOLUTE: Prefs<'static> = Prefs { relative: false, date_format: "" };

    #[test]
    fn tc_27_d_zone_conversion_reads_the_wall_clock_in_the_other_zone() {
        assert_eq!(convert_epoch_millis_zone(0, "UTC", "Asia/Tokyo"), Some(-32_400_000));
        assert_eq!(convert_epoch_millis_zone(0, "Asia/Tokyo", "UTC"), Some(32_400_000));
        assert_eq!(convert_epoch_millis_zone(5, "UTC", "UTC"), Some(5));
        assert_eq!(convert_epoch_millis_zone(0, "UTC", "Not/AZone"), None);
    }

    #[test]
    fn tc_27_a_and_b_short_datetime_is_the_short_date_then_the_short_time() {
        let when = NaiveDate::from_ymd_opt(2024, 3, 9).unwrap().and_hms_opt(15, 5, 0).unwrap();
        let date = DateFormat::LocaleShort.format(when.date(), "en");
        let text = short_datetime(when, "en");
        assert!(text.starts_with(&format!("{date} ")), "{text}");
        assert_eq!(text, format!("{date} {}", short_time(when.time(), "en")));
    }

    #[test]
    fn tc_27_c_the_short_time_follows_the_locale_clock() {
        let t = chrono::NaiveTime::from_hms_opt(15, 5, 0).unwrap();
        let en = short_time(t, "en-US");
        assert!(en.starts_with("3:05") && en.ends_with("PM"), "{en}");
        assert_eq!(short_time(t, "de-DE"), "15:05");
        assert_eq!(short_time(t, "not a locale!"), short_time(t, "en"));
    }

    #[test]
    fn zero_and_negative_timestamps_render_not_applicable() {
        let na = tr(MsgKey::not_applicable);
        assert_eq!(day_text(0, noon(day(2024, 3, 9)), ABSOLUTE, "en"), na);
        assert_eq!(day_text(-5, noon(day(2024, 3, 9)), ABSOLUTE, "en"), na);
        assert_eq!(datetime_text(0, noon(day(2024, 3, 9)), ABSOLUTE, "en"), na);
    }

    #[test]
    fn a_timestamp_on_today_is_relative_when_relative_time_is_on() {
        let now = noon(day(2024, 3, 9));
        let relative = Prefs { relative: true, date_format: "" };
        assert_eq!(day_text(now, now, relative, "en"), tr(MsgKey::relative_time_today));
    }

    #[test]
    fn relative_off_uses_the_pattern_and_empty_uses_the_short_date() {
        let now = noon(day(2024, 3, 9));
        let iso = Prefs { relative: false, date_format: "yyyy-MM-dd" };
        assert_eq!(day_text(noon(day(2024, 3, 2)), now, iso, "en"), "2024-03-02");
        assert_eq!(day_text(noon(day(2024, 3, 2)), now, ABSOLUTE, "en"), kuro_i18n::date::DateFormat::LocaleShort.format(day(2024, 3, 2), "en"));
    }

    #[test]
    fn an_invalid_pattern_falls_back_to_the_locale_short_date() {
        assert_eq!(format_for("EEEE"), DateFormat::LocaleShort);
        assert_eq!(format_for("yyyy-MM-dd"), DateFormat::IsoDate);
        assert_eq!(format_for(""), DateFormat::LocaleShort);
    }

    #[test]
    fn datetime_text_uses_the_local_zone_for_the_hour_part() {
        let now = noon(day(2024, 3, 9));
        let two_hours_earlier = now - TimeDelta::hours(2).num_milliseconds();
        let relative = Prefs { relative: true, date_format: "" };
        let expected = relative_datetime(
            local_naive(two_hours_earlier),
            local_naive(now),
            DateStyle { relative: true, format: DateFormat::LocaleShort, locale: "en" },
        );
        assert_eq!(datetime_text(two_hours_earlier, now, relative, "en"), expected);
        assert_ne!(expected, tr(MsgKey::not_applicable));
    }
}
