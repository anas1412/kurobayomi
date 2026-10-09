//! Absolute date formats for the `app_date_format` preference (plan 51-i18n T-51a-017).
//!
//! Numeric patterns come from chrono. Month names and the locale short date come from ICU4X
//! (`icu_datetime`), so they follow the resolved UI locale (test T24).

use chrono::{Datelike, NaiveDate};
use icu_datetime::input::Date as IcuDate;
use icu_datetime::{DateTimeFormatter, fieldsets};
use icu_locale::Locale;

/// One choice of the Appearance "Date format" list. The stored value of each choice is the
/// Java pattern it came from, so existing preferences load unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateFormat {
    /// `""` (Default): the locale's short date.
    LocaleShort,
    /// `MM/dd/yy`
    MonthDayYear,
    /// `dd/MM/yy`
    DayMonthYear,
    /// `yyyy-MM-dd`
    IsoDate,
    /// `dd MMM yyyy`
    DayMonthNameYear,
    /// `MMM dd, yyyy`
    MonthNameDayYear,
}

impl DateFormat {
    /// The format for a stored `app_date_format` value. An unknown value is the locale short date.
    pub fn from_pref(raw: &str) -> Self {
        match raw {
            "MM/dd/yy" => Self::MonthDayYear,
            "dd/MM/yy" => Self::DayMonthYear,
            "yyyy-MM-dd" => Self::IsoDate,
            "dd MMM yyyy" => Self::DayMonthNameYear,
            "MMM dd, yyyy" => Self::MonthNameDayYear,
            _ => Self::LocaleShort,
        }
    }

    /// The value to store in `app_date_format`.
    pub fn pref(self) -> &'static str {
        match self {
            Self::LocaleShort => "",
            Self::MonthDayYear => "MM/dd/yy",
            Self::DayMonthYear => "dd/MM/yy",
            Self::IsoDate => "yyyy-MM-dd",
            Self::DayMonthNameYear => "dd MMM yyyy",
            Self::MonthNameDayYear => "MMM dd, yyyy",
        }
    }

    /// `date` in this format, with month names in `locale` (a BCP 47 tag such as `de`).
    pub fn format(self, date: NaiveDate, locale: &str) -> String {
        let day = date.format("%d").to_string();
        let year = date.format("%Y").to_string();
        match self {
            Self::LocaleShort => short_date(date, locale),
            Self::MonthDayYear => date.format("%m/%d/%y").to_string(),
            Self::DayMonthYear => date.format("%d/%m/%y").to_string(),
            Self::IsoDate => date.format("%Y-%m-%d").to_string(),
            Self::DayMonthNameYear => format!("{day} {} {year}", month_abbr(date, locale)),
            Self::MonthNameDayYear => format!("{} {day}, {year}", month_abbr(date, locale)),
        }
    }
}

/// The locale for a tag, or English when the tag does not parse.
fn locale_or_english(tag: &str) -> Locale {
    Locale::try_from_str(tag)
        .or_else(|_| Locale::try_from_str("en"))
        .expect("en is a valid locale")
}

/// The locale short date (ICU4X, length Short). Falls back to ISO when ICU data is unavailable.
// ponytail: builds the formatter per call; cache per locale if date lists redraw slowly.
fn short_date(date: NaiveDate, locale: &str) -> String {
    let fallback = || date.format("%Y-%m-%d").to_string();
    let Ok(input) = IcuDate::try_new_iso(date.year(), date.month() as u8, date.day() as u8) else {
        return fallback();
    };
    match DateTimeFormatter::try_new(locale_or_english(locale).into(), fieldsets::YMD::short()) {
        Ok(formatter) => formatter.format(&input).to_string(),
        Err(_) => fallback(),
    }
}

/// The abbreviated month name for `date` in `locale` (ICU4X, standalone month).
fn month_abbr(date: NaiveDate, locale: &str) -> String {
    let fallback = || date.format("%b").to_string();
    let Ok(input) = IcuDate::try_new_iso(date.year(), date.month() as u8, date.day() as u8) else {
        return fallback();
    };
    match DateTimeFormatter::try_new(locale_or_english(locale).into(), fieldsets::M::medium()) {
        Ok(formatter) => formatter.format(&input).to_string(),
        Err(_) => fallback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 9).expect("valid date")
    }

    #[test]
    fn fixed_patterns_use_chrono_digits() {
        assert_eq!(DateFormat::MonthDayYear.format(day(), "en"), "10/09/26");
        assert_eq!(DateFormat::DayMonthYear.format(day(), "en"), "09/10/26");
        assert_eq!(DateFormat::IsoDate.format(day(), "en"), "2026-10-09");
    }

    #[test]
    fn month_names_come_from_the_locale() {
        // T24: dd MMM yyyy and MMM dd, yyyy in de use the German month name.
        let de = DateFormat::DayMonthNameYear.format(day(), "de");
        assert!(de.starts_with("09 ") && de.ends_with(" 2026"), "{de}");
        assert!(de.contains("Okt"), "{de}");
        let en = DateFormat::MonthNameDayYear.format(day(), "en");
        assert_eq!(en, "Oct 09, 2026");
    }

    #[test]
    fn locale_short_date_is_locale_specific_and_non_empty() {
        // T23: the short date is ICU output for the locale (snapshot for en-US).
        let us = DateFormat::LocaleShort.format(day(), "en-US");
        assert_eq!(us, "10/9/26");
        assert!(!DateFormat::LocaleShort.format(day(), "de").is_empty());
    }

    #[test]
    fn stored_values_round_trip_and_unknown_is_default() {
        for format in [
            DateFormat::LocaleShort,
            DateFormat::MonthDayYear,
            DateFormat::DayMonthYear,
            DateFormat::IsoDate,
            DateFormat::DayMonthNameYear,
            DateFormat::MonthNameDayYear,
        ] {
            assert_eq!(DateFormat::from_pref(format.pref()), format);
        }
        assert_eq!(DateFormat::from_pref("bogus"), DateFormat::LocaleShort);
    }
}
