//! UTC timestamps formatted by hand (no chrono): shared by build.rs and the crash report.

/// Formats Unix seconds as `yyyy-MM-ddTHH:mm:ssZ` (RFC 3339, UTC).
pub fn rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    // Howard Hinnant's civil_from_days: days since 1970-01-01 to (year, month, day)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::rfc3339;

    #[test]
    fn epoch_zero_is_1970() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn leap_day_and_time_of_day() {
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn negative_seconds_before_epoch() {
        assert_eq!(rfc3339(-1), "1969-12-31T23:59:59Z");
    }
}
