//! The checker: what it prints, and when it fails (which is when `check_recipe` exits non-zero).

mod common;

use common::{ANIME, MANGA, dir};
use kuro_source_scrape::Recipe;
use kuro_source_scrape::check::{FixtureFeed, polite, run};

async fn check(recipe_text: &str, kind: &str) -> kuro_source_scrape::check::Report {
    let recipe = Recipe::parse(recipe_text).unwrap();
    run(&recipe, &mut FixtureFeed::new(&recipe, dir(kind))).await
}

#[tokio::test]
async fn the_manga_example_passes_every_operation_and_prints_what_it_found() {
    let report = check(MANGA, "manga").await;
    assert!(!report.failed(), "{report}");
    let ops: Vec<_> = report.lines.iter().map(|l| l.op).collect();
    assert_eq!(ops, ["popular", "latest", "search", "details", "chapters", "pages"]);
    let text = report.to_string();
    assert!(text.contains("2 entries, next page: yes"), "{text}");
    assert!(text.contains("Blue Lake -> /series/blue-lake"), "{text}");
    assert!(text.contains("3 found"), "{text}");
    assert!(text.contains("3 images"), "{text}");
    assert!(text.contains("author: Mina Okada"), "{text}");
}

#[tokio::test]
async fn the_anime_example_passes_every_operation() {
    let report = check(ANIME, "anime").await;
    assert!(!report.failed(), "{report}");
    let ops: Vec<_> = report.lines.iter().map(|l| l.op).collect();
    assert_eq!(ops, ["popular", "search", "details", "episodes", "videos"]);
    assert!(report.to_string().contains("3 videos"), "{report}");
}

#[tokio::test]
async fn a_chapter_address_off_the_expected_shape_fails_the_check() {
    let wrong = MANGA.replace("query = \"one piece\"", "query = \"one piece\"\nitem_url = \"^/nope/\"");
    let report = check(&wrong, "manga").await;
    let line = report.lines.iter().find(|l| l.op == "chapters").unwrap();
    assert!(!line.ok && line.text.contains("does not match tests.item_url"), "{report}");
    let right = MANGA.replace("query = \"one piece\"", "query = \"one piece\"\nitem_url = \"^/series/\"");
    let report = check(&right, "manga").await;
    assert!(!report.failed(), "{report}");
}

#[tokio::test]
async fn an_operation_that_returns_nothing_fails_the_check() {
    let broken = MANGA.replace("items = \"div.reader img\"", "items = \"div.reader video\"");
    let report = check(&broken, "manga").await;
    assert!(report.failed());
    let pages = report.lines.iter().find(|l| l.op == "pages").unwrap();
    assert!(!pages.ok && pages.text.contains("empty"), "{}", pages.text);
    assert!(report.lines.iter().filter(|l| l.op != "pages").all(|l| l.ok), "the other operations still ran");
}

#[tokio::test]
async fn a_listing_with_no_entries_fails() {
    let broken = MANGA.replace("items = \"ul.updates li\"", "items = \"ul.nothing li\"");
    let report = check(&broken, "manga").await;
    let latest = report.lines.iter().find(|l| l.op == "latest").unwrap();
    assert!(report.failed() && !latest.ok && latest.text == "returned nothing");
}

#[tokio::test]
async fn an_operation_without_a_fixture_fails() {
    let report = check(&MANGA.replace("search = \"search.html\"\n", ""), "manga").await;
    let search = report.lines.iter().find(|l| l.op == "search").unwrap();
    assert!(report.failed() && search.text.contains("no fixture named for `search`"), "{}", search.text);
}

#[tokio::test]
async fn a_fixture_file_that_is_missing_fails() {
    let report = check(&MANGA.replace("popular = \"popular.html\"", "popular = \"gone.html\""), "manga").await;
    let popular = &report.lines[0];
    assert!(!popular.ok && popular.text.contains("gone.html"), "{}", popular.text);
    assert!(report.failed());
}

#[test]
fn the_live_check_is_polite() {
    let recipe = polite(Recipe::parse(&MANGA.replace("rate_limit_ms = 1000", "rate_limit_ms = 10")).unwrap());
    assert_eq!(recipe.rate_limit_ms, 1000);
    assert!(recipe.headers["User-Agent"].starts_with("KurobayomiRecipeCheck/"));
    assert_eq!(recipe.headers["Referer"], "https://manga.example.org/", "the recipe's other headers stay");
    let slow = polite(Recipe::parse(&MANGA.replace("rate_limit_ms = 1000", "rate_limit_ms = 3000")).unwrap());
    assert_eq!(slow.rate_limit_ms, 3000, "a slower recipe keeps its own limit");
}

#[tokio::test]
async fn a_configured_part_that_finds_nothing_fails_even_when_the_operation_returns_something() {
    let report = |from: &str, to: &str| {
        let broken = MANGA.replace(from, to);
        assert_ne!(broken, MANGA, "{from:?} is not in the recipe");
        async move { check(&broken, "manga").await }
    };
    let line = |report: &kuro_source_scrape::check::Report, op: &str| report.lines.iter().find(|l| l.op == op).unwrap().clone();

    let dates = report("date_format = \"%Y-%m-%d\"", "date_format = \"%d/%m/%Y\"").await;
    let chapters = line(&dates, "chapters");
    assert!(!chapters.ok && chapters.text.contains("found on none of the items: date"), "{}", chapters.text);

    let numbers = report("regex = \"Chapter ([0-9.]+)\"", "regex = \"Nr ([0-9.]+)\"").await;
    assert!(line(&numbers, "chapters").text.contains("number"), "{numbers}");

    let status = report("ongoing = [\"ongoing\"]\ncompleted = [\"completed\", \"finished\"]", "ongoing = [\"running\"]\ncompleted = [\"done\"]").await;
    let details = line(&status, "details");
    assert!(!details.ok && details.text.contains("found nothing: status"), "{}", details.text);

    let title = report("title = { select = \"h1.title\" }", "title = { select = \"h1.name\" }").await;
    assert!(line(&title, "details").text.contains("found nothing: title"), "{title}");
}
