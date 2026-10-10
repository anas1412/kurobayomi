//! The HTTP cache end to end (T-12a-027): a fake server counts the calls that reach the network.

use std::sync::Arc;

use kuro_net::{NetworkClient, get};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(dir: &std::path::Path) -> NetworkClient {
    let prefs = kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory());
    let (toaster, _feed) = kuro_platform::toast::toast_channel();
    NetworkClient::new(
        &prefs,
        dir.join("cookies.json"),
        Arc::new(kuro_platform::webview::NoWebView),
        toaster,
    )
    .unwrap()
    .with_disk_cache(dir.join("network_cache"))
    .unwrap()
}

async fn body_of(client: &NetworkClient, url: &str) -> (String, bool) {
    let resp = client
        .execute(get(url).unwrap())
        .await
        .unwrap()
        .await_success()
        .unwrap();
    let from_cache = resp.from_cache;
    (resp.text().await.unwrap(), from_cache)
}

#[tokio::test]
async fn a_fresh_response_is_served_from_the_cache_without_a_second_call() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/list"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("first")
                .insert_header("cache-control", "max-age=60"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    let url = format!("{}/list", server.uri());
    assert_eq!(body_of(&client, &url).await, ("first".to_owned(), false));
    assert_eq!(body_of(&client, &url).await, ("first".to_owned(), true));
}

#[tokio::test]
async fn a_request_without_the_cache_never_reads_or_writes_it() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bulk"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("x")
                .insert_header("cache-control", "max-age=60"),
        )
        .expect(2)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    let url = format!("{}/bulk", server.uri());
    let mut req = get(&url).unwrap();
    req.opts.use_cache = false;
    client.execute(req).await.unwrap().await_success().unwrap();
    let mut again = get(&url).unwrap();
    again.opts.use_cache = false;
    client
        .execute(again)
        .await
        .unwrap()
        .await_success()
        .unwrap();
}

#[tokio::test]
async fn a_stale_entry_is_revalidated_and_a_304_keeps_the_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/etag"))
        .and(header("if-none-match", "\"v1\""))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/etag"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("kept")
                .insert_header("etag", "\"v1\""),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    let url = format!("{}/etag", server.uri());
    assert_eq!(body_of(&client, &url).await, ("kept".to_owned(), false));
    assert_eq!(body_of(&client, &url).await, ("kept".to_owned(), false));
}

#[tokio::test]
async fn a_write_removes_the_cached_entry_for_its_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/item"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("v")
                .insert_header("cache-control", "max-age=60"),
        )
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/item"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    let url = format!("{}/item", server.uri());
    body_of(&client, &url).await;
    client.post(&url, kuro_net::Body::Empty).await.unwrap();
    assert_eq!(body_of(&client, &url).await, ("v".to_owned(), false));
}
