//! Store entries to domain rows (T-19a-017 and T-19a-012, `p1-rust-design.md` section 3.3 and 3.5). Every entry is kept:
//! a bad `kuroext` block lowers the entry's kind, it never drops the entry.

use std::collections::BTreeSet;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use url::Url;

use super::StoreError;
use super::wire::{ExtensionJson, KuroextJson, LegacyExtJson, RepoJson, StoreJson};
use crate::anime::model::{AnimeSourceRef, Available, Kind, KuroextRef, StoreRef, StoreRow};

const ANIYOMI_PREFIX: &str = "Aniyomi: ";

/// The display name: `Aniyomi: ` is stripped, the rest is kept.
pub fn display_name(raw: &str) -> &str {
    raw.strip_prefix(ANIYOMI_PREFIX).unwrap_or(raw)
}

/// The row to save for a fetched store document. The name keeps its prefix here.
pub fn store_row(doc: &StoreJson, index_url: &str) -> StoreRow {
    StoreRow {
        index_url: index_url.to_owned(),
        name: doc.name.clone(),
        badge_label: doc.badge_label.clone(),
        signing_key: doc.signing_key.clone(),
        contact_website: doc.contact.website.clone(),
        contact_discord: doc.contact.discord.clone(),
        is_legacy: false,
        extension_list_url: doc.extension_list_url.clone(),
    }
}

/// The row to save for a legacy `repo.json` (`NetworkLegacyAnimeExtensionRepo.toExtensionStore`). The badge falls back
/// to the name, the signing key is the fingerprint, and the store has no list URL of its own.
pub fn legacy_row(repo: &RepoJson, index_url: &str) -> StoreRow {
    let meta = &repo.meta;
    StoreRow {
        index_url: index_url.to_owned(),
        name: meta.name.clone(),
        badge_label: meta.short_name.clone().unwrap_or_else(|| meta.name.clone()),
        signing_key: meta.signing_key_fingerprint.clone(),
        contact_website: meta.website.clone(),
        contact_discord: None,
        is_legacy: true,
        extension_list_url: None,
    }
}

/// One language when the languages are all the same, otherwise `all`. No languages also gives `all`.
pub fn derive_lang<'a>(langs: impl IntoIterator<Item = &'a str>) -> String {
    let distinct: BTreeSet<&str> = langs.into_iter().collect();
    if distinct.len() == 1 {
        if let Some(lang) = distinct.into_iter().next() {
            return lang.to_owned();
        }
    }
    "all".to_owned()
}

/// Maps one listed extension. Fails only when `extensionLib` is not a number, which fails the whole store (WIRE:110).
pub fn available(ext: &ExtensionJson, store: &StoreRef) -> Result<Available, StoreError> {
    let aniyomi_lib = ext
        .extension_lib
        .parse::<f64>()
        .map_err(|_| StoreError::Decode(format!("extensionLib {:?} is not a number", ext.extension_lib)))?;
    let sources: Vec<AnimeSourceRef> = ext
        .sources
        .iter()
        .map(|source| AnimeSourceRef {
            id: source.id,
            name: source.name.clone(),
            lang: source.language.clone(),
            base_url: source.home_url.clone(),
        })
        .collect();
    let (kind, kuroext) = classify(&ext.package_name, ext.resources.kuroext.as_ref(), &store.index_url);
    Ok(Available {
        name: display_name(&ext.name).to_owned(),
        raw_name: ext.name.clone(),
        pkg: ext.package_name.clone(),
        version_name: ext.version_name.clone(),
        version_code: ext.version_code,
        abi: None,
        aniyomi_lib: (kind == Kind::ApkOnly).then_some(aniyomi_lib),
        lang: derive_lang(sources.iter().map(|source| source.lang.as_str())),
        nsfw: ext.content_warning.is_nsfw(),
        torrent: ext.is_torrent,
        sources,
        kind,
        apk_url: ext.resources.apk_url.clone(),
        icon_url: ext.resources.icon_url.clone(),
        kuroext,
        store: store.clone(),
    })
}

/// Maps one legacy list entry (`NetworkLegacyAnimeExtension.toAvailableExtension`). `base` is the store URL without
/// `/repo.json`. The library version is the part of `version` before its last dot, and a number that does not parse
/// fails the whole store, as `toDouble()` does. A legacy entry has no signed package, so it is always `ApkOnly`.
pub fn legacy_available(ext: &LegacyExtJson, store: &StoreRef, base: &str) -> Result<Available, StoreError> {
    let lib = ext.version.rsplit_once('.').map_or(ext.version.as_str(), |(head, _)| head);
    let aniyomi_lib = lib
        .parse::<f64>()
        .map_err(|_| StoreError::Decode(format!("version {:?} has no numeric library part", ext.version)))?;
    let sources = match ext.sources.as_deref() {
        Some(list) if !list.is_empty() => list
            .iter()
            .map(|source| AnimeSourceRef {
                id: source.id,
                name: source.name.clone(),
                lang: source.lang.clone(),
                base_url: source.base_url.clone(),
            })
            .collect(),
        _ => vec![AnimeSourceRef { id: 0, name: ext.name.clone(), lang: ext.lang.clone(), base_url: String::new() }],
    };
    Ok(Available {
        name: display_name(&ext.name).to_owned(),
        raw_name: ext.name.clone(),
        pkg: ext.pkg.clone(),
        version_name: ext.version.clone(),
        version_code: ext.code,
        abi: None,
        aniyomi_lib: Some(aniyomi_lib),
        lang: ext.lang.clone(),
        nsfw: ext.nsfw == 1,
        torrent: false,
        sources,
        kind: Kind::ApkOnly,
        apk_url: format!("{base}/apk/{}", ext.apk),
        icon_url: format!("{base}/icon/{}.png", ext.pkg),
        kuroext: None,
        store: store.clone(),
    })
}

/// The kind of an entry and its validated block. No block is `ApkOnly`. A malformed `sha256` or `url` is `ApkOnly`.
/// A missing or malformed `ed25519` or `publicKey` is `Unsigned`.
fn classify(pkg: &str, block: Option<&KuroextJson>, index_url: &str) -> (Kind, Option<KuroextRef>) {
    let Some(block) = block else {
        return (Kind::ApkOnly, None);
    };
    let Some(sha256) = block.sha256.as_deref().and_then(parse_sha256) else {
        tracing::warn!(pkg, "kuroext sha256 is malformed; the entry is listed as not runnable");
        return (Kind::ApkOnly, None);
    };
    let Some(url) = block.url.as_deref().and_then(|raw| resolve(raw, index_url)) else {
        tracing::warn!(pkg, "kuroext url is malformed; the entry is listed as not runnable");
        return (Kind::ApkOnly, None);
    };
    let ed25519 = block.ed25519.as_deref().and_then(|text| decode_exact(text, 64));
    let public_key = block.public_key.as_deref().and_then(|text| decode_exact(text, 32));
    match (ed25519, public_key) {
        (Some(ed25519), Some(public_key)) => (Kind::Wasm, Some(KuroextRef { url, sha256, ed25519, public_key })),
        _ => (Kind::Unsigned, None),
    }
}

/// Exactly 64 lowercase hex characters.
fn parse_sha256(text: &str) -> Option<[u8; 32]> {
    let valid = text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !valid {
        return None;
    }
    let mut out = [0u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&text[2 * index..2 * index + 2], 16).ok()?;
    }
    Some(out)
}

/// An absolute http or https URL, or a relative one resolved against the index URL.
fn resolve(raw: &str, index_url: &str) -> Option<String> {
    let parsed = match Url::parse(raw) {
        Ok(url) => url,
        Err(_) => Url::parse(index_url).ok()?.join(raw).ok()?,
    };
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

/// Standard base64 with padding, decoded to exactly `len` bytes.
fn decode_exact(text: &str, len: usize) -> Option<Vec<u8>> {
    BASE64_STANDARD.decode(text).ok().filter(|bytes| bytes.len() == len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::store::wire::ExtensionListJson;
    use crate::anime::testkit::store_extension;
    use serde_json::{Value, json};

    const INDEX: &str = "https://example.org/store/index.json";

    fn store() -> StoreRef {
        StoreRef { index_url: INDEX.into(), name: "Example".into() }
    }

    fn parse_one(entry: Value) -> Result<Available, StoreError> {
        let list: ExtensionListJson = serde_json::from_value(json!({ "extensions": [entry] })).expect("decodes");
        available(&list.extensions[0], &store())
    }

    fn valid_block() -> Value {
        json!({
            "url": "https://example.org/kuroext/demo.kuroext",
            "sha256": "ab".repeat(32),
            "ed25519": BASE64_STANDARD.encode([3u8; 64]),
            "publicKey": BASE64_STANDARD.encode([4u8; 32]),
        })
    }

    fn with_block(patch: Value) -> Value {
        let mut block = valid_block();
        for (key, value) in patch.as_object().expect("object") {
            block[key] = value.clone();
        }
        block
    }

    #[test]
    fn a_safe_entry_is_not_nsfw_and_a_mixed_one_is() {
        let safe = parse_one(store_extension("p.safe", 1, None)).unwrap();
        assert!(!safe.nsfw);
        let mut mixed = store_extension("p.mixed", 1, None);
        mixed["contentWarning"] = json!("CONTENT_WARNING_MIXED");
        assert!(parse_one(mixed).unwrap().nsfw);
        let mut nsfw = store_extension("p.nsfw", 1, None);
        nsfw["contentWarning"] = json!("NSFW");
        assert!(parse_one(nsfw).unwrap().nsfw);
    }

    #[test]
    fn an_unknown_content_warning_fails_the_whole_document() {
        let mut entry = store_extension("p.bad", 1, None);
        entry["contentWarning"] = json!("FOO");
        assert!(serde_json::from_value::<ExtensionListJson>(json!({ "extensions": [entry] })).is_err());
    }

    #[test]
    fn a_non_numeric_extension_lib_fails_the_store() {
        let mut entry = store_extension("p.lib", 1, None);
        entry["extensionLib"] = json!("abc");
        assert!(matches!(parse_one(entry), Err(StoreError::Decode(_))));
    }

    #[test]
    fn the_language_is_the_one_shared_language_or_all() {
        assert_eq!(derive_lang(["en"]), "en");
        assert_eq!(derive_lang(["en", "en"]), "en");
        assert_eq!(derive_lang(["en", "ja"]), "all");
        assert_eq!(derive_lang(std::iter::empty()), "all", "zero sources give all");
    }

    #[test]
    fn a_zero_source_entry_is_all() {
        let mut entry = store_extension("p.none", 1, None);
        entry["sources"] = json!([]);
        assert_eq!(parse_one(entry).unwrap().lang, "all");
    }

    #[test]
    fn the_display_name_strips_the_prefix_and_the_raw_name_keeps_it() {
        let listed = parse_one(store_extension("p.name", 1, None)).unwrap();
        assert_eq!(listed.name, "Demo");
        assert_eq!(listed.raw_name, "Aniyomi: Demo");
        assert_eq!(display_name("Plain"), "Plain");
    }

    #[test]
    fn a_source_id_round_trips_exactly_and_a_missing_home_url_is_empty() {
        let mut entry = store_extension("p.id", 1, None);
        entry["sources"] = json!([{ "id": 9_007_199_254_740_993_i64, "name": "S", "language": "en" }]);
        let listed = parse_one(entry).unwrap();
        assert_eq!(listed.sources[0].id, 9_007_199_254_740_993);
        assert_eq!(listed.sources[0].base_url, "");
    }

    #[test]
    fn a_valid_block_makes_a_wasm_entry_with_its_url_resolved() {
        let listed = parse_one(store_extension("p.ok", 3, Some(valid_block()))).unwrap();
        assert_eq!(listed.kind, Kind::Wasm);
        let kuroext = listed.kuroext.expect("block");
        assert_eq!(kuroext.url, "https://example.org/kuroext/demo.kuroext");
        assert_eq!(kuroext.sha256, [0xab; 32]);
        assert_eq!(kuroext.ed25519.len(), 64);
        assert_eq!(kuroext.public_key.len(), 32);
        assert_eq!(listed.aniyomi_lib, None);
    }

    #[test]
    fn a_relative_url_resolves_against_the_index() {
        let listed =
            parse_one(store_extension("p.rel", 3, Some(with_block(json!({ "url": "../kuroext/x.kuroext" }))))).unwrap();
        assert_eq!(listed.kuroext.unwrap().url, "https://example.org/kuroext/x.kuroext");
    }

    #[test]
    fn a_malformed_sha256_or_url_is_apk_only() {
        let short_sha = with_block(json!({ "sha256": "abc" }));
        let listed = parse_one(store_extension("p.sha", 3, Some(short_sha))).unwrap();
        assert_eq!(listed.kind, Kind::ApkOnly);
        assert_eq!(listed.aniyomi_lib, Some(16.0));
        let bad_url = with_block(json!({ "url": "ftp://example.org/x" }));
        assert_eq!(parse_one(store_extension("p.url", 3, Some(bad_url))).unwrap().kind, Kind::ApkOnly);
    }

    #[test]
    fn a_missing_or_short_signature_or_key_is_unsigned_and_not_installable() {
        let no_signature = with_block(json!({ "ed25519": null }));
        let unsigned = parse_one(store_extension("p.sig", 3, Some(no_signature))).unwrap();
        assert_eq!(unsigned.kind, Kind::Unsigned);
        assert!(unsigned.kuroext.is_none());
        let short_key = with_block(json!({ "publicKey": BASE64_STANDARD.encode([4u8; 31]) }));
        assert_eq!(parse_one(store_extension("p.key", 3, Some(short_key))).unwrap().kind, Kind::Unsigned);
    }

    #[test]
    fn an_entry_without_a_block_is_apk_only_with_its_lib() {
        let listed = parse_one(store_extension("p.apk", 3, None)).unwrap();
        assert_eq!(listed.kind, Kind::ApkOnly);
        assert_eq!(listed.aniyomi_lib, Some(16.0));
    }

    #[test]
    fn sha256_must_be_lowercase_hex() {
        assert!(parse_sha256(&"AB".repeat(32)).is_none());
        assert!(parse_sha256(&"ab".repeat(31)).is_none());
        assert_eq!(parse_sha256(&"0f".repeat(32)), Some([0x0f; 32]));
    }

    fn legacy(overrides: Value) -> LegacyExtJson {
        let mut entry = json!({
            "name": "Aniyomi: Legacy", "pkg": "eu.example.legacy", "apk": "legacy-v16.3.apk", "lang": "en",
            "code": 3, "version": "16.3", "nsfw": 0,
        });
        for (key, value) in overrides.as_object().expect("object") {
            entry[key] = value.clone();
        }
        serde_json::from_value(entry).expect("decodes")
    }

    #[test]
    fn a_legacy_entry_maps_its_urls_library_version_and_flags() {
        let listed = legacy_available(&legacy(json!({ "nsfw": 1 })), &store(), "https://example.org/repo").unwrap();
        assert_eq!(listed.apk_url, "https://example.org/repo/apk/legacy-v16.3.apk");
        assert_eq!(listed.icon_url, "https://example.org/repo/icon/eu.example.legacy.png");
        assert_eq!((listed.aniyomi_lib, listed.version_code, listed.version_name.as_str()), (Some(16.0), 3, "16.3"));
        assert_eq!((listed.nsfw, listed.torrent, listed.kind), (true, false, Kind::ApkOnly));
        assert_eq!(listed.lang, "en", "the language is taken as written, not derived");
        assert_eq!(listed.name, "Legacy");
        assert_eq!(listed.sources.len(), 1, "no sources gives one synthetic source");
        assert_eq!((listed.sources[0].id, listed.sources[0].base_url.as_str()), (0, ""));
    }

    #[test]
    fn only_a_legacy_nsfw_of_exactly_one_is_nsfw() {
        for (value, expected) in [(0, false), (1, true), (2, false)] {
            let listed = legacy_available(&legacy(json!({ "nsfw": value })), &store(), "b").unwrap();
            assert_eq!(listed.nsfw, expected, "nsfw {value}");
        }
    }

    #[test]
    fn a_legacy_version_without_a_dot_uses_the_whole_string_and_a_bad_one_fails() {
        let whole = legacy_available(&legacy(json!({ "version": "15" })), &store(), "b").unwrap();
        assert_eq!(whole.aniyomi_lib, Some(15.0));
        let bad = legacy_available(&legacy(json!({ "version": "x.1" })), &store(), "b");
        assert!(matches!(bad, Err(StoreError::Decode(_))));
    }

    #[test]
    fn a_legacy_repo_row_uses_the_fingerprint_and_falls_back_to_the_name_for_the_badge() {
        let repo: RepoJson = serde_json::from_value(json!({
            "meta": { "name": "N", "shortName": "S", "website": "w", "signingKeyFingerprint": "f" }
        }))
        .unwrap();
        let row = legacy_row(&repo, "https://x/repo.json");
        assert_eq!((row.badge_label.as_str(), row.signing_key.as_str(), row.is_legacy), ("S", "f", true));
        assert_eq!((row.extension_list_url, row.contact_discord), (None, None));
    }
}
