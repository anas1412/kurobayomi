//! Selector-based parsing for anime sources (`ParsedAnimeHttpSource.kt`; T-16a-029, T-16a-030). A source configures its
//! CSS selectors and calls these helpers over the document of a response. Each helper finishes its selection before it
//! returns, so no `scraper` value lives across an `.await`. Relative links resolve against the document base, which is
//! the final URL after redirects unless the page has a `<base href>` (EC-PH-16).

use kuro_source::SourceError;
use kuro_source::anime::{AnimesPage, Hoster, SAnime, SEpisode, Video};
use scraper::{ElementRef, Html};
use url::Url;

use crate::html::{document_base, parse_selector};
use crate::response::HttpResponse;

/// `document.select(css).map(from)` (behaviours 4.3). Document order, no dedupe, and one failing element fails the
/// list. A selector the parser rejects is `Parse`, never an empty list (E-05).
pub fn parse_all<T>(
    response: &HttpResponse,
    css: &str,
    from: impl Fn(ElementRef<'_>, &Url) -> Result<T, SourceError>,
) -> Result<Vec<T>, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    let selector = parse_selector(css)?;
    document.select(&selector).map(|element| from(element, &base)).collect()
}

/// `parseAnime` for a listing page (behaviour 4.2): the items in document order and `has_next_page`. A `None` next
/// selector means no next page. A selector that matches at least one element means there is one.
pub fn parse_listing(
    response: &HttpResponse,
    item_css: &str,
    next_css: Option<&str>,
    from: impl Fn(ElementRef<'_>, &Url) -> Result<SAnime, SourceError>,
) -> Result<AnimesPage, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    let item = parse_selector(item_css)?;
    let animes = document.select(&item).map(|element| from(element, &base)).collect::<Result<Vec<_>, _>>()?;
    let has_next_page = match next_css {
        Some(css) => document.select(&parse_selector(css)?).next().is_some(),
        None => false,
    };
    Ok(AnimesPage { animes, has_next_page })
}

/// The selector hooks of a Parsed anime source (`ParsedAnimeHttpSource.kt`). Names follow the Kotlin base class in snake
/// case. The element hooks get the document base, so a source resolves a relative link as Jsoup's `abs:` does (EC-PH-16).
/// `hoster_selector` is `None` for a source with no hoster list. A source that sets it provides hosters (E-01).
pub trait ParsedAnimeHttpSource {
    fn episode_selector(&self) -> &'static str;
    fn episode_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SEpisode, SourceError>;

    /// `base` is the document base URI (see [`crate::html::document_base`]).
    fn details_from_document(&self, document: &Html, base: &Url) -> Result<SAnime, SourceError>;

    fn season_selector(&self) -> &'static str;
    fn season_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SAnime, SourceError>;

    fn hoster_selector(&self) -> Option<&'static str>;
    fn hoster_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<Hoster, SourceError>;

    fn video_selector(&self) -> &'static str;
    fn video_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<Video, SourceError>;

    /// The absolute URL of the source video of a page (`videoUrlParse(document)`).
    fn video_url_from_document(&self, document: &Html, base: &Url) -> Result<String, SourceError>;
}

/// `provides_hosters()` of a Parsed source: `true` exactly when a hoster selector is configured (E-01). Its `AnimeSource`
/// implementation returns this value.
pub fn provides_hosters<S: ParsedAnimeHttpSource + ?Sized>(source: &S) -> bool {
    source.hoster_selector().is_some()
}

/// `episodeListParse`: every episode element, in document order.
pub fn episode_list_page<S: ParsedAnimeHttpSource>(source: &S, response: &HttpResponse) -> Result<Vec<SEpisode>, SourceError> {
    parse_all(response, source.episode_selector(), |element, base| source.episode_from_element(element, base))
}

/// `seasonListParse`: every season element, in document order.
pub fn season_list_page<S: ParsedAnimeHttpSource>(source: &S, response: &HttpResponse) -> Result<Vec<SAnime>, SourceError> {
    parse_all(response, source.season_selector(), |element, base| source.season_from_element(element, base))
}

/// `hosterListParse`: every hoster element, in document order. A source without a hoster selector has no hoster list.
pub fn hoster_list_page<S: ParsedAnimeHttpSource>(source: &S, response: &HttpResponse) -> Result<Vec<Hoster>, SourceError> {
    let css = source.hoster_selector().ok_or(SourceError::Unsupported("hosters"))?;
    parse_all(response, css, |element, base| source.hoster_from_element(element, base))
}

/// `videoListParse` (the legacy list with no hoster): every video element, in document order.
pub fn video_list_page<S: ParsedAnimeHttpSource>(source: &S, response: &HttpResponse) -> Result<Vec<Video>, SourceError> {
    parse_all(response, source.video_selector(), |element, base| source.video_from_element(element, base))
}

/// `animeDetailsParse(response)`: the details of one anime, from the whole document.
pub fn details_page<S: ParsedAnimeHttpSource>(source: &S, response: &HttpResponse) -> Result<SAnime, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    source.details_from_document(&document, &base)
}

/// `videoUrlParse(response)`: the absolute URL of the source video of a page.
pub fn video_url_page<S: ParsedAnimeHttpSource>(source: &S, response: &HttpResponse) -> Result<String, SourceError> {
    let document = response.html();
    let base = document_base(&document, &response.url);
    source.video_url_from_document(&document, &base)
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use bytes::Bytes;
    use http::HeaderMap;
    use kuro_source::anime::{AnimeSource, Hoster, SEpisode, http_hosters};
    use kuro_source::{FilterList, SourceError};

    use super::*;
    use crate::html::{resolve, text};

    fn response(url: &str, body: &str) -> HttpResponse {
        HttpResponse {
            status: 200,
            url: Url::parse(url).unwrap(),
            headers: HeaderMap::new(),
            body: Bytes::from(body.to_owned()),
        }
    }

    /// A source whose selectors are configured, and whose pages are canned responses.
    struct Parsed {
        hoster_css: Option<&'static str>,
        page: HttpResponse,
    }

    #[async_trait]
    impl AnimeSource for Parsed {
        fn id(&self) -> i64 {
            9
        }
        fn name(&self) -> &str {
            "Parsed"
        }
        fn supports_latest(&self) -> bool {
            false
        }
        fn provides_hosters(&self) -> bool {
            super::provides_hosters(self)
        }
        async fn popular(&self, _: i32) -> Result<AnimesPage, SourceError> {
            Err(SourceError::Unsupported("popular"))
        }
        async fn latest(&self, _: i32) -> Result<AnimesPage, SourceError> {
            Err(SourceError::Unsupported("latest"))
        }
        async fn search(&self, _: i32, _: &str, _: &FilterList) -> Result<AnimesPage, SourceError> {
            Err(SourceError::Unsupported("search"))
        }
        async fn episode_list(&self, _: &SAnime) -> Result<Vec<SEpisode>, SourceError> {
            episode_list_page(self, &self.page)
        }
        async fn hosters(&self, _: &SEpisode) -> Result<Vec<Hoster>, SourceError> {
            hoster_list_page(self, &self.page)
        }
    }

    /// Element hooks for the test source. Links resolve against the document base, then store the URL without its domain.
    impl ParsedAnimeHttpSource for Parsed {
        fn episode_selector(&self) -> &'static str {
            "li.ep a"
        }
        fn episode_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SEpisode, SourceError> {
            let mut episode = SEpisode { name: text(element), ..Default::default() };
            let href = element.value().attr("href").unwrap_or_default();
            crate::anime_http::stored_episode_url(&mut episode, base, href)?;
            Ok(episode)
        }
        fn details_from_document(&self, document: &Html, _base: &Url) -> Result<SAnime, SourceError> {
            let heading = parse_selector("h1")?;
            let title = document.select(&heading).next().map(text).unwrap_or_default();
            Ok(SAnime { title, ..Default::default() })
        }
        fn season_selector(&self) -> &'static str {
            "a.season"
        }
        fn season_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<SAnime, SourceError> {
            let href = element.value().attr("href").unwrap_or_default();
            let absolute = resolve(base, href).ok_or_else(|| SourceError::Url(format!("cannot resolve {href:?}")))?;
            let mut season = SAnime { title: text(element), ..Default::default() };
            season.set_url_without_domain(&absolute);
            Ok(season)
        }
        fn hoster_selector(&self) -> Option<&'static str> {
            self.hoster_css
        }
        fn hoster_from_element(&self, element: ElementRef<'_>, _base: &Url) -> Result<Hoster, SourceError> {
            Ok(Hoster { hoster_name: text(element), ..Default::default() })
        }
        fn video_selector(&self) -> &'static str {
            "a.video"
        }
        fn video_from_element(&self, element: ElementRef<'_>, base: &Url) -> Result<Video, SourceError> {
            let href = element.value().attr("href").unwrap_or_default();
            let absolute = resolve(base, href).ok_or_else(|| SourceError::Url(format!("cannot resolve {href:?}")))?;
            Ok(Video::new(absolute, text(element)))
        }
        fn video_url_from_document(&self, document: &Html, base: &Url) -> Result<String, SourceError> {
            let source = parse_selector("video source")?;
            let src = document.select(&source).next().and_then(|e| e.value().attr("src")).unwrap_or_default();
            resolve(base, src).ok_or_else(|| SourceError::Url(format!("cannot resolve {src:?}")))
        }
    }

    const EPISODE_LIST: &[u8] = include_bytes!("../tests/fixtures/anime/episode_list.html");

    #[test]
    fn scrape_episode_list_document_order() {
        let page = HttpResponse {
            status: 200,
            url: Url::parse("https://show.test/show/").unwrap(),
            headers: HeaderMap::new(),
            body: Bytes::from_static(EPISODE_LIST),
        };
        let source = Parsed { hoster_css: None, page: page.clone() };
        let episodes = episode_list_page(&source, &page).unwrap();
        let names: Vec<&str> = episodes.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Episode 3", "Episode 1", "Episode 2", "Episode 10 & Recap"]);
        let urls: Vec<&str> = episodes.iter().map(|e| e.url.as_str()).collect();
        assert_eq!(urls, ["/show/ep-3", "/show/ep-1", "/show/ep-2", "/show/ep-10"]);
    }

    #[test]
    fn the_details_come_from_the_whole_document() {
        let page = response("https://show.test/show/", "<h1> Show &amp; Friends </h1><h1>Second</h1>");
        let source = Parsed { hoster_css: None, page: page.clone() };
        assert_eq!(details_page(&source, &page).unwrap().title, "Show & Friends");
        let empty = response("https://show.test/show/", "<p>none</p>");
        assert_eq!(details_page(&source, &empty).unwrap().title, "");
    }

    #[test]
    fn the_season_list_keeps_order_and_resolves_links_against_the_base() {
        let page = response(
            "https://show.test/show/",
            r#"<a class="season" href="season-2/">Two</a><a class="season" href="/show/season-1/">One</a>"#,
        );
        let source = Parsed { hoster_css: None, page: page.clone() };
        let seasons = season_list_page(&source, &page).unwrap();
        let titles: Vec<&str> = seasons.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Two", "One"]);
        let urls: Vec<&str> = seasons.iter().map(|s| s.url.as_str()).collect();
        assert_eq!(urls, ["/show/season-2/", "/show/season-1/"]);
    }

    #[test]
    fn the_video_list_keeps_document_order_and_resolves_links() {
        let page = response(
            "https://show.test/show/",
            r#"<a class="video" href="hd.m3u8">HD</a><a class="video" href="https://cdn.test/sd.m3u8">SD</a>"#,
        );
        let source = Parsed { hoster_css: None, page: page.clone() };
        let videos = video_list_page(&source, &page).unwrap();
        let titles: Vec<&str> = videos.iter().map(|v| v.video_title.as_str()).collect();
        assert_eq!(titles, ["HD", "SD"]);
        let urls: Vec<&str> = videos.iter().map(|v| v.video_url.as_str()).collect();
        assert_eq!(urls, ["https://show.test/show/hd.m3u8", "https://cdn.test/sd.m3u8"]);
    }

    #[test]
    fn the_video_url_is_the_absolute_source_of_the_page() {
        let page = response("https://show.test/watch/1", r#"<video><source src="/media/1.mp4"></video>"#);
        let source = Parsed { hoster_css: None, page: page.clone() };
        assert_eq!(video_url_page(&source, &page).unwrap(), "https://show.test/media/1.mp4");
    }

    #[test]
    fn a_hoster_list_without_a_hoster_selector_is_unsupported() {
        let page = response("https://show.test/", r#"<div class="server">A</div>"#);
        let source = Parsed { hoster_css: None, page: page.clone() };
        assert!(matches!(hoster_list_page(&source, &page), Err(SourceError::Unsupported("hosters"))));
    }

    #[test]
    fn a_base_element_is_the_base_of_relative_links() {
        let page = response(
            "https://show.test/series/",
            r#"<base href="https://mirror.test/x/"><a class="l" href="ep/9">n</a>"#,
        );
        let hrefs = parse_all(&page, "a.l", |element, base| Ok(resolve(base, element.value().attr("href").unwrap()))).unwrap();
        assert_eq!(hrefs, [Some("https://mirror.test/x/ep/9".to_owned())]);
    }

    #[tokio::test]
    async fn a_source_with_only_hoster_selectors_is_routed_to_its_hosters() {
        let source = Parsed { hoster_css: Some("div.server"), page: response("https://show.test/", r#"<div class="server">B</div><div class="server">A</div>"#) };
        assert!(source.provides_hosters());
        let hosters = http_hosters(&source, &SEpisode::default()).await.unwrap();
        let names: Vec<&str> = hosters.iter().map(|h| h.hoster_name.as_str()).collect();
        assert_eq!(names, ["B", "A"]);
    }

    #[tokio::test]
    async fn a_source_without_a_hoster_selector_is_not_asked_for_hosters() {
        let source = Parsed { hoster_css: None, page: response("https://show.test/", "") };
        assert!(!source.provides_hosters());
        assert!(matches!(source.hosters(&SEpisode::default()).await, Err(SourceError::Unsupported("hosters"))));
    }

    #[test]
    fn the_listing_reports_a_next_page_only_when_the_selector_matches() {
        let page = response("https://show.test/list", r#"<div class="item"><a>One</a></div><div class="item"><a>Two</a></div><a class="next">›</a>"#);
        let from = |element: ElementRef<'_>, _: &Url| Ok(SAnime { title: text(element), ..Default::default() });
        let with_next = parse_listing(&page, "div.item", Some("a.next"), from).unwrap();
        assert_eq!(with_next.animes.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(), ["One", "Two"]);
        assert!(with_next.has_next_page);
        assert!(!parse_listing(&page, "div.item", None, from).unwrap().has_next_page);
        assert!(!parse_listing(&page, "div.item", Some("a.missing"), from).unwrap().has_next_page);
    }

    #[test]
    fn a_selector_that_does_not_parse_is_an_error_not_an_empty_list() {
        let page = response("https://show.test/list", r#"<div class="item">x</div>"#);
        let from = |element: ElementRef<'_>, _: &Url| Ok(SAnime { title: text(element), ..Default::default() });
        assert!(matches!(parse_listing(&page, "div.item:contains(x)", None, from), Err(SourceError::Parse(_))));
        assert!(matches!(parse_listing(&page, "div.item", Some("a:matches(x)"), from), Err(SourceError::Parse(_))));
    }
}
