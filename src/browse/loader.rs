//! The production `PageLoader`: a source's popular, latest or search listing, with each listed entry mapped to its
//! library row (`NetworkToLocalManga`, T-40b-033). An unknown `(url, source)` inserts a non-favourite row, so every cover
//! has a library id for the favourite flag and the long-press flow. Runs on the tokio runtime; the source calls block.

use std::sync::Arc;

use kuro_domain::source::browse::{BrowseQuery, route};
use kuro_domain::source::repo::RemoteListings;
use kuro_source::filter_list::FilterList;

use super::entries::FavoriteStore;
use super::listing::{ListItem, ListedPage, LoadFuture, PageLoader};

/// What the loader needs of one remote entry.
pub struct NetworkEntry {
    pub url: String,
    pub title: String,
    pub cover: Option<String>,
}

pub struct RemoteLoader<E> {
    listings: Arc<dyn RemoteListings<E, FilterList> + Send + Sync>,
    source_id: i64,
    store: Arc<dyn FavoriteStore>,
    entry_of: Arc<dyn Fn(&E) -> NetworkEntry + Send + Sync>,
}

impl<E: Send + 'static> RemoteLoader<E> {
    pub fn new(
        listings: Arc<dyn RemoteListings<E, FilterList> + Send + Sync>,
        source_id: i64,
        store: Arc<dyn FavoriteStore>,
        entry_of: impl Fn(&E) -> NetworkEntry + Send + Sync + 'static,
    ) -> Self {
        Self { listings, source_id, store, entry_of: Arc::new(entry_of) }
    }
}

impl<E: Send + 'static> PageLoader for RemoteLoader<E> {
    fn load(&self, query: BrowseQuery, filters: FilterList, page: u32) -> LoadFuture {
        let (listings, store, entry_of, source_id) = (Arc::clone(&self.listings), Arc::clone(&self.store), Arc::clone(&self.entry_of), self.source_id);
        Box::pin(async move {
            let work = move || -> Result<ListedPage, String> {
                let listing = route(&*listings, source_id, &query, filters);
                let (entries, next) = listing.load(i64::from(page)).map_err(|error| error.to_string())?;
                let mut items = Vec::with_capacity(entries.len());
                for entry in &entries {
                    let listed = entry_of(entry);
                    let local = store.local_entry(source_id, &listed.url, &listed.title, listed.cover.as_deref())?;
                    items.push(ListItem {
                        id: local.id,
                        source_id,
                        url: listed.url,
                        title: local.title,
                        favorite: local.favorite,
                        cover: listed.cover,
                    });
                }
                Ok(ListedPage { items, has_next: next.is_some() })
            };
            tokio::task::spawn_blocking(work).await.map_err(|error| error.to_string())?
        })
    }
}

#[cfg(test)]
mod tests {
    use kuro_domain::error::DomainError;
    use kuro_domain::source::model::Id;
    use kuro_domain::source::repo::PageSource;

    use super::*;
    use crate::browse::entries::fake::FakeStore;

    /// One page of `(url, title)` pairs per listing; the search echoes its text and filter count in the titles.
    struct Fake;

    struct Page(Vec<(String, String)>, Option<i64>);

    impl PageSource<(String, String)> for Page {
        fn load(&self, page: i64) -> Result<(Vec<(String, String)>, Option<i64>), DomainError> {
            if page == 99 { Err(DomainError::Net("offline".into())) } else { Ok((self.0.clone(), self.1)) }
        }
    }

    impl RemoteListings<(String, String), FilterList> for Fake {
        fn popular(&self, _: Id) -> Box<dyn PageSource<(String, String)>> {
            Box::new(Page(vec![("/known".into(), "Known".into()), ("/new".into(), "New".into())], Some(2)))
        }
        fn latest(&self, _: Id) -> Box<dyn PageSource<(String, String)>> {
            Box::new(Page(vec![("/latest".into(), "Latest".into())], None))
        }
        fn search(&self, _: Id, query: &str, filters: FilterList) -> Box<dyn PageSource<(String, String)>> {
            Box::new(Page(vec![("/s".into(), format!("{query}:{}", filters.len()))], None))
        }
    }

    fn loader(store: Arc<FakeStore>) -> RemoteLoader<(String, String)> {
        RemoteLoader::new(Arc::new(Fake), 5, store, |(url, title)| NetworkEntry { url: url.clone(), title: title.clone(), cover: None })
    }

    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_multi_thread().worker_threads(1).build().expect("runtime").block_on(future)
    }

    #[test]
    fn each_listed_entry_is_mapped_to_its_library_row_and_unknown_ones_are_inserted_as_non_favourites() {
        let store = Arc::new(FakeStore::default());
        store.known.lock().expect("lock").push((5, "/known".into(), true));
        let page = run(loader(Arc::clone(&store)).load(BrowseQuery::Popular, FilterList::default(), 1)).expect("page");
        assert!(page.has_next);
        assert_eq!(page.items.len(), 2);
        assert_eq!((page.items[0].id, page.items[0].favorite), (7, true), "a known favourite keeps its flag");
        assert_eq!((page.items[1].id, page.items[1].favorite), (8, false), "an unknown url becomes a non-favourite row");
        assert_eq!((page.items[1].source_id, page.items[1].url.as_str()), (5, "/new"));
        assert_eq!(store.calls(), ["local 5 /known", "local 5 /new"]);
    }

    #[test]
    fn a_search_runs_with_its_text_and_its_filters_and_the_last_page_has_no_next() {
        use kuro_source::filter::Filter;
        let store = Arc::new(FakeStore::default());
        let filters = FilterList(vec![Filter::check_box("A"), Filter::check_box("B")]);
        let page = run(loader(store).load(BrowseQuery::Search("one".into()), filters, 1)).expect("page");
        assert_eq!(page.items[0].title, "one:2");
        assert!(!page.has_next);
    }

    #[test]
    fn a_source_error_and_a_store_error_both_reach_the_page_as_text() {
        let store = Arc::new(FakeStore::default());
        let error = run(loader(Arc::clone(&store)).load(BrowseQuery::Popular, FilterList::default(), 99)).expect_err("source error");
        assert!(error.contains("offline"), "{error}");
        store.fail.lock().expect("lock").push("local_entry");
        let error = run(loader(store).load(BrowseQuery::Latest, FilterList::default(), 1)).expect_err("store error");
        assert!(error.contains("local_entry failed"), "{error}");
    }
}
