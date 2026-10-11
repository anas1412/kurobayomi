//! The anime `PageLoader`: a source's popular, latest or search listing, each listed entry mapped to its library row by
//! `lookup` (`NetworkToLocalAnime`, T-41b-013). The source call runs on the tokio runtime and the lookup on the blocking
//! pool, as the manga loader does. `repo_lookup` is the production lookup over the anime repo; the screen that mounts the
//! page supplies the repo handle. A page is looked up entry by entry, each in its own write transaction, in input order
//! (decision: `AnimeRepo` has no batch upsert and its database handle is private to `kuro-data`, so one transaction for
//! the whole page of 25 would need a new `kuro-data` method; the single-transaction write is performance only,
//! `p2-topic-browse-listing.md` section 3, and every entry's lookup-and-insert is already atomic).

use std::sync::Arc;

use kuro_data::anime::{Anime, AnimeRepo};
use kuro_data::LookupOrInsert;
use kuro_domain::source::browse::BrowseQuery;
use kuro_source::anime::{AnimeSource, SAnime};
use kuro_source::filter_list::FilterList;

use crate::browse::entries::LocalEntry;
use crate::browse::listing::{ListItem, ListedPage, LoadFuture, PageLoader};
use crate::browse::loader::NetworkEntry;

/// Looks one listed entry up in the library: its row id, its stored title and its favourite flag.
pub type Lookup = Arc<dyn Fn(&NetworkEntry) -> Result<LocalEntry, String> + Send + Sync>;

/// What a listed anime contributes: its url, its title and its cover (the thumbnail).
pub fn network_entry(anime: &SAnime) -> NetworkEntry {
    NetworkEntry { url: anime.url.clone(), title: anime.title.clone(), cover: anime.thumbnail_url.clone() }
}

/// The production lookup: a listed anime is found by `(url, source)`, or inserted as a non-favourite row with the
/// `Anime::create()` defaults (`NetworkToLocalAnime`). An existing row, favourite or not, shows its stored title: the
/// listed title is kept in memory for a new row only (Q-1 default, E-BR-10). `network_to_local_anime` would return the
/// listed title in memory for a non-favourite, so this reads the repo's lookup directly.
pub fn repo_lookup(repo: AnimeRepo, source_id: i64) -> Lookup {
    Arc::new(move |entry: &NetworkEntry| {
        let listed = Anime {
            source: source_id,
            url: entry.url.clone(),
            title: entry.title.clone(),
            thumbnail_url: entry.cover.clone(),
            ..Anime::create()
        };
        Ok(match repo.lookup_or_insert(&listed).map_err(|error| error.to_string())? {
            LookupOrInsert::Inserted(id) => LocalEntry { id, favorite: false, title: listed.title },
            LookupOrInsert::Found(stored) => LocalEntry { id: stored.id, favorite: stored.favorite, title: stored.title },
        })
    })
}

pub struct AnimeLoader {
    source: Arc<dyn AnimeSource>,
    lookup: Lookup,
}

impl AnimeLoader {
    pub fn new(source: Arc<dyn AnimeSource>, lookup: Lookup) -> Self {
        Self { source, lookup }
    }
}

impl PageLoader for AnimeLoader {
    fn load(&self, query: BrowseQuery, filters: FilterList, page: u32) -> LoadFuture {
        Box::pin(load_page(Arc::clone(&self.source), Arc::clone(&self.lookup), query, filters, page))
    }
}

async fn load_page(
    source: Arc<dyn AnimeSource>,
    lookup: Lookup,
    query: BrowseQuery,
    filters: FilterList,
    page: u32,
) -> Result<ListedPage, String> {
    let page = i32::try_from(page).map_err(|error| error.to_string())?;
    // a panicking source is an error of this listing, not a dead task
    let listed = kuro_source::isolate(async {
        match query {
            BrowseQuery::Popular => source.popular(page).await,
            BrowseQuery::Latest => source.latest(page).await,
            BrowseQuery::Search(text) => source.search(page, &text, &filters).await,
        }
    })
    .await
    .map_err(|error| error.to_string())?;
    let source_id = source.id();
    tokio::task::spawn_blocking(move || -> Result<ListedPage, String> {
        let mut items = Vec::with_capacity(listed.animes.len());
        for anime in &listed.animes {
            let entry = network_entry(anime);
            let local = lookup(&entry)?;
            items.push(ListItem {
                id: local.id,
                source_id,
                url: entry.url,
                title: local.title,
                favorite: local.favorite,
                cover: entry.cover,
            });
        }
        Ok(ListedPage { items, has_next: listed.has_next_page })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::anime::fixtures::{FakeSource, anime, lookup, page};
    use kuro_source::SourceError;
    use kuro_source::filter::Filter;

    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(future)
    }

    fn repo() -> (tempfile::TempDir, AnimeRepo) {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("tachiyomi.db");
        let writer = kuro_db::schema_anime::open(&path).expect("schema");
        let db = Arc::new(kuro_db::handle::Db::new(&path, writer).expect("db"));
        (dir, AnimeRepo::new(db))
    }

    fn listed(url: &str, title: &str) -> NetworkEntry {
        NetworkEntry { url: url.into(), title: title.into(), cover: Some(format!("{url}.jpg")) }
    }

    #[test]
    fn an_unknown_listing_is_inserted_as_a_non_favourite_row_and_found_again() {
        let (_dir, repo) = repo();
        let lookup = repo_lookup(repo, 7);
        let first = lookup(&listed("/a", "Listed")).expect("row");
        assert!(!first.favorite);
        assert_eq!(first.title, "Listed");
        assert_eq!(lookup(&listed("/a", "Renamed")).expect("row").id, first.id, "one row per (url, source)");
    }

    #[test]
    fn an_existing_row_shows_its_stored_title_favourite_or_not() {
        let (_dir, repo) = repo();
        let stored = repo.insert(&Anime { source: 7, url: "/fav".into(), title: "Stored".into(), favorite: true, ..Anime::create() }).expect("insert");
        repo.insert(&Anime { source: 7, url: "/plain".into(), title: "Old".into(), ..Anime::create() }).expect("insert");
        let lookup = repo_lookup(repo, 7);
        let fav = lookup(&listed("/fav", "Listed")).expect("row");
        assert_eq!((fav.id, fav.favorite, fav.title.as_str()), (stored, true, "Stored"));
        let plain = lookup(&listed("/plain", "New")).expect("row");
        assert_eq!((plain.favorite, plain.title.as_str()), (false, "Old"), "a non-favourite keeps its stored title too");
    }

    #[test]
    fn a_page_gets_its_row_ids_in_input_order_and_a_repeated_url_gets_one_row() {
        let (_dir, repo) = repo();
        let lookup = repo_lookup(repo, 7);
        let ids: Vec<i64> = ["/c", "/a", "/b", "/a"].iter().map(|url| lookup(&listed(url, url)).expect("row").id).collect();
        assert!(ids[0] < ids[1] && ids[1] < ids[2], "new rows are numbered in the order they were listed");
        assert_eq!(ids[3], ids[1], "the same (url, source) is the same row");
    }

    #[test]
    fn a_listed_anime_keeps_its_url_title_and_thumbnail_as_its_cover() {
        let entry = network_entry(&anime("/a", "A"));
        assert_eq!((entry.url.as_str(), entry.title.as_str(), entry.cover.as_deref()), ("/a", "A", Some("/a.jpg")));
    }

    #[test]
    fn each_listed_anime_becomes_its_library_row_in_source_order_and_the_next_page_flag_is_kept() {
        let source = FakeSource::new(7, vec![Ok(page(vec![anime("/a", "A"), anime("/b", "B")], true))]);
        let (lookup, lookups) = lookup(&[("/b", 42)]);
        let loader = AnimeLoader::new(source.clone(), lookup);
        let got = run(loader.load(BrowseQuery::Popular, FilterList::default(), 1)).expect("page");
        assert_eq!(
            got.items.iter().map(|i| (i.id, i.source_id, i.favorite, i.title.as_str())).collect::<Vec<_>>(),
            [(900, 7, false, "A"), (42, 7, true, "stored /b")],
            "a favourite shows its stored title, an unknown url becomes a non-favourite row"
        );
        assert!(got.has_next);
        assert_eq!(source.calls(), ["popular 1"]);
        assert_eq!(lookups.lock().expect("log").as_slice(), ["local /a", "local /b"]);
    }

    #[test]
    fn a_search_passes_its_text_and_its_filters_and_the_last_page_has_no_next() {
        let source = FakeSource::new(3, vec![Ok(page(vec![anime("/s", "S")], false))]);
        let (lookup, _) = lookup(&[]);
        let loader = AnimeLoader::new(source.clone(), lookup);
        let filters = FilterList(vec![Filter::check_box("A"), Filter::check_box("B")]);
        let got = run(loader.load(BrowseQuery::Search("one".into()), filters, 2)).expect("page");
        assert_eq!(source.calls(), ["search 2 one 2"]);
        assert!(!got.has_next);
    }

    #[test]
    fn a_latest_listing_calls_the_latest_endpoint() {
        let source = FakeSource::new(3, vec![Ok(page(vec![], false))]);
        let (lookup, _) = lookup(&[]);
        let loader = AnimeLoader::new(source.clone(), lookup);
        run(loader.load(BrowseQuery::Latest, FilterList::default(), 1)).expect("page");
        assert_eq!(source.calls(), ["latest 1"]);
    }

    #[test]
    fn a_source_error_reaches_the_page_as_text_and_no_row_is_looked_up() {
        let source = FakeSource::new(3, vec![Err(SourceError::Http(503))]);
        let (lookup, lookups) = lookup(&[]);
        let loader = AnimeLoader::new(source, lookup);
        let error = run(loader.load(BrowseQuery::Popular, FilterList::default(), 1)).expect_err("source error");
        assert!(error.contains("503"), "{error}");
        assert!(lookups.lock().expect("log").is_empty());
    }
}
