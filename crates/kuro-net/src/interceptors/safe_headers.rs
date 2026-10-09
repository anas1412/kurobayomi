//! The request-header filter for the WebView (T-12a-020, `WebViewInterceptor.kt:94-113`). WebKit fails the load
//! with `ERR_INVALID_ARGUMENT` when one of these headers is set, so they are dropped.

use http::header::{HeaderMap, HeaderName, HeaderValue};

/// Names that are never forwarded to the WebView (lower case).
const UNSAFE_NAMES: [&str; 9] = [
    "content-length",
    "host",
    "trailer",
    "te",
    "upgrade",
    "cookie2",
    "keep-alive",
    "transfer-encoding",
    "set-cookie",
];

/// `isRequestHeaderSafe(name, value)`. `name` and `value` are compared lower-cased.
pub fn is_request_header_safe(name: &str, value: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if UNSAFE_NAMES.contains(&name.as_str()) || name.starts_with("proxy-") {
        return false;
    }
    !(name == "connection" && value.eq_ignore_ascii_case("upgrade"))
}

/// The headers WebKit may receive: safe ones only, and the first value of each name (Kotlin `groupBy` and
/// `getOrNull(0)`). Order follows the first appearance of each name.
pub fn safe_request_headers(headers: &HeaderMap) -> Vec<(HeaderName, HeaderValue)> {
    let mut out: Vec<(HeaderName, HeaderValue)> = Vec::new();
    for (name, value) in headers {
        let Ok(text) = value.to_str() else { continue };
        if !is_request_header_safe(name.as_str(), text) {
            continue;
        }
        if out.iter().any(|(seen, _)| seen == name) {
            continue;
        }
        out.push((name.clone(), value.clone()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `fixtures/safe_headers.json` as a table: (name, value, safe).
    const TABLE: [(&str, &str, bool); 14] = [
        ("Connection", "upgrade", false),
        ("connection", "keep-alive", true),
        ("Cookie", "a=b", true),
        ("Proxy-Authorization", "x", false),
        ("proxy-anything", "x", false),
        ("Content-Length", "12", false),
        ("Host", "x.test", false),
        ("Trailer", "x", false),
        ("TE", "trailers", false),
        ("Upgrade", "h2c", false),
        ("Cookie2", "x", false),
        ("Keep-Alive", "300", false),
        ("Transfer-Encoding", "chunked", false),
        ("Set-Cookie", "a=b", false),
    ];

    #[test]
    fn the_safe_header_rules_match_the_table() {
        for (name, value, expected) in TABLE {
            assert_eq!(is_request_header_safe(name, value), expected, "{name}: {value}");
        }
    }

    #[test]
    fn the_first_value_per_name_is_kept_and_cookie_passes() {
        let mut h = HeaderMap::new();
        h.append("accept", HeaderValue::from_static("text/html"));
        h.append("accept", HeaderValue::from_static("application/json"));
        h.append("cookie", HeaderValue::from_static("a=b"));
        h.append("host", HeaderValue::from_static("x.test"));
        let kept = safe_request_headers(&h);
        let names: Vec<&str> = kept.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["accept", "cookie"]);
        assert_eq!(kept[0].1, "text/html");
    }
}
