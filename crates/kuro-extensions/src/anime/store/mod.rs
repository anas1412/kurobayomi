//! Store index fetch and detection (T-19a-013 to T-19a-016, `p1-rust-design.md` section 3.2). A store is a JSON or protobuf
//! document, optionally gzipped, or a legacy repo with a legacy list. The first byte picks the format, a legacy repo may
//! redirect to another store at most four times, and a document is limited to 16 MiB. Each request gives up cleanly
//! when its cancel token fires.

pub mod map;
pub mod refresh;
pub mod wire;

use std::io::Read;
use std::time::Duration;

use flate2::read::GzDecoder;
use kuro_net::NetworkClient;
use kuro_net::transport::TransportLimits;
use reqwest::header::CACHE_CONTROL;
use tokio_util::sync::CancellationToken;

use crate::anime::model::{Available, StoreRef, StoreRow};
use wire::{ExtensionListJson, LegacyExtJson, RepoJson, StoreJson};

pub const STORE_BODY_LIMIT: usize = 16 * 1024 * 1024;
/// The most a gzip body may expand to. It stops a small download from filling memory.
pub const GUNZIP_LIMIT: usize = 64 * 1024 * 1024;
pub const STORE_TIMEOUT: Duration = Duration::from_secs(30);
/// How many `index_v2` redirects a legacy repo may follow. A fifth one is a `RedirectLoop`.
pub const MAX_REDIRECTS: usize = 4;
/// The message of a `[` body served from a URL that is not `/index.min.json` (kept verbatim, UI K-3).
pub const LEGACY_URL_MESSAGE: &str = "Provided legacy store url is not valid";

const LEGACY_LIST: &str = "/index.min.json";
const LEGACY_REPO: &str = "/repo.json";
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error("the store could not be reached: {0}")]
    Network(String),
    #[error("the store answered HTTP {0}")]
    Status(u16),
    #[error("the store is larger than 16 MiB")]
    TooLarge,
    #[error("the store could not be decoded: {0}")]
    Decode(String),
    #[error("{0}")]
    Invalid(String),
    #[error("the store redirects to another store too many times")]
    RedirectLoop,
    #[error("the refresh was cancelled")]
    Cancelled,
}

/// The transport limits of a store request: the kuro-net values, with the 30 second call timeout of data section 6.
fn store_limits() -> TransportLimits {
    TransportLimits { call_timeout: STORE_TIMEOUT, ..TransportLimits::default() }
}

/// The HTTP client for store requests. It is built with the kuro-net transport (rustls, connect and read timeouts,
/// redirect and pool limits), shares the app's cookie jar and sends its default `User-Agent`. The call timeout is 30
/// seconds. Callers keep one and reuse it.
pub fn http_client(net: &NetworkClient) -> Result<reqwest::Client, StoreError> {
    kuro_net::transport::client_builder(&store_limits(), net.cookies().provider())
        .user_agent(net.default_user_agent())
        .build()
        .map_err(|err| StoreError::Network(err.to_string()))
}

/// What one store fetch returns: the row to save and the listing from the same document.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchedStore {
    pub row: StoreRow,
    pub extensions: Vec<Available>,
}

/// Fetches `index_url`, reads the store, and lists its extensions. A separate `extensionListUrl` is fetched when
/// the store names one, and it wins over an inline list (TST-19-022). A legacy store lists through its
/// `index.min.json`.
pub async fn fetch_store(
    client: &reqwest::Client,
    index_url: &str,
    cancel: &CancellationToken,
) -> Result<FetchedStore, StoreError> {
    let Found { row, inline } = fetch_row(client, index_url, cancel).await?;
    let extensions = match inline {
        Some(list) if row.extension_list_url.is_none() => map_list(&list, &row.store_ref())?,
        _ => fetch_extensions(client, &row, cancel).await?,
    };
    Ok(FetchedStore { row, extensions })
}

/// The extensions a saved store lists (`getExtensions`). The branch comes from the row, in this order: its
/// `extensionListUrl`, then the index of a non-legacy store, then the `index.min.json` beside a legacy store. A legacy
/// list is never gunzipped. A migrated row whose flag does not fit its URL fails here (TST-19-049).
pub async fn fetch_extensions(
    client: &reqwest::Client,
    row: &StoreRow,
    cancel: &CancellationToken,
) -> Result<Vec<Available>, StoreError> {
    let store = row.store_ref();
    if let Some(list_url) = row.extension_list_url.as_deref() {
        let body = get_document(client, list_url, cancel).await?;
        return map_list(&decode_list(&body)?, &store);
    }
    if !row.is_legacy {
        let body = get_document(client, &row.index_url, cancel).await?;
        let list = decode_store(&body)?
            .extension_list
            .ok_or_else(|| StoreError::Invalid("extensionList is missing".into()))?;
        return map_list(&list, &store);
    }
    let base = row.index_url.strip_suffix(LEGACY_REPO).unwrap_or(&row.index_url);
    let body = get_body(client, &format!("{base}{LEGACY_LIST}"), cancel).await?;
    let entries: Vec<LegacyExtJson> = decode_json(&body)?;
    entries.iter().map(|entry| map::legacy_available(entry, &store, base)).collect()
}

fn map_list(list: &ExtensionListJson, store: &StoreRef) -> Result<Vec<Available>, StoreError> {
    list.extensions.iter().map(|ext| map::available(ext, store)).collect()
}

/// A store row and the inline listing of the document it came from, when that document had one.
struct Found {
    row: StoreRow,
    inline: Option<ExtensionListJson>,
}

/// The first document of a fetch is a store or a legacy repo.
enum Document {
    Store(StoreJson),
    Repo(RepoJson),
}

/// `fetch` of the Kotlin service. The stored `index_url` is the final one, after the legacy rewrite and the redirects.
async fn fetch_row(client: &reqwest::Client, index_url: &str, cancel: &CancellationToken) -> Result<Found, StoreError> {
    let mut url = index_url.to_owned();
    let mut redirects = 0;
    loop {
        let body = get_document(client, &url, cancel).await?;
        let mut stored_url = url.clone();
        let document = if body.first() == Some(&b'[') {
            if !url.ends_with(LEGACY_LIST) {
                return Err(StoreError::Invalid(LEGACY_URL_MESSAGE.into()));
            }
            stored_url = url.replace(LEGACY_LIST, LEGACY_REPO);
            let repo = get_body(client, &stored_url, cancel).await?;
            Document::Repo(decode_json(&repo)?)
        } else {
            decode_document(&body)?
        };
        match document {
            Document::Store(store) => {
                return Ok(Found { row: map::store_row(&store, &stored_url), inline: store.extension_list });
            }
            Document::Repo(repo) => match repo.index_v2.as_deref() {
                None => return Ok(Found { row: map::legacy_row(&repo, &stored_url), inline: None }),
                Some(next) => {
                    redirects += 1;
                    if redirects > MAX_REDIRECTS {
                        return Err(StoreError::RedirectLoop);
                    }
                    url = next.to_owned();
                }
            },
        }
    }
}

/// A `{` body is a legacy repo when it has `meta`, and a store otherwise. Anything else is a protobuf store.
fn decode_document(body: &[u8]) -> Result<Document, StoreError> {
    if body.first() == Some(&b'{') {
        if let Ok(repo) = serde_json::from_slice::<RepoJson>(body) {
            return Ok(Document::Repo(repo));
        }
    }
    decode_store(body).map(Document::Store)
}

/// The first byte picks the format: `{` is JSON, anything else is protobuf. An empty body is an error, not a panic.
fn decode_store(body: &[u8]) -> Result<StoreJson, StoreError> {
    match body.first() {
        None => Err(StoreError::Decode("the store document is empty".into())),
        Some(b'{') => decode_json(body),
        Some(_) => wire::decode_store_pb(body),
    }
}

fn decode_list(body: &[u8]) -> Result<ExtensionListJson, StoreError> {
    match body.first() {
        None => Err(StoreError::Decode("the extension list is empty".into())),
        Some(b'{') => decode_json(body),
        Some(_) => wire::decode_list_pb(body),
    }
}

fn decode_json<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, StoreError> {
    serde_json::from_slice(body).map_err(|err| StoreError::Decode(err.to_string()))
}

/// A body that starts with the gzip magic is gunzipped, and nothing else is touched. The output is read up to
/// [`GUNZIP_LIMIT`] and then held to the document limit, so a bomb cannot fill memory.
fn gunzip_if_gzipped(body: Vec<u8>) -> Result<Vec<u8>, StoreError> {
    if !body.starts_with(&GZIP_MAGIC) {
        return Ok(body);
    }
    let mut out = Vec::new();
    GzDecoder::new(body.as_slice())
        .take(GUNZIP_LIMIT as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|err| StoreError::Decode(format!("the gzip body could not be read: {err}")))?;
    if out.len() > GUNZIP_LIMIT || out.len() > STORE_BODY_LIMIT {
        return Err(StoreError::TooLarge);
    }
    Ok(out)
}

/// One GET whose body is gunzipped when it needs it: the store, its extension list and its index.
async fn get_document(client: &reqwest::Client, url: &str, cancel: &CancellationToken) -> Result<Vec<u8>, StoreError> {
    gunzip_if_gzipped(get_body(client, url, cancel).await?)
}

/// One GET with `Cache-Control: no-cache`. A cancel wins over a request in flight and leaves no partial body. The body
/// is as it came, so a legacy repo and a legacy list are not gunzipped.
async fn get_body(client: &reqwest::Client, url: &str, cancel: &CancellationToken) -> Result<Vec<u8>, StoreError> {
    let fetch = async {
        let mut response = client
            .get(url)
            .header(CACHE_CONTROL, "no-cache")
            .send()
            .await
            .map_err(|err| StoreError::Network(err.to_string()))?;
        if !response.status().is_success() {
            return Err(StoreError::Status(response.status().as_u16()));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|err| StoreError::Network(err.to_string()))? {
            if body.len() + chunk.len() > STORE_BODY_LIMIT {
                return Err(StoreError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    };
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(StoreError::Cancelled),
        result = fetch => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::model::{AnimeSourceRef, Kind};
    use crate::anime::testkit::{
        MANIFEST, fixture_key, kuroext_block, signed_package, signer_hex, store_bytes, store_document, store_extension,
        test_client, test_net_at,
    };
    use prost::Message;
    use serde_json::{Value, json};
    use std::io::Write;
    use wire::{PbContact, PbExtension, PbExtensionList, PbResources, PbSource, PbStore};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn serve(server: &MockServer, route: &str, body: Vec<u8>) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn a_json_store_is_read_as_a_non_legacy_row_with_its_listing() {
        let server = MockServer::start().await;
        let key = signer_hex(&fixture_key());
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let block = kuroext_block(&fixture, &format!("{}/pkg/demo.kuroext", server.uri()));
        let document =
            store_document("Example Store", &key, vec![store_extension("eu.example.animeext.en.demo", 7, Some(block))]);
        serve(&server, "/index.json", store_bytes(&document)).await;

        let url = format!("{}/index.json", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.expect("fetches");
        assert!(!fetched.row.is_legacy);
        assert_eq!(fetched.row.index_url, url);
        assert_eq!(fetched.row.name, "Example Store");
        assert_eq!(fetched.row.signing_key, key);
        assert_eq!(fetched.extensions.len(), 1);
        assert_eq!(fetched.extensions[0].pkg, "eu.example.animeext.en.demo");
        assert_eq!(fetched.extensions[0].kind, Kind::Wasm);
    }

    #[tokio::test]
    async fn a_store_with_no_list_and_no_list_url_is_invalid() {
        let server = MockServer::start().await;
        let mut document = store_document("S", "k", Vec::new());
        document["extensionList"] = serde_json::Value::Null;
        serve(&server, "/index.json", store_bytes(&document)).await;
        let url = format!("{}/index.json", server.uri());
        let result = fetch_store(&test_client(), &url, &CancellationToken::new()).await;
        assert_eq!(result, Err(StoreError::Invalid("extensionList is missing".into())));
    }

    #[tokio::test]
    async fn the_extension_list_url_wins_over_the_inline_list() {
        let server = MockServer::start().await;
        let mut document = store_document("S", "k", vec![store_extension("p.inline", 1, None)]);
        document["extensionListUrl"] = serde_json::json!(format!("{}/list.json", server.uri()));
        serve(&server, "/index.json", store_bytes(&document)).await;
        let list = serde_json::json!({ "extensions": [store_extension("p.from.url", 2, None)] });
        serve(&server, "/list.json", serde_json::to_vec(&list).unwrap()).await;
        let url = format!("{}/index.json", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        assert_eq!(fetched.extensions[0].pkg, "p.from.url");
    }

    #[tokio::test]
    async fn a_null_discord_is_none_and_unknown_keys_are_ignored() {
        let server = MockServer::start().await;
        let mut document = store_document("S", "k", vec![store_extension("p.keys", 1, None)]);
        document["foo"] = serde_json::json!(1);
        document["contact"]["discord"] = serde_json::Value::Null;
        serve(&server, "/index.json", store_bytes(&document)).await;
        let url = format!("{}/index.json", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        assert_eq!(fetched.row.contact_discord, None);
    }

    #[tokio::test]
    async fn a_server_error_is_a_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
        let url = format!("{}/index.json", server.uri());
        let result = fetch_store(&test_client(), &url, &CancellationToken::new()).await;
        assert_eq!(result, Err(StoreError::Status(500)));
    }

    #[tokio::test]
    async fn a_body_over_the_limit_is_too_large() {
        let server = MockServer::start().await;
        let body = vec![b' '; STORE_BODY_LIMIT + 1];
        serve(&server, "/index.json", body).await;
        let url = format!("{}/index.json", server.uri());
        let result = fetch_store(&test_client(), &url, &CancellationToken::new()).await;
        assert_eq!(result, Err(StoreError::TooLarge));
    }

    #[tokio::test]
    async fn a_cancelled_fetch_returns_cancelled_and_no_row() {
        let server = MockServer::start().await;
        let document = store_document("S", "k", Vec::new());
        serve(&server, "/index.json", store_bytes(&document)).await;
        let cancel = CancellationToken::new();
        cancel.cancel();
        let url = format!("{}/index.json", server.uri());
        let result = fetch_store(&test_client(), &url, &cancel).await;
        assert_eq!(result, Err(StoreError::Cancelled));
    }

    #[test]
    fn the_first_byte_picks_the_format() {
        assert!(decode_store(b"{}").is_err(), "a JSON object that is not a store fails to decode");
        assert!(matches!(decode_store(&[0x0a, 0x01]), Err(StoreError::Decode(_))), "anything else is protobuf");
        assert!(matches!(decode_store(b""), Err(StoreError::Decode(_))), "an empty body is an error, not a panic");
        assert!(matches!(decode_list(b""), Err(StoreError::Decode(_))));
    }

    // ---- the shared client (T-19a-013) ----

    #[test]
    fn the_store_limits_are_the_kuro_net_values_with_a_30_second_call_timeout() {
        let limits = store_limits();
        assert_eq!(limits.call_timeout, Duration::from_secs(30));
        assert_eq!(limits.call_timeout, STORE_TIMEOUT);
        let shared = TransportLimits::default();
        assert_eq!(limits.connect_timeout, shared.connect_timeout);
        assert_eq!(limits.read_timeout, shared.read_timeout);
        assert_eq!(limits.max_redirects, shared.max_redirects);
    }

    #[tokio::test]
    async fn the_store_client_sends_the_network_user_agent_and_no_cache_and_shares_the_cookie_jar() {
        let dir = tempfile::tempdir().unwrap();
        let net = test_net_at(dir.path().join("cookies.json"));
        let server = MockServer::start().await;
        let document = store_document("S", "k", vec![store_extension("p.ua", 1, None)]);
        Mock::given(method("GET"))
            .and(path("/index.json"))
            .and(header("user-agent", net.default_user_agent().as_str()))
            .and(header("cache-control", "no-cache"))
            .respond_with(
                ResponseTemplate::new(200).set_body_bytes(store_bytes(&document)).insert_header("set-cookie", "sid=1"),
            )
            .mount(&server)
            .await;
        let url = format!("{}/index.json", server.uri());
        let client = http_client(&net).expect("builds");
        fetch_store(&client, &url, &CancellationToken::new()).await.expect("the mock matched the headers");
        let jar = net.cookies().get(&url.parse().unwrap());
        assert_eq!(jar.iter().map(|cookie| cookie.name()).collect::<Vec<_>>(), ["sid"], "the app jar got the cookie");
    }

    // ---- legacy stores (T-19a-016) ----

    fn legacy_repo(index_v2: Option<&str>) -> Value {
        let mut repo = json!({
            "meta": { "name": "Legacy Store", "website": "https://legacy.example", "signingKeyFingerprint": "ff00ee" }
        });
        if let Some(next) = index_v2 {
            repo["index_v2"] = json!(next);
        }
        repo
    }

    fn legacy_entry(name: &str, sources: Option<Value>) -> Value {
        let mut entry = json!({
            "name": name, "pkg": "eu.example.legacy", "apk": "legacy-v16.3.apk", "lang": "en",
            "code": 3, "version": "16.3", "nsfw": 1,
        });
        if let Some(sources) = sources {
            entry["sources"] = sources;
        }
        entry
    }

    fn json_bytes(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).unwrap()
    }

    #[tokio::test]
    async fn tst_19_010_a_legacy_list_fetches_repo_json_and_is_stored_as_legacy() {
        let server = MockServer::start().await;
        let sources = json!([{ "id": 5, "lang": "en", "name": "S", "baseUrl": "https://s" }]);
        let list = json!([legacy_entry("Aniyomi: With Sources", Some(sources)), legacy_entry("Aniyomi: No Sources", None)]);
        serve(&server, "/index.min.json", json_bytes(&list)).await;
        serve(&server, "/repo.json", json_bytes(&legacy_repo(None))).await;
        let url = format!("{}/index.min.json", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.expect("fetches");
        assert!(fetched.row.is_legacy);
        assert_eq!(fetched.row.index_url, format!("{}/repo.json", server.uri()));
        assert_eq!(fetched.row.name, "Legacy Store");
        assert_eq!(fetched.row.badge_label, "Legacy Store", "no shortName, so the name is the badge");
        assert_eq!(fetched.row.signing_key, "ff00ee");
        assert_eq!(fetched.row.contact_website, "https://legacy.example");
        assert_eq!(fetched.row.contact_discord, None);
        assert_eq!(fetched.row.extension_list_url, None);
        assert_eq!(fetched.extensions.len(), 2);
        let first = &fetched.extensions[0];
        assert_eq!(first.name, "With Sources");
        assert_eq!(first.apk_url, format!("{}/apk/legacy-v16.3.apk", server.uri()));
        assert_eq!(first.icon_url, format!("{}/icon/eu.example.legacy.png", server.uri()));
        assert_eq!((first.version_code, first.version_name.as_str(), first.aniyomi_lib), (3, "16.3", Some(16.0)));
        assert!(first.nsfw && !first.torrent);
        assert_eq!(first.kind, Kind::ApkOnly);
    }

    #[tokio::test]
    async fn tst_19_010_a_legacy_short_name_is_the_badge() {
        let server = MockServer::start().await;
        let mut repo = legacy_repo(None);
        repo["meta"]["shortName"] = json!("LS");
        serve(&server, "/repo.json", json_bytes(&repo)).await;
        serve(&server, "/index.min.json", json_bytes(&json!([]))).await;
        let url = format!("{}/repo.json", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.expect("fetches");
        assert_eq!(fetched.row.badge_label, "LS");
        assert_eq!(fetched.row.index_url, url, "a repo.json URL is stored as typed");
        assert!(fetched.extensions.is_empty());
    }

    #[tokio::test]
    async fn tst_19_011_a_json_array_at_another_url_is_not_a_valid_legacy_url() {
        let server = MockServer::start().await;
        serve(&server, "/y.json", json_bytes(&json!([]))).await;
        let url = format!("{}/y.json", server.uri());
        let result = fetch_store(&test_client(), &url, &CancellationToken::new()).await;
        assert_eq!(result, Err(StoreError::Invalid("Provided legacy store url is not valid".into())));
    }

    #[tokio::test]
    async fn tst_19_012_index_v2_redirects_to_the_store_and_the_final_url_is_stored() {
        let server = MockServer::start().await;
        let target = format!("{}/v2/index.json", server.uri());
        serve(&server, "/repo.json", json_bytes(&legacy_repo(Some(&target)))).await;
        let document = store_document("Redirected Store", "k", vec![store_extension("p.v2", 1, None)]);
        serve(&server, "/v2/index.json", store_bytes(&document)).await;
        let url = format!("{}/repo.json", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.expect("follows");
        assert_eq!(fetched.row.name, "Redirected Store");
        assert_eq!(fetched.row.index_url, target);
        assert!(!fetched.row.is_legacy);
        assert_eq!(fetched.extensions[0].pkg, "p.v2");
    }

    /// `repos` legacy documents in a chain, each redirecting to the next, and a store at the end.
    async fn serve_chain(server: &MockServer, repos: usize) -> String {
        for index in 0..repos {
            let next = if index + 1 == repos { "store.json".to_owned() } else { format!("c{}.json", index + 1) };
            let repo = legacy_repo(Some(&format!("{}/{next}", server.uri())));
            serve(server, &format!("/c{index}.json"), json_bytes(&repo)).await;
        }
        let end = store_document("End", "k", vec![store_extension("p.end", 1, None)]);
        serve(server, "/store.json", store_bytes(&end)).await;
        format!("{}/c0.json", server.uri())
    }

    #[tokio::test]
    async fn tst_19_013_four_redirects_succeed_and_a_fifth_is_a_redirect_loop() {
        let server = MockServer::start().await;
        let four = serve_chain(&server, MAX_REDIRECTS).await;
        let fetched = fetch_store(&test_client(), &four, &CancellationToken::new()).await.expect("five documents work");
        assert_eq!(fetched.row.name, "End");
        let server = MockServer::start().await;
        let five = serve_chain(&server, MAX_REDIRECTS + 1).await;
        let result = fetch_store(&test_client(), &five, &CancellationToken::new()).await;
        assert_eq!(result, Err(StoreError::RedirectLoop), "six documents do not");
    }

    #[tokio::test]
    async fn tst_19_013_a_cycle_is_a_redirect_loop_and_not_a_stack_overflow() {
        let server = MockServer::start().await;
        let (a, b) = (format!("{}/a.json", server.uri()), format!("{}/b.json", server.uri()));
        serve(&server, "/a.json", json_bytes(&legacy_repo(Some(&b)))).await;
        serve(&server, "/b.json", json_bytes(&legacy_repo(Some(&a)))).await;
        let result = fetch_store(&test_client(), &a, &CancellationToken::new()).await;
        assert_eq!(result, Err(StoreError::RedirectLoop));
    }

    #[tokio::test]
    async fn tst_19_025_a_legacy_entry_without_sources_gets_one_synthetic_source() {
        let server = MockServer::start().await;
        let list = json!([legacy_entry("Aniyomi: Null", None), legacy_entry("Aniyomi: Empty", Some(json!([])))]);
        serve(&server, "/index.min.json", json_bytes(&list)).await;
        serve(&server, "/repo.json", json_bytes(&legacy_repo(None))).await;
        let url = format!("{}/index.min.json", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        for (listed, raw) in fetched.extensions.iter().zip(["Aniyomi: Null", "Aniyomi: Empty"]) {
            let synthetic = AnimeSourceRef { id: 0, name: raw.into(), lang: "en".into(), base_url: String::new() };
            assert_eq!(listed.sources, [synthetic]);
        }
    }

    #[tokio::test]
    async fn tst_19_026_a_legacy_name_loses_its_prefix_only_when_it_has_one() {
        let server = MockServer::start().await;
        let list = json!([legacy_entry("Plain", None), legacy_entry("Aniyomi: Prefixed", None)]);
        serve(&server, "/index.min.json", json_bytes(&list)).await;
        serve(&server, "/repo.json", json_bytes(&legacy_repo(None))).await;
        let url = format!("{}/index.min.json", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        let names: Vec<&str> = fetched.extensions.iter().map(|listed| listed.name.as_str()).collect();
        assert_eq!(names, ["Plain", "Prefixed"]);
        assert_eq!(fetched.extensions[1].raw_name, "Aniyomi: Prefixed");
    }

    #[tokio::test]
    async fn a_legacy_version_without_a_number_before_the_last_dot_fails_the_store() {
        let server = MockServer::start().await;
        let mut entry = legacy_entry("Aniyomi: Bad", None);
        entry["version"] = json!("abc.3");
        serve(&server, "/index.min.json", json_bytes(&json!([entry]))).await;
        serve(&server, "/repo.json", json_bytes(&legacy_repo(None))).await;
        let url = format!("{}/index.min.json", server.uri());
        let result = fetch_store(&test_client(), &url, &CancellationToken::new()).await;
        assert!(matches!(result, Err(StoreError::Decode(_))), "{result:?}");
    }

    // ---- gzip (T-19a-015) ----

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[tokio::test]
    async fn tst_19_014_a_gzip_store_reads_the_same_as_a_plain_one() {
        let server = MockServer::start().await;
        let document = store_document("Zipped", "k", vec![store_extension("p.gz", 2, None)]);
        serve(&server, "/index.json", store_bytes(&document)).await;
        let url = format!("{}/index.json", server.uri());
        let plain = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        server.reset().await;
        serve(&server, "/index.json", gzip(&store_bytes(&document))).await;
        let zipped = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        assert_eq!(zipped, plain);
    }

    #[tokio::test]
    async fn tst_19_015_a_gzip_extension_list_reads_the_same_as_a_plain_one() {
        let server = MockServer::start().await;
        let mut document = store_document("S", "k", Vec::new());
        document["extensionListUrl"] = json!(format!("{}/list.json.gz", server.uri()));
        serve(&server, "/index.json", store_bytes(&document)).await;
        let list = json!({ "extensions": [store_extension("p.list.gz", 2, None)] });
        serve(&server, "/list.json.gz", gzip(&json_bytes(&list))).await;
        let url = format!("{}/index.json", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        assert_eq!(fetched.extensions[0].pkg, "p.list.gz");
    }

    #[tokio::test]
    async fn a_gzip_legacy_list_is_not_gunzipped_and_fails_to_parse() {
        let server = MockServer::start().await;
        serve(&server, "/index.min.json", gzip(&json_bytes(&json!([legacy_entry("Aniyomi: Z", None)])))).await;
        let row = legacy_store_row(&server);
        let result = fetch_extensions(&test_client(), &row, &CancellationToken::new()).await;
        assert!(matches!(result, Err(StoreError::Decode(_))), "{result:?}");
    }

    #[tokio::test]
    async fn a_gzip_legacy_repo_is_not_gunzipped_and_fails_to_parse() {
        let server = MockServer::start().await;
        serve(&server, "/index.min.json", json_bytes(&json!([]))).await;
        serve(&server, "/repo.json", gzip(&json_bytes(&legacy_repo(None)))).await;
        let url = format!("{}/index.min.json", server.uri());
        let result = fetch_store(&test_client(), &url, &CancellationToken::new()).await;
        assert!(matches!(result, Err(StoreError::Decode(_))), "{result:?}");
    }

    /// A gzip member of `size` zero bytes, written by hand: one fixed-Huffman block of a literal and then matches of
    /// 258 bytes at distance 1, so 200 MiB is about 1.3 MiB and building it is fast. The checksum is real only when
    /// `checked` is set. A reader that stops at a size limit never gets to it.
    fn gzip_zeros(size: usize, checked: bool) -> Vec<u8> {
        struct Bits {
            bytes: Vec<u8>,
            acc: u64,
            used: u32,
        }
        impl Bits {
            /// Writes the low `len` bits of `value`, lowest bit first.
            fn put(&mut self, value: u32, len: u32) {
                self.acc |= u64::from(value) << self.used;
                self.used += len;
                while self.used >= 8 {
                    self.bytes.push(self.acc as u8);
                    self.acc >>= 8;
                    self.used -= 8;
                }
            }
            /// Writes a Huffman code of `len` bits, highest bit first.
            fn code(&mut self, code: u32, len: u32) {
                self.put(code.reverse_bits() >> (32 - len), len);
            }
        }
        let mut bits = Bits { bytes: Vec::new(), acc: 0, used: 0 };
        bits.put(1, 1); // the last block
        bits.put(1, 2); // fixed Huffman codes
        bits.code(0x30, 8); // the literal 0
        let mut left = size - 1;
        while left >= 258 {
            bits.code(0xc5, 8); // length 258
            bits.code(0, 5); // distance 1
            left -= 258;
        }
        if left > 0 {
            // A length below 258 is the codes 257 to 264, which are lengths 3 to 10, so the tail is one literal each.
            for _ in 0..left {
                bits.code(0x30, 8);
            }
        }
        bits.code(0, 7); // end of block
        bits.put(0, 7); // pad to a byte
        let mut crc = flate2::Crc::new();
        let chunk = vec![0u8; 1024 * 1024];
        let mut remaining = if checked { size } else { 0 };
        while remaining > 0 {
            let take = remaining.min(chunk.len());
            crc.update(&chunk[..take]);
            remaining -= take;
        }
        let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff];
        out.extend_from_slice(&bits.bytes);
        out.extend_from_slice(&crc.sum().to_le_bytes());
        out.extend_from_slice(&(size as u32).to_le_bytes());
        out
    }

    #[test]
    fn the_hand_built_gzip_of_zeros_is_a_valid_member() {
        let body = gzip_zeros(1_000_003, true);
        let mut out = Vec::new();
        GzDecoder::new(body.as_slice()).read_to_end(&mut out).expect("a valid gzip member with a good checksum");
        assert_eq!(out.len(), 1_000_003);
        assert!(out.iter().all(|byte| *byte == 0));
    }

    #[tokio::test]
    async fn tst_19_029_a_gzip_bomb_is_too_large_after_the_gunzip_limit() {
        let server = MockServer::start().await;
        let bomb = gzip_zeros(200 * 1024 * 1024, false);
        assert!(bomb.len() < 2 * 1024 * 1024, "200 MiB of zeros is a small download, not {} bytes", bomb.len());
        serve(&server, "/index.json", bomb).await;
        let url = format!("{}/index.json", server.uri());
        let result = fetch_store(&test_client(), &url, &CancellationToken::new()).await;
        assert_eq!(result, Err(StoreError::TooLarge));
    }

    #[test]
    fn gunzip_output_over_the_document_limit_is_too_large_and_a_short_gzip_header_is_an_error() {
        assert_eq!(gunzip_if_gzipped(gzip_zeros(STORE_BODY_LIMIT + 1, true)), Err(StoreError::TooLarge));
        assert_eq!(gunzip_if_gzipped(gzip_zeros(STORE_BODY_LIMIT, true)).map(|out| out.len()), Ok(STORE_BODY_LIMIT));
        assert!(matches!(gunzip_if_gzipped(vec![0x1f, 0x8b, 0x08]), Err(StoreError::Decode(_))));
        assert_eq!(gunzip_if_gzipped(vec![0x1f]), Ok(vec![0x1f]), "one byte is not gzip");
        assert_eq!(gunzip_if_gzipped(gzip(b"{}")), Ok(b"{}".to_vec()));
    }

    // ---- protobuf (T-19a-014) ----

    fn pb_extension(pkg: &str, version_code: i64, content_warning: i32) -> PbExtension {
        PbExtension {
            name: "Aniyomi: Demo".into(),
            package_name: pkg.into(),
            resources: Some(PbResources {
                apk_url: "https://example.org/apk/demo.apk".into(),
                icon_url: "https://example.org/icon/demo.png".into(),
            }),
            extension_lib: "16".into(),
            version_code,
            version_name: "1.0.7".into(),
            content_warning,
            is_torrent: false,
            sources: vec![PbSource {
                id: crate::anime::testkit::SOURCE_ID,
                name: "Demo".into(),
                language: "en".into(),
                home_url: "https://example.org".into(),
                ..PbSource::default()
            }],
        }
    }

    fn pb_store(list: Option<Vec<PbExtension>>, list_url: Option<String>) -> PbStore {
        PbStore {
            name: "Example Store".into(),
            badge_label: "EX".into(),
            signing_key: "k".into(),
            contact: Some(PbContact { website: "https://example.org".into(), discord: None }),
            extension_list: list.map(|extensions| PbExtensionList { extensions }),
            extension_list_url: list_url,
        }
    }

    #[tokio::test]
    async fn tst_19_016_a_protobuf_store_decodes_to_the_same_row_and_listing_as_the_json_store() {
        let server = MockServer::start().await;
        let document = store_document("Example Store", "k", vec![store_extension("p.pb", 3, None)]);
        serve(&server, "/index", store_bytes(&document)).await;
        let url = format!("{}/index", server.uri());
        let from_json = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        server.reset().await;
        let store = pb_store(Some(vec![pb_extension("p.pb", 3, 1)]), None);
        serve(&server, "/index", store.encode_to_vec()).await;
        let from_pb = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        assert_eq!(from_pb, from_json);
    }

    #[tokio::test]
    async fn tst_19_016_a_gzip_protobuf_store_decodes_too() {
        let server = MockServer::start().await;
        let store = pb_store(Some(vec![pb_extension("p.pbgz", 3, 2)]), None);
        serve(&server, "/index", gzip(&store.encode_to_vec())).await;
        let url = format!("{}/index", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        assert!(fetched.extensions[0].nsfw, "contentWarning 2 is MIXED");
    }

    #[tokio::test]
    async fn tst_19_051_a_protobuf_list_behind_extension_list_url_is_decoded() {
        let server = MockServer::start().await;
        let list_url = format!("{}/list.pb", server.uri());
        serve(&server, "/index", pb_store(None, Some(list_url)).encode_to_vec()).await;
        let list = PbExtensionList { extensions: vec![pb_extension("p.pb.list", 4, 1)] };
        serve(&server, "/list.pb", list.encode_to_vec()).await;
        let url = format!("{}/index", server.uri());
        let fetched = fetch_store(&test_client(), &url, &CancellationToken::new()).await.unwrap();
        assert_eq!(fetched.row.extension_list_url.as_deref(), Some(format!("{}/list.pb", server.uri()).as_str()));
        assert_eq!(fetched.extensions.len(), 1);
        assert_eq!(fetched.extensions[0].pkg, "p.pb.list");
    }

    #[tokio::test]
    async fn tst_19_055_a_protobuf_content_warning_of_9_fails_the_whole_store_and_list() {
        let server = MockServer::start().await;
        let store = pb_store(Some(vec![pb_extension("p.ok", 1, 1), pb_extension("p.bad", 1, 9)]), None);
        serve(&server, "/index", store.encode_to_vec()).await;
        let url = format!("{}/index", server.uri());
        let result = fetch_store(&test_client(), &url, &CancellationToken::new()).await;
        assert!(matches!(result, Err(StoreError::Decode(text)) if text.contains("contentWarning 9")));
        let list = PbExtensionList { extensions: vec![pb_extension("p.bad", 1, 9)] };
        assert!(matches!(wire::decode_list_pb(&list.encode_to_vec()), Err(StoreError::Decode(_))));
    }

    // ---- a row that does not fit its URL (T-19a-022, T-19a-018) ----

    fn legacy_store_row(server: &MockServer) -> StoreRow {
        StoreRow {
            index_url: format!("{}/repo.json", server.uri()),
            name: "Legacy Store".into(),
            badge_label: "LS".into(),
            signing_key: "ff00ee".into(),
            contact_website: "https://legacy.example".into(),
            contact_discord: None,
            is_legacy: true,
            extension_list_url: None,
        }
    }

    #[tokio::test]
    async fn tst_19_049_a_migrated_row_with_a_repo_url_and_no_legacy_flag_fails_the_list_fetch() {
        let server = MockServer::start().await;
        serve(&server, "/repo.json", json_bytes(&legacy_repo(None))).await;
        let row = StoreRow { is_legacy: false, ..legacy_store_row(&server) };
        let result = fetch_extensions(&test_client(), &row, &CancellationToken::new()).await;
        assert!(matches!(result, Err(StoreError::Decode(_))), "a repo document is not a store: {result:?}");
    }

    #[tokio::test]
    async fn a_saved_legacy_row_lists_through_the_index_min_json_beside_its_repo() {
        let server = MockServer::start().await;
        serve(&server, "/index.min.json", json_bytes(&json!([legacy_entry("Aniyomi: Saved", None)]))).await;
        let row = legacy_store_row(&server);
        let listed = fetch_extensions(&test_client(), &row, &CancellationToken::new()).await.unwrap();
        assert_eq!(listed[0].store, row.store_ref());
        assert_eq!(listed[0].apk_url, format!("{}/apk/legacy-v16.3.apk", server.uri()));
    }
}
