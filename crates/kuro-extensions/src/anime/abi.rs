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
}
