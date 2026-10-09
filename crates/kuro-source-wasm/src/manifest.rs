//! `manifest.json` of a `.kuroext` package and its field rules (data file section 7.3, T-18a-048).
//!
//! [`Manifest::parse`] is the only way to get a `Manifest`, so a value of the type has passed every rule.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use url::Url;

/// The only manifest schema this host reads.
pub const SCHEMA: i64 = 1;
/// Longest `pkg`, in bytes.
pub const MAX_PKG_BYTES: usize = 255;
/// Longest `name`, in Unicode scalar values.
pub const MAX_NAME_CHARS: usize = 128;
/// The prefix Aniyomi extensions carry in their name (behaviour 3.8 step 1). It is stripped before the name is kept.
const NAME_PREFIX: &str = "Tachiyomi: ";
/// The ABI versions this host accepts (`LIB_VERSION_MIN` and the top of the range).
pub const ABIS: [&str; 2] = ["1.4", "1.5"];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest.json is not valid JSON: {0}")]
    Json(String),
    #[error("unsupported manifest schema {0}")]
    UnsupportedSchema(i64),
    #[error("invalid {field}: {reason}")]
    Field { field: &'static str, reason: String },
    #[error("pkg mismatch: the index lists {index:?} but the manifest names {manifest:?}")]
    PkgMismatch { index: String, manifest: String },
}

fn bad(field: &'static str, reason: impl Into<String>) -> ManifestError {
    ManifestError::Field { field, reason: reason.into() }
}

/// One source a package declares.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSource {
    pub id: i64,
    pub lang: String,
    pub name: String,
    pub base_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    schema: i64,
    pkg: String,
    name: String,
    version_name: String,
    version_code: i64,
    abi: String,
    lang: String,
    #[serde(default)]
    nsfw: bool,
    #[serde(default)]
    factory: Option<String>,
    hosts: Vec<String>,
    #[serde(default)]
    signer_pub: Option<String>,
    #[serde(default)]
    sources: Vec<ManifestSource>,
    #[serde(default)]
    icon: Option<String>,
}

/// A manifest that passed every field rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub pkg: String,
    pub name: String,
    pub version_name: String,
    pub version_code: i64,
    pub abi: String,
    pub lang: String,
    pub nsfw: bool,
    pub factory: Option<String>,
    pub hosts: Vec<String>,
    /// The key the installer recorded after it verified the package. `None` in a package straight from a repo.
    pub signer_pub: Option<[u8; 32]>,
    pub sources: Vec<ManifestSource>,
    pub icon: Option<String>,
}

impl Manifest {
    pub fn parse(json: &[u8]) -> Result<Manifest, ManifestError> {
        let raw: Raw = serde_json::from_slice(json).map_err(|e| ManifestError::Json(e.to_string()))?;
        if raw.schema != SCHEMA {
            return Err(ManifestError::UnsupportedSchema(raw.schema));
        }
        check_pkg(&raw.pkg)?;
        let name = raw.name.trim_start();
        let name = name.strip_prefix(NAME_PREFIX).unwrap_or(name).trim().to_owned();
        let chars = name.chars().count();
        if !(1..=MAX_NAME_CHARS).contains(&chars) {
            return Err(bad("name", format!("{chars} characters, expected 1 to {MAX_NAME_CHARS}")));
        }
        check_version_name(&raw.version_name, &raw.abi)?;
        if raw.version_code < 0 {
            return Err(bad("versionCode", "must not be negative"));
        }
        let lang_len = raw.lang.chars().count();
        if !(2..=8).contains(&lang_len) {
            return Err(bad("lang", format!("{lang_len} characters, expected 2 to 8")));
        }
        if let Some(host) = raw.hosts.iter().find(|h| h.is_empty()) {
            return Err(bad("hosts", format!("empty host pattern {host:?}")));
        }
        let signer_pub = raw.signer_pub.as_deref().map(parse_signer).transpose()?;
        check_sources(&raw.sources)?;
        Ok(Manifest {
            pkg: raw.pkg,
            name,
            version_name: raw.version_name,
            version_code: raw.version_code,
            abi: raw.abi,
            lang: raw.lang,
            nsfw: raw.nsfw,
            factory: raw.factory,
            hosts: raw.hosts,
            signer_pub,
            sources: raw.sources,
            icon: raw.icon,
        })
    }

    /// Install check of edge case I12: the package must be the one the index entry listed.
    pub fn check_index_pkg(&self, index_pkg: &str) -> Result<(), ManifestError> {
        if self.pkg == index_pkg {
            Ok(())
        } else {
            Err(ManifestError::PkgMismatch { index: index_pkg.to_owned(), manifest: self.pkg.clone() })
        }
    }
}

fn check_pkg(pkg: &str) -> Result<(), ManifestError> {
    if pkg.is_empty() || pkg.len() > MAX_PKG_BYTES {
        return Err(bad("pkg", format!("{} bytes, expected 1 to {MAX_PKG_BYTES}", pkg.len())));
    }
    if !pkg.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.') {
        return Err(bad("pkg", "only a-z, 0-9, '_' and '.' are allowed"));
    }
    Ok(())
}

/// `^\d+\.\d+\.\d+$`, with `major.minor` equal to `abi`, and `abi` one of [`ABIS`].
fn check_version_name(version_name: &str, abi: &str) -> Result<(), ManifestError> {
    if !ABIS.contains(&abi) {
        return Err(bad("abi", format!("{abi:?} is not one of {ABIS:?}")));
    }
    let parts: Vec<&str> = version_name.split('.').collect();
    let digits = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    if parts.len() != 3 || !parts.iter().all(digits) {
        return Err(bad("versionName", format!("{version_name:?} is not <major>.<minor>.<patch>")));
    }
    if format!("{}.{}", parts[0], parts[1]) != abi {
        return Err(bad("versionName", format!("{version_name:?} does not start with the abi {abi}")));
    }
    Ok(())
}

fn parse_signer(text: &str) -> Result<[u8; 32], ManifestError> {
    let bytes = STANDARD.decode(text).map_err(|e| bad("signerPub", format!("not base64: {e}")))?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| bad("signerPub", format!("{} bytes, expected 32", bytes.len())))
}

fn check_sources(sources: &[ManifestSource]) -> Result<(), ManifestError> {
    let mut seen = std::collections::HashSet::new();
    for source in sources {
        if !seen.insert(source.id) {
            return Err(bad("sources", format!("duplicate source id {}", source.id)));
        }
        match Url::parse(&source.base_url) {
            Ok(url) if matches!(url.scheme(), "http" | "https") => {}
            _ => return Err(bad("sources", format!("baseUrl {:?} is not an absolute http(s) URL", source.base_url))),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    pub(crate) fn valid() -> Value {
        json!({
            "schema": 1, "pkg": "eu.example.one", "name": "Example", "versionName": "1.4.13", "versionCode": 13,
            "abi": "1.4", "lang": "en", "nsfw": false, "factory": null, "hosts": ["example.org", "*.cdn.net"],
            "sources": [{"id": 1234567890123456789i64, "lang": "en", "name": "Example", "baseUrl": "https://example.org"}],
            "icon": "icon.png"
        })
    }

    fn parse(value: &Value) -> Result<Manifest, ManifestError> {
        Manifest::parse(value.to_string().as_bytes())
    }

    fn with(field: &str, value: Value) -> Value {
        let mut manifest = valid();
        manifest[field] = value;
        manifest
    }

    fn field_error(value: &Value) -> &'static str {
        match parse(value) {
            Err(ManifestError::Field { field, .. }) => field,
            other => panic!("expected a field error, got {other:?}"),
        }
    }

    #[test]
    fn a_valid_manifest_parses_with_big_source_ids_exact() {
        let manifest = parse(&valid()).expect("valid");
        assert_eq!(manifest.version_code, 13);
        assert_eq!(manifest.sources[0].id, 1234567890123456789);
        assert_eq!(manifest.signer_pub, None);
    }

    #[test]
    fn schema_must_be_one() {
        assert_eq!(parse(&with("schema", json!(2))), Err(ManifestError::UnsupportedSchema(2)));
        assert_eq!(parse(&with("schema", json!(0))), Err(ManifestError::UnsupportedSchema(0)));
    }

    #[test]
    fn pkg_pattern_and_length_boundaries() {
        for ok in ["a", &"a".repeat(255), "a.b_c.9"] {
            assert!(parse(&with("pkg", json!(ok))).is_ok(), "{ok}");
        }
        for bad in ["", &"a".repeat(256), "A.b", "a-b", "a/b", "é"] {
            assert_eq!(field_error(&with("pkg", json!(bad))), "pkg", "{bad}");
        }
    }

    #[test]
    fn name_counts_scalars_trims_and_drops_the_aniyomi_prefix() {
        assert!(parse(&with("name", json!("é".repeat(128)))).is_ok());
        assert_eq!(field_error(&with("name", json!("é".repeat(129)))), "name");
        assert_eq!(field_error(&with("name", json!("   "))), "name");
        assert_eq!(parse(&with("name", json!("  Tachiyomi: Foo "))).unwrap().name, "Foo");
        assert_eq!(field_error(&with("name", json!("Tachiyomi: "))), "name");
    }

    #[test]
    fn version_name_must_be_three_numbers_that_match_the_abi() {
        for bad in ["1.4", "1.4.x", "1.4.1.2", "1.4.", "", "1.5.0", "01.4.1x"] {
            assert_eq!(field_error(&with("versionName", json!(bad))), "versionName", "{bad}");
        }
        assert!(parse(&with("versionName", json!("1.4.0"))).is_ok());
        let mut manifest = with("abi", json!("1.5"));
        manifest["versionName"] = json!("1.5.2");
        assert!(parse(&manifest).is_ok());
    }

    #[test]
    fn version_code_range_is_zero_to_i64_max() {
        assert!(parse(&with("versionCode", json!(0))).is_ok());
        assert_eq!(parse(&with("versionCode", json!(i64::MAX))).unwrap().version_code, i64::MAX);
        assert_eq!(field_error(&with("versionCode", json!(-1))), "versionCode");
        let too_big = format!("{}", (i64::MAX as u64) + 1);
        let text = valid().to_string().replace("\"versionCode\":13", &format!("\"versionCode\":{too_big}"));
        assert!(matches!(Manifest::parse(text.as_bytes()), Err(ManifestError::Json(_))));
    }

    #[test]
    fn abi_is_one_four_or_one_five_only() {
        for bad in ["1.3", "1.6", "2.4", "1", ""] {
            assert_eq!(field_error(&with("abi", json!(bad))), "abi", "{bad}");
        }
    }

    #[test]
    fn lang_is_two_to_eight_characters() {
        for ok in ["en", "all", "zh-Hans-"] {
            assert!(parse(&with("lang", json!(ok))).is_ok(), "{ok}");
        }
        for bad in ["e", "123456789", ""] {
            assert_eq!(field_error(&with("lang", json!(bad))), "lang", "{bad}");
        }
    }

    #[test]
    fn hosts_is_required_and_may_be_empty() {
        let mut manifest = valid();
        manifest.as_object_mut().unwrap().remove("hosts");
        assert!(matches!(parse(&manifest), Err(ManifestError::Json(_))));
        assert_eq!(parse(&with("hosts", json!([]))).unwrap().hosts.len(), 0);
        assert_eq!(field_error(&with("hosts", json!([""]))), "hosts");
        assert!(matches!(parse(&with("hosts", json!("example.org"))), Err(ManifestError::Json(_))));
    }

    #[test]
    fn signer_pub_must_be_exactly_32_bytes_when_present() {
        let key = |n: usize| json!(STANDARD.encode(vec![7u8; n]));
        assert_eq!(parse(&with("signerPub", key(32))).unwrap().signer_pub, Some([7u8; 32]));
        assert_eq!(parse(&with("signerPub", json!(null))).unwrap().signer_pub, None);
        for n in [31, 33, 0] {
            assert_eq!(field_error(&with("signerPub", key(n))), "signerPub", "{n}");
        }
        assert_eq!(field_error(&with("signerPub", json!("!!!"))), "signerPub");
    }

    #[test]
    fn sources_need_unique_ids_and_absolute_http_urls() {
        let one = |id: i64, url: &str| json!({"id": id, "lang": "en", "name": "S", "baseUrl": url});
        assert!(parse(&with("sources", json!([]))).is_ok());
        assert_eq!(field_error(&with("sources", json!([one(1, "https://a.b"), one(1, "http://c.d")]))), "sources");
        assert_eq!(field_error(&with("sources", json!([one(1, "/relative")]))), "sources");
        assert_eq!(field_error(&with("sources", json!([one(1, "ftp://a.b")]))), "sources");
    }

    #[test]
    fn a_pkg_that_differs_from_the_index_entry_is_refused() {
        let manifest = parse(&valid()).unwrap();
        assert_eq!(manifest.check_index_pkg("eu.example.one"), Ok(()));
        assert!(matches!(manifest.check_index_pkg("eu.example.two"), Err(ManifestError::PkgMismatch { .. })));
    }
}
