//! `AnimeExtensionStoreService.fetch` and `getExtensions` (behavior 7.1 and 7.4, T-09a-031, T-09a-033): the GET of a store
//! document, the decision on its shape, the one `indexV2` follow, and the choice of the list URL for each store.

use super::available::{AvailableExtension, from_legacy_list};
use super::model::AnimeExtensionStore;
use super::wire::{LegacyRepoDto, StoreWire, gunzip_if_needed, sniff, strip_bom_and_ws};
use crate::error::DomainError;

const LEGACY_SUFFIX: &str = "/index.min.json";
const REPO_SUFFIX: &str = "/repo.json";
/// How many `indexV2` pointers one store may follow. The first document is hop 0, so a self-referencing store makes
/// four requests before it fails.
// ponytail: the Kotlin flow recurses without a limit; the topic doc's cap of 3 is kept as the smallest safe choice.
const MAX_INDEX_V2_HOPS: u8 = 3;

/// The one network call of the store flow: a GET whose body comes back as bytes (the body may be gzipped).
pub trait StoreHttp: Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// Fetches and decodes the store at `index_url` (`AnimeExtensionStoreService.fetch`). The result is keyed by `index_url`,
/// the URL the user entered (Q-STORE-1, fixed in T-09a-031). `resolved_index_url` is the document the data came from:
/// `repo.json` for a legacy list, the `indexV2` target for a followed store, else `index_url`.
pub fn fetch_store(http: &dyn StoreHttp, index_url: &str) -> Result<AnimeExtensionStore, DomainError> {
    fetch_document(http, index_url, 0)
}

/// A document GET: gunzipped when it starts with the gzip magic, with any BOM and leading whitespace dropped.
fn get(http: &dyn StoreHttp, url: &str) -> Result<Vec<u8>, DomainError> {
    http.get(url).map_err(DomainError::Net).and_then(gunzip_if_needed).map(strip_bom_and_ws)
}

/// A GET whose body is taken as it is. Kotlin does not gzip-sniff the second request of a legacy list.
fn get_raw(http: &dyn StoreHttp, url: &str) -> Result<Vec<u8>, DomainError> {
    http.get(url).map_err(DomainError::Net)
}

/// `hops` is how many `indexV2` pointers were followed to reach this document.
fn fetch_document(http: &dyn StoreHttp, index_url: &str, hops: u8) -> Result<AnimeExtensionStore, DomainError> {
    let body = get(http, index_url)?;
    match sniff(&body)? {
        StoreWire::LegacyList => {
            if !index_url.ends_with(LEGACY_SUFFIX) {
                return Err(DomainError::Store("Provided legacy store url is not valid"));
            }
            let repo_url = index_url.replace(LEGACY_SUFFIX, REPO_SUFFIX);
            let repo: LegacyRepoDto = serde_json::from_slice(&get_raw(http, &repo_url)?)?;
            legacy_or_follow(http, index_url, &repo_url, &repo, hops)
        }
        StoreWire::LegacyRepo(repo) => legacy_or_follow(http, index_url, index_url, &repo, hops),
        StoreWire::CurrentJson(dto) => Ok(dto.to_store(index_url)),
        StoreWire::Proto(_) => Err(DomainError::Invalid("protobuf store: schema owned by area:19-extensions-anime")),
    }
}

/// `document_url` is the document `repo` was decoded from (the key, or `repo.json` for a legacy list). A legacy repo
/// with an `indexV2` pointer is never stored itself: the followed document is the store.
fn legacy_or_follow(
    http: &dyn StoreHttp,
    index_url: &str,
    document_url: &str,
    repo: &LegacyRepoDto,
    hops: u8,
) -> Result<AnimeExtensionStore, DomainError> {
    let Some(next) = repo.index_v2.as_deref() else {
        let mut store = repo.to_store(index_url);
        store.resolved_index_url = document_url.to_owned();
        return Ok(store);
    };
    if hops >= MAX_INDEX_V2_HOPS {
        return Err(DomainError::Store("Too many index_v2 redirects"));
    }
    let mut store = fetch_document(http, next, hops + 1)?;
    store.index_url = index_url.to_owned();
    Ok(store)
}

/// Where a store's extension list is read from (behavior 7.4). The order is fixed: an explicit list URL, then a
/// non-legacy index, then the legacy `index.min.json` beside the store. The index and the legacy base are read from the
/// resolved document, so a followed `indexV2` store lists from its target, not from the URL the user entered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListSource {
    ExtensionList(String),
    Index(String),
    LegacyIndex(String),
}

/// The list source of `store`.
pub fn list_source(store: &AnimeExtensionStore) -> ListSource {
    if let Some(url) = &store.extension_list_url {
        ListSource::ExtensionList(url.clone())
    } else if !store.is_legacy {
        ListSource::Index(store.resolved_index_url.clone())
    } else {
        ListSource::LegacyIndex(format!("{}{LEGACY_SUFFIX}", legacy_base(&store.resolved_index_url)))
    }
}

/// The base of a legacy store: its key without `/repo.json` (Kotlin `removeSuffix`) or `/index.min.json` (the key the
/// user may have entered). Any other key is its own base.
pub fn legacy_base(index_url: &str) -> &str {
    index_url
        .strip_suffix(REPO_SUFFIX)
        .or_else(|| index_url.strip_suffix(LEGACY_SUFFIX))
        .unwrap_or(index_url)
}

/// The extensions one legacy store lists: a GET of its `index.min.json` (not gzip-sniffed, as Kotlin does), then
/// [`from_legacy_list`]. This is the fetch `fetch_extensions` takes; its caller passes it as
/// `|store| legacy_extensions(&http, store)`. A store whose list is current-format is read by area 19, not here.
pub fn legacy_extensions(http: &dyn StoreHttp, store: &AnimeExtensionStore) -> Result<Vec<AvailableExtension>, DomainError> {
    let ListSource::LegacyIndex(url) = list_source(store) else {
        return Err(DomainError::Invalid("current-format extension list is read by area 19"));
    };
    from_legacy_list(&get_raw(http, &url)?, legacy_base(&store.resolved_index_url))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    const CURRENT: &[u8] = include_bytes!("../../tests/fixtures/store_current.json");
    const LEGACY_LIST: &[u8] = include_bytes!("../../tests/fixtures/store_legacy_list.json");
    const GZIPPED: &[u8] = include_bytes!("../../tests/fixtures/store_gzipped.json.gz");

    /// Serves fixed bodies by URL and records every GET. An unknown URL is a 404.
    #[derive(Default)]
    pub struct FakeHttp {
        pub routes: HashMap<String, Result<Vec<u8>, String>>,
        pub calls: Mutex<Vec<String>>,
    }

    impl FakeHttp {
        pub fn route(mut self, url: &str, body: &[u8]) -> Self {
            self.routes.insert(url.to_owned(), Ok(body.to_vec()));
            self
        }

        pub fn fail(mut self, url: &str) -> Self {
            self.routes.insert(url.to_owned(), Err("HTTP 500".into()));
            self
        }

        pub fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl StoreHttp for FakeHttp {
        fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.calls.lock().unwrap().push(url.to_owned());
            self.routes.get(url).cloned().unwrap_or_else(|| Err("HTTP 404".into()))
        }
    }

    #[test]
    fn a_legacy_list_fetches_repo_json_and_keys_the_store_by_the_entered_url() {
        let repo = br#"{"meta":{"name":"Legacy Store","shortName":"LS","website":"https://legacy.test","signingKeyFingerprint":"ff00ee11"}}"#;
        let http = FakeHttp::default()
            .route("https://s.test/index.min.json", LEGACY_LIST)
            .route("https://s.test/repo.json", repo);
        let store = fetch_store(&http, "https://s.test/index.min.json").expect("fetch");
        assert_eq!(store.index_url, "https://s.test/index.min.json");
        assert_eq!(store.resolved_index_url, "https://s.test/repo.json", "the document the fields came from");
        assert_eq!(store.name, "Legacy Store");
        assert_eq!(store.badge_label, "LS");
        assert!(store.is_legacy);
        assert_eq!(http.calls(), ["https://s.test/index.min.json", "https://s.test/repo.json"]);
    }

    #[test]
    fn a_followed_current_index_is_listed_from_its_target_not_the_entered_url() {
        let pointer = br#"{"index_v2":"https://s.test/v2/index.min.json","meta":{"name":"A","website":"w","signingKeyFingerprint":"k"}}"#;
        let followed = br#"{"name":"Followed","badgeLabel":"F","signingKey":"k","contact":{"website":"https://f.test"}}"#;
        let http = FakeHttp::default()
            .route("https://s.test/a.json", pointer)
            .route("https://s.test/v2/index.min.json", followed);
        let store = fetch_store(&http, "https://s.test/a.json").expect("follow");
        assert_eq!(store.index_url, "https://s.test/a.json", "the key is the entered url");
        assert_eq!(store.resolved_index_url, "https://s.test/v2/index.min.json", "the list is read from the target");
        assert!(!store.is_legacy);
        assert_eq!(list_source(&store), ListSource::Index("https://s.test/v2/index.min.json".into()));
    }

    #[test]
    fn a_followed_legacy_repo_is_listed_from_its_own_base() {
        let pointer = br#"{"index_v2":"https://t.test/v2/repo.json","meta":{"name":"A","website":"w","signingKeyFingerprint":"k"}}"#;
        let repo = br#"{"meta":{"name":"Target","shortName":"T","website":"w","signingKeyFingerprint":"k"}}"#;
        let http = FakeHttp::default()
            .route("https://s.test/a.json", pointer)
            .route("https://t.test/v2/repo.json", repo);
        let store = fetch_store(&http, "https://s.test/a.json").expect("follow");
        assert!(store.is_legacy);
        assert_eq!(store.resolved_index_url, "https://t.test/v2/repo.json");
        assert_eq!(list_source(&store), ListSource::LegacyIndex("https://t.test/v2/index.min.json".into()));
    }

    #[test]
    fn a_legacy_list_url_without_index_min_json_is_rejected_with_the_upstream_message() {
        let http = FakeHttp::default().route("https://s.test/repo.json", LEGACY_LIST);
        let err = fetch_store(&http, "https://s.test/repo.json").expect_err("rejected");
        assert!(matches!(err, DomainError::Store("Provided legacy store url is not valid")));
        assert_eq!(http.calls().len(), 1, "no second GET");
    }

    #[test]
    fn one_index_v2_is_followed_and_the_hop_is_stored_under_its_own_url() {
        let first = br#"{"index_v2":"https://s.test/v2/index.min.json","meta":{"name":"A","website":"w","signingKeyFingerprint":"k"}}"#;
        let one = FakeHttp::default()
            .route("https://s.test/a.json", first)
            .route("https://s.test/v2/index.min.json", CURRENT);
        let store = fetch_store(&one, "https://s.test/a.json").expect("one follow");
        assert_eq!(store.index_url, "https://s.test/a.json", "keyed by the entered url");
        assert_eq!(store.resolved_index_url, "https://s.test/v2/index.min.json", "the v2 hop is the resolved url");
        assert_eq!(store.name, "Example Store", "the followed document supplies the data");
    }

    #[test]
    fn a_self_referencing_index_v2_ends_with_too_many_hops_after_four_requests() {
        let first = br#"{"index_v2":"https://s.test/v2/index.min.json","meta":{"name":"A","website":"w","signingKeyFingerprint":"k"}}"#;
        let looping = FakeHttp::default()
            .route("https://s.test/a.json", first)
            .route("https://s.test/v2/index.min.json", first);
        let err = fetch_store(&looping, "https://s.test/a.json").expect_err("loops");
        assert!(matches!(err, DomainError::Store("Too many index_v2 redirects")));
        assert_eq!(looping.calls().len(), 4, "at most four requests");
    }

    #[test]
    fn the_legacy_repo_is_fetched_without_gzip_sniffing() {
        let http = FakeHttp::default()
            .route("https://s.test/index.min.json", LEGACY_LIST)
            .route("https://s.test/repo.json", GZIPPED);
        assert!(matches!(fetch_store(&http, "https://s.test/index.min.json"), Err(DomainError::Json(_))));
    }

    #[test]
    fn every_index_min_json_suffix_is_replaced_with_repo_json() {
        let repo = br#"{"meta":{"name":"A","website":"w","signingKeyFingerprint":"k"}}"#;
        let http = FakeHttp::default()
            .route("https://s.test/x/index.min.json/index.min.json", LEGACY_LIST)
            .route("https://s.test/x/repo.json/repo.json", repo);
        let store = fetch_store(&http, "https://s.test/x/index.min.json/index.min.json").expect("fetch");
        assert!(http.calls().contains(&"https://s.test/x/repo.json/repo.json".to_owned()));
        assert_eq!(store.resolved_index_url, "https://s.test/x/repo.json/repo.json");
    }

    #[test]
    fn a_bom_before_the_document_is_skipped() {
        let body = [&[0xEF, 0xBB, 0xBF][..], CURRENT].concat();
        let http = FakeHttp::default().route("https://s.test/bom.json", &body);
        assert_eq!(fetch_store(&http, "https://s.test/bom.json").expect("bom").name, "Example Store");
    }

    #[test]
    fn an_empty_body_is_an_empty_response() {
        let http = FakeHttp::default().route("https://s.test/empty.json", b"");
        assert!(matches!(fetch_store(&http, "https://s.test/empty.json"), Err(DomainError::Store("Empty response"))));
    }

    #[test]
    fn a_gzipped_current_store_is_decoded() {
        let http = FakeHttp::default().route("https://s.test/index.min.json", GZIPPED);
        let store = fetch_store(&http, "https://s.test/index.min.json").expect("fetch");
        assert_eq!(store.signing_key, "a1b2c3d4");
    }

    #[test]
    fn a_protobuf_store_waits_for_the_area_19_schema() {
        let http = FakeHttp::default().route("https://s.test/p", &[0x0A, 0x03, b'a']);
        let err = fetch_store(&http, "https://s.test/p").expect_err("protobuf");
        assert!(matches!(err, DomainError::Invalid(m) if m.starts_with("protobuf store")));
    }

    #[test]
    fn a_failed_get_is_a_network_error() {
        let http = FakeHttp::default().fail("https://s.test/index.min.json");
        assert!(matches!(
            fetch_store(&http, "https://s.test/index.min.json"),
            Err(DomainError::Net(m)) if m == "HTTP 500"
        ));
    }

    #[test]
    fn a_legacy_store_lists_its_index_and_drops_the_unsupported_lib_versions() {
        let repo = br#"{"meta":{"name":"Legacy Store","shortName":"LS","website":"w","signingKeyFingerprint":"k"}}"#;
        let index = br#"[{"name":"Aniyomi: Kept","pkg":"eu.test.kept","apk":"kept.apk","lang":"en","code":2,"version":"1.5.1","nsfw":0},
            {"name":"Aniyomi: Old","pkg":"eu.test.old","apk":"old.apk","lang":"en","code":1,"version":"1.3.0","nsfw":0}]"#;
        let http = FakeHttp::default()
            .route("https://s.test/index.min.json", index)
            .route("https://s.test/repo.json", repo);
        let store = fetch_store(&http, "https://s.test/index.min.json").expect("fetch");
        let listed = legacy_extensions(&http, &store).expect("list");
        assert_eq!(listed.len(), 1, "1.3 is outside the supported set");
        assert_eq!(listed[0].pkg, "eu.test.kept");
        assert_eq!(listed[0].apk_url, "https://s.test/apk/kept.apk", "built from the store base");
        assert_eq!(http.calls().last().map(String::as_str), Some("https://s.test/index.min.json"));
    }

    #[test]
    fn list_source_picks_the_three_branches_in_order() {
        let mut store = AnimeExtensionStore {
            index_url: "https://s.test/index.min.json".into(),
            name: "S".into(),
            badge_label: "S".into(),
            signing_key: "k".into(),
            contact: crate::extension_store::model::Contact { website: "w".into(), discord: None },
            is_legacy: false,
            extension_list_url: Some("https://s.test/list.json".into()),
            resolved_index_url: "https://s.test/index.min.json".into(),
        };
        assert_eq!(list_source(&store), ListSource::ExtensionList("https://s.test/list.json".into()));
        store.extension_list_url = None;
        assert_eq!(list_source(&store), ListSource::Index("https://s.test/index.min.json".into()));
        store.is_legacy = true;
        store.index_url = "https://s.test/repo.json".into();
        store.resolved_index_url = "https://s.test/repo.json".into();
        assert_eq!(list_source(&store), ListSource::LegacyIndex("https://s.test/index.min.json".into()));
        store.index_url = "https://s.test/index.min.json".into();
        store.resolved_index_url = "https://s.test/index.min.json".into();
        assert_eq!(list_source(&store), ListSource::LegacyIndex("https://s.test/index.min.json".into()));
        store.is_legacy = false;
        store.index_url = "https://s.test/a.json".into();
        store.resolved_index_url = "https://s.test/v2/index.min.json".into();
        assert_eq!(list_source(&store), ListSource::Index("https://s.test/v2/index.min.json".into()), "the resolved url, not the key");
    }
}
