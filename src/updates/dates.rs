//! Date headers of the History and Updates lists (T-42a-002). The absolute date is the `app_date_format` preference
//! through `kuro_i18n::date::DateFormat` (an empty value is the locale short date); the relative words are the
//! `relative_time_today` and `relative_time` messages. Pure: the caller passes today, the preference and the locale.

use chrono::{NaiveDate, TimeZone};
use kuro_domain::dates::{ListRow, local_date};
use kuro_i18n::date::DateFormat;
use kuro_i18n::{MsgKey, tr, tr_plural};

/// The text of a date header: Today, Yesterday and "N days ago" (days 2 to 6) while `relative` is on, otherwise the
/// absolute date in the stored `pref` format.
pub fn header_text(date: NaiveDate, today: NaiveDate, relative: bool, pref: &str, locale: &str) -> String {
    let days = (today - date).num_days();
    if relative {
        match days {
            0 => return tr(MsgKey::relative_time_today),
            1..=6 => return tr_plural(MsgKey::relative_time, days, &[]),
            _ => {}
        }
    }
    DateFormat::from_pref(pref).format(date, locale)
}

/// Groups `(item, epoch ms)` pairs, already in display order, under a header per local date. An item whose timestamp
/// cannot be placed on a date stays in the list under the previous header.
pub fn grouped<T, Tz: TimeZone>(
    items: impl IntoIterator<Item = (T, i64)>,
    zone: &Tz,
    today: NaiveDate,
    relative: bool,
    pref: &str,
    locale: &str,
) -> Vec<ListRow<T>> {
    let dated = items.into_iter().map(|(item, ms)| (item, local_date(ms, zone)));
    crate::updates::list::group_by_date(dated.collect(), |date| header_text(date, today, relative, pref, locale))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    #[test]
    fn dates_relative_words() {
        assert_eq!(header_text(d(9), d(9), true, "", "en"), "Today");
        assert_eq!(header_text(d(8), d(9), true, "", "en"), "Yesterday");
        assert_eq!(header_text(d(4), d(9), true, "", "en"), "5 days ago");
    }

    #[test]
    fn dates_empty_pattern_is_the_locale_short_date() {
        let text = header_text(d(1), d(30), true, "", "en");
        assert_eq!(text, DateFormat::LocaleShort.format(d(1), "en"));
        assert_ne!(text, "2026-10-01", "the short date of the locale is not the ISO date");
    }

    #[test]
    fn dates_java_pattern_is_translated_not_printed() {
        assert_eq!(header_text(d(1), d(30), false, "dd/MM/yy", "en"), "01/10/26");
        assert_eq!(header_text(d(1), d(30), false, "yyyy-MM-dd", "en"), "2026-10-01");
        assert_eq!(header_text(d(1), d(30), false, "dd MMM yyyy", "en"), "01 Oct 2026");
    }

    #[test]
    fn dates_relative_off_shows_the_absolute_date_even_for_today() {
        assert_eq!(header_text(d(9), d(9), false, "yyyy-MM-dd", "en"), "2026-10-09");
    }

    #[test]
    fn dates_group_under_one_header_per_local_day() {
        let ms = |day: u32, hour: u32| Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap().timestamp_millis();
        let rows = grouped([("a", ms(9, 12)), ("b", ms(9, 1)), ("c", ms(8, 12))], &Utc, d(9), true, "", "en");
        assert_eq!(
            rows,
            vec![
                ListRow::Header("Today".into()),
                ListRow::Item("a"),
                ListRow::Item("b"),
                ListRow::Header("Yesterday".into()),
                ListRow::Item("c"),
            ]
        );
    }
}
