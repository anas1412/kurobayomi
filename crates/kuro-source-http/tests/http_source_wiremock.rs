//! End-to-end tests of the HTTP source base against a local wiremock server (T-15a-020, T-15a-023, T-15a-034,
//! T-15a-048). A source built on `HttpSource` goes through the real `kuro-net` client, so these tests cover the
//! request path, the status rule, redirects and the page-list numbering.

use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use kuro_net::NetworkClient;
use kuro_source::model::{MangasPage, Page, SChapter, SManga};
use kuro_source::{CatalogueSource, FilterList, MangaSource, SourceError};
use kuro_source_http::request;
use kuro_source_http::{Http, HttpCore, HttpResponse, HttpSource, map_net_error};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client() -> (NetworkClient, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let prefs = kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory());
    let (toaster, _feed) = kuro_platform::toast::toast_channel();
    let client = NetworkClient::new(
        &prefs,
        dir.path().join("cookies.json"),
        Arc::new(kuro_platform::webview::NoWebView),
        toaster,
    )
    .expect("client");
    (client, dir)
}

/// A test site. Bodies are plain text: the details title is the body, the page list is one page per line.
struct Site {
    core: HttpCore,
    base: String,
    unmetered: bool,
}

impl Site {
    fn new(base: String) -> (Self, tempfile::TempDir) {
        let (client, dir) = client();
        (Self { core: HttpCore::new(client), base, unmetered: false }, dir)
    }

    /// A site on a client that lets one request an hour across all hosts, so a second request waits for a permit.
    fn one_per_hour(base: String, unmetered: bool) -> (Self, tempfile::TempDir) {
        let (client, dir) = client();
        let limited = client.new_builder().rate_limit(1, Duration::from_secs(3600)).unwrap().build();
        (Self { core: HttpCore::new(limited), base, unmetered }, dir)
    }
}

impl HttpSource for Site {
    fn core(&self) -> &HttpCore {
        &self.core
    }
    fn name(&self) -> &str {
        "Site"
    }
    fn is_unmetered(&self) -> bool {
        self.unmetered
    }
    fn lang(&self) -> &str {
        "en"
    }
    fn base_url(&self) -> &str {
        &self.base
    }
    fn supports_latest(&self) -> bool {
        true
    }
    fn popular_request(&self, page: i32) -> Result<kuro_net::Request, SourceError> {
        self.join_base(&format!("/popular?page={page}"))
    }
    fn search_request(&self, page: i32, query: &str, _: &FilterList) -> Result<kuro_net::Request, SourceError> {
        self.join_base(&format!("/search?page={page}&q={query}"))
    }
    fn latest_request(&self, page: i32) -> Result<kuro_net::Request, SourceError> {
        self.join_base(&format!("/latest?page={page}"))
    }
    fn popular_parse(&self, response: &HttpResponse) -> Result<MangasPage, SourceError> {
        let text = response.text();
        let mangas = if text.is_empty() { Vec::new() } else { vec![SManga::new(response.url.path(), text)] };
        Ok(MangasPage { mangas, has_next_page: false })
    }
    fn search_parse(&self, response: &HttpResponse) -> Result<MangasPage, SourceError> {
        self.popular_parse(response)
    }
    fn latest_parse(&self, response: &HttpResponse) -> Result<MangasPage, SourceError> {
        self.popular_parse(response)
    }
    fn details_parse(&self, response: &HttpResponse) -> Result<SManga, SourceError> {
        Ok(SManga::new(response.url.path(), response.text()))
    }
    fn chapter_list_parse(&self, response: &HttpResponse) -> Result<Vec<SChapter>, SourceError> {
        Ok(response.text().lines().map(|line| SChapter::new(line, line)).collect())
    }
    fn page_list_parse(&self, response: &HttpResponse) -> Result<Vec<Page>, SourceError> {
        // The source numbers its own pages from 100, to show that the app numbers them by position.
        Ok(response.text().lines().enumerate().map(|(i, line)| Page::new(100 + i as i32, line)).collect())
    }
    fn image_url_parse(&self, response: &HttpResponse) -> Result<String, SourceError> {
        Ok(response.text())
    }
}

/// Compile-time check (T-15a-030): the public futures of the adapter are `Send`, so a source call can run on the tokio
/// runtime while the `scraper` document of a parse is only ever used synchronously.
#[allow(dead_code)]
fn the_public_futures_are_send(adapter: &Http<Site>, manga: &SManga, chapter: &SChapter) {
    fn assert_send<T: Send>(_: &T) {}
    assert_send(&adapter.get_manga_details(manga));
    assert_send(&adapter.get_chapter_list(manga));
    assert_send(&adapter.get_page_list(chapter));
    assert_send(&adapter.popular(1));
    assert_send(&adapter.search(1, "q", &FilterList::default()));
    assert_send(&adapter.latest(1));
}

#[tokio::test]
async fn details_are_requested_at_the_manga_url_and_parsed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/manga/1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Parsed Title"))
        .mount(&server)
        .await;
    let (site, _dir) = Site::new(server.uri());
    let adapter = Http(site);
    let details = adapter.get_manga_details(&SManga::new("/manga/1", "Old")).await.unwrap();
    assert_eq!(details.title, "Parsed Title");
    assert_eq!(details.url, "/manga/1");
    assert!(details.initialized, "the details path marks the manga initialised (HttpSource.kt:223)");
}

#[tokio::test]
async fn an_unmetered_source_skips_the_rate_limit_and_a_metered_one_waits_for_its_permit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Title"))
        .mount(&server)
        .await;
    let (unmetered, _free_dir) = Site::one_per_hour(server.uri(), true);
    let unmetered = Http(unmetered);
    for n in 1..=3 {
        let manga = SManga::new(format!("/free/{n}"), "M");
        tokio::time::timeout(Duration::from_secs(5), unmetered.get_manga_details(&manga))
            .await
            .expect("an unmetered source never waits for a permit")
            .unwrap();
    }
    let (metered, _paid_dir) = Site::one_per_hour(server.uri(), false);
    let metered = Http(metered);
    metered.get_manga_details(&SManga::new("/paid/1", "M")).await.unwrap();
    let second = tokio::time::timeout(Duration::from_millis(300), metered.get_manga_details(&SManga::new("/paid/2", "M"))).await;
    assert!(second.is_err(), "a metered source waits behind the one-per-hour limit");
}

#[tokio::test]
async fn a_redirect_gives_the_final_url_to_the_parser() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/old"))
        .respond_with(ResponseTemplate::new(301).insert_header("location", "/final"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/final"))
        .respond_with(ResponseTemplate::new(200).set_body_string("moved"))
        .mount(&server)
        .await;
    let (site, _dir) = Site::new(server.uri());
    let details = Http(site).get_manga_details(&SManga::new("/old", "Old")).await.unwrap();
    assert_eq!(details.url, "/final", "the parser sees the post-redirect URL as its base");
    assert_eq!(details.title, "moved");
}

/// One request through the real client and the status rule, as the adapter does it.
async fn fetch(site: &Site, path: &str) -> Result<HttpResponse, SourceError> {
    let request = site.join_base(path)?;
    let response = site.core().client().execute(request).await.map_err(map_net_error)?;
    HttpResponse::read(response).await
}

#[tokio::test]
async fn a_status_outside_2xx_is_http_and_the_body_is_never_parsed() {
    let server = MockServer::start().await;
    for code in [404_u16, 503, 599] {
        Mock::given(method("GET"))
            .and(path(format!("/popular-{code}")))
            .respond_with(ResponseTemplate::new(code).set_body_string("not parsed"))
            .mount(&server)
            .await;
    }
    let (site, _dir) = Site::new(server.uri());
    for code in [404_u16, 503, 599] {
        let err = fetch(&site, &format!("/popular-{code}")).await.unwrap_err();
        assert_eq!(err, SourceError::Http(code));
    }
}

#[tokio::test]
async fn a_204_passes_and_the_parser_decides_on_an_empty_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/popular"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let (site, _dir) = Site::new(server.uri());
    let page = Http(site).popular(1).await.unwrap();
    assert!(page.mangas.is_empty(), "the parser decides: an empty body gives an empty listing");
}

#[tokio::test]
async fn a_black_hole_server_times_out_as_transport() {
    // Accepts connections and never answers.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming().flatten() {
            held.push(stream);
        }
    });
    let (client, _dir) = client();
    let mut request = request::get(&format!("http://{addr}/slow"), &http::HeaderMap::new()).unwrap();
    request.opts.timeout = Some(Duration::from_millis(300));
    let err = match client.execute(request).await {
        Ok(_) => panic!("a black-hole server must not answer"),
        Err(err) => map_net_error(err),
    };
    assert!(matches!(err, SourceError::Transport(_)), "{err:?}");
}

#[tokio::test]
async fn the_page_list_is_numbered_by_position_and_keeps_its_urls() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ch/1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("https://img.test/a.jpg\nhttps://img.test/b.jpg"))
        .mount(&server)
        .await;
    let (site, _dir) = Site::new(server.uri());
    let pages = Http(site).get_page_list(&SChapter::new("/ch/1", "1")).await.unwrap();
    let indexes: Vec<i32> = pages.iter().map(|p| p.index).collect();
    assert_eq!(indexes, [0, 1]);
    assert_eq!(pages[1].url, "https://img.test/b.jpg");
}

#[tokio::test]
async fn an_empty_page_list_is_returned_as_is_for_the_caller_to_judge() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ch/2"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&server)
        .await;
    let (site, _dir) = Site::new(server.uri());
    let pages = Http(site).get_page_list(&SChapter::new("/ch/2", "2")).await.unwrap();
    assert!(pages.is_empty());
}

#[tokio::test]
async fn the_listing_calls_reach_the_site_with_their_page_numbers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Latest"))
        .mount(&server)
        .await;
    let (site, _dir) = Site::new(server.uri());
    let adapter = Http(site);
    assert!(adapter.supports_latest());
    assert_eq!(adapter.latest(1).await.unwrap().mangas[0].title, "Latest");
}
