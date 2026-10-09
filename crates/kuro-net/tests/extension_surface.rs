//! Compile test of the extension-facing network surface (T-12a-029). This file sees the crate the way an extension
//! crate does: only the public paths are used.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use kuro_net::{Body, Chain, DEFAULT_RATE_PERIOD, Interceptor, NetError, NetworkClient, Request, Response, get, post};

/// An extension interceptor that adds a header to every request.
struct Tag;

#[async_trait]
impl Interceptor for Tag {
    async fn intercept(&self, chain: Chain<'_>, mut req: Request) -> Result<Response, NetError> {
        req.headers.insert("x-extension", http::HeaderValue::from_static("tag"));
        chain.proceed(req).await
    }
}

#[derive(serde::Deserialize)]
struct Ping {
    ok: bool,
}

/// The shape an extension source is written against. Nothing here reaches into crate internals.
async fn an_extension_source(client: &NetworkClient) -> Result<bool, NetError> {
    let extended = client
        .new_builder()
        .add_interceptor(Arc::new(Tag))
        .rate_limit(5, DEFAULT_RATE_PERIOD)?
        .rate_limit_host("https://api.example.test/", 2, Duration::from_secs(1))?
        .build();
    let _agent: String = extended.default_user_agent();
    let _form = post("https://api.example.test/form", Body::Empty)?;
    let ping: Ping = extended
        .execute(get("https://api.example.test/ping")?)
        .await?
        .await_success()?
        .json()
        .await?;
    Ok(ping.ok)
}

#[test]
fn the_extension_surface_compiles_and_is_reachable_from_the_crate_root() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory());
    let (toaster, _feed) = kuro_platform::toast::toast_channel();
    let client = NetworkClient::new(
        &prefs,
        dir.path().join("cookies.json"),
        Arc::new(kuro_platform::webview::NoWebView),
        toaster,
    )
    .unwrap();
    // The future is built to type-check the extension code and dropped unpolled: no request is made.
    drop(an_extension_source(&client));
    let _cache = kuro_net::CacheControl::force_network();
    let _progress: Option<Arc<dyn kuro_net::ProgressListener>> = None;
    let _limiter: Option<kuro_net::RateLimit> = None;
    assert_eq!(DEFAULT_RATE_PERIOD, Duration::from_secs(1));
}
