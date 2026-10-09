//! The anime `PageLoader`: a source's popular, latest or search listing, each listed entry mapped to its library row by
//! `lookup` (`NetworkToLocalAnime`, T-41b-013). The source call runs on the tokio runtime and the lookup on the blocking
//! pool, as the manga loader does. The production lookup (the anime repo's lookup-or-insert in one transaction) is not
//! wired yet, because the app holds no anime repo handle for the browse page. The caller supplies it.

use std::sync::Arc;

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
    let listed = match query {
        BrowseQuery::Popular => source.popular(page).await,
        BrowseQuery::Latest => source.latest(page).await,
        BrowseQuery::Search(text) => source.search(page, &text, &filters).await,
    }
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
    use crate::browse::anime::fixtures::{ScriptedSource, anime, lookup, page};
    use kuro_source::SourceError;
    use kuro_source::filter::Filter;

    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(future)
    }

    #[test]
    fn a_listed_anime_keeps_its_url_title_and_thumbnail_as_its_cover() {
        let entry = network_entry(&anime("/a", "A"));
        assert_eq!((entry.url.as_str(), entry.title.as_str(), entry.cover.as_deref()), ("/a", "A", Some("/a.jpg")));
    }

    #[test]
    fn each_listed_anime_becomes_its_library_row_in_source_order_and_the_next_page_flag_is_kept() {
        let source = ScriptedSource::new(7, vec![Ok(page(vec![anime("/a", "A"), anime("/b", "B")], true))]);
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
        let source = ScriptedSource::new(3, vec![Ok(page(vec![anime("/s", "S")], false))]);
        let (lookup, _) = lookup(&[]);
        let loader = AnimeLoader::new(source.clone(), lookup);
        let filters = FilterList(vec![Filter::check_box("A"), Filter::check_box("B")]);
        let got = run(loader.load(BrowseQuery::Search("one".into()), filters, 2)).expect("page");
        assert_eq!(source.calls(), ["search 2 one 2"]);
        assert!(!got.has_next);
    }

    #[test]
    fn a_latest_listing_calls_the_latest_endpoint() {
        let source = ScriptedSource::new(3, vec![Ok(page(vec![], false))]);
        let (lookup, _) = lookup(&[]);
        let loader = AnimeLoader::new(source.clone(), lookup);
        run(loader.load(BrowseQuery::Latest, FilterList::default(), 1)).expect("page");
        assert_eq!(source.calls(), ["latest 1"]);
    }

    #[test]
    fn a_source_error_reaches_the_page_as_text_and_no_row_is_looked_up() {
        let source = ScriptedSource::new(3, vec![Err(SourceError::Http(503))]);
        let (lookup, lookups) = lookup(&[]);
        let loader = AnimeLoader::new(source, lookup);
        let error = run(loader.load(BrowseQuery::Popular, FilterList::default(), 1)).expect_err("source error");
        assert!(error.contains("503"), "{error}");
        assert!(lookups.lock().expect("log").is_empty());
    }
}
