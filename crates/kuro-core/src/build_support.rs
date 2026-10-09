//! Build-time time decision, shared by build.rs and the crate's tests.

/// Build time as unix seconds. `SOURCE_DATE_EPOCH` wins when it parses, then the git commit time,
/// then the current time.
pub fn build_time_from_epoch(env: Option<&str>, git_commit_time: Option<i64>, now: i64) -> i64 {
    env.and_then(|v| v.trim().parse().ok()).or(git_commit_time).unwrap_or(now)
}

/// The Cargo profile a build runs under, from the `OUT_DIR` of a build script
/// (`<target>/[<triple>/]<profile dir>/build/<package>-<hash>/out`). Cargo's own `PROFILE` variable is only `debug` or
/// `release`, so a custom profile such as `preview` would read as `release` there. The `dev` profile's directory is
/// `debug`. `fallback` (`PROFILE`) is used when the path has no such shape.
pub fn build_type_from_out_dir(out_dir: &str, fallback: &str) -> String {
    let path = std::path::Path::new(out_dir);
    let mut parts = path.ancestors().skip(3);
    let (Some(profile_dir), Some(build)) = (parts.next(), path.ancestors().nth(2)) else {
        return fallback.to_owned();
    };
    let is_build_dir = build.file_name().is_some_and(|n| n == "build");
    match profile_dir.file_name().and_then(|n| n.to_str()) {
        Some(name) if is_build_dir => name.to_owned(),
        _ => fallback.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_build_type_is_the_profile_directory() {
        let out = |profile: &str| format!("/w/target/{profile}/build/kuro-core-0123abcd/out");
        assert_eq!(build_type_from_out_dir(&out("debug"), "debug"), "debug");
        assert_eq!(build_type_from_out_dir(&out("release"), "release"), "release");
        assert_eq!(build_type_from_out_dir(&out("preview"), "release"), "preview");
        assert_eq!(build_type_from_out_dir("/w/target/x86_64-unknown-linux-gnu/dist/build/k-1/out", "release"), "dist");
        assert_eq!(build_type_from_out_dir("/somewhere/else", "release"), "release");
        assert_eq!(build_type_from_out_dir("", "debug"), "debug");
    }

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
