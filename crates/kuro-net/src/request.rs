//! Request model and helpers (`Requests.kt`, T-12a-002): `GET`, `POST`, `PUT`, `DELETE`, `CacheControl`.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http::header::{CACHE_CONTROL, HeaderMap, HeaderValue};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::error::NetError;
use crate::progress::ProgressListener;

/// The HTTP methods the app uses (no `PATCH`, OkHttp helpers have none either).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
        }
    }
}

/// A request body. `Empty` is Kotlin's `DEFAULT_BODY`, an empty form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Empty,
    Form(Vec<(String, String)>),
    Bytes { content_type: String, data: Bytes },
}

/// `Cache-Control` request directives (OkHttp `CacheControl`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheControl {
    pub no_cache: bool,
    pub no_store: bool,
    pub max_age: Option<u32>,
    pub max_stale: Option<u32>,
    pub min_fresh: Option<u32>,
    pub only_if_cached: bool,
}

/// `Int.MAX_VALUE` seconds, the `max-stale` of `FORCE_CACHE`.
const MAX_STALE_FOREVER: u32 = i32::MAX as u32;

impl CacheControl {
    /// No directives: no `Cache-Control` header is sent.
    pub fn none() -> Self {
        Self::default()
    }

    /// `max-age=600`, the default of every request helper (`DEFAULT_CACHE_CONTROL`).
    pub fn default_request() -> Self {
        Self {
            max_age: Some(600),
            ..Self::none()
        }
    }

    /// `no-cache`.
    pub fn force_network() -> Self {
        Self {
            no_cache: true,
            ..Self::none()
        }
    }

    /// `only-if-cached, max-stale=2147483647`.
    pub fn force_cache() -> Self {
        Self {
            only_if_cached: true,
            max_stale: Some(MAX_STALE_FOREVER),
            ..Self::none()
        }
    }

    /// The header value, or `None` when there are no directives (the header is then not sent).
    pub fn to_header(&self) -> Option<HeaderValue> {
        let mut parts: Vec<String> = Vec::new();
        if self.no_cache {
            parts.push("no-cache".into());
        }
        if self.no_store {
            parts.push("no-store".into());
        }
        if let Some(v) = self.max_age {
            parts.push(format!("max-age={v}"));
        }
        if self.only_if_cached {
            parts.push("only-if-cached".into());
        }
        if let Some(v) = self.max_stale {
            parts.push(format!("max-stale={v}"));
        }
        if let Some(v) = self.min_fresh {
            parts.push(format!("min-fresh={v}"));
        }
        if parts.is_empty() {
            return None;
        }
        HeaderValue::from_str(&parts.join(", ")).ok()
    }
}

/// Per-call options. `timeout` overrides the 2 minute call timeout; the progress call sets 30 hours.
/// `skip_rate_limit` lets the call past the rate limiter without a permit (an unmetered source, T-16a-037).
#[derive(Clone)]
pub struct CallOptions {
    pub timeout: Option<Duration>,
    pub use_cache: bool,
    pub progress: Option<Arc<dyn ProgressListener>>,
    pub cancel: CancellationToken,
    pub skip_rate_limit: bool,
}

impl Default for CallOptions {
    fn default() -> Self {
        Self {
            timeout: None,
            use_cache: true,
            progress: None,
            cancel: CancellationToken::new(),
            skip_rate_limit: false,
        }
    }
}

/// One outgoing request. `headers` never holds a `Cache-Control` that disagrees with `cache`.
#[derive(Clone)]
pub struct Request {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Body,
    pub cache: CacheControl,
    pub opts: CallOptions,
}

impl Request {
    fn new(method: Method, url: Url, body: Body) -> Self {
        let mut req = Self {
            method,
            url,
            headers: HeaderMap::new(),
            body,
            cache: CacheControl::default_request(),
            opts: CallOptions::default(),
        };
        req.sync_cache_header();
        req
    }

    /// Replaces every header (OkHttp `.headers(h)`). The `cache` setting still decides `Cache-Control`.
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self.sync_cache_header();
        self
    }

    /// Sets the cache directives (OkHttp `.cacheControl(c)`). This overrides a `Cache-Control` in the headers.
    pub fn cache(mut self, cache: CacheControl) -> Self {
        self.cache = cache;
        self.sync_cache_header();
        self
    }

    fn sync_cache_header(&mut self) {
        self.headers.remove(CACHE_CONTROL);
        if let Some(value) = self.cache.to_header() {
            self.headers.insert(CACHE_CONTROL, value);
        }
    }
}

/// Parses a URL the way the Kotlin helpers do: trim ASCII whitespace, then require `http` or `https`.
/// `ws` and `wss` are rewritten to `http` and `https` only when `rewrite_websocket` is set (POST, PUT, DELETE).
pub fn parse_http_url(raw: &str, rewrite_websocket: bool) -> Result<Url, NetError> {
    let trimmed = raw.trim_matches(|c: char| c.is_ascii_whitespace());
    let mut url = Url::parse(trimmed).map_err(|e| NetError::InvalidUrl(e.to_string()))?;
    if rewrite_websocket {
        let rewritten = match url.scheme() {
            "ws" => Some("http"),
            "wss" => Some("https"),
            _ => None,
        };
        if let Some(scheme) = rewritten {
            url.set_scheme(scheme)
                .map_err(|()| NetError::InvalidUrl(format!("cannot rewrite scheme of {trimmed}")))?;
        }
    }
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(NetError::InvalidUrl(format!("unsupported scheme {other}"))),
    }
}

/// `GET(url)`: the default cache control and an empty body.
pub fn get(url: &str) -> Result<Request, NetError> {
    Ok(Request::new(Method::Get, parse_http_url(url, false)?, Body::Empty))
}

/// `POST(url, body)`.
pub fn post(url: &str, body: Body) -> Result<Request, NetError> {
    Ok(Request::new(Method::Post, parse_http_url(url, true)?, body))
}

/// `PUT(url, body)`.
pub fn put(url: &str, body: Body) -> Result<Request, NetError> {
    Ok(Request::new(Method::Put, parse_http_url(url, true)?, body))
}

/// `DELETE(url, body)`.
pub fn delete(url: &str, body: Body) -> Result<Request, NetError> {
    Ok(Request::new(Method::Delete, parse_http_url(url, true)?, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cache_control_serialises_to_max_age_600() {
        let req = get("https://x.test/").unwrap();
        assert_eq!(req.headers.get(CACHE_CONTROL).unwrap(), "max-age=600");
    }

    #[test]
    fn force_network_is_no_cache_and_force_cache_is_only_if_cached_with_max_stale() {
        assert_eq!(
            CacheControl::force_network().to_header().unwrap(),
            "no-cache"
        );
        assert_eq!(
            CacheControl::force_cache().to_header().unwrap(),
            "only-if-cached, max-stale=2147483647"
        );
    }

    #[test]
    fn an_empty_cache_control_sends_no_header() {
        let req = get("https://x.test/").unwrap().cache(CacheControl::none());
        assert!(req.headers.get(CACHE_CONTROL).is_none());
    }

    #[test]
    fn post_rewrites_the_websocket_scheme_to_http() {
        let req = post("ws://x.test/a", Body::Empty).unwrap();
        assert_eq!(req.url.scheme(), "http");
        assert_eq!(req.url.as_str(), "http://x.test/a");
    }

    #[test]
    fn get_does_not_rewrite_websocket_and_rejects_ftp() {
        assert!(matches!(
            get("ws://x.test/"),
            Err(NetError::InvalidUrl(_))
        ));
        assert!(matches!(
            get("ftp://x.test/"),
            Err(NetError::InvalidUrl(msg)) if msg == "unsupported scheme ftp"
        ));
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let req = get("  https://x.test/ ").unwrap();
        assert_eq!(req.url.as_str(), "https://x.test/");
    }

    #[test]
    fn explicit_cache_overrides_a_cache_control_header() {
        let mut headers = HeaderMap::new();
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=5"));
        let req = get("https://x.test/")
            .unwrap()
            .headers(headers)
            .cache(CacheControl::force_network());
        assert_eq!(req.headers.get_all(CACHE_CONTROL).iter().count(), 1);
        assert_eq!(req.headers.get(CACHE_CONTROL).unwrap(), "no-cache");
    }

    #[test]
    fn cache_set_before_headers_still_wins() {
        let mut headers = HeaderMap::new();
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=5"));
        let req = get("https://x.test/")
            .unwrap()
            .cache(CacheControl::force_network())
            .headers(headers);
        assert_eq!(req.headers.get(CACHE_CONTROL).unwrap(), "no-cache");
    }

    #[test]
    fn put_and_delete_build_their_methods() {
        assert_eq!(put("https://x.test/", Body::Empty).unwrap().method, Method::Put);
        assert_eq!(delete("https://x.test/", Body::Empty).unwrap().method, Method::Delete);
    }
}
