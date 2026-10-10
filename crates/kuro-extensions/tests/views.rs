//! Edge cases 12.5 (T-18a-044): the Browse lists (section 6, V1 to V12). `anime::browse::buckets` builds them from the
//! extension state, the enabled languages and the NSFW setting; `anime::languages::language_list` orders the language
//! filter. Sorting is by `to_lowercase` (V3 notes the difference from Kotlin).

mod common;

use std::collections::BTreeSet;

use common::{block, entry, installed, key, listed, parse_list};
use kuro_extensions::anime::browse::buckets;
use kuro_extensions::anime::languages::language_list;
use kuro_extensions::anime::manager::ExtensionState;
use kuro_extensions::anime::model::{LoadResult, SignerDigest, Untrusted};
use kuro_extensions::anime::store::map::display_name;
use serde_json::json;

fn langs(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|l| (*l).to_owned()).collect()
}

fn state(installed_packages: Vec<LoadResult>, available: Vec<kuro_extensions::anime::model::Available>) -> ExtensionState {
    let mut state = ExtensionState::new();
    state.apply_loaded(installed_packages);
    state.apply_available(available);
    state
}

#[test]
fn installed_obsolete_first_then_name() {
    let mut names = vec![installed("p.1", "abc", 1), installed("p.2", "ABD", 1), installed("p.3", "abb", 1), installed("p.4", "zzz", 1)];
    names[3].is_obsolete = true;
    // an obsolete flag is recomputed from the listing, so list everything but p.4
    let available = vec![listed("p.1", "abc", 1), listed("p.2", "ABD", 1), listed("p.3", "abb", 1)];
    let state = state(names.into_iter().map(LoadResult::Success).collect(), available);
    let shown: Vec<_> = buckets(&state, &langs(&["en"]), true, true).installed.into_iter().map(|e| e.name).collect();
    assert_eq!(shown, ["zzz", "abb", "abc", "ABD"], "obsolete first (V1), then case-insensitive by name (V2)");
}

#[test]
fn available_split_rows_suffixed() {
    let three = entry("p.multi", "Multi", 1, &[("en", 11), ("en", 12), ("ja", 13)], Some(block(&key(7))));
    let state = state(Vec::new(), parse_list(vec![three]).unwrap());
    let rows = buckets(&state, &langs(&["en", "ja"]), true, true).available;
    let keys: Vec<_> = rows.iter().map(|r| r.row_key.as_str()).collect();
    assert_eq!(keys, ["p.multi-11", "p.multi-12", "p.multi-13"], "one row per source in an enabled language (V4)");
    let only_en = buckets(&state, &langs(&["en"]), true, true).available;
    assert_eq!(only_en.len(), 2, "a language that is off gives no row");
}

#[test]
fn hides_installed_and_untrusted_from_available() {
    let mut state = state(
        vec![
            LoadResult::Success(installed("p.have", "Have", 1)),
            LoadResult::Untrusted(Untrusted {
                name: "Dubious".into(),
                pkg: "p.dubious".into(),
                version_name: "1".into(),
                version_code: 1,
                abi: kuro_extensions::anime::abi::AbiVersion { major: 1, minor: 0 },
                nsfw: false,
                signer: SignerDigest("ab".into()),
            }),
        ],
        Vec::new(),
    );
    state.apply_available(vec![listed("p.have", "Have", 1), listed("p.dubious", "Dubious", 1), listed("p.new", "New", 1)]);
    let shown = buckets(&state, &langs(&["en"]), true, true);
    let available: Vec<_> = shown.available.iter().map(|r| r.ext.pkg.as_str()).collect();
    assert_eq!(available, ["p.new"], "installed (V6) and untrusted (V7) packages are not offered");
    assert_eq!(shown.untrusted.len(), 1);
}

#[test]
fn nsfw_filter_applies_to_available_not_untrusted() {
    let mut hot = entry("p.hot", "Hot", 1, &[("en", 1)], Some(block(&key(7))));
    hot["contentWarning"] = json!("NSFW");
    let untrusted = Untrusted {
        name: "Hot untrusted".into(),
        pkg: "p.hot2".into(),
        version_name: "1".into(),
        version_code: 1,
        abi: kuro_extensions::anime::abi::AbiVersion { major: 1, minor: 0 },
        nsfw: true,
        signer: SignerDigest("ab".into()),
    };
    let state = state(vec![LoadResult::Untrusted(untrusted)], parse_list(vec![hot]).unwrap());
    let off = buckets(&state, &langs(&["en"]), false, true);
    assert!(off.available.is_empty(), "NSFW listings are hidden while the setting is off (V8)");
    assert_eq!(off.untrusted.len(), 1, "an untrusted package is shown whatever its NSFW flag (V9)");
    assert_eq!(buckets(&state, &langs(&["en"]), true, true).available.len(), 1);
}

#[test]
fn languages_enabled_first_all_first() {
    let multi = entry("p.m", "M", 1, &[("all", 1), ("de", 2), ("en", 3), ("fr", 4)], Some(block(&key(7))));
    let available = parse_list(vec![multi]).unwrap();
    assert_eq!(language_list(&available, &langs(&["en", "fr"])), ["en", "fr", "all", "de"], "enabled first, then the rest (V10)");
    assert_eq!(language_list(&available, &langs(&["all", "fr"])), ["all", "fr", "de", "en"], "`all` leads its group");
}

#[test]
fn label_as_name_rules() {
    assert_eq!(display_name("Aniyomi: Demo"), "Demo", "the store prefix is not shown (V11)");
    assert_eq!(display_name("Demo"), "Demo");
    assert_eq!(display_name("Aniyomi:Demo"), "Aniyomi:Demo", "only the exact prefix goes");
    let one = entry("p.one", "Aniyomi: Pair", 1, &[("en", 1), ("ja", 2)], Some(block(&key(7))));
    let state = state(Vec::new(), parse_list(vec![one]).unwrap());
    let rows = buckets(&state, &langs(&["en", "ja"]), true, true).available;
    let names: Vec<_> = rows.iter().map(|r| r.source.as_ref().unwrap().name.as_str()).collect();
    assert_eq!(names, ["Aniyomi: Pair en", "Aniyomi: Pair ja"], "a split row is labelled with its source name (V12)");
}
