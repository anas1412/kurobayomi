//! Image fetch with progress for a manga page (`HttpSource.kt` image call, `OkHttpExtensions.kt` progress listener;
//! behaviour 17; T-16a-054). The request is sent with caching off, the body reports `100 * read / length` as an integer
//! (or `-1` when the length is unknown), and the value is published on a `watch` channel, so the reader sees the
//! latest value in order. A progress call gets a 30 hour call timeout, as the Kotlin `callTimeout(30, HOURS)` does.

use std::sync::Arc;

use bytes::Bytes;
use http::HeaderMap;
use kuro_net::{CacheControl, ProgressListener};
use kuro_source::SourceError;
use tokio::sync::watch;

use crate::error::map_net_error;
use crate::response::HttpResponse;

/// Publishes the percent of an image read. `-1` when the total is unknown.
struct PercentProgress {
    tx: watch::Sender<i32>,
}

impl ProgressListener for PercentProgress {
    fn update(&self, bytes_read: u64, content_length: i64, _done: bool) {
        self.tx.send_replace(percent(bytes_read, content_length));
    }
}

/// `100 * read / length` as an integer, capped at 100. `-1` when the length is unknown or zero.
pub fn percent(bytes_read: u64, content_length: i64) -> i32 {
    match u64::try_from(content_length) {
        Ok(total) if total > 0 => i32::try_from((u128::from(bytes_read) * 100 / u128::from(total)).min(100)).unwrap_or(100),
        _ => -1,
    }
}

/// Fetches one page image. A non-2xx status is `Http(code)`. `progress` gets the percent as the body arrives.
pub async fn fetch_image(
    client: &kuro_net::NetworkClient,
    url: &str,
    headers: &HeaderMap,
    progress: watch::Sender<i32>,
) -> Result<Bytes, SourceError> {
    let request = kuro_net::request::get(url).map_err(map_net_error)?.headers(headers.clone()).cache(CacheControl::force_network());
    let request = kuro_net::client::progress_call(request, Arc::new(PercentProgress { tx: progress }));
    let response = client.execute(request).await.map_err(map_net_error)?;
    Ok(HttpResponse::read(response).await?.body)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::test_support::test_client;

    #[test]
    fn the_percent_is_read_over_length_and_minus_one_when_unknown() {
        assert_eq!(percent(50, 200), 25);
        assert_eq!(percent(200, 200), 100);
        assert_eq!(percent(10, -1), -1, "unknown length");
        assert_eq!(percent(10, 0), -1, "zero length is unknown");
        assert_eq!(percent(300, 200), 100, "never above 100");
    }

    #[tokio::test]
    async fn a_page_image_is_fetched_uncached_and_its_progress_ends_at_one_hundred() {
        let server = MockServer::start().await;
        let body = vec![7u8; 4096];
        Mock::given(method("GET"))
            .and(path("/page.jpg"))
            .and(header("cache-control", "no-cache"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;
        let (client, _dir) = test_client();
        let (tx, rx) = watch::channel(0);
        let url = format!("{}/page.jpg", server.uri());
        let bytes = tokio::time::timeout(Duration::from_secs(10), fetch_image(&client, &url, &HeaderMap::new(), tx))
            .await
            .expect("no hang")
            .expect("fetched");
        assert_eq!(bytes.as_ref(), body.as_slice());
        assert_eq!(*rx.borrow(), 100, "the last published value is complete");
    }

    #[tokio::test]
    async fn a_failed_status_is_an_http_error_and_the_body_is_not_read() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/gone.jpg")).respond_with(ResponseTemplate::new(404)).mount(&server).await;
        let (client, _dir) = test_client();
        let (tx, _rx) = watch::channel(0);
        let url = format!("{}/gone.jpg", server.uri());
        let err = fetch_image(&client, &url, &HeaderMap::new(), tx).await.unwrap_err();
        assert_eq!(err, SourceError::Http(404));
    }
}
