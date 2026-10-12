mod common;

use kuro_platform::kind::Kind;
use common::{ANIME, MANGA, with};
use kuro_source_scrape::{Recipe, load_dir};
use url::Url;

#[test]
fn both_example_recipes_are_valid() {
    let manga = Recipe::parse(MANGA).expect("manga");
    assert_eq!((manga.kind, manga.id.as_str(), manga.base(), manga.version), (Kind::Manga, "example-manga", "https://manga.example.org", 1));
    let anime = Recipe::parse(ANIME).expect("anime");
    assert_eq!((anime.kind, anime.lang.as_str(), anime.rate_limit_ms), (Kind::Anime, "en", 1000));
}

/// The two recipes in `docs/source-recipes.md` are the tested fixtures, word for word.
#[test]
fn the_recipes_in_the_docs_are_the_tested_ones() {
    let docs = include_str!("../../../docs/source-recipes.md");
    let block = |tag: &str| {
        let start = docs.find(&format!("```toml {tag}\n")).expect("block") + tag.len() + 9;
        let end = start + docs[start..].find("\n```").expect("end");
        format!("{}\n", &docs[start..end])
    };
    assert_eq!(block("manga"), MANGA);
    assert_eq!(block("anime"), ANIME);
}

#[test]
fn an_unknown_key_is_an_error_at_every_level() {
    assert!(with(MANGA, "lang = \"en\"", "lang = \"en\"\nflavour = 1").unwrap_err().contains("flavour"));
    assert!(with(MANGA, "[popular]\nitems", "[popular]\nsurprise = \"x\"\nitems").unwrap_err().contains("surprise"));
    assert!(with(MANGA, "attr = \"data-src\" }\nnext_page", "attr = \"data-src\", nope = 1 }\nnext_page").unwrap_err().contains("nope"));
    assert!(with(MANGA, "[tests]\n", "[tests]\nbogus = \"x\"\n").unwrap_err().contains("bogus"));
    assert!(with(MANGA, "[urls]\n", "[urls]\nrandom = \"/x\"\n").unwrap_err().contains("random"));
}

#[test]
fn a_selector_or_a_regex_that_does_not_compile_is_an_error() {
    let err = with(MANGA, "items = \"div.series-card\"\ntitle = { select = \"h3 a\" }\nurl = { select = \"h3 a\", attr = \"href\" }\ncover = { select = \"img\", attr = \"data-src\" }\nnext_page", "items = \"div.series-card[[\"\ntitle = { select = \"h3 a\" }\nurl = { select = \"h3 a\", attr = \"href\" }\ncover = { select = \"img\", attr = \"data-src\" }\nnext_page").unwrap_err();
    assert!(err.contains("bad selector") && err.contains("series-card[["), "{err}");
    let err = with(MANGA, "regex = \"Chapter ([0-9.]+)\"", "regex = \"Chapter ([0-9.]+\"").unwrap_err();
    assert!(err.contains("bad regex"), "{err}");
}

#[test]
fn only_page_and_query_may_appear_in_a_url_template() {
    assert!(with(MANGA, "/popular?page={page}", "/popular?page={offset}").unwrap_err().contains("only {page} and {query}"));
    assert!(with(MANGA, "/popular?page={page}", "/popular?page={page").is_err());
}

#[test]
fn an_absolute_template_must_name_a_listed_host() {
    let err = with(MANGA, "/popular?page={page}", "https://evil.test/popular?page={page}").unwrap_err();
    assert!(err.contains("outside base_urls"), "{err}");
    with(MANGA, "/popular?page={page}", "https://manga.example.org/popular?page={page}").expect("a listed host");
    assert!(with(MANGA, "/popular?page={page}", "popular?page={page}").is_err(), "neither a path nor an address");
}

#[test]
fn requests_outside_base_urls_are_refused() {
    let recipe = Recipe::parse(MANGA).unwrap();
    assert!(recipe.address("/series/x").is_ok());
    assert!(recipe.address("series/x").is_ok());
    assert!(recipe.address("https://manga.example.org/series/x").is_ok());
    assert!(recipe.address("https://evil.test/x").is_err());
    assert!(recipe.address("https://www.manga.example.org/x").is_err(), "subdomains are not covered");
    assert!(recipe.check_host(&Url::parse("ftp://manga.example.org/x").unwrap()).is_err(), "only http and https");
    assert!(recipe.check_host(&Url::parse("https://manga.example.org:8443/x").unwrap()).is_ok(), "a port is not a different host");
}

#[test]
fn the_query_is_percent_encoded_into_the_template() {
    let recipe = Recipe::parse(MANGA).unwrap();
    let url = recipe.listing_url(recipe.urls.search.as_ref().unwrap(), 3, "one piece & more+").unwrap();
    assert_eq!(url.as_str(), "https://manga.example.org/search?q=one%20piece%20%26%20more%2B&page=3");
}

#[test]
fn a_kind_needs_its_own_operations_and_refuses_the_other_kinds() {
    let err = with(MANGA, "[pages]\nitems = \"div.reader img\"\nimage = { attr = \"data-src\" }\n", "").unwrap_err();
    assert!(err.contains("needs `pages`"), "{err}");
    let err = with(MANGA, "[tests]", "[episodes]\nitems = \"li\"\nname = {}\nurl = { attr = \"href\" }\n\n[tests]").unwrap_err();
    assert!(err.contains("`episodes` does not belong"), "{err}");
    let err = with(ANIME, "[videos]\nitems = \"video source\"\nurl = { attr = \"src\" }\ntitle = { attr = \"label\" }\ndefault_title = \"Video\"\n", "").unwrap_err();
    assert!(err.contains("needs `videos`"), "{err}");
}

#[test]
fn latest_and_search_need_their_url_and_the_other_way_round() {
    assert!(with(MANGA, "latest = \"/latest?page={page}\"\n", "").unwrap_err().contains("latest"));
    assert!(with(ANIME, "search = \"/search?keyword={query}&page={page}\"\n", "").unwrap_err().contains("search"));
}

#[test]
fn a_value_table_rejects_contradictions() {
    assert!(with(MANGA, "{ select = \"dl dd\", nth = 0 }", "{ select = \"dl dd\", nth = 1, all = true }").unwrap_err().contains("`all` and `nth`"));
    assert!(with(MANGA, "{ select = \"dl dd\", nth = 0 }", "{ nth = 1 }").unwrap_err().contains("`nth` needs a `select`"));
    assert!(with(MANGA, "{ select = \"h3 a\" }\nurl = { select = \"h3 a\", attr = \"href\" }\ncover = { select = \"img\", attr = \"data-src\" }\nnext_page", "{ select = \"h3 a\", join = \"-\" }\nurl = { select = \"h3 a\", attr = \"href\" }\ncover = { select = \"img\", attr = \"data-src\" }\nnext_page").unwrap_err().contains("`join` needs"));
}

#[test]
fn small_rules_of_the_top_level() {
    assert!(with(MANGA, "id = \"example-manga\"", "id = \"../etc\"").unwrap_err().contains("id"));
    assert!(with(MANGA, "base_urls = [\"https://manga.example.org\"]", "base_urls = []").unwrap_err().contains("base_urls"));
    assert!(with(MANGA, "base_urls = [\"https://manga.example.org\"]", "base_urls = [\"ftp://manga.example.org\"]").unwrap_err().contains("base_urls"));
    assert!(with(MANGA, "[headers]\n", "[headers]\nHost = \"evil.test\"\n").unwrap_err().contains("Host"));
    assert!(with(MANGA, "rate_limit_ms = 1000", "rate_limit_ms = 600000").unwrap_err().contains("rate_limit_ms"));
    assert!(with(MANGA, "next_page = \"a.next\"\n\n[latest]", "next_page = \"a.next\"\nlast_page = { select = \"b\" }\n\n[latest]").unwrap_err().contains("not both"));
    assert!(with(MANGA, "date_format = \"%Y-%m-%d\"\n", "").unwrap_err().contains("date"));
    assert!(with(MANGA, "[details.status_map]\nongoing = [\"ongoing\"]\ncompleted = [\"completed\", \"finished\"]\n", "").unwrap_err().contains("status_map"));
}

#[test]
fn a_folder_loads_its_valid_recipes_and_reports_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a-manga.toml"), MANGA).unwrap();
    std::fs::write(dir.path().join("b-copy.toml"), MANGA).unwrap();
    std::fs::write(dir.path().join("c-anime.toml"), ANIME).unwrap();
    std::fs::write(dir.path().join("d-broken.toml"), "id = \"x\"\nsurprise = 1\n").unwrap();
    std::fs::write(dir.path().join("notes.txt"), "not a recipe").unwrap();
    let (recipes, problems) = load_dir(dir.path());
    assert_eq!(recipes.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["example-manga", "example-anime"]);
    assert_eq!(problems.len(), 2);
    assert_eq!(problems[0].0, "b-copy.toml");
    assert!(problems[0].1.contains("already used"));
    assert!(problems[1].0 == "d-broken.toml" && problems[1].1.contains("surprise"));
    assert_eq!(load_dir(&dir.path().join("missing")).0.len(), 0, "no folder, no recipes");
}
