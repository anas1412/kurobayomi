//! The ABI version of an anime package (T-19a-002). `AbiVersion` compares as a tuple, so `1.10` is newer than `1.1`.

use std::fmt;

/// `<major>.<minor>`, the only shape a package may declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbiVersion {
    pub major: u32,
    pub minor: u32,
}

/// The ABI set this host can load. A change here is a fixture change too (see the test below).
pub const SUPPORTED_ABI: &[AbiVersion] = &[AbiVersion { major: 1, minor: 0 }];

/// Aniyomi `extensionLib` values. They label `ApkOnly` listings and have no load path on Linux.
pub const ANIYOMI_LIBS: &[f64] = &[14.0, 16.0, 17.0];

impl AbiVersion {
    /// Accepts exactly `<digits>.<digits>`: one dot, ASCII digits on both sides, no sign and no spaces.
    pub fn parse(text: &str) -> Option<AbiVersion> {
        let (major, minor) = text.split_once('.')?;
        Some(AbiVersion { major: digits(major)?, minor: digits(minor)? })
    }

    /// True when this version is in [`SUPPORTED_ABI`].
    pub fn is_supported(self) -> bool {
        SUPPORTED_ABI.contains(&self)
    }
}

/// The loader's rule for an Aniyomi lib (`AnimeExtensionLoader`): the integer metadata wins, else the `versionName`
/// prefix before its last dot, parsed as a number. Used by the tests only (TST-19-060). The store path reads
/// `extensionLib` instead.
#[cfg(test)]
pub(crate) fn aniyomi_lib_from_metadata(metadata_lib: Option<i64>, version_name: &str) -> Option<f64> {
    match metadata_lib {
        Some(lib) => Some(lib as f64),
        None => version_name.rsplit_once('.').map_or(version_name, |(head, _)| head).parse::<f64>().ok(),
    }
}

fn digits(part: &str) -> Option<u32> {
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

impl fmt::Display for AbiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_major_dot_minor_of_digits_is_accepted() {
        assert_eq!(AbiVersion::parse("1.0"), Some(AbiVersion { major: 1, minor: 0 }));
        for bad in ["1", "1.2.3", "-1.0", " 1.0", "1.0 ", "+1.0", "a.b", "", ".", "1.", ".1"] {
            assert_eq!(AbiVersion::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn minor_versions_compare_as_numbers() {
        let ten = AbiVersion::parse("1.10").unwrap();
        let one = AbiVersion::parse("1.1").unwrap();
        assert!(ten > one, "1.10 is newer than 1.1");
    }

    #[test]
    fn the_supported_set_is_pinned() {
        assert_eq!(SUPPORTED_ABI, &[AbiVersion { major: 1, minor: 0 }]);
        assert!(AbiVersion { major: 1, minor: 0 }.is_supported());
        assert!(!AbiVersion { major: 2, minor: 0 }.is_supported());
    }

    #[test]
    fn display_is_major_dot_minor() {
        assert_eq!(AbiVersion::parse("1.4").unwrap().to_string(), "1.4");
    }

    #[test]
    fn tst_19_060_the_metadata_integer_wins_over_the_version_name() {
        assert_eq!(aniyomi_lib_from_metadata(Some(16), "15.2"), Some(16.0));
    }

    #[test]
    fn tst_19_060_without_metadata_the_version_prefix_before_the_last_dot_is_the_lib() {
        assert_eq!(aniyomi_lib_from_metadata(None, "16.3"), Some(16.0));
        assert_eq!(aniyomi_lib_from_metadata(None, "16"), Some(16.0));
        assert_eq!(aniyomi_lib_from_metadata(None, "16.3.1"), Some(16.3));
        assert_eq!(aniyomi_lib_from_metadata(None, "abc.1"), None);
    }
}
