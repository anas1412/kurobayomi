//! The `manifest.json` of an anime package (`p1-rust-design.md` section 4.1, data section 8.1). Unknown keys are an error
//! here, unlike the store reader. The eleven rules run in order, and the first failure wins.

use std::collections::BTreeSet;

use serde::Deserialize;
use url::Url;

use crate::anime::abi::AbiVersion;
use crate::anime::model::LoadError;

/// The longest name the manifest may give, counted in Unicode scalar values after trimming.
const MAX_NAME_CHARS: usize = 128;
/// `pkg` is at most 255 bytes.
const MAX_PKG_BYTES: usize = 255;
const ANIYOMI_PREFIX: &str = "Aniyomi: ";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub kind: String,
    pub pkg: String,
    pub name: String,
    #[serde(rename = "versionName")]
    pub version_name: String,
    #[serde(rename = "versionCode")]
    pub version_code: i64,
    pub abi: String,
    pub lang: String,
    pub nsfw: bool,
    #[serde(default)]
    pub torrent: bool,
    pub sources: Vec<ManifestSource>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestSource {
    pub id: i64,
    pub lang: String,
    pub name: String,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
}

impl Manifest {
    /// Parses the JSON text. The caller has already applied the 64 KiB cap.
    pub fn parse(text: &[u8]) -> Result<Manifest, LoadError> {
        serde_json::from_slice(text).map_err(|err| LoadError::Manifest(format!("parse: {err}")))
    }

    /// Checks rules 1 to 11. `has_entry` tells whether the archive holds a file with that name.
    pub fn validate(&self, has_entry: &dyn Fn(&str) -> bool) -> Result<(), LoadError> {
        let bad = |rule: &str| LoadError::Manifest(format!("rule {rule} failed"));
        // 1. schema
        if self.schema != 1 {
            return Err(bad("1 schema"));
        }
        // 2. kind
        if self.kind != "anime" {
            return Err(bad("2 kind"));
        }
        // 3. pkg
        if !valid_pkg(&self.pkg) {
            return Err(bad("3 pkg"));
        }
        // 4. name
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || name.starts_with(ANIYOMI_PREFIX) {
            return Err(bad("4 name"));
        }
        // 5. versionName
        if self.version_name.is_empty() {
            return Err(LoadError::MissingVersionName(self.name.clone()));
        }
        let version_parts = semver_triplet(&self.version_name).ok_or_else(|| bad("5 versionName"))?;
        // 6. versionCode
        if self.version_code < 0 {
            return Err(bad("6 versionCode"));
        }
        // 7. abi parses and matches the versionName prefix
        let abi = AbiVersion::parse(&self.abi).ok_or_else(|| bad("7 abi"))?;
        if (abi.major, abi.minor) != (version_parts[0], version_parts[1]) {
            return Err(bad("7 abi versus versionName"));
        }
        // 8. abi is supported
        if !abi.is_supported() {
            return Err(LoadError::UnsupportedAbi(self.abi.clone()));
        }
        // 9. lang
        if !valid_lang(&self.lang) {
            return Err(bad("9 lang"));
        }
        // 10. source ids are unique and base URLs are absolute http or https
        let mut ids = BTreeSet::new();
        for source in &self.sources {
            if !ids.insert(source.id) || !absolute_http(&source.base_url) {
                return Err(bad("10 sources"));
            }
        }
        // 11. icon, when named, is a relative path that the archive holds
        if let Some(icon) = &self.icon {
            if icon.is_empty() || icon.starts_with('/') || icon.split('/').any(|part| part == "..") {
                return Err(bad("11 icon path"));
            }
            if !has_entry(icon) {
                return Err(bad("11 icon missing"));
            }
        }
        Ok(())
    }
}

/// `[a-z0-9_.]`, 1 to 255 bytes.
pub(crate) fn valid_pkg(pkg: &str) -> bool {
    !pkg.is_empty()
        && pkg.len() <= MAX_PKG_BYTES
        && pkg.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
}

/// `^\d+\.\d+\.\d+$`, returned as its three numbers.
fn semver_triplet(text: &str) -> Option<[u32; 3]> {
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let mut out = [0u32; 3];
    for (slot, part) in out.iter_mut().zip(parts) {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse().ok()?;
    }
    Some(out)
}

/// 2 to 8 ASCII letters or `-`, or `all`.
fn valid_lang(lang: &str) -> bool {
    lang == "all" || ((2..=8).contains(&lang.len()) && lang.bytes().all(|b| b.is_ascii_alphabetic() || b == b'-'))
}

fn absolute_http(url: &str) -> bool {
    Url::parse(url).is_ok_and(|parsed| matches!(parsed.scheme(), "http" | "https"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
        "schema": 1, "kind": "anime", "pkg": "eu.example.animeext.en.demo",
        "name": "Demo", "versionName": "1.0.7", "versionCode": 7, "abi": "1.0",
        "lang": "en", "nsfw": false,
        "sources": [ { "id": 1234567890123456789, "lang": "en", "name": "Demo", "baseUrl": "https://example.org" } ]
    }"#;

    fn manifest_with(from: &str, to: &str) -> Manifest {
        Manifest::parse(VALID.replace(from, to).as_bytes()).expect("parses")
    }

    fn none(_: &str) -> bool {
        false
    }

    fn rule_of(err: &LoadError) -> String {
        match err {
            LoadError::Manifest(text) => text.clone(),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn the_valid_manifest_passes_every_rule() {
        let manifest = Manifest::parse(VALID.as_bytes()).unwrap();
        assert_eq!(manifest.validate(&none), Ok(()));
    }

    #[test]
    fn an_unknown_key_is_refused() {
        let text = VALID.replace("\"nsfw\": false,", "\"nsfw\": false, \"surprise\": 1,");
        assert!(matches!(Manifest::parse(text.as_bytes()), Err(LoadError::Manifest(_))));
    }

    #[test]
    fn a_name_with_the_aniyomi_prefix_is_rejected() {
        let manifest = manifest_with("\"name\": \"Demo\"", "\"name\": \"Aniyomi: X\"");
        assert!(rule_of(&manifest.validate(&none).unwrap_err()).contains("4 name"));
    }

    #[test]
    fn an_uppercase_pkg_is_rejected() {
        let manifest = manifest_with("eu.example.animeext.en.demo", "Eu.Example");
        assert!(rule_of(&manifest.validate(&none).unwrap_err()).contains("3 pkg"));
    }

    #[test]
    fn an_empty_version_name_is_missing_not_malformed() {
        let manifest = manifest_with("\"versionName\": \"1.0.7\"", "\"versionName\": \"\"");
        assert_eq!(manifest.validate(&none), Err(LoadError::MissingVersionName("Demo".into())));
    }

    #[test]
    fn an_abi_that_disagrees_with_the_version_name_is_rejected() {
        let manifest = manifest_with("\"versionName\": \"1.0.7\"", "\"versionName\": \"2.0.1\"");
        assert!(rule_of(&manifest.validate(&none).unwrap_err()).contains("7 abi"));
    }

    #[test]
    fn an_unsupported_abi_is_its_own_error() {
        let manifest = manifest_with("\"abi\": \"1.0\"", "\"abi\": \"2.0\"");
        let manifest = Manifest { version_name: "2.0.1".into(), ..manifest };
        assert_eq!(manifest.validate(&none), Err(LoadError::UnsupportedAbi("2.0".into())));
    }

    #[test]
    fn duplicate_source_ids_and_relative_base_urls_are_rejected() {
        let duplicate = manifest_with(
            "\"sources\": [",
            "\"sources\": [ { \"id\": 1, \"lang\": \"en\", \"name\": \"a\", \"baseUrl\": \"https://a\" },
                          { \"id\": 1, \"lang\": \"en\", \"name\": \"b\", \"baseUrl\": \"https://b\" },",
        );
        assert!(rule_of(&duplicate.validate(&none).unwrap_err()).contains("10 sources"));
        let relative = manifest_with("https://example.org", "/relative");
        assert!(rule_of(&relative.validate(&none).unwrap_err()).contains("10 sources"));
    }

    #[test]
    fn an_icon_outside_the_archive_or_with_dot_dot_is_rejected() {
        let missing = manifest_with("\"nsfw\": false,", "\"nsfw\": false, \"icon\": \"icon.png\",");
        assert!(rule_of(&missing.validate(&none).unwrap_err()).contains("11 icon missing"));
        let escaping = manifest_with("\"nsfw\": false,", "\"nsfw\": false, \"icon\": \"../icon.png\",");
        assert!(rule_of(&escaping.validate(&|_| true).unwrap_err()).contains("11 icon path"));
        let present = manifest_with("\"nsfw\": false,", "\"nsfw\": false, \"icon\": \"icon.png\",");
        assert_eq!(present.validate(&|name| name == "icon.png"), Ok(()));
    }

    #[test]
    fn version_name_must_be_three_numbers() {
        assert_eq!(semver_triplet("16.3.1"), Some([16, 3, 1]));
        for bad in ["16.3", "16.3.1.1", "16.a.1", "16..1", "v1.0.0", "+1.0.0"] {
            assert_eq!(semver_triplet(bad), None, "{bad}");
        }
    }

    #[test]
    fn lang_is_all_or_two_to_eight_letters() {
        for good in ["en", "all", "zh-Hans", "pt-BR"] {
            assert!(valid_lang(good), "{good}");
        }
        for bad in ["e", "english-long", "en_US", "", "e1"] {
            assert!(!valid_lang(bad), "{bad}");
        }
    }
}
