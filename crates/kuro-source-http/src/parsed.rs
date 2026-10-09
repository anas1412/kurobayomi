//! `ParsedHttpSource`: selector-based parsing over `scraper` (`ParsedHttpSource.kt`; T-15a-030, T-15a-031).
//!
//! A source implements the selector and element hooks. The free functions here run the parse for each Kotlin parser.
//! Every function is synchronous and finishes all selection before it returns, so no `scraper` value is alive across
//! an `.await` (pitfall 6).

use kuro_source::model::{MangasPage, Page, SChapter, SManga};
use kuro_source::SourceError;
use scraper::{ElementRef, Html};
use url::Url;

use crate::html::{document_base, parse_selector};
use crate::http_source::HttpSource;
use crate::response::HttpResponse;

/// The selector hooks of a parsed source. Names follow the Kotlin base class in snake case. The element hooks get the
/// document base (see [`crate::html::document_base`]), so a source can resolve a relative link as Jsoup's `abs:` does
/// (EC-PH-16).
///
/// Port notes (pitfall 1): the Jsoup-only pseudo-classes `:contains(text)`, `:containsOwn(text)`, `:matches(regex)`,
/// `:matchesOwn(regex)`, `:containsData(text)`, `:eq(n)`, `:gt(n)` and `:lt(n)` are not CSS and `scraper` rejects
/// them. A port writes the standard CSS part as the selector hook and the rest as a Rust filter over the matched
/// elements, and covers that conversion with a fixture test. There is no Jsoup-compatible selector engine.
pub trait ParsedHttpSource: HttpSource {
    fn popular_selector(&self) -> &'static str;
    /// `None` when the source has no next-page link for popular.
    fn popular_next_page_selector(&self) -> Option<&'static str>;
    fn popular_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SManga, SourceError>;

    fn search_selector(&self) -> &'static str;
    fn search_next_page_selector(&self) -> Option<&'static str>;
    fn search_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SManga, SourceError>;

    fn latest_selector(&self) -> &'static str;
    fn latest_next_page_selector(&self) -> Option<&'static str>;
    fn latest_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SManga, SourceError>;

    /// `base` is the document base URI (see [`crate::html::document_base`]).
    fn details_from_document(&self, document: &Html, base: &url::Url) -> Result<SManga, SourceError>;

    fn chapter_selector(&self) -> &'static str;
    fn chapter_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SChapter, SourceError>;

    fn page_list_from_document(&self, document: &Html, base: &url::Url) -> Result<Vec<Page>, SourceError>;
    fn image_url_from_document(&self, document: &Html, base: &url::Url) -> Result<String, SourceError>;
}

/// Selects `item` elements and maps each one. One failing element fails the whole page (EC-PH-14). `has_next_page` is
/// true only when the `next` selector matches something; its text does not matter (EC-PH-10..13).
pub fn parse_list(
    document: &Html,
    base: &Url,
    item: &str,
    from_element: impl Fn(ElementRef<'_>, &Url) -> Result<SManga, SourceError>,
    next: Option<&str>,
) -> Result<MangasPage, SourceError> {
    let items = parse_selector(item)?;
    let mangas = document.select(&items).map(|element| from_element(element, base)).collect::<Result<Vec<_>, _>>()?;
    let has_next_page = match next {
        Some(css) => document.select(&parse_selector(css)?).next().is_some(),
        None => false,
    };
    Ok(MangasPage { mangas, has_next_page })
}

/// `popularMangaParse`: the popular listing of a response.
pub fn popular_page<S: ParsedHttpSource>(source: &S, response: &HttpResponse) -> Result<MangasPage, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    parse_list(
        &document,
        &base,
        source.popular_selector(),
        |element, base| source.popular_from_element(element, base),
        source.popular_next_page_selector(),
    )
}

/// `searchMangaParse`: the search listing of a response.
pub fn search_page<S: ParsedHttpSource>(source: &S, response: &HttpResponse) -> Result<MangasPage, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    parse_list(
        &document,
        &base,
        source.search_selector(),
        |element, base| source.search_from_element(element, base),
        source.search_next_page_selector(),
    )
}

/// `latestUpdatesParse`: the latest listing of a response.
pub fn latest_page<S: ParsedHttpSource>(source: &S, response: &HttpResponse) -> Result<MangasPage, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    parse_list(
        &document,
        &base,
        source.latest_selector(),
        |element, base| source.latest_from_element(element, base),
        source.latest_next_page_selector(),
    )
}

/// `mangaDetailsParse`: the details of a response.
pub fn details_page<S: ParsedHttpSource>(source: &S, response: &HttpResponse) -> Result<SManga, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    source.details_from_document(&document, &base)
}

/// `chapterListParse`: every chapter element, in document order.
pub fn chapter_list_page<S: ParsedHttpSource>(source: &S, response: &HttpResponse) -> Result<Vec<SChapter>, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    let chapters = parse_selector(source.chapter_selector())?;
    document.select(&chapters).map(|element| source.chapter_from_element(element, &base)).collect()
}

/// `pageListParse`: the pages of a response. Numbering is applied later by the HTTP adapter.
pub fn page_list_page<S: ParsedHttpSource>(source: &S, response: &HttpResponse) -> Result<Vec<Page>, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    source.page_list_from_document(&document, &base)
}

/// `imageUrlParse`: the image URL of a response.
pub fn image_url_page<S: ParsedHttpSource>(source: &S, response: &HttpResponse) -> Result<String, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    source.image_url_from_document(&document, &base)
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http::HeaderMap;
    use http::HeaderValue;
    use http::header::CONTENT_TYPE;
    use kuro_net::Request;
    use kuro_source::FilterList;
    use url::Url;

    use super::*;
    use crate::html::{resolve, select_text, text};
    use crate::http_source::{HttpCore, HttpSource};
    use crate::test_support::test_client;

    const LIST_POPULAR: &[u8] = include_bytes!("../tests/fixtures/source/list_popular.html");
    const LIST_POPULAR_NO_NEXT: &[u8] = include_bytes!("../tests/fixtures/source/list_popular_no_next.html");
    const LIST_EMPTY: &[u8] = include_bytes!("../tests/fixtures/source/list_empty.html");
    const WINDOWS_1252: &[u8] = include_bytes!("../tests/fixtures/source/windows1252.html");
    const IMAGE_URL: &[u8] = include_bytes!("../tests/fixtures/source/image_url.html");
    const DETAILS: &[u8] = include_bytes!("../tests/fixtures/source/details.html");
    const CHAPTERS: &[u8] = include_bytes!("../tests/fixtures/source/chapters.html");
    const PAGES: &[u8] = include_bytes!("../tests/fixtures/source/pages.html");

    /// A parsed source over the catalogue fixtures. `absolute` resolves a list link against the document base, the way
    /// a source using Jsoup's `abs:` does.
    struct Catalogue {
        core: HttpCore,
        next: Option<&'static str>,
        absolute: bool,
    }

    impl Catalogue {
        fn new(next: Option<&'static str>) -> (Self, tempfile::TempDir) {
            let (client, dir) = test_client();
            (Self { core: HttpCore::new(client), next, absolute: false }, dir)
        }
    }

    impl HttpSource for Catalogue {
        fn core(&self) -> &HttpCore {
            &self.core
        }
        fn name(&self) -> &str {
            "Catalogue"
        }
        fn lang(&self) -> &str {
            "en"
        }
        fn base_url(&self) -> &str {
            "https://a.test"
        }
        fn supports_latest(&self) -> bool {
            false
        }
        fn popular_request(&self, _: i32) -> Result<Request, SourceError> {
            Err(SourceError::Unsupported("popular_request"))
        }
        fn search_request(&self, _: i32, _: &str, _: &FilterList) -> Result<Request, SourceError> {
            Err(SourceError::Unsupported("search_request"))
        }
        fn latest_request(&self, _: i32) -> Result<Request, SourceError> {
            Err(SourceError::Unsupported("latest_request"))
        }
        fn popular_parse(&self, r: &HttpResponse) -> Result<MangasPage, SourceError> {
            popular_page(self, r)
        }
        fn search_parse(&self, r: &HttpResponse) -> Result<MangasPage, SourceError> {
            search_page(self, r)
        }
        fn latest_parse(&self, r: &HttpResponse) -> Result<MangasPage, SourceError> {
            latest_page(self, r)
        }
        fn details_parse(&self, r: &HttpResponse) -> Result<SManga, SourceError> {
            details_page(self, r)
        }
        fn chapter_list_parse(&self, r: &HttpResponse) -> Result<Vec<SChapter>, SourceError> {
            chapter_list_page(self, r)
        }
        fn page_list_parse(&self, r: &HttpResponse) -> Result<Vec<Page>, SourceError> {
            page_list_page(self, r)
        }
        fn image_url_parse(&self, r: &HttpResponse) -> Result<String, SourceError> {
            image_url_page(self, r)
        }
    }

    impl ParsedHttpSource for Catalogue {
        fn popular_selector(&self) -> &'static str {
            "div.item"
        }
        fn popular_next_page_selector(&self) -> Option<&'static str> {
            self.next
        }
        fn popular_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SManga, SourceError> {
            let link = element.select(&parse_selector("a.title")?).next().ok_or_else(|| {
                SourceError::Parse("item without a title link".into())
            })?;
            let href = link.value().attr("href").unwrap_or("");
            let url = if self.absolute { resolve(base, href).unwrap_or_default() } else { href.to_owned() };
            Ok(SManga::new(url, text(link)))
        }
        fn search_selector(&self) -> &'static str {
            "div.item"
        }
        fn search_next_page_selector(&self) -> Option<&'static str> {
            None
        }
        fn search_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SManga, SourceError> {
            self.popular_from_element(element, base)
        }
        fn latest_selector(&self) -> &'static str {
            "div.item"
        }
        fn latest_next_page_selector(&self) -> Option<&'static str> {
            None
        }
        fn latest_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SManga, SourceError> {
            self.popular_from_element(element, base)
        }
        fn details_from_document(&self, document: &Html, _base: &Url) -> Result<SManga, SourceError> {
            let title = select_text(document.root_element(), "title")?.unwrap_or_default();
            Ok(SManga::new("/manga/1", title))
        }
        fn chapter_selector(&self) -> &'static str {
            "li.chapter"
        }
        fn chapter_from_element(&self, element: ElementRef<'_>, _base: &Url) -> Result<SChapter, SourceError> {
            Ok(SChapter::new("/ch", text(element)))
        }
        fn page_list_from_document(&self, document: &Html, base: &Url) -> Result<Vec<Page>, SourceError> {
            let imgs = parse_selector("img.page")?;
            document
                .select(&imgs)
                .enumerate()
                .map(|(i, img)| {
                    let src = img.value().attr("src").unwrap_or("");
                    let mut page = Page::new(i as i32, src);
                    page.image_url = resolve(base, src);
                    Ok(page)
                })
                .collect()
        }
        fn image_url_from_document(&self, document: &Html, base: &Url) -> Result<String, SourceError> {
            let img = document
                .select(&parse_selector("img.page")?)
                .next()
                .ok_or_else(|| SourceError::Parse("no page image".into()))?;
            let src = img.value().attr("src").unwrap_or("");
            resolve(base, src).ok_or_else(|| SourceError::Url(src.to_owned()))
        }
    }

    fn response(url: &str, body: &'static [u8]) -> HttpResponse {
        HttpResponse { status: 200, url: Url::parse(url).unwrap(), headers: HeaderMap::new(), body: Bytes::from_static(body) }
    }

    fn titles(page: &MangasPage) -> Vec<String> {
        page.mangas.iter().map(|m| m.title.clone()).collect()
    }

    #[test]
    fn the_listing_maps_every_item_and_reports_a_next_page() {
        let (source, _dir) = Catalogue::new(Some("a.next"));
        let page = popular_page(&source, &response("https://a.test/popular", LIST_POPULAR)).unwrap();
        assert_eq!(titles(&page), ["One & Two", "Three Four", "Five"]);
        assert_eq!(page.mangas[0].url, "/manga/1");
        assert!(page.has_next_page);
    }

    #[test]
    fn a_relative_list_link_resolves_against_the_base_element() {
        let (source, _dir) = Catalogue::new(None);
        let source = Catalogue { absolute: true, ..source };
        let body: &'static [u8] = br#"<html><head><base href="https://cdn.test/site/"></head><body><div class="item"><a class="title" href="m/1">One</a></div></body></html>"#;
        let page = popular_page(&source, &response("https://a.test/popular", body)).unwrap();
        assert_eq!(page.mangas[0].url, "https://cdn.test/site/m/1");
    }

    #[test]
    fn no_next_link_means_no_next_page() {
        let (source, _dir) = Catalogue::new(Some("a.next"));
        let page = popular_page(&source, &response("https://a.test/popular", LIST_POPULAR_NO_NEXT)).unwrap();
        assert_eq!(page.mangas.len(), 3);
        assert!(!page.has_next_page);
    }

    #[test]
    fn a_none_next_selector_is_never_a_next_page() {
        let (source, _dir) = Catalogue::new(None);
        let page = popular_page(&source, &response("https://a.test/popular", LIST_POPULAR)).unwrap();
        assert!(!page.has_next_page);
    }

    #[test]
    fn a_next_link_with_no_text_still_counts_as_a_next_page() {
        let (source, _dir) = Catalogue::new(Some("a.next"));
        let body: &'static [u8] = br#"<div class="item"><a class="title" href="/m/1">A</a></div><a class="next"></a>"#;
        let page = popular_page(&source, &response("https://a.test/popular", body)).unwrap();
        assert!(page.has_next_page);
    }

    #[test]
    fn zero_items_with_a_next_link_is_an_empty_page_with_a_next_page() {
        let (source, _dir) = Catalogue::new(Some("a.next"));
        let page = popular_page(&source, &response("https://a.test/popular", LIST_EMPTY)).unwrap();
        assert!(page.mangas.is_empty());
        assert!(page.has_next_page);
    }

    #[test]
    fn one_element_that_fails_fails_the_whole_page() {
        let (source, _dir) = Catalogue::new(Some("a.next"));
        let body: &'static [u8] = br#"<div class="item"><a class="title" href="/m/1">A</a></div><div class="item">no link</div>"#;
        let err = popular_page(&source, &response("https://a.test/popular", body)).unwrap_err();
        assert!(matches!(err, SourceError::Parse(_)));
    }

    #[test]
    fn an_invalid_selector_hook_is_a_parse_error() {
        let base = Url::parse("https://a.test/").unwrap();
        let items = parse_list(&Html::parse_document("<p></p>"), &base, "p[[[", |_, _| unreachable_manga(), None);
        assert!(matches!(items, Err(SourceError::Parse(_))));
        let next = parse_list(&Html::parse_document("<p></p>"), &base, "p", |_, _| unreachable_manga(), Some("a[["));
        assert!(matches!(next, Err(SourceError::Parse(_))));
    }

    fn unreachable_manga() -> Result<SManga, SourceError> {
        Ok(SManga::new("/x", "x"))
    }

    #[test]
    fn the_windows_1252_fixture_decodes_with_its_charset() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/html; charset=windows-1252"));
        let response = HttpResponse {
            status: 200,
            url: Url::parse("https://a.test/w").unwrap(),
            headers,
            body: Bytes::from_static(WINDOWS_1252),
        };
        let document = response.html();
        let title = select_text(document.root_element(), "h1").unwrap();
        assert_eq!(title.as_deref(), Some("Caf\u{e9} au lait"));
    }

    #[test]
    fn a_relative_image_resolves_against_the_base_element() {
        let (source, _dir) = Catalogue::new(None);
        let url = image_url_page(&source, &response("https://a.test/page/1", IMAGE_URL)).unwrap();
        assert_eq!(url, "https://cdn.test/img/1.jpg");
    }

    #[test]
    fn the_page_list_takes_its_base_from_the_document() {
        let (source, _dir) = Catalogue::new(None);
        let pages = page_list_page(&source, &response("https://a.test/page/1", IMAGE_URL)).unwrap();
        assert_eq!(pages[0].image_url.as_deref(), Some("https://cdn.test/img/1.jpg"));
        assert_eq!(pages[0].url, "1.jpg");
    }

    #[test]
    fn the_details_fixture_gives_its_title() {
        let (source, _dir) = Catalogue::new(None);
        let details = details_page(&source, &response("https://a.test/manga/1", DETAILS)).unwrap();
        assert_eq!(details.title, "Synthetic Details Title");
    }

    #[test]
    fn the_chapter_fixture_maps_every_row_with_nbsp_and_text_normalised() {
        let (source, _dir) = Catalogue::new(None);
        let chapters = chapter_list_page(&source, &response("https://a.test/list", CHAPTERS)).unwrap();
        let names: Vec<&str> = chapters.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Chapter 3", "Chapter 2 2 Jan 2024", "Chapter 2 again", "Chapter 1", "Prologue"]);
    }

    #[test]
    fn the_page_fixture_keeps_document_order_and_resolves_against_the_response() {
        let (source, _dir) = Catalogue::new(None);
        let pages = page_list_page(&source, &response("https://a.test/ch/1", PAGES)).unwrap();
        let urls: Vec<&str> = pages.iter().map(|p| p.url.as_str()).collect();
        assert_eq!(urls, ["p1.jpg", "p2.jpg", "p3.jpg"]);
        assert_eq!(pages[2].image_url.as_deref(), Some("https://a.test/ch/p3.jpg"));
    }

    #[test]
    fn a_details_page_without_a_title_still_parses() {
        let (source, _dir) = Catalogue::new(None);
        let body: &'static [u8] = b"<html><head></head><body></body></html>";
        let details = details_page(&source, &response("https://a.test/manga/1", body)).unwrap();
        assert_eq!(details.title, "");
    }
}
