mod common;

use common::{ANIME, MANGA, fixture, with};
use kuro_source::SourceError;
use kuro_source_scrape::Recipe;
use kuro_source_scrape::engine::{self, parse_date};
use url::Url;

fn manga() -> Recipe {
    Recipe::parse(MANGA).unwrap()
}

fn anime() -> Recipe {
    Recipe::parse(ANIME).unwrap()
}

fn at(text: &str) -> Url {
    Url::parse(text).unwrap()
}

#[test]
fn a_listing_gives_titles_stored_links_and_resolved_covers() {
    let recipe = manga();
    let page = engine::listing(&recipe.popular, &fixture("manga", "popular.html"), &at("https://manga.example.org/popular?page=2"), 2).unwrap();
    assert_eq!(page.entries.len(), 2);
    assert_eq!(page.entries[0].title, "Blue Lake");
    assert_eq!(page.entries[0].url, "/series/blue-lake");
    assert_eq!(page.entries[0].cover.as_deref(), Some("https://cdn.example.org/blue-lake.jpg"));
    assert_eq!(page.entries[1].title, "Night Train", "whitespace is collapsed");
    assert_eq!(page.entries[1].cover.as_deref(), Some("https://manga.example.org/covers/night-train.jpg"), "a relative cover is resolved");
    assert!(page.has_next, "a.next matches");
}

#[test]
fn a_next_page_selector_that_matches_nothing_is_the_last_page() {
    let recipe = manga();
    let page = engine::listing(&recipe.search.unwrap(), &fixture("manga", "search.html"), &at("https://manga.example.org/search"), 1).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert!(!page.has_next);
}

#[test]
fn the_last_page_rule_compares_the_page_with_the_number_on_the_site() {
    let recipe = manga();
    let latest = recipe.latest.as_ref().unwrap();
    let html = fixture("manga", "latest.html");
    let url = at("https://manga.example.org/latest");
    assert!(engine::listing(latest, &html, &url, 2).unwrap().has_next, "page 2 of 5");
    assert!(engine::listing(latest, &html, &url, 4).unwrap().has_next);
    assert!(!engine::listing(latest, &html, &url, 5).unwrap().has_next, "page 5 of 5");
    let none = engine::listing(latest, "<ul class=updates><li><a class=name href=/a>A</a></li></ul>", &url, 1).unwrap();
    assert!(!none.has_next, "no page count on the page: one page");
    let err = engine::listing(latest, "<span class=pages>of x</span>", &url, 1);
    assert!(err.is_ok(), "the regex needs digits, so this is no match");
}

#[test]
fn a_page_count_that_is_not_a_number_is_an_error() {
    let text = MANGA.replace("regex = \"of (\\\\d+)\"", "regex = \"of (\\\\w+)\"");
    let recipe = Recipe::parse(&text).unwrap();
    let html = "<ul class=updates></ul><span class=pages>Page 1 of many</span>";
    let err = engine::listing(recipe.latest.as_ref().unwrap(), html, &at("https://manga.example.org/latest"), 1).unwrap_err();
    assert!(matches!(err, SourceError::Parse(ref m) if m.contains("many")), "{err}");
}

#[test]
fn an_item_without_a_title_fails_the_page_and_names_the_item() {
    let recipe = manga();
    let html = r#"<div class="series-card"><h3><a href="/a">A</a></h3></div><div class="series-card"><img></div>"#;
    let err = engine::listing(&recipe.popular, html, &at("https://manga.example.org/"), 1).unwrap_err();
    assert_eq!(err, SourceError::Parse("title: nothing matched (item 1)".into()));
}

#[test]
fn an_empty_value_counts_as_no_match() {
    let recipe = manga();
    let html = r#"<div class="series-card"><h3><a href="/a">  </a></h3></div>"#;
    assert!(engine::listing(&recipe.popular, html, &at("https://manga.example.org/"), 1).is_err());
}

#[test]
fn details_pick_nth_matches_join_all_and_map_the_status() {
    let recipe = manga();
    let found = engine::details(&recipe.details, &fixture("manga", "series.html"), &at("https://manga.example.org/series/blue-lake")).unwrap();
    assert_eq!(found.title.as_deref(), Some("Blue Lake"));
    assert_eq!(found.author.as_deref(), Some("Mina Okada"), "nth 0");
    assert_eq!(found.artist.as_deref(), Some("Ken Sato"), "nth 1");
    assert_eq!(found.description.as_deref(), Some("A lake town keeps a secret.\n\nSomeone is going to find out."));
    assert_eq!(found.genre.as_deref(), Some("Drama, Slice of Life, Mystery"));
    assert_eq!(found.cover.as_deref(), Some("https://manga.example.org/covers/blue-lake-large.jpg"));
    assert_eq!(found.status, Some(1), "ongoing");
}

#[test]
fn the_status_words_are_matched_anywhere_in_the_lower_cased_text() {
    let recipe = manga();
    let status = |text: &str| {
        let html = format!("<span class=status>{text}</span>");
        engine::details(&recipe.details, &html, &at("https://manga.example.org/")).unwrap().status
    };
    assert_eq!(status("FINISHED in 2020"), Some(2));
    assert_eq!(status("Hiatus?"), Some(0), "no word matches: unknown");
    assert_eq!(status("Completed (was ongoing)"), Some(1), "two entries match: the first in enum order (ongoing) wins, as the docs say");
    let none = engine::details(&recipe.details, "<p>nothing</p>", &at("https://manga.example.org/")).unwrap();
    assert_eq!(none, Default::default());
}

#[test]
fn text_has_a_space_at_block_and_br_boundaries_but_not_between_inline_elements() {
    let from = "description = { select = \"div.summary p\", all = true, join = \"\\n\\n\" }";
    let recipe = with(MANGA, from, "description = { select = \"div.summary\" }").unwrap();
    let html = "<div class=summary><p>A secret.</p><p>Someone knows.<br>Run<b>ning</b></p></div>";
    let found = engine::details(&recipe.details, html, &at("https://manga.example.org/")).unwrap();
    assert_eq!(found.description.as_deref(), Some("A secret. Someone knows. Running"));
}

#[test]
fn nth_takes_the_nth_match_and_no_such_match_is_no_value() {
    let text = MANGA.replace("{ select = \"h3 a\" }\nurl", "{ select = \"h3 a\", nth = 1 }\nurl");
    let recipe = Recipe::parse(&text).unwrap();
    let html = r#"<div class="series-card"><h3><a href="/a">First</a><a href="/a">Second</a></h3></div>"#;
    let page = engine::listing(&recipe.popular, html, &at("https://manga.example.org/"), 1).unwrap();
    assert_eq!(page.entries[0].title, "Second");
    let single = r#"<div class="series-card"><h3><a href="/a">Only</a></h3></div>"#;
    assert!(engine::listing(&recipe.popular, single, &at("https://manga.example.org/"), 1).is_err());
}

#[test]
fn a_regex_takes_its_first_group_or_the_whole_match() {
    let text = MANGA.replace("regex = \"Chapter ([0-9.]+)\"", "regex = \"Chapter [0-9.]+\"");
    let recipe = Recipe::parse(&text).unwrap();
    let items = engine::chapters(recipe.chapters.as_ref().unwrap(), &fixture("manga", "series.html"), &at("https://manga.example.org/series/blue-lake")).unwrap();
    assert_eq!(items[0].number, None, "the whole match is `Chapter 2.5`, which is not a number");
    let plain = manga();
    let items = engine::chapters(plain.chapters.as_ref().unwrap(), &fixture("manga", "series.html"), &at("https://manga.example.org/series/blue-lake")).unwrap();
    assert_eq!(items[0].number, Some(2.5), "the group is the number");
}

#[test]
fn format_builds_a_link_from_a_bare_value_after_the_regex() {
    let text = MANGA.replace("url = { select = \"a\", attr = \"href\" }\nnumber", "url = { select = \"a\", regex = \"Chapter ([0-9.]+)\", format = \"/read?nro={}\" }\nnumber");
    let recipe = Recipe::parse(&text).unwrap();
    let items = engine::chapters(recipe.chapters.as_ref().unwrap(), &fixture("manga", "series.html"), &at("https://manga.example.org/series/blue-lake")).unwrap();
    assert_eq!(items[0].url, "/read?nro=2.5");
    assert!(with(MANGA, "url = { select = \"a\", attr = \"href\" }\nnumber", "url = { select = \"a\", format = \"/read\" }\nnumber").unwrap_err().contains("`format` needs"));
}

#[test]
fn chapters_come_newest_first_with_number_and_date() {
    let recipe = manga();
    let items = engine::chapters(recipe.chapters.as_ref().unwrap(), &fixture("manga", "series.html"), &at("https://manga.example.org/series/blue-lake")).unwrap();
    assert_eq!(items.iter().map(|i| i.url.as_str()).collect::<Vec<_>>(), ["/series/blue-lake/chapter-2.5", "/series/blue-lake/chapter-2", "/series/blue-lake/chapter-1"], "reverse = true");
    assert_eq!(items[0].name, "Chapter 2.5: Extra");
    assert_eq!(items[2].number, Some(1.0));
    assert_eq!(items[2].date, Some(1_709_251_200_000), "2024-03-01 UTC");
    assert_eq!(items[0].scanlator, None);
}

#[test]
fn dates_read_the_formats_a_recipe_names() {
    assert_eq!(parse_date("2024-03-01", "%Y-%m-%d"), Some(1_709_251_200_000));
    assert_eq!(parse_date("March 1, 2024", "%B %d, %Y"), Some(1_709_251_200_000));
    assert_eq!(parse_date("2024-03-01 12:00:30", "%Y-%m-%d %H:%M:%S"), Some(1_709_251_200_000 + 43_230_000));
    assert_eq!(parse_date("1709251200", "unix"), Some(1_709_251_200_000));
    assert_eq!(parse_date("1709251200123", "unix_ms"), Some(1_709_251_200_123));
    assert_eq!(parse_date("yesterday", "%Y-%m-%d"), None);
    assert_eq!(parse_date("2024-03-01", "unix"), None);
}

#[test]
fn pages_resolve_against_the_base_element_of_the_page() {
    let recipe = manga();
    let images = engine::pages(recipe.pages.as_ref().unwrap(), &fixture("manga", "chapter.html"), &at("https://manga.example.org/read/1")).unwrap();
    assert_eq!(
        images,
        ["https://manga.example.org/series/blue-lake/chapter-1/001.jpg", "https://cdn.example.org/blue-lake/1/002.jpg", "https://manga.example.org/img/blue-lake/1/003.jpg"]
    );
}

#[test]
fn a_chapter_without_images_is_an_empty_page_list() {
    let recipe = manga();
    let err = engine::pages(recipe.pages.as_ref().unwrap(), "<div class=reader></div>", &at("https://manga.example.org/")).unwrap_err();
    assert_eq!(err, SourceError::EmptyPageList);
}

#[test]
fn episodes_are_listed_with_their_numbers_newest_first() {
    let recipe = anime();
    let items = engine::chapters(recipe.episodes.as_ref().unwrap(), &fixture("anime", "show.html"), &at("https://anime.example.org/watch/sky-harbor")).unwrap();
    assert_eq!(items.iter().map(|i| (i.name.as_str(), i.number)).collect::<Vec<_>>(), [("Episode 12 - The Captain", Some(12.0)), ("Episode 11 - Storm", Some(11.0))], "the site lists newest first, so the recipe does not reverse");
    assert_eq!(items[0].url, "/watch/sky-harbor/episode-12");
}

#[test]
fn videos_are_the_direct_files_and_playlists_as_the_page_gives_them() {
    let recipe = anime();
    let found = engine::videos(recipe.videos.as_ref().unwrap(), &fixture("anime", "episode.html"), &at("https://anime.example.org/watch/sky-harbor/episode-12")).unwrap();
    let rows: Vec<_> = found.iter().map(|v| (v.title.as_str(), v.url.as_str())).collect();
    assert_eq!(
        rows,
        [
            ("720p", "https://stream.example.org/sky-harbor/12/720.mp4"),
            ("Auto (HLS)", "https://anime.example.org/hls/sky-harbor/12/master.m3u8"),
            ("Video", "https://stream.example.org/sky-harbor/12/360.mp4"),
        ]
    );
}

/// The regex over the page scripts that `docs/source-recipes.md` shows.
#[test]
fn a_stream_address_can_be_read_out_of_a_script() {
    let text = ANIME.replace(
        "items = \"video source\"\nurl = { attr = \"src\" }\ntitle = { attr = \"label\" }",
        "url = { select = \"script\", all = true, regex = \"file: \\\"(https?://[^\\\"]+\\\\.m3u8)\\\"\" }",
    );
    let recipe = Recipe::parse(&text).unwrap();
    let html = r#"<script>player.setup({ file: "https://stream.example.org/x/master.m3u8", poster: "p.jpg" });</script><script>var other = 1;</script>"#;
    let found = engine::videos(recipe.videos.as_ref().unwrap(), html, &at("https://anime.example.org/ep")).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].url.as_str(), found[0].title.as_str()), ("https://stream.example.org/x/master.m3u8", "Video"));
    let err = engine::videos(recipe.videos.as_ref().unwrap(), "<p>no player</p>", &at("https://anime.example.org/ep")).unwrap_err();
    assert!(matches!(err, SourceError::Parse(_)));
}
