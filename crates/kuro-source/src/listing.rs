//! What a remote listing shows (T-06a-043, T-15a-040). The enum replaces the sentinel query strings that Aniyomi
//! used for the popular and latest lists. `F` is the source's filter value type.

use crate::filter_list::FilterList;

/// The query string Aniyomi sends for the popular list (`GetRemoteManga.QUERY_POPULAR`).
pub const QUERY_POPULAR: &str = "eu.kanade.domain.source.manga.interactor.POPULAR";
/// The query string Aniyomi sends for the latest list (`GetRemoteManga.QUERY_LATEST`).
pub const QUERY_LATEST: &str = "eu.kanade.domain.source.manga.interactor.LATEST";

/// One remote listing of a source. `query` is `None` when the search has no text (Kotlin `null`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listing<F> {
    Popular,
    Latest,
    Search { query: Option<String>, filters: F },
}

impl Listing<FilterList> {
    /// `valueOf(query)` of the browse screen (filter topic 3.1): the two sentinels map to their listings, and any other
    /// value, including none, is a search with empty filters. The filters are filled in later.
    pub fn value_of(query: Option<&str>) -> Self {
        match query {
            Some(QUERY_POPULAR) => Self::Popular,
            Some(QUERY_LATEST) => Self::Latest,
            other => Self::Search { query: other.map(str::to_owned), filters: FilterList::default() },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_carries_its_query_and_filters_and_the_other_listings_carry_nothing() {
        let search = Listing::Search { query: Some("naruto".to_owned()), filters: vec![1_u8, 2] };
        assert_eq!(search, Listing::Search { query: Some("naruto".to_owned()), filters: vec![1, 2] });
        assert_ne!(Listing::<Vec<u8>>::Popular, Listing::Latest);
        assert_ne!(Listing::<Vec<u8>>::Latest, search);
    }

    #[test]
    fn value_of_maps_the_two_sentinels_and_everything_else_to_a_search() {
        assert_eq!(Listing::value_of(Some(QUERY_POPULAR)), Listing::Popular);
        assert_eq!(Listing::value_of(Some(QUERY_LATEST)), Listing::Latest);
        assert_eq!(
            Listing::value_of(Some("naruto")),
            Listing::Search { query: Some("naruto".to_owned()), filters: FilterList::default() }
        );
        assert_eq!(Listing::value_of(None), Listing::Search { query: None, filters: FilterList::default() });
    }
}
