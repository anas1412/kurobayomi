//! The HTTP base of an anime source (`AnimeHttpSource.kt`, `Requests.kt`, `OkHttpExtensions.kt`; T-16a-025 to T-16a-028,
//! T-16a-032). A source embeds [`AnimeHttpBase`], as a Kotlin source inherits `AnimeHttpSource`. The client, the id and
//! the default headers are the same [`HttpCore`] the manga side uses.

use bytes::Bytes;
use http::HeaderMap;
use kuro_net::{NetworkClient, Request};
use kuro_source::SourceError;
use kuro_source::anime::{SAnime, SEpisode};
use url::Url;

use crate::error::map_net_error;
use crate::html::resolve;
use crate::http_source::{HttpCore, default_headers};
use crate::request;
use crate::response::HttpResponse;
use crate::urls::join_base;

/// The identity, base URL and client of an anime HTTP source.
pub struct AnimeHttpBase {
    core: HttpCore,
    name: String,
    lang: String,
    base_url: String,
    version_id: i32,
}

impl AnimeHttpBase {
    /// `base_url` has no trailing slash (behaviour 1.3). `version_id` is part of the id and is 1 by default.
    pub fn new(client: NetworkClient, name: &str, lang: &str, base_url: &str, version_id: i32) -> Self {
        Self { core: HttpCore::new(client), name: name.into(), lang: lang.into(), base_url: base_url.into(), version_id }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The stable id, computed once (behaviour 1.1).
    pub fn id(&self) -> i64 {
        self.core.id(&self.name, &self.lang, self.version_id)
    }

    /// The default headers: exactly one `User-Agent` pair from the network preference. Built once per instance, so a
    /// second call returns the same value (E-33).
    pub fn headers(&self) -> &HeaderMap {
        self.core.headers(|| default_headers(&self.core))
    }

    /// `GET(url, headers)` with `Cache-Control: max-age=600` (`Requests.kt`, T-16a-025).
    pub fn get_request(&self, url: &str) -> Result<Request, SourceError> {
        request::get(url, self.headers())
    }

    /// `GET(baseUrl + path)`, the form of the default request URLs (behaviour 2.2).
    pub fn join_request(&self, path: &str) -> Result<Request, SourceError> {
        self.get_request(&join_base(&self.base_url, path))
    }

    /// Runs one request (T-16a-026). A status outside 200 to 299 is `Http(code)` and the body is never read. Dropping
    /// the returned future drops the request in flight, which closes its connection.
    pub async fn fetch(&self, request: Request) -> Result<HttpResponse, SourceError> {
        let response = self.core.client().execute(request).await.map_err(map_net_error)?;
        HttpResponse::read(response).await
    }

    /// `getImageTile` (T-16a-032): an async fetch with the status checked. A tile answered with a status outside 2xx
    /// is `None`. A transport failure is an error for the caller to log. The bytes are not decoded here (E-09).
    pub async fn image_tile(&self, url: &str) -> Result<Option<Bytes>, SourceError> {
        match self.fetch(self.get_request(url)?).await {
            Ok(response) => Ok(Some(response.body)),
            Err(SourceError::Http(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// `getAnimeUrl(anime)`: the absolute `base + anime.url`, normalised (T-16a-028). A base that is not a URL is
    /// `Url`, never a panic.
    pub fn anime_url(&self, anime: &SAnime) -> Result<String, SourceError> {
        let joined = join_base(&self.base_url, &anime.url);
        Url::parse(&joined).map(String::from).map_err(|err| SourceError::Url(format!("{joined}: {err}")))
    }
}

/// `getEpisodeUrl(episode)`: the stored URL unchanged. Callers must not mix the absolute and the stored forms (E-23).
pub fn episode_url(episode: &SEpisode) -> &str {
    &episode.url
}

/// The stored form of an absolute episode link found in a page, resolved against the document base (`abs:`).
pub fn stored_episode_url(episode: &mut SEpisode, base: &Url, href: &str) -> Result<(), SourceError> {
    let absolute = resolve(base, href).ok_or_else(|| SourceError::Url(format!("cannot resolve {href:?}")))?;
    episode.set_url_without_domain(&absolute);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    use http::header::{CACHE_CONTROL, USER_AGENT};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::test_support::test_client;

    fn base(client: NetworkClient, url: &str) -> AnimeHttpBase {
        AnimeHttpBase::new(client, "Site", "en", url, 1)
    }

    #[test]
    fn the_headers_are_one_user_agent_pair_and_the_same_allocation_each_time() {
        let (client, _dir) = test_client();
        let expected = client.default_user_agent();
        let site = base(client, "https://a.test");
        let first = site.headers();
        assert_eq!(first.len(), 1);
        let (name, value) = first.iter().next().unwrap();
        assert_eq!(name, USER_AGENT);
        assert_eq!(value.to_str().unwrap(), expected);
        assert!(std::ptr::eq(first, site.headers()));
    }

    #[test]
    fn every_get_request_carries_the_ten_minute_cache_directive() {
        let (client, _dir) = test_client();
        let site = base(client, "https://a.test");
        let request = site.get_request("https://a.test/list?page=2").unwrap();
        assert_eq!(request.headers.get(CACHE_CONTROL).unwrap(), "max-age=600");
        assert!(request.headers.get(USER_AGENT).is_some());
    }

    #[test]
    fn the_kotlin_id_vectors_hold_for_the_anime_base() {
        let (client, _dir) = test_client();
        let example = AnimeHttpBase::new(client, "Example", "en", "https://a.test", 1);
        assert_eq!(example.id(), 4851548778462321757);
        let (client, _dir) = test_client();
        let other = AnimeHttpBase::new(client, "Aniyomi Test", "ja", "https://a.test", 2);
        assert_eq!(other.id(), 7915262807009061958);
    }

    #[test]
    fn the_anime_url_is_absolute_and_a_broken_base_is_an_error_not_a_panic() {
        let (client, _dir) = test_client();
        let site = base(client, "https://a.test");
        let anime = SAnime { url: "/show/1".into(), ..Default::default() };
        assert_eq!(site.anime_url(&anime).unwrap(), "https://a.test/show/1");
        let (client, _dir) = test_client();
        let broken = base(client, "not a base");
        assert!(matches!(broken.anime_url(&anime), Err(SourceError::Url(_))));
    }

    #[test]
    fn the_episode_url_is_the_stored_form_unchanged() {
        let episode = SEpisode { url: "/ep/1".into(), ..Default::default() };
        assert_eq!(episode_url(&episode), "/ep/1");
    }

    #[tokio::test]
    async fn a_2xx_status_passes_and_an_error_status_is_http() {
        let server = MockServer::start().await;
        for (route, code) in [("/ok", 200_u16), ("/partial", 206), ("/gone", 404), ("/broken", 500)] {
            Mock::given(method("GET"))
                .and(path(route))
                .respond_with(ResponseTemplate::new(code).set_body_string("body"))
                .mount(&server)
                .await;
        }
        let (client, _dir) = test_client();
        let site = base(client, &server.uri());
        assert_eq!(site.fetch(site.join_request("/ok").unwrap()).await.unwrap().status, 200);
        assert_eq!(site.fetch(site.join_request("/partial").unwrap()).await.unwrap().status, 206);
        assert_eq!(site.fetch(site.join_request("/gone").unwrap()).await.unwrap_err(), SourceError::Http(404));
        assert_eq!(site.fetch(site.join_request("/broken").unwrap()).await.unwrap_err(), SourceError::Http(500));
    }

    #[tokio::test]
    async fn an_image_tile_that_is_not_found_is_none_and_a_found_one_is_its_bytes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tile-1.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1_u8, 2, 3]))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/tile-404.jpg"))
            .respond_with(ResponseTemplate::new(404).set_body_string("<html>missing</html>"))
            .mount(&server)
            .await;
        let (client, _dir) = test_client();
        let site = base(client, &server.uri());
        let tile = site.image_tile(&format!("{}/tile-1.jpg", server.uri())).await.unwrap();
        assert_eq!(tile.as_deref(), Some(&[1_u8, 2, 3][..]));
        assert_eq!(site.image_tile(&format!("{}/tile-404.jpg", server.uri())).await.unwrap(), None);
    }

    /// Dropping the fetch closes its in-flight connection. The server thread waits for that close, so the test joins it
    /// with `spawn_blocking`: a blocking `join` would hold the current-thread runtime, and the connection task could not
    /// run to close the socket.
    #[tokio::test]
    async fn dropping_the_fetch_closes_the_connection_within_a_second() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        // The server reads the request head and then waits. It only sees its read end when the client closes.
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream.set_read_timeout(Some(Duration::from_secs(5))).expect("timeout");
            let mut seen = Vec::new();
            let mut buf = [0_u8; 1024];
            while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buf).expect("request head");
                assert!(n > 0, "the request head ended early");
                seen.extend_from_slice(&buf[..n]);
            }
            let started = Instant::now();
            let n = stream.read(&mut buf).expect("the client should close the socket");
            (n, started.elapsed())
        });
        let (client, _dir) = test_client();
        let site = base(client, &format!("http://{addr}"));
        let request = site.join_request("/slow").unwrap();
        let outcome = tokio::time::timeout(Duration::from_millis(300), site.fetch(request)).await;
        assert!(outcome.is_err(), "the fetch must still be in flight");
        let (n, waited) = tokio::task::spawn_blocking(move || server.join()).await.expect("join task").expect("server thread");
        assert_eq!(n, 0, "end of stream");
        assert!(waited < Duration::from_secs(1), "closed after {waited:?}");
    }
}
