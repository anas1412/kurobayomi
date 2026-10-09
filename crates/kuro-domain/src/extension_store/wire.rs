//! The store wire format (behavior 7.1, T-09a-031): gunzip when the body has the gzip magic, then sniff the first byte.
//! `[` is a legacy extension list, `{` is a legacy repo or the current JSON store, anything else is protobuf. Protobuf
//! decoding waits for the schema of area 19; `fetch` reports it as an error.

use std::io::Read;

use serde::Deserialize;

use super::model::{AnimeExtensionStore, Contact};
use crate::error::DomainError;

/// `NetworkLegacyAnimeExtensionRepo`: the legacy repo document. `index_v2` points at a newer index, followed once.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRepoDto {
    #[serde(rename = "index_v2")]
    pub index_v2: Option<String>,
    pub meta: LegacyMetaDto,
}

/// The `meta` object of a legacy repo.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyMetaDto {
    pub name: String,
    pub short_name: Option<String>,
    pub website: String,
    pub signing_key_fingerprint: String,
}

/// `NetworkAnimeExtensionStore` in its JSON form. The extension list itself is area 19's shape and is not read here.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentStoreDto {
    pub name: String,
    pub badge_label: String,
    pub signing_key: String,
    pub contact: ContactDto,
    pub extension_list_url: Option<String>,
}

/// The `contact` object of a current store.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactDto {
    pub website: String,
    pub discord: Option<String>,
}

/// What a store body is, decided by its first byte (and, for `{`, by which shape decodes).
#[derive(Clone, Debug, PartialEq)]
pub enum StoreWire {
    /// `[`: a legacy extension list. The store document is then at `repo.json` beside it.
    LegacyList,
    LegacyRepo(LegacyRepoDto),
    CurrentJson(CurrentStoreDto),
    /// Any other first byte. The bytes are kept for the protobuf decoder of area 19.
    Proto(Vec<u8>),
}

/// Largest gunzipped store body. The topic doc's decompression cap is 32 MiB.
const MAX_GUNZIPPED: u64 = 32 * 1024 * 1024;

/// Gunzips `body` when it starts with the gzip magic `0x1f 0x8b`; any other body comes back unchanged (TC-STO-02).
pub fn gunzip_if_needed(body: Vec<u8>) -> Result<Vec<u8>, DomainError> {
    if body.len() < 2 || body[..2] != [0x1f, 0x8b] {
        return Ok(body);
    }
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(body.as_slice())
        .take(MAX_GUNZIPPED + 1)
        .read_to_end(&mut out)
        .map_err(|_| DomainError::Invalid("corrupt gzip store body"))?;
    if out.len() as u64 > MAX_GUNZIPPED {
        return Err(DomainError::Invalid("gzip store body too large"));
    }
    Ok(out)
}

/// Drops a UTF-8 byte order mark and the ASCII whitespace before the first byte, so the first-byte sniff sees the
/// document (`fetch` in `p1-topic-extension-store-index.md` section 3).
pub fn strip_bom_and_ws(mut body: Vec<u8>) -> Vec<u8> {
    let bom = if body.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    let ws = body[bom..].iter().take_while(|b| b.is_ascii_whitespace()).count();
    body.drain(..bom + ws);
    body
}

/// Classifies a (gunzipped) store body. A `{` body is a legacy repo when it decodes as one, else the current store.
pub fn sniff(body: &[u8]) -> Result<StoreWire, DomainError> {
    match body.first().copied() {
        None => Err(DomainError::Store("Empty response")),
        Some(b'[') => Ok(StoreWire::LegacyList),
        Some(b'{') => match serde_json::from_slice::<LegacyRepoDto>(body) {
            Ok(dto) => Ok(StoreWire::LegacyRepo(dto)),
            Err(_) => Ok(StoreWire::CurrentJson(serde_json::from_slice(body)?)),
        },
        // A web page (for example a Cloudflare challenge) is not a store; say so instead of a protobuf error (E2).
        Some(b'<') => Err(DomainError::Store("received an HTML page")),
        Some(_) => Ok(StoreWire::Proto(body.to_vec())),
    }
}

impl LegacyRepoDto {
    /// `toExtensionStore(indexUrl)` for a legacy repo: the badge is the short name, and there is no Discord link.
    pub fn to_store(&self, index_url: &str) -> AnimeExtensionStore {
        AnimeExtensionStore {
            index_url: index_url.to_owned(),
            name: self.meta.name.clone(),
            badge_label: self.meta.short_name.clone().unwrap_or_else(|| self.meta.name.clone()),
            signing_key: self.meta.signing_key_fingerprint.clone(),
            contact: Contact { website: self.meta.website.clone(), discord: None },
            is_legacy: true,
            extension_list_url: None,
            resolved_index_url: index_url.to_owned(),
        }
    }
}

impl CurrentStoreDto {
    /// `toExtensionStore(indexUrl)` for the current format.
    pub fn to_store(&self, index_url: &str) -> AnimeExtensionStore {
        AnimeExtensionStore {
            index_url: index_url.to_owned(),
            name: self.name.clone(),
            badge_label: self.badge_label.clone(),
            signing_key: self.signing_key.clone(),
            contact: Contact { website: self.contact.website.clone(), discord: self.contact.discord.clone() },
            is_legacy: false,
            extension_list_url: self.extension_list_url.clone(),
            resolved_index_url: index_url.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const CURRENT: &[u8] = include_bytes!("../../tests/fixtures/store_current.json");
    const LEGACY_REPO: &[u8] = include_bytes!("../../tests/fixtures/store_legacy_repo.json");
    const LEGACY_LIST: &[u8] = include_bytes!("../../tests/fixtures/store_legacy_list.json");
    const GZIPPED: &[u8] = include_bytes!("../../tests/fixtures/store_gzipped.json.gz");

    #[test]
    fn sniff_classifies_each_first_byte() {
        assert_eq!(sniff(LEGACY_LIST).expect("list"), StoreWire::LegacyList);
        assert!(matches!(sniff(LEGACY_REPO).expect("legacy"), StoreWire::LegacyRepo(_)));
        assert!(matches!(sniff(CURRENT).expect("current"), StoreWire::CurrentJson(_)));
        assert!(matches!(sniff(&[0x0A, 0x01]).expect("proto"), StoreWire::Proto(b) if b == [0x0A, 0x01]));
    }

    #[test]
    fn a_legacy_repo_keeps_its_index_v2_pointer() {
        let StoreWire::LegacyRepo(dto) = sniff(LEGACY_REPO).expect("legacy") else { panic!("legacy repo") };
        assert_eq!(dto.index_v2.as_deref(), Some("https://example.test/v2/index.min.json"));
        let store = dto.to_store("https://legacy.test/repo.json");
        assert_eq!(store.badge_label, "LS");
        assert_eq!(store.signing_key, "ff00ee11");
        assert!(store.is_legacy);
        assert_eq!(store.contact.discord, None);
    }

    #[test]
    fn an_oversize_gzip_body_is_refused() {
        use std::io::Write;
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(&vec![0u8; (MAX_GUNZIPPED + 1) as usize]).expect("write");
        let body = enc.finish().expect("finish");
        assert!(matches!(gunzip_if_needed(body), Err(DomainError::Invalid("gzip store body too large"))));
    }

    #[test]
    fn an_empty_body_is_an_empty_response() {
        assert!(matches!(sniff(&[]), Err(DomainError::Store("Empty response"))));
    }

    #[test]
    fn an_html_page_is_not_mistaken_for_a_protobuf_store() {
        assert!(matches!(sniff(b"<!doctype html><html>"), Err(DomainError::Store("received an HTML page"))));
    }

    #[test]
    fn a_bom_and_leading_whitespace_are_dropped_before_the_sniff() {
        let body = [&[0xEF, 0xBB, 0xBF][..], b"  \n[1]"].concat();
        assert_eq!(strip_bom_and_ws(body), b"[1]");
        assert_eq!(strip_bom_and_ws(b"{}".to_vec()), b"{}");
    }

    #[test]
    fn a_gzipped_body_round_trips_and_a_plain_one_is_unchanged() {
        assert_eq!(gunzip_if_needed(GZIPPED.to_vec()).expect("gunzip"), CURRENT);
        assert_eq!(gunzip_if_needed(CURRENT.to_vec()).expect("plain"), CURRENT);
        assert_eq!(gunzip_if_needed(vec![0x1f]).expect("one byte"), vec![0x1f]);
    }

    #[test]
    fn a_corrupt_gzip_body_is_invalid_not_a_panic() {
        let mut bad = vec![0x1f, 0x8b];
        bad.extend_from_slice(b"not gzip at all");
        assert!(matches!(gunzip_if_needed(bad), Err(DomainError::Invalid(_))));
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(CURRENT).expect("write");
        assert_eq!(gunzip_if_needed(enc.finish().expect("finish")).expect("again"), CURRENT);
    }

    #[test]
    fn a_current_store_maps_its_fields_and_keeps_the_list_url() {
        let StoreWire::CurrentJson(dto) = sniff(CURRENT).expect("current") else { panic!("current") };
        let store = dto.to_store("https://example.test/index.min.json");
        assert_eq!(store.name, "Example Store");
        assert_eq!(store.contact.discord.as_deref(), Some("https://discord.test/invite"));
        assert_eq!(store.extension_list_url.as_deref(), Some("https://example.test/extensions.json"));
        assert!(!store.is_legacy);
    }
}
