//! Browse routing (`GetRemoteManga.kt`, `GetRemoteAnime.kt`). The browse screens pass a [`BrowseQuery`] value, never
//! a sentinel string, so typing the sentinel text into search performs a search (Q-BR-1 fixed in the UI path).

use crate::source::model::Id;
use crate::source::repo::{PageSource, RemoteListings};

/// `GetRemoteManga.QUERY_POPULAR`. Accepted by [`BrowseQuery::from_legacy`] only.
pub const LEGACY_MANGA_POPULAR: &str = "eu.kanade.domain.source.manga.interactor.POPULAR";
/// `GetRemoteManga.QUERY_LATEST`. Accepted by [`BrowseQuery::from_legacy`] only.
pub const LEGACY_MANGA_LATEST: &str = "eu.kanade.domain.source.manga.interactor.LATEST";
/// `GetRemoteAnime.QUERY_POPULAR`. Accepted by [`BrowseQuery::from_legacy`] only.
pub const LEGACY_ANIME_POPULAR: &str = "eu.kanade.domain.source.anime.interactor.POPULAR";
/// `GetRemoteAnime.QUERY_LATEST`. Accepted by [`BrowseQuery::from_legacy`] only.
pub const LEGACY_ANIME_LATEST: &str = "eu.kanade.domain.source.anime.interactor.LATEST";

/// What a remote browse list shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowseQuery {
    Popular,
    Latest,
    Search(String),
}

impl BrowseQuery {
    /// Maps the four Kotlin sentinel strings to their listings, and any other text to a search. Used only for the
    /// old backup and intent path; the UI never builds a query from text.
    pub fn from_legacy(query: &str) -> Self {
        match query {
            LEGACY_MANGA_POPULAR | LEGACY_ANIME_POPULAR => Self::Popular,
            LEGACY_MANGA_LATEST | LEGACY_ANIME_LATEST => Self::Latest,
            other => Self::Search(other.to_owned()),
        }
    }
}

/// The listing a query selects: popular, latest, or a search with `filters`.
pub fn route<E, F>(
    listings: &dyn RemoteListings<E, F>,
    source_id: Id,
    query: &BrowseQuery,
    filters: F,
) -> Box<dyn PageSource<E>> {
    match query {
        BrowseQuery::Popular => listings.popular(source_id),
        BrowseQuery::Latest => listings.latest(source_id),
        BrowseQuery::Search(text) => listings.search(source_id, text, filters),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::error::DomainError;

    /// Records which listing was asked for; the page it returns is empty.
    #[derive(Default)]
    struct Recorder {
        calls: RefCell<Vec<String>>,
    }

    struct Empty;

    impl PageSource<u8> for Empty {
        fn load(&self, _page: i64) -> Result<(Vec<u8>, Option<i64>), DomainError> {
            Ok((Vec::new(), None))
        }
    }

    impl RemoteListings<u8, Vec<u8>> for Recorder {
        fn popular(&self, source_id: Id) -> Box<dyn PageSource<u8>> {
            self.calls.borrow_mut().push(format!("popular {source_id}"));
            Box::new(Empty)
        }
        fn latest(&self, source_id: Id) -> Box<dyn PageSource<u8>> {
            self.calls.borrow_mut().push(format!("latest {source_id}"));
            Box::new(Empty)
        }
        fn search(&self, source_id: Id, query: &str, filters: Vec<u8>) -> Box<dyn PageSource<u8>> {
            self.calls.borrow_mut().push(format!("search {source_id} {query} {filters:?}"));
            Box::new(Empty)
        }
    }

    #[test]
    fn from_legacy_maps_the_four_sentinels_and_passes_other_text_as_search() {
        assert_eq!(BrowseQuery::from_legacy(LEGACY_MANGA_POPULAR), BrowseQuery::Popular, "TC-SRC-05");
        assert_eq!(BrowseQuery::from_legacy(LEGACY_ANIME_POPULAR), BrowseQuery::Popular);
        assert_eq!(BrowseQuery::from_legacy(LEGACY_MANGA_LATEST), BrowseQuery::Latest);
        assert_eq!(BrowseQuery::from_legacy(LEGACY_ANIME_LATEST), BrowseQuery::Latest);
        assert_eq!(BrowseQuery::from_legacy("naruto"), BrowseQuery::Search("naruto".into()));
        assert_eq!(BrowseQuery::from_legacy(""), BrowseQuery::Search(String::new()));
    }

    #[test]
    fn route_calls_the_listing_the_query_names() {
        let rec = Recorder::default();
        let _ = route(&rec, 5, &BrowseQuery::Popular, vec![]);
        let _ = route(&rec, 5, &BrowseQuery::Latest, vec![]);
        let _ = route(&rec, 5, &BrowseQuery::Search("x".into()), vec![1]);
        assert_eq!(*rec.calls.borrow(), ["popular 5", "latest 5", "search 5 x [1]"]);
    }

    #[test]
    fn typing_the_sentinel_text_into_search_performs_a_search() {
        let rec = Recorder::default();
        let _ = route(&rec, 1, &BrowseQuery::Search(LEGACY_MANGA_POPULAR.into()), vec![]);
        assert_eq!(rec.calls.borrow()[0], format!("search 1 {LEGACY_MANGA_POPULAR} []"), "Q-BR-1 guard");
    }
}
