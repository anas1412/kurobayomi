//! Request builders (`Requests.kt`, T-15a-022). Every default is a GET with the given headers and the 600 second cache
//! directive. The POST helper is not ported: no default source request needs it.

use http::HeaderMap;
use kuro_net::{CacheControl, Request};
use kuro_source::SourceError;

use crate::error::map_net_error;

/// `GET(url, headers)` with `Cache-Control: max-age=600` (`DEFAULT_CACHE_CONTROL`). A URL that does not parse is `Url`.
pub fn get(url: &str, headers: &HeaderMap) -> Result<Request, SourceError> {
    let request = kuro_net::request::get(url).map_err(map_net_error)?;
    Ok(request.headers(headers.clone()).cache(CacheControl::default_request()))
}

#[cfg(test)]
mod tests {
    use http::header::{CACHE_CONTROL, USER_AGENT};
    use http::{HeaderMap, HeaderValue};

    use super::*;

    #[test]
    fn a_default_request_is_a_get_with_the_headers_and_a_600_second_cache() {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("Agent/1"));
        let request = get("https://a.test/list?page=2", &headers).unwrap();
        assert_eq!(request.method, kuro_net::Method::Get);
        assert_eq!(request.url.as_str(), "https://a.test/list?page=2");
        assert_eq!(request.headers.get(USER_AGENT).unwrap(), "Agent/1");
        assert_eq!(request.cache.max_age, Some(600));
        assert_eq!(request.headers.get(CACHE_CONTROL).unwrap(), "max-age=600");
    }

    #[test]
    fn a_url_that_does_not_parse_is_a_url_error() {
        assert!(matches!(get("::::", &HeaderMap::new()), Err(SourceError::Url(_))));
    }
}
