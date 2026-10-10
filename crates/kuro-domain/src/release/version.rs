//! The version rule of the release check. Replaces `GetApplicationRelease.isNewVersion`, which compared components by
//! index and so panicked on short versions, and read pre-release text as digits.

use semver::Version;

/// Parses a release tag or a package version. A leading `v` or `V` is dropped, and a `-pre-release` or `+build`
/// suffix is cut off. Missing minor and patch parts count as 0 (`1.2` is `1.2.0`). Any other text, a tag with more
/// than three numeric parts included, is `None`. It is never a panic.
pub fn parse_tag(tag: &str) -> Option<Version> {
    let t = tag.trim();
    let t = t.strip_prefix(['v', 'V']).unwrap_or(t);
    let core = t.split(['-', '+']).next().unwrap_or("");
    let digits: String = core.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    let parts: Vec<&str> = digits.split('.').collect();
    if parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let mut nums = [0_u64; 3];
    for (slot, part) in nums.iter_mut().zip(&parts) {
        *slot = part.parse().ok()?;
    }
    Some(Version::new(nums[0], nums[1], nums[2]))
}

/// True when `candidate` (a release tag) is a newer version than `current` (the installed version). An unparseable
/// side is never newer, so a `nightly` tag or a tag with no digits raises no update.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_tag(candidate), parse_tag(current)) {
        (Some(new), Some(old)) => new > old,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_v_and_missing_parts_are_read_as_zero() {
        assert_eq!(parse_tag("v1.2").unwrap(), Version::new(1, 2, 0));
        assert_eq!(parse_tag("V3").unwrap(), Version::new(3, 0, 0));
        assert_eq!(parse_tag(" 0.17.1 ").unwrap(), Version::new(0, 17, 1));
    }

    #[test]
    fn suffixes_are_cut_so_a_pre_release_reads_as_its_base_version() {
        assert_eq!(parse_tag("v1.2.3-rc1").unwrap(), Version::new(1, 2, 3));
        assert_eq!(parse_tag("1.2.3+build7").unwrap(), Version::new(1, 2, 3));
    }

    #[test]
    fn text_without_a_version_is_none() {
        for bad in ["nightly", "", "v", "1..2", "1.2.3.4"] {
            assert_eq!(parse_tag(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn newer_is_strictly_greater_with_missing_parts_as_zero() {
        assert!(is_newer("v2.0.0", "1.0.0"));
        assert!(!is_newer("v1.0.0", "2.0.0"));
        assert!(!is_newer("1.0.5", "1.1.0"), "a patch of an older minor is not newer");
        assert!(!is_newer("1.2", "1.2.0"), "1.2 is the same version as 1.2.0");
        assert!(is_newer("1.2.1", "1.2"));
        assert!(!is_newer("1.2", "1.2.3"), "short version against a long one does not panic");
        assert!(!is_newer("0.9.9", "1.0.0"), "Q-REL-1: a lower major is not newer");
        assert!(!is_newer("v0.16.9", "0.17.1"), "the upstream index compare called this newer");
        assert!(!is_newer("v0.17.1-beta", "0.17.1"));
    }

    #[test]
    fn an_unparseable_side_is_never_newer() {
        assert!(!is_newer("nightly", "1.0.0"));
        assert!(!is_newer("2.0.0", "nightly"));
    }
}
