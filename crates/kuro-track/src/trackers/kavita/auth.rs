//! Kavita JWT table and request headers (`Kavita.kt:118-148`, `KavitaInterceptor.kt`, `KavitaModels.kt` OAuth table;
//! T-33a-047, T-33a-048).
//!
//! The URL and API key come from the Kavita sources' preferences, slots 1 to 3. Each slot is authenticated once and
//! kept with its JWT. A slot whose load fails is skipped and the next one still loads (D-05). A 401 clears the JWT of
//! one slot, and the next call authenticates that slot again (D-07).

use std::sync::Arc;

use reqwest::RequestBuilder;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use tokio::sync::Mutex;

use super::api::authenticate;
use crate::error::redact;
use crate::trackers::enhanced::EnhancedError;
use crate::trackers::http::USER_AGENT as APP_USER_AGENT;
use crate::trackers::sources::{SourceDirectory, generate_source_id};

/// Number of Kavita source slots (`Kavita.kt:118`, `for (id in 1..3)`).
pub const KAVITA_SLOTS: u32 = 3;

/// One authenticated server. `jwt` is empty after a 401 until the slot is authenticated again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub n: u32,
    pub api_url: String,
    pub jwt: String,
}

pub struct KavitaAuth {
    dir: Arc<dyn SourceDirectory>,
    http: reqwest::Client,
    /// `None` until the first load. The lock is held across the load, so one load runs at a time (E-08).
    table: Mutex<Option<Vec<Slot>>>,
}

impl KavitaAuth {
    pub fn new(dir: Arc<dyn SourceDirectory>, http: reqwest::Client) -> Self {
        Self {
            dir,
            http,
            table: Mutex::new(None),
        }
    }

    /// The JWT of the slot whose URL is `api_base`. The table loads on first use. A base that no slot has fails with
    /// `Unauthorized` and sends no request (E-01, E-12).
    pub async fn token_for(&self, api_base: &str) -> Result<String, EnhancedError> {
        let mut table = self.table.lock().await;
        if table.is_none() {
            let mut slots = Vec::new();
            for n in 1..=KAVITA_SLOTS {
                if let Some(slot) = self.login_slot(n).await {
                    slots.push(slot);
                }
            }
            *table = Some(slots);
        }
        let slots = table.get_or_insert_with(Vec::new);
        let slot = slots
            .iter_mut()
            .find(|s| s.api_url == api_base)
            .ok_or(EnhancedError::Unauthorized)?;
        if slot.jwt.is_empty() {
            *slot = self
                .login_slot(slot.n)
                .await
                .ok_or(EnhancedError::Unauthorized)?;
        }
        Ok(slot.jwt.clone())
    }

    /// Drops the JWT of the slot for `api_base`, so the next `token_for` authenticates it again (D-07, E-09).
    pub async fn invalidate(&self, api_base: &str) {
        let mut table = self.table.lock().await;
        if let Some(slots) = table.as_mut() {
            for slot in slots.iter_mut().filter(|s| s.api_url == api_base) {
                slot.jwt.clear();
            }
        }
    }

    /// Authenticates slot `n`. `None` (with a WARN, never the key) when the source is missing, not configured, refused
    /// or unreachable, or returned an empty token (E-02, E-03, E-04, E-05, E-06, E-10).
    async fn login_slot(&self, n: u32) -> Option<Slot> {
        let source_id = generate_source_id(&format!("kavita_{n}"), "all", 1);
        let Some(prefs) = self.dir.source_prefs(source_id) else {
            tracing::warn!("Kavita source slot {n} is not installed, skipping");
            return None;
        };
        let api_url = prefs.get_string("APIURL", "");
        let api_key = prefs.get_string("APIKEY", "");
        if api_url.is_empty() || api_key.is_empty() {
            return None;
        }
        match authenticate(&self.http, &api_url, &api_key).await {
            Ok(Some(jwt)) if !jwt.is_empty() => Some(Slot { n, api_url, jwt }),
            Ok(_) => None,
            Err(e) => {
                // The transport error text carries the request URL, which has the key in its query.
                tracing::warn!("Kavita slot {n} skipped: {}", redact(&e.to_string()));
                None
            }
        }
    }
}

/// Adds the bearer token and the app User-Agent to a request (`KavitaInterceptor.kt:23-24`). The token is required,
/// so no `Bearer null` header can be built.
pub fn authorize(req: RequestBuilder, jwt: &str) -> RequestBuilder {
    req.header(AUTHORIZATION, format!("Bearer {jwt}"))
        .header(USER_AGENT, APP_USER_AGENT)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::trackers::http::tracker_client;
    use crate::trackers::sources::memory::MemDirectory;

    const BODY: &str = r#"{"username":"u","token":"tok","apiKey":"k"}"#;

    fn auth(dir: MemDirectory) -> KavitaAuth {
        KavitaAuth::new(Arc::new(dir), tracker_client().unwrap())
    }

    /// Installs Kavita slot `n` with `base` as its APIURL.
    fn install(dir: &mut MemDirectory, n: u32, base: &str, key: &str) {
        dir.install(
            &format!("kavita_{n}"),
            "all",
            &[("APIURL", base), ("APIKEY", key)],
        );
    }

    async fn mount_auth(server: &MockServer, token_body: &str, times: Option<u64>) {
        let mock = Mock::given(method("POST"))
            .and(path("/api/Plugin/authenticate"))
            .and(query_param("pluginName", "Tachiyomi-Kavita"))
            .respond_with(ResponseTemplate::new(200).set_body_string(token_body.to_owned()));
        match times {
            Some(n) => mock.up_to_n_times(n).mount(server).await,
            None => mock.mount(server).await,
        }
    }

    #[test]
    fn t_33a_048_the_header_helper_sends_the_bearer_token_and_the_app_agent() {
        let client = tracker_client().unwrap();
        let req = authorize(client.get("http://127.0.0.1:1/x"), "abc")
            .build()
            .unwrap();
        assert_eq!(req.headers()[AUTHORIZATION], "Bearer abc");
        assert_eq!(req.headers()[USER_AGENT], APP_USER_AGENT);
    }

    #[tokio::test]
    async fn t_33a_047_e01_no_installed_source_fails_fast_without_a_request() {
        let auth = auth(MemDirectory::default());
        let err = auth.token_for("http://h/api").await.unwrap_err();
        assert!(matches!(err, EnhancedError::Unauthorized));
    }

    #[tokio::test]
    async fn t_33a_047_e02_a_refused_slot_does_not_block_the_next_slot() {
        let refused = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/Plugin/authenticate"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&refused)
            .await;
        let good = MockServer::start().await;
        mount_auth(&good, BODY, None).await;
        let mut dir = MemDirectory::default();
        install(&mut dir, 1, &format!("{}/api", refused.uri()), "k1");
        install(&mut dir, 2, &format!("{}/api", good.uri()), "k2");
        let auth = auth(dir);
        let jwt = auth
            .token_for(&format!("{}/api", good.uri()))
            .await
            .unwrap();
        assert_eq!(jwt, "tok");
    }

    #[tokio::test]
    async fn t_33a_047_e10_a_missing_slot_is_skipped_and_slot_three_loads() {
        let server = MockServer::start().await;
        mount_auth(&server, BODY, None).await;
        let mut dir = MemDirectory::default();
        install(&mut dir, 3, &format!("{}/api", server.uri()), "k3");
        let jwt = auth(dir)
            .token_for(&format!("{}/api", server.uri()))
            .await
            .unwrap();
        assert_eq!(jwt, "tok");
    }

    #[tokio::test]
    async fn t_33a_047_e04_a_server_error_skips_the_slot() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/Plugin/authenticate"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let mut dir = MemDirectory::default();
        install(&mut dir, 1, &format!("{}/api", server.uri()), "k");
        let err = auth(dir)
            .token_for(&format!("{}/api", server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, EnhancedError::Unauthorized));
    }

    #[tokio::test]
    async fn t_33a_047_e06_an_empty_token_skips_the_slot() {
        let server = MockServer::start().await;
        mount_auth(&server, r#"{"token":""}"#, None).await;
        let mut dir = MemDirectory::default();
        install(&mut dir, 1, &format!("{}/api", server.uri()), "k");
        let err = auth(dir)
            .token_for(&format!("{}/api", server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, EnhancedError::Unauthorized));
    }

    #[tokio::test]
    async fn t_33a_047_e12_an_unknown_base_fails_fast_after_the_load() {
        let server = MockServer::start().await;
        mount_auth(&server, BODY, None).await;
        let mut dir = MemDirectory::default();
        install(&mut dir, 1, &format!("{}/api", server.uri()), "k");
        let auth = auth(dir);
        let err = auth.token_for("http://other/api").await.unwrap_err();
        assert!(matches!(err, EnhancedError::Unauthorized));
    }

    #[tokio::test]
    async fn t_33a_047_e08_concurrent_first_use_authenticates_each_slot_once() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/Plugin/authenticate"))
            .respond_with(ResponseTemplate::new(200).set_body_string(BODY))
            .expect(1)
            .mount(&server)
            .await;
        let mut dir = MemDirectory::default();
        install(&mut dir, 1, &format!("{}/api", server.uri()), "k");
        let auth = Arc::new(auth(dir));
        let base = format!("{}/api", server.uri());
        let mut tasks = Vec::new();
        for _ in 0..32 {
            let (auth, base) = (Arc::clone(&auth), base.clone());
            tasks.push(tokio::spawn(async move { auth.token_for(&base).await }));
        }
        for task in tasks {
            assert_eq!(task.await.unwrap().unwrap(), "tok");
        }
        // `expect(1)` is verified when `server` is dropped.
    }

    #[tokio::test]
    async fn t_33a_047_invalidate_forces_one_new_login_for_that_slot() {
        let server = MockServer::start().await;
        mount_auth(&server, r#"{"token":"old"}"#, Some(1)).await;
        mount_auth(&server, r#"{"token":"new"}"#, None).await;
        let mut dir = MemDirectory::default();
        install(&mut dir, 1, &format!("{}/api", server.uri()), "k");
        let auth = auth(dir);
        let base = format!("{}/api", server.uri());
        assert_eq!(auth.token_for(&base).await.unwrap(), "old");
        auth.invalidate(&base).await;
        assert_eq!(auth.token_for(&base).await.unwrap(), "new");
    }

    #[tokio::test]
    async fn t_33a_047_the_key_is_query_encoded_and_sent_as_apikey() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/Plugin/authenticate"))
            .and(query_param("apiKey", "a&b=c d"))
            .and(header("content-type", "application/json; charset=utf-8"))
            .respond_with(ResponseTemplate::new(200).set_body_string(BODY))
            .expect(1)
            .mount(&server)
            .await;
        let mut dir = MemDirectory::default();
        install(&mut dir, 1, &format!("{}/api", server.uri()), "a&b=c d");
        let base = format!("{}/api", server.uri());
        assert_eq!(auth(dir).token_for(&base).await.unwrap(), "tok");
    }
}
