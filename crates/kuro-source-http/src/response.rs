//! `HttpResponse`: a source's view of one response (`Response.asJsoup`, `awaitSuccess`, `OkHttpExtensions.kt`;
//! T-15a-023, T-15a-033). The body is read in full, so parsers are synchronous.

use bytes::Bytes;
use http::{HeaderMap, header::CONTENT_TYPE};
use kuro_source::SourceError;
use kuro_source::json::decode_source_json;
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::map_net_error;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    /// The final URL after redirects. It is the base URI of the document (EC-HT-03).
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl HttpResponse {
    /// Reads a response. A status outside 200 to 299 is `Http(code)` and the body is never read (EC-HT-04..06).
    pub async fn read(response: kuro_net::Response) -> Result<Self, SourceError> {
        let response = response.await_success().map_err(map_net_error)?;
        let status = response.status.as_u16();
        let url = response.url.clone();
        let headers = response.headers.clone();
        let body = response.bytes().await.map_err(map_net_error)?;
        Ok(Self { status, url, headers, body })
    }

    /// The body as text. The charset is the one `Content-Type` names, UTF-8 when it names none or an unknown one. A
    /// byte-order mark is removed (EC-PH-17, EC-PH-18).
    pub fn text(&self) -> String {
        let encoding = self.charset().unwrap_or(encoding_rs::UTF_8);
        let (text, _, _) = encoding.decode(&self.body);
        text.into_owned()
    }

    /// The body parsed as HTML. Parse it and finish all selection in the same synchronous call.
    pub fn html(&self) -> scraper::Html {
        scraper::Html::parse_document(&self.text())
    }

    /// The body decoded as JSON. An empty or broken body is `Parse` (EC-PH-19).
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, SourceError> {
        decode_source_json(&self.body)
    }

    fn charset(&self) -> Option<&'static encoding_rs::Encoding> {
        let content_type = self.headers.get(CONTENT_TYPE)?.to_str().ok()?;
        let label = content_type.split(';').find_map(|part| {
            let (key, value) = part.split_once('=')?;
            key.trim().eq_ignore_ascii_case("charset").then(|| value.trim().trim_matches('"'))
        })?;
        encoding_rs::Encoding::for_label(label.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;
    use kuro_net::Response;

    use super::*;

    fn url() -> Url {
        Url::parse("https://a.test/list").unwrap()
    }

    /// A network response with the given status, headers and body bytes.
    fn network(status: u16, headers: &[(&'static str, &'static str)], body: &'static [u8]) -> Response {
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            map.insert(http::HeaderName::from_static(name), HeaderValue::from_static(value));
        }
        Response::new(
            http::StatusCode::from_u16(status).unwrap(),
            map,
            url(),
            kuro_net::response::body_from_chunks(&[body]),
        )
    }

    async fn read(status: u16, body: &'static [u8]) -> Result<HttpResponse, SourceError> {
        HttpResponse::read(network(status, &[], body)).await
    }

    #[tokio::test]
    async fn two_hundred_and_two_hundred_four_pass_and_their_body_is_kept() {
        assert_eq!(read(200, b"ok").await.unwrap().text(), "ok");
        let empty = read(204, b"").await.unwrap();
        assert_eq!(empty.status, 204);
        assert!(empty.body.is_empty());
    }

    #[tokio::test]
    async fn an_error_status_is_http_and_the_body_is_not_read() {
        assert_eq!(read(404, b"<html>").await.unwrap_err(), SourceError::Http(404));
        assert_eq!(read(503, b"").await.unwrap_err(), SourceError::Http(503));
        assert_eq!(read(599, b"").await.unwrap_err(), SourceError::Http(599));
    }

    #[tokio::test]
    async fn the_final_url_is_the_base_of_the_response() {
        let redirected = Url::parse("https://b.test/final").unwrap();
        let response = Response::new(
            http::StatusCode::OK,
            HeaderMap::new(),
            redirected.clone(),
            kuro_net::response::body_from_chunks(&[b"x"]),
        );
        assert_eq!(HttpResponse::read(response).await.unwrap().url, redirected);
    }

    #[tokio::test]
    async fn the_charset_from_content_type_decodes_the_body() {
        // "é" is 0xE9 in windows-1252 and not valid UTF-8 on its own.
        let response = HttpResponse::read(network(
            200,
            &[("content-type", "text/html; charset=windows-1252")],
            b"caf\xE9",
        ))
        .await
        .unwrap();
        assert_eq!(response.text(), "caf\u{e9}");
    }

    #[tokio::test]
    async fn a_missing_charset_is_utf8_and_a_bom_is_removed() {
        let response = read(200, b"\xEF\xBB\xBF<p>hi</p>").await.unwrap();
        assert_eq!(response.text(), "<p>hi</p>");
    }

    #[tokio::test]
    async fn an_empty_json_body_is_a_parse_error() {
        let response = read(200, b"").await.unwrap();
        assert!(matches!(response.json::<bool>(), Err(SourceError::Parse(_))));
    }
}
