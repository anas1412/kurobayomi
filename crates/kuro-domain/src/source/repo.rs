//! Ports of the source layer (`MangaSourceRepository.kt`, `AnimeSourceRepository.kt`, `MangaStubSourceRepository.kt`,
//! `AnimeStubSourceRepository.kt`). The remote entry type `E` and filter type `F` belong to the source layer (areas
//! 15 and 16), so they are parameters here. The database-backed implementations are in `kuro-data`.

use kuro_db::subscribe::Subscription;
use tokio::sync::watch;

use crate::error::DomainError;
use crate::source::model::{Id, Source};
use crate::source::non_library::{SourceWithCount, SourceWithIds};
use crate::source::stub::StubSource;

/// One remote listing, read page by page. The key is the page number, starting at 1 (Kotlin `PagingSource<Long, _>`).
pub trait PageSource<T>: Send + Sync {
    /// The items of `page`, and the next page number, or `None` when `page` is the last one.
    fn load(&self, page: i64) -> Result<(Vec<T>, Option<i64>), DomainError>;
}

/// The three remote listings of a source.
pub trait RemoteListings<E, F> {
    fn popular(&self, source_id: Id) -> Box<dyn PageSource<E>>;
    fn latest(&self, source_id: Id) -> Box<dyn PageSource<E>>;
    fn search(&self, source_id: Id, query: &str, filters: F) -> Box<dyn PageSource<E>>;
}

/// Manga source lists: the online sources and the non-library counts.
pub trait MangaSourceRepo<E, F>: RemoteListings<E, F> {
    fn online_sources(&self) -> watch::Receiver<Vec<Source>>;
    fn sources_with_favorite_count(&self) -> watch::Receiver<Vec<(Source, i64)>>;
    fn sources_with_non_library(&self) -> watch::Receiver<Vec<SourceWithCount>>;
}

/// Anime source lists: as [`MangaSourceRepo`], with the non-library rows grouped by source with their ids.
pub trait AnimeSourceRepo<E, F>: RemoteListings<E, F> {
    fn online_sources(&self) -> watch::Receiver<Vec<Source>>;
    fn sources_with_favorite_count(&self) -> watch::Receiver<Vec<(Source, i64)>>;
    fn sources_with_non_library(&self) -> watch::Receiver<Vec<SourceWithIds>>;
}

/// The extension index as the source registry reads it (area 17 section 2.7). The extension layer owns the index. This
/// port gives only the stub data of one source id, for the fallback chain of `get_or_stub` (T-17a-008).
pub trait ExtensionCatalog: Send + Sync {
    /// The stub data of a source from the available index, or `None` when the index has none (or is not loaded yet).
    fn index_source(&self, id: Id) -> Option<StubSource>;
}

/// Persisted stub sources (T-09a-008). `upsert` inserts or replaces `lang` and `name`.
pub trait StubSourcePort: Send + Sync {
    fn get(&self, id: Id) -> Result<Option<StubSource>, DomainError>;
    fn all(&self) -> Result<Vec<StubSource>, DomainError>;
    fn upsert(&self, id: Id, lang: &str, name: &str) -> Result<(), DomainError>;
    /// Every stub, published again after each upsert.
    fn subscribe_all(&self) -> Subscription<Vec<StubSource>>;
}

/// The old name of [`StubSourcePort`] (phase E deletes it).
pub use StubSourcePort as StubSourceRepo;

/// The port over a repository with the stub-source methods: the shared `StubSourceRepo` and, until phase E, the two
/// per-kind wrappers (their methods are the same, they only fix the kind).
macro_rules! stub_port {
    ($($repo:ty),+) => {$(
        impl StubSourcePort for $repo {
            fn get(&self, id: Id) -> Result<Option<StubSource>, DomainError> {
                Ok(<$repo>::get(self, id)?)
            }
            fn all(&self) -> Result<Vec<StubSource>, DomainError> {
                Ok(<$repo>::all(self)?)
            }
            fn upsert(&self, id: Id, lang: &str, name: &str) -> Result<(), DomainError> {
                Ok(<$repo>::upsert(self, id, lang, name)?)
            }
            fn subscribe_all(&self) -> Subscription<Vec<StubSource>> {
                <$repo>::watch_all(self)
            }
        }
    )+};
}

stub_port!(kuro_data::source::StubSourceRepo, kuro_data::source::MangaStubSourceRepo, kuro_data::source::AnimeStubSourceRepo);

#[cfg(test)]
mod tests {
    use tokio::sync::watch;

    use super::*;

    /// A fake that serves fixed listings. Compiles against both traits, which is the T-09a-004 contract check.
    struct FakeSources;

    struct Nothing;

    impl PageSource<u8> for Nothing {
        fn load(&self, _page: i64) -> Result<(Vec<u8>, Option<i64>), DomainError> {
            Ok((Vec::new(), None))
        }
    }

    impl RemoteListings<u8, ()> for FakeSources {
        fn popular(&self, _source_id: Id) -> Box<dyn PageSource<u8>> {
            Box::new(Nothing)
        }
        fn latest(&self, _source_id: Id) -> Box<dyn PageSource<u8>> {
            Box::new(Nothing)
        }
        fn search(&self, _source_id: Id, _query: &str, _filters: ()) -> Box<dyn PageSource<u8>> {
            Box::new(Nothing)
        }
    }

    impl MangaSourceRepo<u8, ()> for FakeSources {
        fn online_sources(&self) -> watch::Receiver<Vec<Source>> {
            watch::channel(vec![Source::new(1, "A", "en")]).1
        }
        fn sources_with_favorite_count(&self) -> watch::Receiver<Vec<(Source, i64)>> {
            watch::channel(vec![(Source::new(1, "A", "en"), 3)]).1
        }
        fn sources_with_non_library(&self) -> watch::Receiver<Vec<SourceWithCount>> {
            watch::channel(vec![SourceWithCount { source: Source::new(1, "A", "en"), count: 2 }]).1
        }
    }

    impl AnimeSourceRepo<u8, ()> for FakeSources {
        fn online_sources(&self) -> watch::Receiver<Vec<Source>> {
            watch::channel(Vec::new()).1
        }
        fn sources_with_favorite_count(&self) -> watch::Receiver<Vec<(Source, i64)>> {
            watch::channel(Vec::new()).1
        }
        fn sources_with_non_library(&self) -> watch::Receiver<Vec<SourceWithIds>> {
            watch::channel(Vec::new()).1
        }
    }

    #[test]
    fn a_fake_implements_both_source_repo_traits_and_serves_its_listings() {
        let manga: &dyn MangaSourceRepo<u8, ()> = &FakeSources;
        assert_eq!(manga.sources_with_non_library().borrow()[0].count, 2);
        assert_eq!(manga.online_sources().borrow().len(), 1);
        let anime: &dyn AnimeSourceRepo<u8, ()> = &FakeSources;
        assert!(anime.sources_with_non_library().borrow().is_empty());
        assert!(anime.popular(1).load(1).unwrap().0.is_empty());
    }
}
