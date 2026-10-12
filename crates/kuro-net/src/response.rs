//! Response model (T-12a-008 `await_success`, T-12a-013 non-2xx mapping). The body is a byte stream; it is read
//! once and dropped with the response.

use bytes::{Bytes, BytesMut};
use futures::{StreamExt, stream::BoxStream};
use http::{HeaderMap, StatusCode, header::HeaderName};
use url::Url;

use crate::error::NetError;

/// The body as a stream of chunks. Dropping it releases the connection and any dispatcher permit.
pub type BodyStream = BoxStream<'static, Result<Bytes, NetError>>;

pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    /// The final URL after redirects.
    pub url: Url,
    /// `true` when the response came from a cache and no network I/O happened (OkHttp `networkResponse == null`).
    pub from_cache: bool,
    body: BodyStream,
}

impl Response {
    pub fn new(status: StatusCode, headers: HeaderMap, url: Url, body: BodyStream) -> Self {
        Self {
            status,
            headers,
            url,
            from_cache: false,
            body,
        }
    }

    /// The same response, marked as served from a cache.
    pub fn from_cache(mut self) -> Self {
        self.from_cache = true;
        self
    }

    /// `true` for 200 to 299, the Kotlin `isSuccessful` range.
    pub fn is_successful(&self) -> bool {
        self.status.is_success()
    }

    /// The last value of a header (OkHttp `Headers.get`), or `None`.
    pub fn header(&self, name: &HeaderName) -> Option<&str> {
        self.headers
            .get_all(name)
            .iter()
            .last()
            .and_then(|v| v.to_str().ok())
    }

    /// `awaitSuccess`: a status outside 200 to 299 becomes `NetError::Http`, and the body is dropped first.
    pub fn await_success(self) -> Result<Response, NetError> {
        if self.is_successful() {
            Ok(self)
        } else {
            let code = self.status.as_u16();
            drop(self);
            Err(NetError::Http { code })
        }
    }

    /// Reads the whole body into memory.
    pub async fn bytes(self) -> Result<Bytes, NetError> {
        self.read_limited(usize::MAX).await
    }

    /// The body as text. Invalid UTF-8 is replaced, so only a transport error can fail this.
    pub async fn text(self) -> Result<String, NetError> {
        let bytes = self.bytes().await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Decodes the body as JSON (`Response::json` in OkHttpExtensions.kt). Unknown fields are ignored, and a `null`
    /// for a field that is not an `Option` is an error. A body over [`MAX_DECODE_BYTES`] is `Decode`.
    pub async fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, NetError> {
        let body = self.read_limited(MAX_DECODE_BYTES).await?;
        serde_json::from_slice(&body).map_err(|e| NetError::Decode(e.to_string()))
    }

    /// Reads at most `limit` bytes. The body is dropped on every path, so the connection is released even when the
    /// read fails or the body is too large.
    async fn read_limited(self, limit: usize) -> Result<Bytes, NetError> {
        let mut body = self.body;
        let mut out = BytesMut::new();
        while let Some(chunk) = body.next().await {
            let chunk = chunk?;
            if out.len().saturating_add(chunk.len()) > limit {
                return Err(NetError::Decode("response too large".to_owned()));
            }
            out.extend_from_slice(&chunk);
        }
        Ok(out.freeze())
    }
}

/// The largest body [`Response::json`] and the other decoders read into memory.
pub const MAX_DECODE_BYTES: usize = 64 * 1024 * 1024;

/// A body that yields the given chunks. Used by tests and by code that builds responses itself.
pub fn body_from_chunks(chunks: &[&[u8]]) -> BodyStream {
    let owned: Vec<Result<Bytes, NetError>> = chunks
        .iter()
        .map(|c| Ok(Bytes::copy_from_slice(c)))
        .collect();
    futures::stream::iter(owned).boxed()
}

/// An empty response for `status` at `url`. Used by interceptor tests and by code that fakes a response.
pub fn empty_response(status: u16, url: Url) -> Response {
    Response::new(
        StatusCode::from_u16(status).expect("valid status"),
        HeaderMap::new(),
        url,
        futures::stream::empty().boxed(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url() -> Url {
        Url::parse("https://x.test/").unwrap()
    }

    #[test]
    fn two_hundred_range_is_successful_and_304_is_not() {
        assert!(empty_response(200, url()).is_successful());
        assert!(empty_response(299, url()).is_successful());
        assert!(!empty_response(304, url()).is_successful());
    }

    #[test]
    fn await_success_maps_non_2xx_to_http_error() {
        for code in [103u16, 304, 404, 503] {
            let err = empty_response(code, url()).await_success().err();
            assert!(matches!(err, Some(NetError::Http { code: c }) if c == code), "{code}");
        }
        assert!(empty_response(204, url()).await_success().is_ok());
    }

    #[tokio::test]
    async fn bytes_reads_every_chunk() {
        let mut resp = empty_response(200, url());
        resp.body = body_from_chunks(&[b"ab", b"cd"]);
        assert_eq!(resp.bytes().await.unwrap(), Bytes::from_static(b"abcd"));
    }

    /// A body of `chunks` that counts its drops, so a test can see whether the body was released.
    fn counted_body(chunks: Vec<Vec<u8>>, drops: std::sync::Arc<std::sync::atomic::AtomicUsize>) -> BodyStream {
        struct Guard(std::sync::Arc<std::sync::atomic::AtomicUsize>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let guard = Guard(drops);
        futures::stream::iter(chunks.into_iter().map(|c| Ok(Bytes::from(c))))
            .map(move |item| {
                let _ = &guard;
                item
            })
            .boxed()
    }

    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct Item {
        id: u32,
        name: String,
    }

    fn with_body(body: BodyStream) -> Response {
        Response::new(http::StatusCode::OK, HeaderMap::new(), url(), body)
    }

    #[tokio::test]
    async fn json_ignores_unknown_fields() {
        let mut resp = empty_response(200, url());
        resp.body = body_from_chunks(&[br#"{"id": 3, "name": "x", "extra": [1, 2]}"#]);
        let item: Item = resp.json().await.unwrap();
        assert_eq!(item, Item { id: 3, name: "x".into() });
    }

    #[tokio::test]
    async fn json_null_for_a_required_field_is_a_decode_error() {
        let mut resp = empty_response(200, url());
        resp.body = body_from_chunks(&[br#"{"id": null, "name": "x"}"#]);
        let err = resp.json::<Item>().await.unwrap_err();
        assert!(matches!(err, NetError::Decode(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_body_over_the_limit_is_a_decode_error_and_is_released() {
        let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let big = vec![b' '; MAX_DECODE_BYTES + 1];
        let err = with_body(counted_body(vec![big], drops.clone()))
            .json::<serde_json::Value>()
            .await
            .unwrap_err();
        assert!(matches!(&err, NetError::Decode(m) if m == "response too large"), "{err:?}");
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_body_is_released_on_success_and_on_a_decode_error() {
        let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let ok: Item = with_body(counted_body(vec![br#"{"id": 1, "name": "a"}"#.to_vec()], drops.clone()))
            .json()
            .await
            .unwrap();
        assert_eq!(ok.id, 1);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
        let bad = with_body(counted_body(vec![b"not json".to_vec()], drops.clone()))
            .json::<Item>()
            .await;
        assert!(bad.is_err());
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn text_replaces_invalid_utf8_instead_of_failing() {
        let mut resp = empty_response(200, url());
        resp.body = body_from_chunks(&[b"ok ", &[0xff, 0xfe]]);
        assert_eq!(resp.text().await.unwrap(), "ok \u{fffd}\u{fffd}");
    }

    #[test]
    fn header_returns_the_last_value() {
        let mut resp = empty_response(200, url());
        resp.headers.append("user-agent", "first".parse().unwrap());
        resp.headers.append("user-agent", "last".parse().unwrap());
        assert_eq!(resp.header(&http::header::USER_AGENT), Some("last"));
    }
}
