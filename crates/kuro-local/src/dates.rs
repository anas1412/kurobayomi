//! Local UTC offset and the strict date parsers (area:14 behavior 4.6, design 19, T-14a-056).
//!
//! The offset is captured once, at startup, before any worker thread (`capture_local_offset`). The parsers are strict:
//! a value that does not match the pattern exactly, or names a day that does not exist, is `None`, which the callers
//! read as "no date" (E-DATE-1, E-DATE-2).

use std::sync::OnceLock;

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, Offset, TimeZone};

/// The zone captured once, read from the OS at first use.
static LOCAL_OFFSET: OnceLock<FixedOffset> = OnceLock::new();

/// `chapters.json` and `episodes.json` date pattern: `yyyy-MM-dd'T'HH:mm:ss` (behavior 4.6).
const CHAPTER_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";
/// EPUB `dc:date` pattern with a numeric offset (E-DATE-3). A bare `Z` is rewritten to `+0000` first.
const EPUB_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%z";

/// Captures the local offset now. Call first thing in `main`, before worker threads start.
pub fn capture_local_offset() -> FixedOffset {
    *LOCAL_OFFSET.get_or_init(|| Local::now().offset().fix())
}

/// The captured offset. A caller that never captured one gets the current offset, read once.
pub fn local_offset() -> FixedOffset {
    capture_local_offset()
}

/// True when `text` is exactly 19 bytes of `dddd-dd-ddTdd:dd:dd`: chrono accepts unpadded fields and short years,
/// the pattern does not.
fn has_exact_shape(text: &str) -> bool {
    text.len() == 19
        && text.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            10 => b == b'T',
            13 | 16 => b == b':',
            _ => b.is_ascii_digit(),
        })
}

/// `yyyy-MM-dd'T'HH:mm:ss` read as local time in `offset`, as epoch milliseconds.
pub fn parse_local_date_in(text: &str, offset: FixedOffset) -> Option<i64> {
    if !has_exact_shape(text) {
        return None;
    }
    let naive = NaiveDateTime::parse_from_str(text, CHAPTER_FORMAT).ok()?;
    let local = offset.from_local_datetime(&naive).single()?;
    Some(local.timestamp_millis())
}

/// `parse_local_date_in` with the captured local offset.
pub fn parse_local_date(text: &str) -> Option<i64> {
    parse_local_date_in(text, local_offset())
}

/// The EPUB date: `yyyy-MM-dd'T'HH:mm:ss` with `+hhmm`, `-hhmm` or `Z`, as epoch milliseconds. Anything else is `None`.
pub fn parse_epub_date(text: &str) -> Option<i64> {
    let normalized = match text.strip_suffix('Z') {
        Some(rest) => format!("{rest}+0000"),
        None => text.to_owned(),
    };
    // 19 shape bytes, then `+hhmm` or `-hhmm`.
    let (stamp, zone) = normalized.split_at_checked(19)?;
    let zone = zone.as_bytes();
    if !has_exact_shape(stamp)
        || zone.len() != 5
        || !matches!(zone[0], b'+' | b'-')
        || !zone[1..].iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let parsed = DateTime::parse_from_str(&normalized, EPUB_FORMAT).ok()?;
    Some(parsed.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset(hours: i32) -> FixedOffset {
        FixedOffset::east_opt(hours * 3600).expect("valid offset")
    }

    #[test]
    fn t_14_dates_use_local_offset() {
        // 03:04:05 at +01:00 is 02:04:05 UTC.
        let utc = parse_local_date_in("2024-01-02T03:04:05", offset(1));
        assert_eq!(utc, parse_local_date_in("2024-01-02T02:04:05", offset(0)));
        assert!(utc.is_some());
    }

    #[test]
    fn the_captured_offset_is_the_same_on_every_read() {
        assert_eq!(capture_local_offset(), local_offset());
        assert_eq!(
            parse_local_date("2024-01-02T03:04:05"),
            parse_local_date("2024-01-02T03:04:05")
        );
    }

    #[test]
    fn t_14_dates_strict_invalid_no_override() {
        assert_eq!(parse_local_date_in("2024-02-31T10:00:00", offset(0)), None);
        assert_eq!(parse_local_date_in("2024-01-02 03:04:05", offset(0)), None);
        assert_eq!(parse_local_date_in("", offset(0)), None);
        // chrono alone would accept these; the pattern does not.
        assert_eq!(parse_local_date_in("2024-1-2T3:4:5", offset(0)), None);
        assert_eq!(parse_local_date_in("24-01-02T03:04:05", offset(0)), None);
        assert!(parse_local_date_in("2024-01-02T03:04:05", offset(0)).is_some());
    }

    #[test]
    fn t_14_dates_trailing_text_rejected() {
        assert_eq!(parse_local_date_in("2024-01-02T03:04:05x", offset(0)), None);
        assert_eq!(parse_epub_date("2024-01-02T03:04:05+0100x"), None);
    }

    #[test]
    fn t_14_epub_date_formats() {
        let plus = parse_epub_date("2024-01-02T03:04:05+0100");
        assert_eq!(plus, parse_epub_date("2024-01-02T02:04:05Z"));
        assert_eq!(
            parse_epub_date("2024-01-02T03:04:05-0800"),
            parse_epub_date("2024-01-02T11:04:05Z")
        );
        assert!(plus.is_some());
        assert_eq!(parse_epub_date("2024-1-2T3:4:5+0100"), None);
        assert_eq!(parse_epub_date("24-01-02T03:04:05+0100"), None);
        assert_eq!(parse_epub_date("2024-01-02"), None);
        assert_eq!(parse_epub_date("2024-01-02T03:04:05"), None);
    }
}
