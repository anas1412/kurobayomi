//! Edge cases 12.3 (T-18a-044): store index parsing and listing rules (section 3, X1 to X8, X13, X23, and the kind
//! rule of 8.1). The shape differs from the manga plan: the store document of area 19 is decoded by serde and mapped by
//! `anime::store::map`, a required field that is missing fails the whole document (as Aniyomi does, so X2 to X4 are not
//! the plan's "entry dropped"), and a bad `kuroext` block lowers the entry's kind instead of dropping it.
//!
//! | 12.3 case | here |
//! |---|---|
//! | X1 root is not an object | `a_document_that_is_not_an_object_does_not_decode` |
//! | X2 entry without `pkg` | `a_required_field_missing_fails_the_whole_document` (whole document, not the entry) |
//! | X3 `code` as a string | `a_wrong_typed_field_fails_the_whole_document` |
//! | X4, X5 non-numeric lib | `a_non_numeric_extension_lib_fails_the_store` (the library version is a string field here) |
//! | X6 `sources` absent | `a_missing_sources_list_fails_the_document_but_an_empty_one_is_all` |
//! | X7 `nsfw` | `only_mixed_and_nsfw_warnings_are_nsfw` (a warning enum, not an integer) |
//! | X8 unknown keys | `unknown_keys_are_ignored` |
//! | X13 same pkg twice | `duplicate_pkg_first_listing_wins` |
//! | X23 unsigned entry | `classifies_apk_only_and_wasm` |
//! | `classifies_apk_only_and_wasm` | same name |

mod common;

use common::{block, entry, key, listed, parse_list, signer_hex, store_ref};
use kuro_extensions::anime::browse::buckets;
use kuro_extensions::anime::manager::ExtensionState;
use kuro_extensions::anime::model::{Kind, LoadResult};
use kuro_extensions::anime::store::wire::ExtensionListJson;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn decode(value: Value) -> Result<ExtensionListJson, serde_json::Error> {
    serde_json::from_value(value)
}

#[test]
fn a_document_that_is_not_an_object_does_not_decode() {
    assert!(decode(json!([entry("p.a", "A", 1, &[("en", 1)], None)])).is_err());
    assert!(decode(json!("index")).is_err());
}

#[test]
fn a_required_field_missing_fails_the_whole_document() {
    let mut missing = entry("p.a", "A", 1, &[("en", 1)], None);
    missing.as_object_mut().unwrap().remove("packageName");
    let good = entry("p.b", "B", 1, &[("en", 1)], None);
    assert!(decode(json!({ "extensions": [good, missing] })).is_err(), "the good entry goes down with it");
}

#[test]
fn a_wrong_typed_field_fails_the_whole_document() {
    let mut wrong = entry("p.a", "A", 1, &[("en", 1)], None);
    wrong["versionCode"] = json!("13");
    assert!(decode(json!({ "extensions": [wrong] })).is_err());
}

#[test]
fn a_non_numeric_extension_lib_fails_the_store() {
    for lib in ["abc", "abc.1"] {
        let mut entry = entry("p.a", "A", 1, &[("en", 1)], None);
        entry["extensionLib"] = json!(lib);
        assert!(parse_list(vec![entry]).is_err(), "{lib}");
    }
}

#[test]
fn a_missing_sources_list_fails_the_document_but_an_empty_one_is_all() {
    let mut absent = entry("p.a", "A", 1, &[], None);
    absent.as_object_mut().unwrap().remove("sources");
    assert!(decode(json!({ "extensions": [absent] })).is_err());
    let listed = parse_list(vec![entry("p.a", "A", 1, &[], None)]).unwrap();
    assert!(listed[0].sources.is_empty());
    assert_eq!(listed[0].lang, "all");
}

#[test]
fn only_mixed_and_nsfw_warnings_are_nsfw() {
    let with = |warning: &str| {
        let mut e = entry("p.a", "A", 1, &[("en", 1)], None);
        e["contentWarning"] = json!(warning);
        parse_list(vec![e]).unwrap().remove(0).nsfw
    };
    assert!(!with("SAFE") && !with("UNSPECIFIED"));
    assert!(with("MIXED") && with("NSFW") && with("CONTENT_WARNING_NSFW"));
}

#[test]
fn unknown_keys_are_ignored() {
    let mut e = entry("p.a", "A", 1, &[("en", 1)], None);
    e["somethingNew"] = json!({ "nested": true });
    e["resources"]["alsoNew"] = json!(1);
    assert_eq!(parse_list(vec![e]).unwrap()[0].pkg, "p.a");
}

#[test]
fn duplicate_pkg_first_listing_wins() {
    let first = entry("p.dup", "First", 1, &[("en", 1)], Some(block(&key(7))));
    let second = entry("p.dup", "Second", 9, &[("en", 1)], Some(block(&key(7))));
    let mut state = ExtensionState::new();
    state.apply_loaded(Vec::<LoadResult>::new());
    state.apply_available(parse_list(vec![first, second]).unwrap());
    let rows = buckets(&state, &BTreeSet::from(["en".to_owned()]), true, true).available;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].ext.name, "First");
}

#[test]
fn classifies_apk_only_and_wasm() {
    let signing = key(7);
    let mut bad_sha = block(&signing);
    bad_sha["sha256"] = json!("not hex");
    let mut no_signature = block(&signing);
    no_signature.as_object_mut().unwrap().remove("ed25519");
    let mut short_key = block(&signing);
    short_key["publicKey"] = json!("AAAA");
    let kinds: Vec<Kind> = parse_list(vec![
        entry("p.apk", "Apk", 1, &[("en", 1)], None),
        entry("p.wasm", "Wasm", 1, &[("en", 1)], Some(block(&signing))),
        entry("p.sha", "Sha", 1, &[("en", 1)], Some(bad_sha)),
        entry("p.unsigned", "Unsigned", 1, &[("en", 1)], Some(no_signature)),
        entry("p.key", "Key", 1, &[("en", 1)], Some(short_key)),
    ])
    .unwrap()
    .iter()
    .map(|a| a.kind)
    .collect();
    assert_eq!(kinds, [Kind::ApkOnly, Kind::Wasm, Kind::ApkOnly, Kind::Unsigned, Kind::Unsigned]);
    let wasm = listed("p.w", "W", 1);
    assert_eq!(wasm.kind, Kind::Wasm);
    assert_eq!(wasm.kuroext.unwrap().url, "https://example.org/demo.kuroext");
    assert_eq!(wasm.store, store_ref());
    assert_eq!(signer_hex(&signing).len(), 64);
}
