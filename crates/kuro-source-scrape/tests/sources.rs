//! The two sources against a local mock site: the traits, the headers, the rate limit and the host rule.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{ANIME, MANGA, client, fixture};
use kuro_source::anime::AnimeSource;
use kuro_source::model::{SChapter, SManga};
use kuro_source::{CatalogueSource, FilterList, MangaSource, SourceError};
use kuro_source_scrape::{Recipe, ScrapeAnime, ScrapeManga};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The recipe with its site (only `base_urls`) moved to the mock server, one host, and no wait between requests.
fn pointed(text: &str, server: &MockServer) -> Arc<Recipe> {
    let start = text.find("base_urls = [\"").expect("base_urls");
    let end = start + text[start..].find("]\n").expect("end") + 1;
    let text = format!("{}base_urls = [\"{}\"]{}", &text[..start], server.uri(), &text[end..]).replace("rate_limit_ms = 1000", "rate_limit_ms = 0");
    Arc::new(Recipe::parse(&text).expect("recipe"))
}

async fn serve(server: &MockServer, route: &str, body: String) {
    Mock::given(method("GET")).and(path(route)).respond_with(ResponseTemplate::new(200).set_body_string(body)).mount(server).await;
}

#[tokio::test]
async fn the_manga_source_lists_searches_and_reads_a_series() {
    let server = MockServer::start().await;
    serve(&server, "/popular", fixture("manga", "popular.html")).await;
    serve(&server, "/latest", fixture("manga", "latest.html")).await;
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "blue lake"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("manga", "search.html")))
        .mount(&server)
        .await;
    serve(&server, "/series/blue-lake", fixture("manga", "series.html")).await;
    serve(&server, "/series/blue-lake/chapter-1", fixture("manga", "chapter.html")).await;
    let (client, _dir) = client();
    let source = ScrapeManga::new(client, pointed(MANGA, &server)).expect("manga");

    assert_eq!((source.name(), source.lang(), source.supports_latest()), ("Example Manga", "en", true));
    assert_eq!(source.id(), kuro_source::generate_id("Example Manga", "en", 1));
    let popular = source.popular(1).await.unwrap();
    assert_eq!((popular.mangas.len(), popular.has_next_page), (2, true));
    assert_eq!(popular.mangas[0].url, "/series/blue-lake");
    assert_eq!(popular.mangas[0].thumbnail_url.as_deref(), Some("https://cdn.example.org/blue-lake.jpg"));
    let latest = source.latest(2).await.unwrap();
    assert_eq!((latest.mangas.len(), latest.has_next_page), (2, true));
    let found = source.search(1, "blue lake", &FilterList::default()).await.unwrap();
    assert_eq!(found.mangas[0].title, "Blue Lake");

    let manga = SManga::new("/series/blue-lake", "Blue Lake (from the list)");
    let details = source.get_manga_details(&manga).await.unwrap();
    assert_eq!((details.title.as_str(), details.author.as_deref(), details.status, details.initialized), ("Blue Lake", Some("Mina Okada"), 1, true));
    assert_eq!(manga.title, "Blue Lake (from the list)", "the input is never changed");
    let chapters = source.get_chapter_list(&manga).await.unwrap();
    assert_eq!(chapters.len(), 3);
    assert_eq!((chapters[0].chapter_number, chapters[2].date_upload), (2.5, 1_709_251_200_000));
    let pages = source.get_page_list(&SChapter::new("/series/blue-lake/chapter-1", "Chapter 1")).await.unwrap();
    assert_eq!(pages.len(), 3);
    assert_eq!(pages[1].image_url.as_deref(), Some("https://cdn.example.org/blue-lake/1/002.jpg"));
    assert_eq!(pages[2].index, 2);
}

#[tokio::test]
async fn a_recipe_without_latest_or_search_says_so() {
    let server = MockServer::start().await;
    let text = MANGA.replace("latest = \"/latest?page={page}\"\n", "").replace("search = \"/search?q={query}&page={page}\"\n", "");
    let latest_start = text.find("[latest]").unwrap();
    let details_start = text.find("[details]").unwrap();
    let text = format!("{}{}", &text[..latest_start], &text[details_start..]);
    let recipe = Arc::new(Recipe::parse(&text.replace("https://manga.example.org", &server.uri())).expect("recipe"));
    let (client, _dir) = client();
    let source = ScrapeManga::new(client, recipe).unwrap();
    assert!(!source.supports_latest());
    assert_eq!(source.latest(1).await.unwrap_err(), SourceError::Unsupported("latest"));
    assert_eq!(source.search(1, "x", &FilterList::default()).await.unwrap_err(), SourceError::Unsupported("search"));
}

#[tokio::test]
async fn a_source_is_only_made_for_its_own_kind() {
    let (client, _dir) = client();
    assert!(ScrapeAnime::new(client, Arc::new(Recipe::parse(MANGA).unwrap())).is_none());
    let (client, _dir) = common::client();
    assert!(ScrapeManga::new(client, Arc::new(Recipe::parse(ANIME).unwrap())).is_none());
}

#[tokio::test]
async fn the_recipe_headers_are_sent_with_every_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/popular"))
        .and(header("referer", "https://manga.example.org/"))
        .and(header("x-extra", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("manga", "popular.html")))
        .expect(1)
        .mount(&server)
        .await;
    let text = MANGA.replace("Referer = \"https://manga.example.org/\"", "Referer = \"https://manga.example.org/\"\nX-Extra = \"1\"");
    let (client, _dir) = client();
    let source = ScrapeManga::new(client, pointed(&text, &server)).unwrap();
    // a request without both headers finds no mock and is answered 404
    source.popular(1).await.expect("the headers were sent");
}

#[tokio::test]
async fn requests_are_spaced_by_the_rate_limit() {
    let server = MockServer::start().await;
    serve(&server, "/popular", fixture("manga", "popular.html")).await;
    let text = MANGA.replace("https://manga.example.org\"]", &format!("{}\"]", server.uri())).replace("rate_limit_ms = 1000", "rate_limit_ms = 400");
    let (client, _dir) = client();
    let source = ScrapeManga::new(client, Arc::new(Recipe::parse(&text).unwrap())).unwrap();
    let started = Instant::now();
    source.popular(1).await.unwrap();
    assert!(started.elapsed() < Duration::from_millis(300), "the first request does not wait");
    source.popular(2).await.unwrap();
    source.popular(3).await.unwrap();
    assert!(started.elapsed() >= Duration::from_millis(800), "two waits of 400 ms, took {:?}", started.elapsed());
}

#[tokio::test]
async fn requests_never_overlap() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/popular"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("manga", "popular.html")).set_delay(Duration::from_millis(200)))
        .mount(&server)
        .await;
    let (client, _dir) = client();
    let source = ScrapeManga::new(client, pointed(MANGA, &server)).unwrap();
    let started = Instant::now();
    let (a, b) = tokio::join!(source.popular(1), source.popular(2));
    a.unwrap();
    b.unwrap();
    assert!(started.elapsed() >= Duration::from_millis(400), "one after the other, took {:?}", started.elapsed());
}

#[tokio::test]
async fn a_redirect_to_another_host_is_an_error() {
    let server = MockServer::start().await;
    let elsewhere = format!("http://localhost:{}/popular", server.address().port());
    Mock::given(method("GET"))
        .and(path("/popular"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", elsewhere.as_str()))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    serve(&server, "/popular", fixture("manga", "popular.html")).await;
    let (client, _dir) = client();
    let source = ScrapeManga::new(client, pointed(MANGA, &server)).unwrap();
    let err = source.popular(1).await.unwrap_err();
    assert!(matches!(err, SourceError::Url(ref m) if m.contains("outside base_urls")), "{err:?}");
}

#[tokio::test]
async fn a_stored_link_to_another_host_is_never_requested() {
    let server = MockServer::start().await;
    let (client, _dir) = client();
    let source = ScrapeManga::new(client, pointed(MANGA, &server)).unwrap();
    let err = source.get_manga_details(&SManga::new("https://evil.test/series/x", "X")).await.unwrap_err();
    assert!(matches!(err, SourceError::Url(_)), "{err:?}");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn an_error_status_is_an_http_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(503)).mount(&server).await;
    let (client, _dir) = client();
    let source = ScrapeManga::new(client, pointed(MANGA, &server)).unwrap();
    assert_eq!(source.popular(1).await.unwrap_err(), SourceError::Http(503));
}

#[tokio::test]
async fn the_anime_source_lists_episodes_and_gives_the_direct_video_addresses() {
    let server = MockServer::start().await;
    serve(&server, "/anime", fixture("anime", "popular.html")).await;
    serve(&server, "/watch/sky-harbor", fixture("anime", "show.html")).await;
    Mock::given(method("GET"))
        .and(path("/watch/sky-harbor/episode-12"))
        .and(header("referer", "https://anime.example.org/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("anime", "episode.html")))
        .mount(&server)
        .await;
    let (client, _dir) = client();
    let source = ScrapeAnime::new(client, pointed(ANIME, &server)).expect("anime");

    assert_eq!((source.name(), source.supports_latest(), source.provides_hosters(), source.supports_season_list()), ("Example Anime", false, false, false));
    assert_eq!(source.id(), kuro_source::generate_id("Example Anime", "en", 1));
    let popular = source.popular(1).await.unwrap();
    assert_eq!((popular.animes.len(), popular.has_next_page), (2, true));
    assert_eq!(popular.animes[0].url, "/watch/sky-harbor");
    assert_eq!(popular.animes[1].url, "/watch/tin-robot");
    assert_eq!(source.latest(1).await.unwrap_err(), SourceError::Unsupported("latest"));

    let anime = source.anime_details(&popular.animes[0]).await.unwrap();
    assert_eq!((anime.title.as_str(), anime.initialized), ("Sky Harbor", true));
    assert_eq!(anime.status, kuro_source::anime::AnimeStatus::Ongoing);
    assert_eq!(anime.genre.as_deref(), Some("Adventure, Fantasy"));
    let episodes = source.episode_list(&anime).await.unwrap();
    assert_eq!((episodes.len(), episodes[0].episode_number), (2, 12.0));
    assert_eq!(episodes[0].url, "/watch/sky-harbor/episode-12");

    let videos = source.videos_for_episode(&episodes[0]).await.unwrap();
    assert_eq!(videos.len(), 3);
    assert_eq!(videos[0].video_url, "https://stream.example.org/sky-harbor/12/720.mp4");
    assert!(videos[1].video_url.ends_with("/hls/sky-harbor/12/master.m3u8"), "{}", videos[1].video_url);
    assert!(videos.iter().all(|v| v.initialized));
    assert_eq!(videos[0].headers.as_ref().map(|h| h.0.clone()), Some(vec![("Referer".to_owned(), "https://anime.example.org/".to_owned())]));
}
