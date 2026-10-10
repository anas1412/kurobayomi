//! Edge cases 12.1 (T-18a-044): the library version gate. The Android rule (`versionName` prefix before the last dot,
//! range 1.4 to 1.5) has no desktop twin. A `.kuroext` states its ABI as `<major>.<minor>` and the host loads a fixed
//! set, and the only place a `versionName` prefix still decides anything is a legacy store listing (the Aniyomi
//! `extensionLib` label of an APK-only row). The 12.1 cases are tested on those two rules.
//!
//! | 12.1 case | here |
//! |---|---|
//! | `parses_prefix_before_last_dot` | `a_legacy_listing_takes_the_prefix_before_the_last_dot` (store listing) and `the_abi_is_exactly_major_dot_minor` |
//! | `rejects_missing_patch` | `a_legacy_listing_without_a_numeric_prefix_fails_the_store`, `the_abi_is_exactly_major_dot_minor` |
//! | `range_inclusive_bounds` | `the_supported_abi_set_has_no_range` (a set of one, so 1.0 in and its neighbours out) |
//! | `abi` of `1.4.13` | refused: a third part is not an ABI |

mod common;

use kuro_extensions::anime::abi::{ANIYOMI_LIBS, AbiVersion, SUPPORTED_ABI};
use kuro_extensions::anime::store::map::legacy_available;
use kuro_extensions::anime::store::wire::LegacyExtJson;
use serde_json::json;

fn legacy(version: &str) -> LegacyExtJson {
    serde_json::from_value(json!({
        "name": "Aniyomi: Demo", "pkg": "eu.example.legacy", "apk": "demo.apk", "lang": "en",
        "code": 3, "version": version, "nsfw": 0,
    }))
    .unwrap()
}

#[test]
fn the_abi_is_exactly_major_dot_minor() {
    assert_eq!(AbiVersion::parse("1.0"), Some(AbiVersion { major: 1, minor: 0 }));
    assert_eq!(AbiVersion::parse("1.4"), Some(AbiVersion { major: 1, minor: 4 }));
    for bad in ["1.4.13", "1", "1.", ".4", "v1.4", "1.4 ", ""] {
        assert_eq!(AbiVersion::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn the_supported_abi_set_has_no_range() {
    let supported = |text: &str| AbiVersion::parse(text).is_some_and(AbiVersion::is_supported);
    assert_eq!(SUPPORTED_ABI, &[AbiVersion { major: 1, minor: 0 }]);
    assert!(supported("1.0"));
    for outside in ["0.9", "1.1", "1.10", "2.0"] {
        assert!(!supported(outside), "{outside}");
    }
    assert!(AbiVersion::parse("1.10") > AbiVersion::parse("1.1"), "minor versions compare as numbers");
}

#[test]
fn a_legacy_listing_takes_the_prefix_before_the_last_dot() {
    let store = common::store_ref();
    let lib = |version: &str| legacy_available(&legacy(version), &store, "https://example.org").unwrap().aniyomi_lib;
    assert_eq!(lib("16.3"), Some(16.0));
    assert_eq!(lib("16.3.1"), Some(16.3));
    assert_eq!(lib("14"), Some(14.0), "no dot: the whole text is the lib");
    assert!(ANIYOMI_LIBS.contains(&16.0));
}

#[test]
fn a_legacy_listing_without_a_numeric_prefix_fails_the_store() {
    let store = common::store_ref();
    assert!(legacy_available(&legacy("abc.1"), &store, "https://example.org").is_err());
}
