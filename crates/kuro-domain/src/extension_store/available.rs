//! The extension list of an anime store and its mapping to available extensions (T-06a-052,
//! `toAvailableExtensions` and `NetworkLegacyAnimeExtension.toAvailableExtension`). Only the mapping is here: the
//! GET of a list and the choice of its URL belong to the store flow in `fetch`.

use std::ops::RangeInclusive;

use serde::Deserialize;

use crate::error::DomainError;

/// The lib versions a repo list may carry: `LIB_VERSION_MIN` to `LIB_VERSION_MAX` of the Kotlin manga index filter
/// (`MangaExtensionApi.toExtensions`). Other entries are dropped when the list is read.
pub const SUPPORTED_LIB_VERSIONS: RangeInclusive<f64> = 1.4..=1.5;

/// `ContentWarning` of a current-format extension. Ordered: `Mixed` and `Nsfw` count as adult content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentWarning {
    Unspecified,
    Safe,
    Mixed,
    Nsfw,
}

impl ContentWarning {
    /// Accepts the enum constant name or its alternate `CONTENT_WARNING_*` name (`NES:59-75`).
    pub fn from_name(name: &str) -> Result<Self, DomainError> {
        match name {
            "UNSPECIFIED" | "CONTENT_WARNING_UNSPECIFIED" => Ok(Self::Unspecified),
            "SAFE" | "CONTENT_WARNING_SAFE" => Ok(Self::Safe),
            "MIXED" | "CONTENT_WARNING_MIXED" => Ok(Self::Mixed),
            "NSFW" | "CONTENT_WARNING_NSFW" => Ok(Self::Nsfw),
            _ => Err(DomainError::Invalid("contentWarning")),
        }
    }
}

/// `sources[].id` is a JSON string in the real indexes (`"4222017068256633289"`) and a number in some older ones. Both
/// give the same `i64`, which serde cannot read from a string by itself.
fn deserialize_source_id<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SourceId {
        Number(i64),
        Text(String),
    }
    match SourceId::deserialize(deserializer)? {
        SourceId::Number(id) => Ok(id),
        SourceId::Text(text) => text.parse().map_err(serde::de::Error::custom),
    }
}

/// One source of a current-format extension (`Source` in the wire table).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceDto {
    #[serde(deserialize_with = "deserialize_source_id")]
    pub id: i64,
    pub name: String,
    pub language: String,
    #[serde(default)]
    pub home_url: String,
}

/// `Resources` of a current-format extension.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesDto {
    pub apk_url: String,
    pub icon_url: String,
}

/// One entry of a current-format extension list (`Extension` in the wire table). `mirrorUrls` and `message` are
/// unused here and dropped at mapping.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct ExtensionDto {
    pub name: String,
    pub package_name: String,
    pub resources: Option<ResourcesDto>,
    pub extension_lib: String,
    pub version_code: i64,
    pub version_name: String,
    /// The enum name; a missing value reads as `UNSPECIFIED`.
    pub content_warning: Option<String>,
    pub is_torrent: bool,
    pub sources: Vec<SourceDto>,
}

/// One entry of a legacy `index.min.json` list (`NetworkLegacyAnimeExtension`).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyExtensionDto {
    pub name: String,
    pub pkg: String,
    pub apk: String,
    pub lang: String,
    pub code: i64,
    pub version: String,
    /// `1` marks adult content.
    pub nsfw: i64,
    pub sources: Option<Vec<LegacySourceDto>>,
}

/// One source of a legacy list entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacySourceDto {
    #[serde(deserialize_with = "deserialize_source_id")]
    pub id: i64,
    pub lang: String,
    pub name: String,
    pub base_url: String,
}

/// A source an available extension provides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableSource {
    pub id: i64,
    pub name: String,
    pub lang: String,
    pub base_url: String,
}

/// An extension a store offers (`AnimeExtension` before it is installed).
#[derive(Clone, Debug, PartialEq)]
pub struct AvailableExtension {
    pub name: String,
    pub pkg: String,
    pub lang: String,
    pub version_code: i64,
    pub version_name: String,
    pub lib_version: f64,
    pub is_nsfw: bool,
    pub apk_url: String,
    pub icon_url: String,
    pub sources: Vec<AvailableSource>,
}

/// The one language of the sources, or `"all"` when they differ or there are none (E13, E14).
fn language_of(sources: &[AvailableSource]) -> String {
    match sources.split_first() {
        Some((first, rest)) if rest.iter().all(|s| s.lang == first.lang) => first.lang.clone(),
        _ => "all".to_owned(),
    }
}

/// `toAvailableExtensions` for one current-format entry. A bad `extensionLib` fails the whole list (E12).
pub fn from_current(dto: ExtensionDto) -> Result<AvailableExtension, DomainError> {
    let lib_version: f64 = dto.extension_lib.parse().map_err(|_| DomainError::Invalid("extensionLib"))?;
    let warning = match dto.content_warning.as_deref() {
        Some(name) => ContentWarning::from_name(name)?,
        None => ContentWarning::Unspecified,
    };
    let resources = dto.resources.ok_or(DomainError::Invalid("resources"))?;
    let sources: Vec<AvailableSource> = dto
        .sources
        .into_iter()
        .map(|s| AvailableSource { id: s.id, name: s.name, lang: s.language, base_url: s.home_url })
        .collect();
    Ok(AvailableExtension {
        lang: language_of(&sources),
        name: dto.name,
        pkg: dto.package_name,
        version_code: dto.version_code,
        version_name: dto.version_name,
        lib_version,
        is_nsfw: warning >= ContentWarning::Mixed,
        apk_url: resources.apk_url,
        icon_url: resources.icon_url,
        sources,
    })
}

/// `NetworkLegacyAnimeExtension.toAvailableExtension`. `base` is the store's base URL (`legacy_base`), which the
/// APK and icon URLs are built from.
pub fn from_legacy(dto: LegacyExtensionDto, base: &str) -> Result<AvailableExtension, DomainError> {
    // `substringBeforeLast('.')`: "1.5.12" gives "1.5", and a version without a dot is taken whole.
    let major_minor = dto.version.rfind('.').map_or(dto.version.as_str(), |at| &dto.version[..at]);
    let lib_version: f64 = major_minor.parse().map_err(|_| DomainError::Invalid("version"))?;
    let name = dto.name.strip_prefix("Aniyomi: ").unwrap_or(&dto.name).to_owned();
    let sources = match dto.sources {
        Some(list) => list
            .into_iter()
            .map(|s| AvailableSource { id: s.id, name: s.name, lang: s.lang, base_url: s.base_url })
            .collect(),
        // A list entry without sources gets one synthetic source with the extension's own name and language (E16).
        None => vec![AvailableSource { id: 0, name: name.clone(), lang: dto.lang.clone(), base_url: String::new() }],
    };
    Ok(AvailableExtension {
        lang: dto.lang,
        name,
        pkg: dto.pkg.clone(),
        version_code: dto.code,
        version_name: dto.version,
        lib_version,
        is_nsfw: dto.nsfw == 1,
        apk_url: format!("{base}/apk/{}", dto.apk),
        icon_url: format!("{base}/icon/{}.png", dto.pkg),
        sources,
    })
}

/// The body of a legacy `index.min.json`: a JSON array of entries. Each entry goes through [`from_legacy`], so one bad
/// entry fails the list as the Kotlin decode does. Entries whose lib version is outside [`SUPPORTED_LIB_VERSIONS`] are
/// dropped.
pub fn from_legacy_list(body: &[u8], base: &str) -> Result<Vec<AvailableExtension>, DomainError> {
    let entries: Vec<LegacyExtensionDto> = serde_json::from_slice(body)?;
    let mapped = entries.into_iter().map(|dto| from_legacy(dto, base)).collect::<Result<Vec<_>, _>>()?;
    Ok(mapped.into_iter().filter(|ext| SUPPORTED_LIB_VERSIONS.contains(&ext.lib_version)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current(extension_lib: &str, warning: Option<&str>, langs: &[&str]) -> ExtensionDto {
        ExtensionDto {
            name: "Ext".into(),
            package_name: "eu.test.ext".into(),
            resources: Some(ResourcesDto { apk_url: "https://a/x.apk".into(), icon_url: "https://a/x.png".into() }),
            extension_lib: extension_lib.into(),
            version_code: 3,
            version_name: "1.0".into(),
            content_warning: warning.map(str::to_owned),
            is_torrent: false,
            sources: langs
                .iter()
                .enumerate()
                .map(|(i, l)| SourceDto { id: i as i64 + 1, name: format!("S{i}"), language: (*l).into(), home_url: String::new() })
                .collect(),
        }
    }

    fn legacy(name: &str, version: &str, nsfw: i64, sources: Option<Vec<LegacySourceDto>>) -> LegacyExtensionDto {
        LegacyExtensionDto {
            name: name.into(),
            pkg: "eu.test.old".into(),
            apk: "old.apk".into(),
            lang: "en".into(),
            code: 7,
            version: version.into(),
            nsfw,
            sources,
        }
    }

    #[test]
    fn a_bad_extension_lib_fails_the_whole_entry() {
        assert!(matches!(from_current(current("abc", None, &["en"])), Err(DomainError::Invalid("extensionLib"))));
    }

    #[test]
    fn zero_sources_and_two_languages_both_read_as_all() {
        assert_eq!(from_current(current("1.5", None, &[])).unwrap().lang, "all");
        assert_eq!(from_current(current("1.5", None, &["en", "ja"])).unwrap().lang, "all");
        assert_eq!(from_current(current("1.5", None, &["ja", "ja"])).unwrap().lang, "ja");
    }

    #[test]
    fn only_mixed_and_nsfw_are_adult() {
        assert!(from_current(current("1.5", Some("MIXED"), &["en"])).unwrap().is_nsfw);
        assert!(from_current(current("1.5", Some("CONTENT_WARNING_NSFW"), &["en"])).unwrap().is_nsfw);
        assert!(!from_current(current("1.5", Some("SAFE"), &["en"])).unwrap().is_nsfw);
        assert!(!from_current(current("1.5", Some("UNSPECIFIED"), &["en"])).unwrap().is_nsfw);
        assert!(!from_current(current("1.5", None, &["en"])).unwrap().is_nsfw);
    }

    #[test]
    fn an_unknown_content_warning_name_fails() {
        assert!(matches!(from_current(current("1.5", Some("ADULT"), &["en"])), Err(DomainError::Invalid("contentWarning"))));
    }

    #[test]
    fn a_legacy_entry_without_sources_gets_one_synthetic_source() {
        let ext = from_legacy(legacy("Foo", "1.5.12", 0, None), "https://h").unwrap();
        assert_eq!(ext.sources, vec![AvailableSource { id: 0, name: "Foo".into(), lang: "en".into(), base_url: String::new() }]);
    }

    #[test]
    fn the_legacy_prefix_is_stripped_only_when_present() {
        assert_eq!(from_legacy(legacy("Aniyomi: Foo", "1", 0, None), "https://h").unwrap().name, "Foo");
        assert_eq!(from_legacy(legacy("Foo", "1", 0, None), "https://h").unwrap().name, "Foo");
    }

    #[test]
    fn a_legacy_entry_with_string_source_ids_parses_to_the_same_i64() {
        // The observed index writes `sources[].id` as a JSON string (T-20a-020, E3.5). The number form still parses.
        let text = r#"{"name":"Aniyomi: Google Drive","pkg":"eu.test.gd","apk":"gd.apk","lang":"all","code":15,
            "version":"14.15","nsfw":0,"sources":[{"name":"Google Drive","lang":"all","id":"4222017068256633289",
            "baseUrl":"https://drive.google.com"}]}"#;
        let entry: LegacyExtensionDto = serde_json::from_str(text).expect("a string id parses");
        assert_eq!(entry.sources.expect("sources")[0].id, 4_222_017_068_256_633_289);
        let numbered = text.replace("\"4222017068256633289\"", "4222017068256633289");
        let entry: LegacyExtensionDto = serde_json::from_str(&numbered).expect("a number id parses");
        assert_eq!(entry.sources.expect("sources")[0].id, 4_222_017_068_256_633_289);
    }

    #[test]
    fn the_legacy_lib_version_drops_the_last_dot_part() {
        assert_eq!(from_legacy(legacy("Foo", "1.5.12", 0, None), "https://h").unwrap().lib_version, 1.5);
        assert_eq!(from_legacy(legacy("Foo", "1", 0, None), "https://h").unwrap().lib_version, 1.0);
    }

    #[test]
    fn a_legacy_index_array_keeps_only_the_supported_lib_versions() {
        let body = br#"[
            {"name":"Aniyomi: Old","pkg":"eu.test.old","apk":"old.apk","lang":"en","code":1,"version":"1.3.2","nsfw":0},
            {"name":"Aniyomi: Edge","pkg":"eu.test.edge","apk":"edge.apk","lang":"en","code":2,"version":"1.4.0","nsfw":0},
            {"name":"Aniyomi: Google Drive","pkg":"eu.test.gd","apk":"gd.apk","lang":"all","code":15,"version":"1.5.12","nsfw":0,
             "sources":[{"name":"Google Drive","lang":"all","id":"4222017068256633289","baseUrl":"https://drive.google.com"}]},
            {"name":"Aniyomi: New","pkg":"eu.test.new","apk":"new.apk","lang":"en","code":3,"version":"1.6.0","nsfw":0}
        ]"#;
        let listed = from_legacy_list(body, "https://h").expect("the array parses");
        let pkgs: Vec<&str> = listed.iter().map(|e| e.pkg.as_str()).collect();
        assert_eq!(pkgs, ["eu.test.edge", "eu.test.gd"]);
        assert_eq!(listed[1].sources[0].id, 4_222_017_068_256_633_289, "the string id parses to the same i64");
        assert_eq!(listed[1].apk_url, "https://h/apk/gd.apk");
        assert!(from_legacy_list(b"{}", "https://h").is_err(), "a body that is not an array is an error");
    }

    #[test]
    fn legacy_urls_are_built_from_the_store_base() {
        let ext = from_legacy(legacy("Foo", "1.5", 1, None), "https://h").unwrap();
        assert_eq!(ext.apk_url, "https://h/apk/old.apk");
        assert_eq!(ext.icon_url, "https://h/icon/eu.test.old.png");
        assert!(ext.is_nsfw, "nsfw 1 is adult");
    }
}
