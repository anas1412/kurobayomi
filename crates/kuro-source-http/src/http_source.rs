//! `HttpSource`, its state `HttpCore`, and the generic `Http<S>` adapter (`HttpSource.kt`; T-15a-020, T-15a-021,
//! T-15a-025, T-15a-026, T-15a-034).
//!
//! Kotlin gives every HTTP source the same base class. Here the base is a trait with default methods, and `Http<S>`
//! gives any `HttpSource` the `MangaSource` and `CatalogueSource` traits, so `kuro-source` never knows HTTP.

use std::sync::OnceLock;

use async_trait::async_trait;
use http::HeaderMap;
use http::HeaderValue;
use http::header::USER_AGENT;
use kuro_net::{NetworkClient, Request};
use kuro_source::model::{MangasPage, Page, SChapter, SManga};
use kuro_source::{CatalogueSource, FilterList, MangaSource, SourceError, generate_id};

use crate::error::map_net_error;
use crate::request;
use crate::response::HttpResponse;
use crate::urls;

/// The state every HTTP source carries: the client, the id (computed once) and the default headers (computed once).
pub struct HttpCore {
    client: NetworkClient,
    id: OnceLock<i64>,
    headers: OnceLock<HeaderMap>,
}

impl HttpCore {
    pub fn new(client: NetworkClient) -> Self {
        Self { client, id: OnceLock::new(), headers: OnceLock::new() }
    }

    pub fn client(&self) -> &NetworkClient {
        &self.client
    }

    /// Behaviour 1.1: the id from name, language and version, computed on first use and kept.
    pub fn id(&self, name: &str, lang: &str, version_id: i32) -> i64 {
        *self.id.get_or_init(|| generate_id(name, lang, version_id))
    }

    /// Behaviour 1.5: the default headers, built on first use and kept for the life of the source.
    pub fn headers(&self, build: impl FnOnce() -> HeaderMap) -> &HeaderMap {
        self.headers.get_or_init(build)
    }
}

/// The default headers of a source (behaviour 1.5): `User-Agent` from the network preference, trimmed. A blank
/// preference adds no header, so the network layer applies its own default.
pub fn default_headers(core: &HttpCore) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let agent = core.client().default_user_agent();
    let agent = agent.trim();
    if !agent.is_empty()
        && let Ok(value) = HeaderValue::from_str(agent)
    {
        headers.insert(USER_AGENT, value);
    }
    headers
}

/// An HTTP-backed source. Required methods are the ones Kotlin left abstract; the rest have the Kotlin defaults.
pub trait HttpSource: Send + Sync {
    fn core(&self) -> &HttpCore;
    fn name(&self) -> &str;
    fn lang(&self) -> &str;
    /// The site root. No trailing slash (behaviour 1.4).
    fn base_url(&self) -> &str;
    /// Part of the id. Defaults to 1.
    fn version_id(&self) -> i32 {
        1
    }
    /// Required: every source states whether it has a "latest" listing.
    fn supports_latest(&self) -> bool;
    fn filter_list(&self) -> FilterList {
        FilterList::default()
    }
    fn is_unmetered(&self) -> bool {
        false
    }

    /// The default headers (behaviour 1.5): `User-Agent` from the network preference, trimmed. A blank preference adds
    /// no header, so the network layer applies its own default.
    fn headers_builder(&self) -> HeaderMap {
        default_headers(self.core())
    }

    /// The default headers, computed once per source instance.
    fn core_headers(&self) -> HeaderMap {
        self.core().headers(|| self.headers_builder()).clone()
    }

    /// `GET(baseUrl + path)` with the default headers (behaviour 2.2).
    fn join_base(&self, path: &str) -> Result<Request, SourceError> {
        request::get(&urls::join_base(self.base_url(), path), &self.core_headers())
    }

    fn popular_request(&self, page: i32) -> Result<Request, SourceError>;
    fn search_request(&self, page: i32, query: &str, filters: &FilterList) -> Result<Request, SourceError>;
    fn latest_request(&self, page: i32) -> Result<Request, SourceError>;

    /// `GET(baseUrl + manga.url)`.
    fn details_request(&self, manga: &SManga) -> Result<Request, SourceError> {
        self.join_base(&manga.url)
    }
    /// `GET(baseUrl + manga.url)`, the same URL as the details.
    fn chapter_list_request(&self, manga: &SManga) -> Result<Request, SourceError> {
        self.join_base(&manga.url)
    }
    /// `GET(baseUrl + chapter.url)`. The page list is read from this URL (behaviour 7).
    fn page_list_request(&self, chapter: &SChapter) -> Result<Request, SourceError> {
        self.join_base(&chapter.url)
    }
    /// `GET(page.url)` with the URL used as it is. A relative URL is `Url` (EC-URL-08).
    fn image_url_request(&self, page: &Page) -> Result<Request, SourceError> {
        request::get(&page.url, &self.core_headers())
    }
    /// `GET(page.imageUrl)`. A page without an image URL is `MissingImageUrl` (EC-URL-09).
    fn image_request(&self, page: &Page) -> Result<Request, SourceError> {
        let url = page.image_url.as_deref().ok_or(SourceError::MissingImageUrl)?;
        request::get(url, &self.core_headers())
    }

    fn popular_parse(&self, response: &HttpResponse) -> Result<MangasPage, SourceError>;
    fn search_parse(&self, response: &HttpResponse) -> Result<MangasPage, SourceError>;
    fn latest_parse(&self, response: &HttpResponse) -> Result<MangasPage, SourceError>;
    fn details_parse(&self, response: &HttpResponse) -> Result<SManga, SourceError>;
    fn chapter_list_parse(&self, response: &HttpResponse) -> Result<Vec<SChapter>, SourceError>;
    fn page_list_parse(&self, response: &HttpResponse) -> Result<Vec<Page>, SourceError>;
    fn image_url_parse(&self, response: &HttpResponse) -> Result<String, SourceError>;

    /// Called for every chapter on every library sync, for HTTP sources only, before the database lookup (behaviour 8).
    /// It may rewrite the chapter, so a rewritten URL makes the chapter look new (EC-CH-05).
    fn prepare_new_chapter(&self, _chapter: &mut SChapter, _manga: &SManga) {}

    /// The web address of a manga: the details request URL (behaviour 7).
    fn manga_web_url(&self, manga: &SManga) -> Result<String, SourceError> {
        Ok(self.details_request(manga)?.url.to_string())
    }
    /// The web address of a chapter: the PAGE-LIST request URL, not the chapter-list URL (behaviour 7).
    fn chapter_web_url(&self, chapter: &SChapter) -> Result<String, SourceError> {
        Ok(self.page_list_request(chapter)?.url.to_string())
    }
}

/// Gives an `HttpSource` the catalogue and manga traits. The generic version of `HttpSource`'s Kotlin base class.
pub struct Http<S>(pub S);

impl<S: HttpSource> Http<S> {
    /// Runs one request. An unmetered source skips the client's rate limiter (T-16a-037).
    async fn fetch(&self, mut request: Request) -> Result<HttpResponse, SourceError> {
        request.opts.skip_rate_limit = self.0.is_unmetered();
        let response = self.0.core().client().execute(request).await.map_err(map_net_error)?;
        HttpResponse::read(response).await
    }
}

/// Page numbers come from the list position, whatever index the source set (EC-PL-02, behaviour 5.2).
fn reindex_pages(pages: Vec<Page>) -> Vec<Page> {
    pages
        .into_iter()
        .enumerate()
        .map(|(position, page)| Page { index: i32::try_from(position).unwrap_or(i32::MAX), ..page })
        .collect()
}

#[async_trait]
impl<S: HttpSource + 'static> MangaSource for Http<S> {
    fn id(&self) -> i64 {
        self.0.core().id(self.0.name(), self.0.lang(), self.0.version_id())
    }

    fn name(&self) -> &str {
        self.0.name()
    }

    fn lang(&self) -> &str {
        self.0.lang()
    }

    fn is_unmetered(&self) -> bool {
        self.0.is_unmetered()
    }

    fn prepare_new_chapter(&self, chapter: &mut SChapter, manga: &SManga) {
        self.0.prepare_new_chapter(chapter, manga);
    }

    async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
        let response = self.fetch(self.0.details_request(manga)?).await?;
        let mut details = self.0.details_parse(&response)?;
        details.initialized = true;
        Ok(details)
    }

    async fn get_chapter_list(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
        let response = self.fetch(self.0.chapter_list_request(manga)?).await?;
        self.0.chapter_list_parse(&response)
    }

    async fn get_page_list(&self, chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
        let response = self.fetch(self.0.page_list_request(chapter)?).await?;
        Ok(reindex_pages(self.0.page_list_parse(&response)?))
    }
}

#[async_trait]
impl<S: HttpSource + 'static> CatalogueSource for Http<S> {
    fn supports_latest(&self) -> bool {
        self.0.supports_latest()
    }

    fn filter_list(&self) -> FilterList {
        self.0.filter_list()
    }

    async fn popular(&self, page: i32) -> Result<MangasPage, SourceError> {
        let response = self.fetch(self.0.popular_request(page)?).await?;
        self.0.popular_parse(&response)
    }

    async fn search(&self, page: i32, query: &str, filters: &FilterList) -> Result<MangasPage, SourceError> {
        let response = self.fetch(self.0.search_request(page, query, filters)?).await?;
        self.0.search_parse(&response)
    }

    async fn latest(&self, page: i32) -> Result<MangasPage, SourceError> {
        let response = self.fetch(self.0.latest_request(page)?).await?;
        self.0.latest_parse(&response)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::Arc;

    use bytes::Bytes;
    use kuro_source::model::PageState;
    use url::Url;

    use super::*;
    use crate::test_support::test_client;

    /// A source whose parsers return fixed values. `agent` replaces the default headers when set.
    struct Plain {
        core: HttpCore,
        base: &'static str,
        agent: Option<&'static str>,
        /// When set, `prepare_new_chapter` appends a version to the chapter URL.
        rewrite: bool,
    }

    impl Plain {
        fn new(base: &'static str) -> (Self, tempfile::TempDir) {
            let (client, dir) = test_client();
            (Self { core: HttpCore::new(client), base, agent: None, rewrite: false }, dir)
        }

        fn with_agent(base: &'static str, agent: &'static str) -> (Self, tempfile::TempDir) {
            let (source, dir) = Self::new(base);
            (Self { agent: Some(agent), ..source }, dir)
        }
    }

    impl HttpSource for Plain {
        fn core(&self) -> &HttpCore {
            &self.core
        }
        fn name(&self) -> &str {
            "Example"
        }
        fn lang(&self) -> &str {
            "en"
        }
        fn base_url(&self) -> &str {
            self.base
        }
        fn supports_latest(&self) -> bool {
            true
        }
        fn headers_builder(&self) -> HeaderMap {
            match self.agent {
                Some(agent) => {
                    let mut headers = HeaderMap::new();
                    headers.insert(USER_AGENT, HeaderValue::from_static(agent));
                    headers
                }
                None => default_headers(&self.core),
            }
        }
        fn popular_request(&self, page: i32) -> Result<Request, SourceError> {
            self.join_base(&format!("/popular?page={page}"))
        }
        fn search_request(&self, page: i32, query: &str, _filters: &FilterList) -> Result<Request, SourceError> {
            self.join_base(&format!("/search?page={page}&q={query}"))
        }
        fn latest_request(&self, page: i32) -> Result<Request, SourceError> {
            self.join_base(&format!("/latest?page={page}"))
        }
        fn popular_parse(&self, _: &HttpResponse) -> Result<MangasPage, SourceError> {
            Ok(MangasPage { mangas: Vec::new(), has_next_page: false })
        }
        fn search_parse(&self, _: &HttpResponse) -> Result<MangasPage, SourceError> {
            Ok(MangasPage { mangas: Vec::new(), has_next_page: false })
        }
        fn latest_parse(&self, _: &HttpResponse) -> Result<MangasPage, SourceError> {
            Ok(MangasPage { mangas: Vec::new(), has_next_page: false })
        }
        fn details_parse(&self, _: &HttpResponse) -> Result<SManga, SourceError> {
            Ok(SManga::new("/manga/1", "Parsed"))
        }
        fn chapter_list_parse(&self, _: &HttpResponse) -> Result<Vec<SChapter>, SourceError> {
            Ok(Vec::new())
        }
        fn page_list_parse(&self, _: &HttpResponse) -> Result<Vec<Page>, SourceError> {
            Ok(vec![Page::new(7, "/p/a"), Page::new(9, "/p/b")])
        }
        fn image_url_parse(&self, _: &HttpResponse) -> Result<String, SourceError> {
            Ok(String::new())
        }
        fn prepare_new_chapter(&self, chapter: &mut SChapter, _manga: &SManga) {
            if self.rewrite {
                chapter.url.push_str("?v=2");
            }
        }
    }

    fn manga(url: &str) -> SManga {
        SManga::new(url, "M")
    }

    fn empty_response(url: &str) -> HttpResponse {
        HttpResponse {
            status: 200,
            url: Url::parse(url).unwrap(),
            headers: HeaderMap::new(),
            body: Bytes::new(),
        }
    }

    #[test]
    fn the_default_headers_carry_the_trimmed_network_user_agent() {
        let (source, _dir) = Plain::new("https://a.test");
        let headers = default_headers(source.core());
        let agent = headers.get(USER_AGENT).unwrap().to_str().unwrap().to_owned();
        assert_eq!(agent, source.core().client().default_user_agent().trim());
        assert!(agent.contains("Firefox/136"), "{agent}");
    }

    #[test]
    fn a_source_own_user_agent_is_sent_unchanged() {
        let (source, _dir) = Plain::with_agent("https://a.test", "Custom/9.9");
        let request = source.details_request(&manga("/manga/1")).unwrap();
        assert_eq!(request.headers.get(USER_AGENT).unwrap(), "Custom/9.9");
    }

    #[test]
    fn the_default_headers_are_built_once_per_source() {
        let (source, _dir) = Plain::new("https://a.test");
        let calls = Cell::new(0);
        let build = || {
            calls.set(calls.get() + 1);
            default_headers(source.core())
        };
        let first = source.core().headers(build);
        let second = source.core().headers(build);
        assert_eq!(calls.get(), 1);
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn the_id_is_the_generated_id_and_is_kept() {
        let (source, _dir) = Plain::new("https://a.test");
        let adapter = Http(source);
        assert_eq!(MangaSource::id(&adapter), 4_851_548_778_462_321_757);
        assert_eq!(adapter.0.core().id("Other", "fr", 9), 4_851_548_778_462_321_757);
    }

    #[test]
    fn the_default_builders_are_gets_under_the_base_url() {
        let (source, _dir) = Plain::new("https://a.test");
        let details = source.details_request(&manga("/manga/1")).unwrap();
        assert_eq!(details.method, kuro_net::Method::Get);
        assert_eq!(details.url.as_str(), "https://a.test/manga/1");
        let chapters = source.chapter_list_request(&manga("/manga/1")).unwrap();
        assert_eq!(chapters.url.as_str(), "https://a.test/manga/1");
        let chapter = SChapter::new("/ch/1", "Ch 1");
        assert_eq!(source.page_list_request(&chapter).unwrap().url.as_str(), "https://a.test/ch/1");
    }

    #[test]
    fn a_trailing_slash_base_does_not_make_a_double_slash() {
        let (source, _dir) = Plain::new("https://a.test/");
        let details = source.details_request(&manga("/manga/1")).unwrap();
        assert_eq!(details.url.as_str(), "https://a.test/manga/1");
    }

    #[test]
    fn image_urls_are_used_as_they_are() {
        let (source, _dir) = Plain::new("https://a.test");
        let mut page = Page::new(0, "https://img.test/p/1.jpg");
        assert_eq!(source.image_url_request(&page).unwrap().url.as_str(), "https://img.test/p/1.jpg");
        page.image_url = Some("https://cdn.test/1.jpg".into());
        assert_eq!(source.image_request(&page).unwrap().url.as_str(), "https://cdn.test/1.jpg");
    }

    #[test]
    fn a_relative_page_url_is_a_url_error_and_a_missing_image_is_missing_image_url() {
        let (source, _dir) = Plain::new("https://a.test");
        let relative = Page::new(0, "img/1.jpg");
        assert!(matches!(source.image_url_request(&relative), Err(SourceError::Url(_))));
        assert!(matches!(source.image_request(&relative), Err(SourceError::MissingImageUrl)));
    }

    #[test]
    fn the_manga_and_chapter_web_urls_are_the_details_and_page_list_urls() {
        let (source, _dir) = Plain::new("https://a.test");
        assert_eq!(source.manga_web_url(&manga("/manga/1")).unwrap(), "https://a.test/manga/1");
        let chapter = SChapter::new("/ch/1", "Ch 1");
        assert_eq!(source.chapter_web_url(&chapter).unwrap(), "https://a.test/ch/1");
    }

    /// Overrides only the page-list request, to show that the chapter web URL follows it.
    struct ReaderPath(Plain);

    impl HttpSource for ReaderPath {
        fn core(&self) -> &HttpCore {
            self.0.core()
        }
        fn name(&self) -> &str {
            self.0.name()
        }
        fn lang(&self) -> &str {
            self.0.lang()
        }
        fn base_url(&self) -> &str {
            self.0.base_url()
        }
        fn supports_latest(&self) -> bool {
            true
        }
        fn popular_request(&self, page: i32) -> Result<Request, SourceError> {
            self.0.popular_request(page)
        }
        fn search_request(&self, page: i32, query: &str, filters: &FilterList) -> Result<Request, SourceError> {
            self.0.search_request(page, query, filters)
        }
        fn latest_request(&self, page: i32) -> Result<Request, SourceError> {
            self.0.latest_request(page)
        }
        fn page_list_request(&self, chapter: &SChapter) -> Result<Request, SourceError> {
            self.join_base(&format!("/reader{}", chapter.url))
        }
        fn popular_parse(&self, r: &HttpResponse) -> Result<MangasPage, SourceError> {
            self.0.popular_parse(r)
        }
        fn search_parse(&self, r: &HttpResponse) -> Result<MangasPage, SourceError> {
            self.0.search_parse(r)
        }
        fn latest_parse(&self, r: &HttpResponse) -> Result<MangasPage, SourceError> {
            self.0.latest_parse(r)
        }
        fn details_parse(&self, r: &HttpResponse) -> Result<SManga, SourceError> {
            self.0.details_parse(r)
        }
        fn chapter_list_parse(&self, r: &HttpResponse) -> Result<Vec<SChapter>, SourceError> {
            self.0.chapter_list_parse(r)
        }
        fn page_list_parse(&self, r: &HttpResponse) -> Result<Vec<Page>, SourceError> {
            self.0.page_list_parse(r)
        }
        fn image_url_parse(&self, r: &HttpResponse) -> Result<String, SourceError> {
            self.0.image_url_parse(r)
        }
    }

    #[test]
    fn overriding_the_page_list_request_changes_only_the_chapter_web_url() {
        let (plain, _dir) = Plain::new("https://a.test");
        let source = ReaderPath(plain);
        let chapter = SChapter::new("/ch/1", "Ch 1");
        assert_eq!(source.chapter_web_url(&chapter).unwrap(), "https://a.test/reader/ch/1");
        assert_eq!(source.manga_web_url(&manga("/manga/1")).unwrap(), "https://a.test/manga/1");
    }

    #[test]
    fn prepare_new_chapter_is_a_no_op_by_default() {
        let (source, _dir) = Plain::new("https://a.test");
        let mut chapter = SChapter::new("/ch/1", "Ch 1");
        source.prepare_new_chapter(&mut chapter, &manga("/manga/1"));
        assert_eq!(chapter, SChapter::new("/ch/1", "Ch 1"));
    }

    #[test]
    fn the_hook_of_an_http_source_is_reached_through_a_dyn_manga_source() {
        let (mut source, _dir) = Plain::new("https://a.test");
        source.rewrite = true;
        let manga_source: Arc<dyn MangaSource> = Arc::new(Http(source));
        let mut chapter = SChapter::new("/ch/1", "Ch 1");
        manga_source.prepare_new_chapter(&mut chapter, &manga("/manga/1"));
        assert_eq!(chapter.url, "/ch/1?v=2");
    }

    #[test]
    fn the_page_list_is_numbered_by_position() {
        let pages = reindex_pages(vec![Page::new(7, "/a"), Page::new(0, "/b"), Page::new(4, "/c")]);
        let indexes: Vec<i32> = pages.iter().map(|p| p.index).collect();
        assert_eq!(indexes, [0, 1, 2]);
        assert_eq!(pages[2].url, "/c");
        assert_eq!(*pages[0].status.state.borrow(), PageState::Queue);
    }

    #[test]
    fn the_adapter_hands_the_parsed_page_list_to_the_numbering() {
        let (source, _dir) = Plain::new("https://a.test");
        let parsed = source.page_list_parse(&empty_response("https://a.test/ch/1")).unwrap();
        let numbered = reindex_pages(parsed);
        assert_eq!(numbered.iter().map(|p| p.index).collect::<Vec<_>>(), [0, 1]);
    }
}
