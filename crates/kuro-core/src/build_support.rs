//! Build-time time decision, shared by build.rs and the crate's tests.

/// Build time as unix seconds. `SOURCE_DATE_EPOCH` wins when it parses, then the git commit time,
/// then the current time.
pub fn build_time_from_epoch(env: Option<&str>, git_commit_time: Option<i64>, now: i64) -> i64 {
    env.and_then(|v| v.trim().parse().ok()).or(git_commit_time).unwrap_or(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_time_from_epoch_uses_source_date_epoch() {
        assert_eq!(build_time_from_epoch(Some("0"), Some(5), 9), 0);
        assert_eq!(build_time_from_epoch(Some(" 1700000000 "), None, 9), 1_700_000_000);
    }

    #[test]
    fn build_time_from_epoch_falls_back_when_epoch_is_invalid() {
        assert_eq!(build_time_from_epoch(Some("soon"), Some(5), 9), 5);
        assert_eq!(build_time_from_epoch(None, Some(5), 9), 5);
        assert_eq!(build_time_from_epoch(Some(""), None, 9), 9);
        assert_eq!(build_time_from_epoch(None, None, 9), 9);
    }
}
