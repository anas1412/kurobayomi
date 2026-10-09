//! Host imports for guests: `http`, `pref-get`, `pref-set`, `log` and the allow-list check (area 18).
//!
//! The guest's `http` import is served by the app's `NetworkClient`, so cookies, user agent and rate limits are the
//! same as for built-in sources. The import is synchronous: it runs on the blocking thread of the guest call and
//! waits on the runtime handle, so it must never run on an async worker.

use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use bytes::Bytes;
use http::header::CONTENT_TYPE;
use http::{HeaderMap, HeaderName, HeaderValue};
use kuro_net::request::{self, Body};
use kuro_net::{NetworkClient, Request};
use kuro_prefs::store::PrefStore;
use kuro_prefs::values::PrefValue;
use kuro_source::prefs::preference_key;
use serde::{Deserialize, Serialize};
use url::Url;
use wasmtime::StoreLimits;

use crate::bindings::kuro::source::host::Host;

/// The state of one guest store: who the source is, what it may reach, and its memory limit.
pub struct HostState {
    pub source_id: i64,
    /// The manifest `hosts` allow-list. Empty means no network.
    pub hosts: Vec<String>,
    pub client: NetworkClient,
    pub prefs: PrefStore,
    pub limits: StoreLimits,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HttpRequest {
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<(String, String)>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
    /// Present only when `body` is base64 of a non-UTF-8 payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    body_encoding: Option<&'static str>,
}

/// The allow-list rule. An entry is a host name matched exactly, or `*.<suffix>` matching any subdomain of the suffix
/// (not the bare suffix). Matching is case-insensitive. A URL without a host never matches.
pub fn check_host(hosts: &[String], url: &Url) -> Result<(), String> {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host.is_empty() {
        return Err("host not allowed: (none)".to_owned());
    }
    if hosts.iter().any(|entry| host_matches(entry, &host)) {
        Ok(())
    } else {
        Err(format!("host not allowed: {host}"))
    }
}

fn host_matches(entry: &str, host: &str) -> bool {
    let entry = entry.to_ascii_lowercase();
    match entry.strip_prefix("*.") {
        Some(suffix) => host.ends_with(&format!(".{suffix}")),
        None => host == entry,
    }
}

/// Parses the guest's request JSON, applies the allow-list, and builds the client request.
fn parse_request(json: &str, hosts: &[String]) -> Result<Request, String> {
    let req: HttpRequest = serde_json::from_str(json).map_err(|e| format!("invalid request: {e}"))?;
    let url = Url::parse(&req.url).map_err(|e| format!("invalid url: {e}"))?;
    check_host(hosts, &url)?;

    let mut headers = HeaderMap::new();
    for (name, value) in &req.headers {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| format!("invalid header name: {name}"))?;
        let value = HeaderValue::from_str(value).map_err(|_| format!("invalid header value for {name}"))?;
        headers.append(name, value);
    }
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let body = match req.body {
        None => Body::Empty,
        Some(text) => Body::Bytes {
            content_type,
            data: Bytes::from(text),
        },
    };
    let raw = url.as_str();
    let request = match req.method.to_ascii_uppercase().as_str() {
        "GET" if matches!(body, Body::Empty) => request::get(raw),
        "GET" => return Err("GET cannot carry a body".to_owned()),
        "POST" => request::post(raw, body),
        "PUT" => request::put(raw, body),
        "DELETE" => request::delete(raw, body),
        other => return Err(format!("unsupported method: {other}")),
    };
    Ok(request.map_err(|e| e.to_string())?.headers(headers))
}

/// Runs the request and returns the response JSON. Non-UTF-8 bodies are sent as base64.
///
/// The transport follows redirects, so the allow-list is checked again on the final URL and a response from a host
/// outside it is dropped unread. ponytail: only the final hop is visible here. A per-hop check needs a redirect hook
/// in kuro-net, which this crate cannot add.
async fn send(client: NetworkClient, request: Request, hosts: &[String]) -> Result<String, String> {
    let response = client.execute(request).await.map_err(|e| e.to_string())?;
    check_host(hosts, &response.url)?;
    let status = response.status.as_u16();
    let headers = response
        .headers
        .iter()
        .filter_map(|(name, value)| value.to_str().ok().map(|v| (name.as_str().to_owned(), v.to_owned())))
        .collect();
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let (body, body_encoding) = match std::str::from_utf8(&bytes) {
        Ok(text) => (text.to_owned(), None),
        Err(_) => (BASE64_STANDARD.encode(&bytes), Some("base64")),
    };
    serde_json::to_string(&HttpResponse {
        status,
        headers,
        body,
        body_encoding,
    })
    .map_err(|e| e.to_string())
}

impl HostState {
    /// The key a source preference is stored under: the source namespace, then the guest's key.
    fn pref_key(&self, key: &str) -> String {
        format!("{}.{key}", preference_key(self.source_id))
    }
}

impl Host for HostState {
    fn http(&mut self, request_json: String) -> Result<String, String> {
        let request = parse_request(&request_json, &self.hosts)?;
        let handle = tokio::runtime::Handle::try_current().map_err(|e| format!("no async runtime: {e}"))?;
        handle.block_on(send(self.client.clone(), request, &self.hosts))
    }

    fn pref_get(&mut self, key: String) -> String {
        match self.prefs.get_raw(&self.pref_key(&key)) {
            Some(PrefValue::Str(value)) => value,
            _ => String::new(),
        }
    }

    fn pref_set(&mut self, key: String, value_json: String) -> Result<(), String> {
        serde_json::from_str::<serde_json::Value>(&value_json).map_err(|e| format!("value is not JSON: {e}"))?;
        self.prefs.set_raw(&self.pref_key(&key), PrefValue::Str(value_json));
        Ok(())
    }

    fn log(&mut self, level: u8, message: String) {
        let source = self.source_id;
        match level {
            0 => tracing::debug!(source, "{message}"),
            2 => tracing::warn!(source, "{message}"),
            3 => tracing::error!(source, "{message}"),
            _ => tracing::info!(source, "{message}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn hosts(list: &[&str]) -> Vec<String> {
        list.iter().map(|h| (*h).to_owned()).collect()
    }

    #[test]
    fn an_exact_entry_matches_only_that_host() {
        let allowed = hosts(&["api.example.com"]);
        assert!(check_host(&allowed, &Url::parse("https://api.example.com/x").unwrap()).is_ok());
        assert!(check_host(&allowed, &Url::parse("https://API.example.com/x").unwrap()).is_ok());
        assert!(check_host(&allowed, &Url::parse("https://cdn.example.com/x").unwrap()).is_err());
    }

    #[test]
    fn a_wildcard_entry_matches_subdomains_but_not_the_bare_suffix() {
        let allowed = hosts(&["*.example.com"]);
        assert!(check_host(&allowed, &Url::parse("https://a.b.example.com/").unwrap()).is_ok());
        assert!(check_host(&allowed, &Url::parse("https://example.com/").unwrap()).is_err());
        assert!(check_host(&allowed, &Url::parse("https://evilexample.com/").unwrap()).is_err());
    }

    #[test]
    fn the_allow_list_does_not_restrict_the_scheme() {
        let allowed = hosts(&["ok.test"]);
        assert!(check_host(&allowed, &Url::parse("http://ok.test/x").unwrap()).is_ok());
        assert!(check_host(&allowed, &Url::parse("https://ok.test/x").unwrap()).is_ok());
    }

    #[test]
    fn a_disallowed_host_is_named_in_the_error() {
        let err = check_host(&hosts(&["ok.test"]), &Url::parse("https://evil.test/x").unwrap()).unwrap_err();
        assert_eq!(err, "host not allowed: evil.test");
    }

    #[test]
    fn an_empty_allow_list_refuses_every_host() {
        assert!(check_host(&[], &Url::parse("https://ok.test/").unwrap()).is_err());
    }

    /// The refusal text of a request that must not be built.
    fn refusal(json: &str, allowed: &[&str]) -> String {
        match parse_request(json, &hosts(allowed)) {
            Err(text) => text,
            Ok(_) => panic!("the request was built, but it should be refused: {json}"),
        }
    }

    #[test]
    fn a_request_to_a_disallowed_host_is_refused_before_any_network_io() {
        let json = r#"{"method":"GET","url":"https://evil.test/x","headers":[],"body":null}"#;
        assert_eq!(refusal(json, &["ok.test"]), "host not allowed: evil.test");
    }

    #[test]
    fn a_request_with_a_body_on_get_is_refused() {
        let json = r#"{"method":"GET","url":"https://ok.test/x","headers":[],"body":"x"}"#;
        assert_eq!(refusal(json, &["ok.test"]), "GET cannot carry a body");
    }

    #[test]
    fn a_malformed_request_is_an_error_not_a_panic() {
        assert!(refusal("not json", &["ok.test"]).starts_with("invalid request"));
        let json = r#"{"method":"GET","url":"nope","headers":[],"body":null}"#;
        assert!(refusal(json, &["ok.test"]).starts_with("invalid url"));
    }

    #[test]
    fn an_allowed_request_builds_the_method_url_and_headers() {
        let json = r#"{"method":"post","url":"https://ok.test/login","headers":[["X-Token","t"]],"body":"a=1"}"#;
        let request = parse_request(json, &hosts(&["ok.test"])).expect("allowed");
        assert_eq!(request.method.as_str(), "POST");
        assert_eq!(request.url.as_str(), "https://ok.test/login");
        assert_eq!(request.headers.get("x-token").and_then(|v| v.to_str().ok()), Some("t"));
        assert!(
            matches!(request.body, Body::Bytes { ref content_type, .. } if content_type == "application/octet-stream")
        );
    }

    /// A host state for one source, with the shared test environment. The directory must outlive the state.
    fn state(source_id: i64, allowed: &[&str]) -> (HostState, tempfile::TempDir) {
        let (env, dir) = crate::test_guest::env();
        let host = HostState {
            source_id,
            hosts: hosts(allowed),
            client: env.client,
            prefs: env.prefs,
            limits: crate::limits::store_limits(),
        };
        (host, dir)
    }

    #[test]
    fn a_preference_is_stored_under_its_own_source_only() {
        let (mut first, _dir) = state(5, &[]);
        assert_eq!(first.pref_get("sort".into()), "");
        first
            .pref_set("sort".into(), r#"{"by":"name"}"#.into())
            .expect("valid JSON");
        assert_eq!(first.pref_get("sort".into()), r#"{"by":"name"}"#);

        let mut other = HostState {
            source_id: 6,
            hosts: Vec::new(),
            client: first.client.clone(),
            prefs: first.prefs.clone(),
            limits: crate::limits::store_limits(),
        };
        assert_eq!(
            other.pref_get("sort".into()),
            "",
            "another source does not see the value"
        );
    }

    #[test]
    fn a_preference_value_must_be_json() {
        let (mut host, _dir) = state(5, &[]);
        assert!(host.pref_set("k".into(), "not json".into()).is_err());
        assert_eq!(host.pref_get("k".into()), "", "a refused write stores nothing");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_allowed_request_is_sent_and_its_response_comes_back_as_json() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/hello"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("x-reply", "yes")
                    .set_body_string("hi"),
            )
            .mount(&server)
            .await;
        let (mut host, _dir) = state(1, &["127.0.0.1"]);
        let request = json!({ "method": "GET", "url": format!("{}/hello", server.uri()), "headers": [["Accept", "text/plain"]], "body": null })
            .to_string();
        let response = tokio::task::spawn_blocking(move || host.http(request))
            .await
            .expect("the call finished")
            .expect("the request was sent");
        let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");
        assert_eq!(value["status"], 200);
        assert_eq!(value["body"], "hi");
        assert!(value.get("bodyEncoding").is_none(), "UTF-8 bodies are sent as text");
        assert!(value["headers"].to_string().contains("x-reply"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_non_utf8_body_is_sent_as_base64_and_an_error_status_is_not_a_host_error() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/bin"))
            .respond_with(wiremock::ResponseTemplate::new(404).set_body_bytes(vec![0xff_u8, 0x00, 0x41]))
            .mount(&server)
            .await;
        let (mut host, _dir) = state(1, &["127.0.0.1"]);
        let request =
            json!({ "method": "GET", "url": format!("{}/bin", server.uri()), "headers": [], "body": null }).to_string();
        let response = tokio::task::spawn_blocking(move || host.http(request))
            .await
            .expect("the call finished")
            .expect("the guest sees the status, the host does not fail");
        let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");
        assert_eq!(value["status"], 404);
        assert_eq!(value["bodyEncoding"], "base64");
        assert_eq!(value["body"], "/wBB");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_request_to_a_host_outside_the_allow_list_never_reaches_the_network() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(wiremock::ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let (mut host, _dir) = state(1, &["api.elsewhere.test"]);
        let request = json!({ "method": "GET", "url": server.uri(), "headers": [], "body": null }).to_string();
        let refusal = tokio::task::spawn_blocking(move || host.http(request))
            .await
            .expect("the call finished")
            .expect_err("refused");
        assert_eq!(refusal, "host not allowed: 127.0.0.1");
        server.verify().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_redirect_to_a_host_outside_the_allow_list_is_refused_and_its_body_not_returned() {
        let server = wiremock::MockServer::start().await;
        let port = server.address().port();
        wiremock::Mock::given(wiremock::matchers::path("/go"))
            .respond_with(
                wiremock::ResponseTemplate::new(302).insert_header("location", format!("http://127.0.0.1:{port}/final")),
            )
            .mount(&server)
            .await;
        wiremock::Mock::given(wiremock::matchers::path("/final"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("leaked"))
            .mount(&server)
            .await;
        let (mut host, _dir) = state(1, &["localhost"]);
        let request =
            json!({ "method": "GET", "url": format!("http://localhost:{port}/go"), "headers": [], "body": null })
                .to_string();
        let refusal = tokio::task::spawn_blocking(move || host.http(request))
            .await
            .expect("the call finished")
            .expect_err("the cross-host redirect is refused");
        assert_eq!(refusal, "host not allowed: 127.0.0.1");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_redirect_within_the_allow_list_is_followed() {
        let server = wiremock::MockServer::start().await;
        let port = server.address().port();
        wiremock::Mock::given(wiremock::matchers::path("/go"))
            .respond_with(wiremock::ResponseTemplate::new(302).insert_header("location", "/final"))
            .mount(&server)
            .await;
        wiremock::Mock::given(wiremock::matchers::path("/final"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;
        let (mut host, _dir) = state(1, &["localhost"]);
        let request =
            json!({ "method": "GET", "url": format!("http://localhost:{port}/go"), "headers": [], "body": null })
                .to_string();
        let response = tokio::task::spawn_blocking(move || host.http(request))
            .await
            .expect("the call finished")
            .expect("same-host redirect allowed");
        let value: serde_json::Value = serde_json::from_str(&response).expect("response JSON");
        assert_eq!(value["body"], "ok");
    }
}
