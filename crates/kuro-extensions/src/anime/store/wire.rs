//! Wire shapes of the store documents (`p1-data-and-config.md` section 3). The JSON store, the legacy repo and the
//! legacy list are serde shapes, and the protobuf store is a set of `prost` messages with the Kotlin `@ProtoNumber`
//! tags. A protobuf document is turned into the JSON shapes, so the mapping code reads one model. Unknown keys are
//! ignored, and a required field that is missing fails the whole document.

use prost::Message;
use serde::Deserialize;

use super::StoreError;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreJson {
    pub name: String,
    pub badge_label: String,
    pub signing_key: String,
    pub contact: ContactJson,
    #[serde(default)]
    pub extension_list: Option<ExtensionListJson>,
    #[serde(default)]
    pub extension_list_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactJson {
    pub website: String,
    #[serde(default)]
    pub discord: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionListJson {
    pub extensions: Vec<ExtensionJson>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionJson {
    pub name: String,
    pub package_name: String,
    pub resources: ResourcesJson,
    pub extension_lib: String,
    pub version_code: i64,
    pub version_name: String,
    pub content_warning: ContentWarning,
    #[serde(default)]
    pub is_torrent: bool,
    pub sources: Vec<SourceJson>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesJson {
    pub apk_url: String,
    pub icon_url: String,
    /// The Kurobayomi addition. Aniyomi ignores it. Every field is optional here so that a malformed block
    /// classifies the entry (see `store::map`) instead of failing the store.
    #[serde(default)]
    pub kuroext: Option<KuroextJson>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KuroextJson {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub ed25519: Option<String>,
    #[serde(default)]
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceJson {
    pub id: i64,
    pub name: String,
    pub language: String,
    #[serde(default)]
    pub home_url: String,
}

/// The `ContentWarning` enum. Accepts the short names and the `CONTENT_WARNING_` names. Any other value fails the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentWarning {
    Unspecified,
    Safe,
    Mixed,
    Nsfw,
}

impl ContentWarning {
    /// The one NSFW rule (data section 2.1): `Mixed` and `Nsfw` are NSFW. The store listing and the package loader both
    /// ask this, so a package and its store entry cannot disagree (TST-19-036).
    pub fn is_nsfw(self) -> bool {
        self >= ContentWarning::Mixed
    }

    /// The protobuf value (`@ProtoNumber`, 0 to 3). Anything else is not a content warning.
    pub fn from_number(number: i32) -> Option<ContentWarning> {
        match number {
            0 => Some(ContentWarning::Unspecified),
            1 => Some(ContentWarning::Safe),
            2 => Some(ContentWarning::Mixed),
            3 => Some(ContentWarning::Nsfw),
            _ => None,
        }
    }

    pub fn parse(raw: &str) -> Option<ContentWarning> {
        match raw.strip_prefix("CONTENT_WARNING_").unwrap_or(raw) {
            "UNSPECIFIED" => Some(ContentWarning::Unspecified),
            "SAFE" => Some(ContentWarning::Safe),
            "MIXED" => Some(ContentWarning::Mixed),
            "NSFW" => Some(ContentWarning::Nsfw),
            _ => None,
        }
    }
}

/// A package manifest carries a flag, not a warning. A flagged package is `Nsfw`, an unflagged one is `Safe`, so the
/// loader asks the same rule as the store listing.
impl From<bool> for ContentWarning {
    fn from(flag: bool) -> Self {
        if flag { ContentWarning::Nsfw } else { ContentWarning::Safe }
    }
}

impl<'de> Deserialize<'de> for ContentWarning {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        ContentWarning::parse(&raw).ok_or_else(|| serde::de::Error::custom(format!("unknown contentWarning {raw:?}")))
    }
}

/// `repo.json` of a legacy store (`NetworkLegacyAnimeExtensionRepo`). `meta` is required, so a store document does
/// not decode as this type, and the caller falls back to the store shape.
#[derive(Debug, Clone, Deserialize)]
pub struct RepoJson {
    #[serde(default, rename = "index_v2")]
    pub index_v2: Option<String>,
    pub meta: RepoMetaJson,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoMetaJson {
    pub name: String,
    #[serde(default)]
    pub short_name: Option<String>,
    pub website: String,
    pub signing_key_fingerprint: String,
}

/// One entry of a legacy `index.min.json` (`NetworkLegacyAnimeExtension`).
#[derive(Debug, Clone, Deserialize)]
pub struct LegacyExtJson {
    pub name: String,
    pub pkg: String,
    pub apk: String,
    pub lang: String,
    pub code: i64,
    pub version: String,
    pub nsfw: i32,
    #[serde(default)]
    pub sources: Option<Vec<LegacySourceJson>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacySourceJson {
    pub id: i64,
    pub lang: String,
    pub name: String,
    pub base_url: String,
}

/// The protobuf store (`p1-data-and-config.md` section 3.1). The tags are the Kotlin `@ProtoNumber` values.
#[derive(Clone, PartialEq, Message)]
pub struct PbStore {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub badge_label: String,
    #[prost(string, tag = "3")]
    pub signing_key: String,
    #[prost(message, optional, tag = "4")]
    pub contact: Option<PbContact>,
    #[prost(message, optional, tag = "101")]
    pub extension_list: Option<PbExtensionList>,
    #[prost(string, optional, tag = "102")]
    pub extension_list_url: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PbContact {
    #[prost(string, tag = "1")]
    pub website: String,
    #[prost(string, optional, tag = "2")]
    pub discord: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PbExtensionList {
    #[prost(message, repeated, tag = "1")]
    pub extensions: Vec<PbExtension>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PbExtension {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub package_name: String,
    #[prost(message, optional, tag = "3")]
    pub resources: Option<PbResources>,
    #[prost(string, tag = "4")]
    pub extension_lib: String,
    #[prost(int64, tag = "5")]
    pub version_code: i64,
    #[prost(string, tag = "6")]
    pub version_name: String,
    /// The enum as its varint. A value outside 0 to 3 fails the document when it is converted.
    #[prost(int32, tag = "7")]
    pub content_warning: i32,
    #[prost(bool, tag = "8")]
    pub is_torrent: bool,
    #[prost(message, repeated, tag = "9")]
    pub sources: Vec<PbSource>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PbResources {
    #[prost(string, tag = "1")]
    pub apk_url: String,
    #[prost(string, tag = "2")]
    pub icon_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct PbSource {
    #[prost(int64, tag = "1")]
    pub id: i64,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(string, tag = "3")]
    pub language: String,
    #[prost(string, tag = "4")]
    pub home_url: String,
    #[prost(string, repeated, tag = "5")]
    pub mirror_urls: Vec<String>,
    #[prost(string, optional, tag = "6")]
    pub message: Option<String>,
}

fn pb_error(err: impl std::fmt::Display) -> StoreError {
    StoreError::Decode(err.to_string())
}

/// Decodes a protobuf store. A missing `contact` or `resources` message, or a content warning outside 0 to 3, fails the
/// whole store (Kotlin throws for a missing required property and for an unknown enum number).
pub fn decode_store_pb(body: &[u8]) -> Result<StoreJson, StoreError> {
    let store = PbStore::decode(body).map_err(pb_error)?;
    let contact = store.contact.ok_or_else(|| pb_error("the store has no contact"))?;
    let extension_list = store.extension_list.map(extension_list_from_pb).transpose()?;
    Ok(StoreJson {
        name: store.name,
        badge_label: store.badge_label,
        signing_key: store.signing_key,
        contact: ContactJson { website: contact.website, discord: contact.discord },
        extension_list,
        extension_list_url: store.extension_list_url,
    })
}

/// Decodes a protobuf extension list (the body of an `extensionListUrl`).
pub fn decode_list_pb(body: &[u8]) -> Result<ExtensionListJson, StoreError> {
    extension_list_from_pb(PbExtensionList::decode(body).map_err(pb_error)?)
}

fn extension_list_from_pb(list: PbExtensionList) -> Result<ExtensionListJson, StoreError> {
    let extensions = list.extensions.into_iter().map(extension_from_pb).collect::<Result<Vec<_>, _>>()?;
    Ok(ExtensionListJson { extensions })
}

fn extension_from_pb(ext: PbExtension) -> Result<ExtensionJson, StoreError> {
    let resources = ext.resources.ok_or_else(|| pb_error(format!("{} has no resources", ext.package_name)))?;
    let content_warning = ContentWarning::from_number(ext.content_warning)
        .ok_or_else(|| pb_error(format!("unknown contentWarning {}", ext.content_warning)))?;
    Ok(ExtensionJson {
        name: ext.name,
        package_name: ext.package_name,
        resources: ResourcesJson { apk_url: resources.apk_url, icon_url: resources.icon_url, kuroext: None },
        extension_lib: ext.extension_lib,
        version_code: ext.version_code,
        version_name: ext.version_name,
        content_warning,
        is_torrent: ext.is_torrent,
        sources: ext
            .sources
            .into_iter()
            .map(|source| SourceJson {
                id: source.id,
                name: source.name,
                language: source.language,
                home_url: source.home_url,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn both_content_warning_spellings_parse_and_others_fail() {
        assert_eq!(ContentWarning::parse("SAFE"), Some(ContentWarning::Safe));
        assert_eq!(ContentWarning::parse("CONTENT_WARNING_MIXED"), Some(ContentWarning::Mixed));
        assert_eq!(ContentWarning::parse("FOO"), None);
    }

    #[test]
    fn a_missing_required_icon_fails_the_document() {
        let text = r#"{"name":"S","badgeLabel":"S","signingKey":"k","contact":{"website":"w"},
            "extensionList":{"extensions":[{"name":"a","packageName":"p","resources":{"apkUrl":"u"},
            "extensionLib":"16","versionCode":1,"versionName":"16.1","contentWarning":"SAFE","sources":[]}]}}"#;
        assert!(serde_json::from_str::<StoreJson>(text).is_err());
    }

    #[test]
    fn the_protobuf_messages_use_the_kotlin_proto_numbers() {
        // The golden bytes are written by hand from the tag table of data section 3.1: a field key is `tag << 3 | type`.
        let contact = PbContact { website: "w".into(), discord: Some("d".into()) };
        assert_eq!(contact.encode_to_vec(), [0x0a, 1, b'w', 0x12, 1, b'd']);
        let url_only = PbStore { extension_list_url: Some("u".into()), ..PbStore::default() };
        assert_eq!(url_only.encode_to_vec(), [0xb2, 0x06, 1, b'u'], "tag 102");
        let list_only = PbStore { extension_list: Some(PbExtensionList::default()), ..PbStore::default() };
        assert_eq!(list_only.encode_to_vec(), [0xaa, 0x06, 0], "tag 101");
        let ext = PbExtension {
            extension_lib: "1".into(),
            version_code: 2,
            version_name: "3".into(),
            content_warning: 9,
            is_torrent: true,
            ..PbExtension::default()
        };
        assert_eq!(ext.encode_to_vec(), [0x22, 1, b'1', 0x28, 2, 0x32, 1, b'3', 0x38, 9, 0x40, 1], "tags 4 to 8");
        let source = PbSource { id: 1, home_url: "h".into(), ..PbSource::default() };
        assert_eq!(source.encode_to_vec(), [0x08, 1, 0x22, 1, b'h'], "tags 1 and 4");
    }

    #[test]
    fn a_protobuf_value_of_9_is_not_a_content_warning() {
        assert_eq!(ContentWarning::from_number(3), Some(ContentWarning::Nsfw));
        assert_eq!(ContentWarning::from_number(9), None);
        assert_eq!(ContentWarning::from_number(-1), None);
        let ext = PbExtension { resources: Some(PbResources::default()), content_warning: 9, ..PbExtension::default() };
        assert!(matches!(extension_from_pb(ext), Err(StoreError::Decode(_))));
    }

    #[test]
    fn a_protobuf_store_without_contact_or_resources_fails() {
        let no_contact = PbStore { name: "S".into(), ..PbStore::default() };
        assert!(matches!(decode_store_pb(&no_contact.encode_to_vec()), Err(StoreError::Decode(_))));
        let no_resources = PbExtensionList { extensions: vec![PbExtension::default()] };
        assert!(matches!(decode_list_pb(&no_resources.encode_to_vec()), Err(StoreError::Decode(_))));
    }

    #[test]
    fn a_legacy_repo_needs_meta_and_a_store_document_is_not_one() {
        let repo: RepoJson = serde_json::from_str(
            r#"{"index_v2":"https://x/v2.json","meta":{"name":"N","website":"w","signingKeyFingerprint":"f"}}"#,
        )
        .unwrap();
        assert_eq!(repo.index_v2.as_deref(), Some("https://x/v2.json"));
        assert_eq!(repo.meta.short_name, None);
        let store = r#"{"name":"S","badgeLabel":"S","signingKey":"k","contact":{"website":"w"}}"#;
        assert!(serde_json::from_str::<RepoJson>(store).is_err());
    }

    #[test]
    fn a_legacy_entry_decodes_with_sources_missing_or_null() {
        let base = r#""name":"N","pkg":"p","apk":"a.apk","lang":"en","code":3,"version":"16.3","nsfw":0"#;
        let missing: LegacyExtJson = serde_json::from_str(&format!("{{{base}}}")).unwrap();
        let null: LegacyExtJson = serde_json::from_str(&format!("{{{base},\"sources\":null}}")).unwrap();
        assert!(missing.sources.is_none() && null.sources.is_none());
    }
}
