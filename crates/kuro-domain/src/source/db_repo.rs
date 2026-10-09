//! Database-backed source lists: the [`MangaSourceRepo`] and [`AnimeSourceRepo`] over the `kuro-data` repositories.
//! Each live list is a `kuro-db` subscription bridged to a `watch` receiver. Every source id in a list is resolved
//! through the registry: the installed source, else its persisted stub (`is_stub`), else a blank stub that shows as
//! the id. The paged remote calls come from the source layer (areas 15 and 16), passed in as `listings`. The impls
//! live here, not in `kuro-data`, because the traits are domain ports and `kuro-data` cannot depend on `kuro-domain`.

use std::collections::BTreeSet;
use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;
use kuro_db::subscribe::Subscription;
use tokio::sync::watch;

use crate::error::DomainError;
use crate::source::model::{Id, Source};
use crate::source::non_library::{SourceWithCount, SourceWithIds, anime_listing, manga_listing};
use crate::source::registry::{InstalledSource, SourceHandle, SourceRegistry};
use crate::source::repo::{AnimeSourceRepo, MangaSourceRepo, PageSource, RemoteListings};

/// The source a list shows for `id`: the installed source as it is, else its persisted stub, marked `is_stub`. An id
/// with no stub row has a blank name, which displays as the id (E-SRC-05).
fn resolve<S: InstalledSource + ?Sized>(registry: &SourceRegistry<S>, id: Id) -> Result<Source, DomainError> {
    Ok(match registry.get_or_stub(id)? {
        SourceHandle::Installed(source) => live(&*source),
        SourceHandle::Stub(stub) => Source {
            is_stub: true,
            ..Source::new(stub.id, &stub.name, &stub.lang)
        },
    })
}

fn favorite_rows<S: InstalledSource + ?Sized>(
    registry: &SourceRegistry<S>,
    rows: &[(Id, i64)],
) -> Result<Vec<(Source, i64)>, DomainError> {
    rows.iter()
        .map(|&(id, count)| resolve(registry, id).map(|source| (source, count)))
        .collect()
}

fn manga_rows<S: InstalledSource + ?Sized>(
    registry: &SourceRegistry<S>,
    counts: &[(Id, i64)],
) -> Result<Vec<SourceWithCount>, DomainError> {
    let sources = counts
        .iter()
        .map(|&(id, _)| resolve(registry, id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(manga_listing(&sources, counts))
}

/// `removable` and `orphaned` are `(source, anime id)` rows, as `kuro-data` gives them.
fn anime_rows<S: InstalledSource + ?Sized>(
    registry: &SourceRegistry<S>,
    (removable, orphaned): &(Vec<(Id, Id)>, Vec<(Id, Id)>),
) -> Result<Vec<SourceWithIds>, DomainError> {
    let ids: BTreeSet<Id> = removable.iter().chain(orphaned).map(|&(source, _)| source).collect();
    let sources = ids
        .into_iter()
        .map(|id| resolve(registry, id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(anime_listing(&sources, removable, orphaned))
}

/// The online sources: the installed sources that talk HTTP.
fn online_rows<S: InstalledSource + ?Sized>(installed: &[Arc<S>]) -> Vec<Source> {
    installed.iter().filter(|s| s.is_http()).map(|s| live(&**s)).collect()
}

/// A live source as a list row. `is_stub` comes from the stub type, never from the live source, so it is false here
/// whatever the source reports (T-17a-012).
fn live<S: InstalledSource + ?Sized>(source: &S) -> Source {
    Source { is_stub: false, ..source.source() }
}

/// Publishes `map(rows)` each time `sub` publishes. A failed query, or a row that does not resolve, is logged and the
/// last list is kept. The task ends with the receiver or with the database handle.
fn bridge<T, U>(
    mut sub: Subscription<T>,
    map: impl Fn(&T) -> Result<U, DomainError> + Send + 'static,
) -> watch::Receiver<U>
where
    T: Send + Sync + 'static,
    U: Default + Send + Sync + 'static,
{
    let (tx, rx) = watch::channel(U::default());
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tx.closed() => break,
                published = sub.changed() => {
                    let Some(published) = published else { break };
                    match published {
                        Ok(rows) => match map(&rows) {
                            Ok(list) => {
                                tx.send_replace(list);
                            }
                            Err(e) => tracing::warn!(error = %e, "source list not resolved"),
                        },
                        Err(e) => tracing::warn!(error = %e, "source list query failed"),
                    }
                }
            }
        }
    });
    rx
}

/// Publishes `map(value)` each time the watched value changes. The first value is `map` of the current one.
fn bridge_watch<T, U>(mut rx: watch::Receiver<T>, map: impl Fn(&T) -> U + Send + 'static) -> watch::Receiver<U>
where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static,
{
    let (tx, out) = watch::channel(map(&rx.borrow_and_update()));
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tx.closed() => break,
                changed = rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    tx.send_replace(map(&rx.borrow_and_update()));
                }
            }
        }
    });
    out
}

/// The two lists both kinds publish: the online sources, which follow the installed list, and the favourite counts.
fn shared<S>(
    registry: Arc<SourceRegistry<S>>,
    favorites: Subscription<Vec<(Id, i64)>>,
) -> (watch::Receiver<Vec<Source>>, watch::Receiver<Vec<(Source, i64)>>)
where
    S: InstalledSource + ?Sized + 'static,
{
    let online = bridge_watch(registry.subscribe_installed(), |installed| online_rows(installed));
    let favorites = bridge(favorites, move |rows| favorite_rows(&registry, rows));
    (online, favorites)
}

/// The manga source lists over `kuro-data`. Built once per database and registry. Each receiver it hands out sees the
/// latest list.
pub struct MangaSourceDb<E: 'static, F: 'static> {
    online: watch::Receiver<Vec<Source>>,
    favorites: watch::Receiver<Vec<(Source, i64)>>,
    non_library: watch::Receiver<Vec<SourceWithCount>>,
    listings: Arc<dyn RemoteListings<E, F>>,
}

impl<E: 'static, F: 'static> MangaSourceDb<E, F> {
    /// Starts the live lists. Must be called inside a tokio runtime.
    pub fn new<S: InstalledSource + ?Sized + 'static>(
        repo: &MangaRepo,
        registry: Arc<SourceRegistry<S>>,
        listings: Arc<dyn RemoteListings<E, F>>,
    ) -> Self {
        let counts = Arc::clone(&registry);
        let non_library = bridge(repo.subscribe_non_library_counts(), move |rows| {
            manga_rows(&counts, rows)
        });
        let (online, favorites) = shared(registry, repo.subscribe_favorite_counts());
        Self {
            online,
            favorites,
            non_library,
            listings,
        }
    }
}

impl<E: 'static, F: 'static> RemoteListings<E, F> for MangaSourceDb<E, F> {
    fn popular(&self, source_id: Id) -> Box<dyn PageSource<E>> {
        self.listings.popular(source_id)
    }
    fn latest(&self, source_id: Id) -> Box<dyn PageSource<E>> {
        self.listings.latest(source_id)
    }
    fn search(&self, source_id: Id, query: &str, filters: F) -> Box<dyn PageSource<E>> {
        self.listings.search(source_id, query, filters)
    }
}

impl<E: 'static, F: 'static> MangaSourceRepo<E, F> for MangaSourceDb<E, F> {
    fn online_sources(&self) -> watch::Receiver<Vec<Source>> {
        self.online.clone()
    }
    fn sources_with_favorite_count(&self) -> watch::Receiver<Vec<(Source, i64)>> {
        self.favorites.clone()
    }
    fn sources_with_non_library(&self) -> watch::Receiver<Vec<SourceWithCount>> {
        self.non_library.clone()
    }
}

/// The anime source lists over `kuro-data`: as [`MangaSourceDb`], with the non-library rows grouped by source with
/// their ids.
pub struct AnimeSourceDb<E: 'static, F: 'static> {
    online: watch::Receiver<Vec<Source>>,
    favorites: watch::Receiver<Vec<(Source, i64)>>,
    non_library: watch::Receiver<Vec<SourceWithIds>>,
    listings: Arc<dyn RemoteListings<E, F>>,
}

impl<E: 'static, F: 'static> AnimeSourceDb<E, F> {
    /// Starts the live lists. Must be called inside a tokio runtime.
    pub fn new<S: InstalledSource + ?Sized + 'static>(
        repo: &AnimeRepo,
        registry: Arc<SourceRegistry<S>>,
        listings: Arc<dyn RemoteListings<E, F>>,
    ) -> Self {
        let tree = Arc::clone(&registry);
        let non_library = bridge(repo.subscribe_non_library_tree(), move |rows| anime_rows(&tree, rows));
        let (online, favorites) = shared(registry, repo.subscribe_favorite_counts());
        Self {
            online,
            favorites,
            non_library,
            listings,
        }
    }
}

impl<E: 'static, F: 'static> RemoteListings<E, F> for AnimeSourceDb<E, F> {
    fn popular(&self, source_id: Id) -> Box<dyn PageSource<E>> {
        self.listings.popular(source_id)
    }
    fn latest(&self, source_id: Id) -> Box<dyn PageSource<E>> {
        self.listings.latest(source_id)
    }
    fn search(&self, source_id: Id, query: &str, filters: F) -> Box<dyn PageSource<E>> {
        self.listings.search(source_id, query, filters)
    }
}

impl<E: 'static, F: 'static> AnimeSourceRepo<E, F> for AnimeSourceDb<E, F> {
    fn online_sources(&self) -> watch::Receiver<Vec<Source>> {
        self.online.clone()
    }
    fn sources_with_favorite_count(&self) -> watch::Receiver<Vec<(Source, i64)>> {
        self.favorites.clone()
    }
    fn sources_with_non_library(&self) -> watch::Receiver<Vec<SourceWithIds>> {
        self.non_library.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use kuro_data::anime::Anime;
    use kuro_data::manga::Manga;
    use kuro_data::source::{AnimeStubSourceRepo, MangaStubSourceRepo};
    use kuro_db::schema_anime::FetchType;

    use super::*;
    use crate::source::model::SourceKind;
    use crate::source::repo::ExtensionCatalog;
    use crate::source::stub::StubSource;
    use crate::testdb::{anime_db, manga_db};

    /// An index with no rows, for a registry that never finds index data.
    struct NoIndex;

    impl ExtensionCatalog for NoIndex {
        fn index_source(&self, _id: Id) -> Option<StubSource> {
            None
        }
    }

    /// An installed source as the registry sees it: the source and whether it talks HTTP.
    struct Installed(Source, bool);

    impl InstalledSource for Installed {
        fn id(&self) -> Id {
            self.0.id
        }
        fn is_http(&self) -> bool {
            self.1
        }
        fn source(&self) -> Source {
            self.0.clone()
        }
    }

    #[test]
    fn a_live_row_is_never_a_stub_whatever_the_live_source_reports() {
        let mut claims_stub = Source::new(3, "Live", "en");
        claims_stub.is_stub = true;
        let source = Installed(claims_stub, true);
        assert!(!live(&source).is_stub, "T-17a-012: is_stub is set from the stub type only");
        assert_eq!(online_rows(&[Arc::new(Installed(Source::new(4, "Web", "fr"), true))])[0].id, 4);
    }

    /// Serves one page, `[7]`, for every remote listing.
    struct Remote;

    struct OnePage;

    impl PageSource<u8> for OnePage {
        fn load(&self, _page: i64) -> Result<(Vec<u8>, Option<i64>), DomainError> {
            Ok((vec![7], None))
        }
    }

    impl RemoteListings<u8, ()> for Remote {
        fn popular(&self, _source_id: Id) -> Box<dyn PageSource<u8>> {
            Box::new(OnePage)
        }
        fn latest(&self, _source_id: Id) -> Box<dyn PageSource<u8>> {
            Box::new(OnePage)
        }
        fn search(&self, _source_id: Id, _query: &str, _filters: ()) -> Box<dyn PageSource<u8>> {
            Box::new(OnePage)
        }
    }

    fn manga(source: i64, url: &str, favorite: bool) -> Manga {
        Manga {
            source,
            url: url.into(),
            title: url.into(),
            favorite,
            ..Manga::create()
        }
    }

    fn anime(source: i64, url: &str, parent_id: Option<i64>, favorite: bool) -> Anime {
        Anime {
            source,
            url: url.into(),
            title: url.into(),
            parent_id,
            favorite,
            ..Anime::create()
        }
    }

    /// Waits until the list holds exactly `want`. Fails after five seconds.
    async fn wait_for<T: PartialEq>(rx: &mut watch::Receiver<Vec<T>>, want: Vec<T>) {
        tokio::time::timeout(Duration::from_secs(5), rx.wait_for(|list| *list == want))
            .await
            .expect("the list was published")
            .expect("the sender is alive");
    }

    #[tokio::test]
    async fn manga_lists_leave_out_favourite_only_sources_and_resolve_stubs() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let stubs = Arc::new(MangaStubSourceRepo::new(db));
        stubs.upsert(2, "en", "Two").unwrap();
        repo.insert(&manga(1, "/fav", true)).unwrap();
        repo.insert(&manga(2, "/a", false)).unwrap();
        repo.insert(&manga(2, "/b", false)).unwrap();
        repo.insert(&manga(2, "/fav2", true)).unwrap();
        let registry = Arc::new(SourceRegistry::<Installed>::new(SourceKind::Manga, stubs, Arc::new(NoIndex)));
        let lists = MangaSourceDb::new(&repo, registry, Arc::new(Remote));

        let two = Source {
            is_stub: true,
            ..Source::new(2, "Two", "en")
        };
        let mut non_library = lists.sources_with_non_library();
        wait_for(
            &mut non_library,
            vec![SourceWithCount {
                source: two.clone(),
                count: 2,
            }],
        )
        .await;
        let mut favorites = lists.sources_with_favorite_count();
        let unknown = Source {
            is_stub: true,
            ..Source::new(1, "", "")
        };
        wait_for(&mut favorites, vec![(unknown, 1), (two, 1)]).await;
        assert_eq!(lists.popular(2).load(1).unwrap().0, vec![7]);
    }

    #[tokio::test]
    async fn anime_lists_count_the_clear_tree_and_list_orphaned_children() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let stubs = Arc::new(AnimeStubSourceRepo::new(db));
        stubs.upsert(3, "ja", "Three").unwrap();
        repo.insert(&anime(5, "/fav", None, true)).unwrap();
        let parent = repo
            .insert(&Anime {
                fetch_type: FetchType::Seasons,
                ..anime(3, "/season", None, false)
            })
            .unwrap();
        let child = repo.insert(&anime(3, "/child", Some(parent), false)).unwrap();
        let orphan = repo.insert(&anime(3, "/kept", Some(parent), true)).unwrap();
        let registry = Arc::new(SourceRegistry::<Installed>::new(SourceKind::Anime, stubs, Arc::new(NoIndex)));
        let lists = AnimeSourceDb::new(&repo, registry, Arc::new(Remote));

        let three = Source {
            is_stub: true,
            ..Source::new(3, "Three", "ja")
        };
        let mut non_library = lists.sources_with_non_library();
        wait_for(
            &mut non_library,
            vec![SourceWithIds {
                source: three,
                ids: vec![parent, child],
                orphaned: vec![orphan],
            }],
        )
        .await;
    }

    #[tokio::test]
    async fn online_sources_are_the_installed_http_sources_and_follow_the_registry() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let registry = Arc::new(SourceRegistry::<Installed>::new(
            SourceKind::Manga,
            Arc::new(MangaStubSourceRepo::new(db)),
            Arc::new(NoIndex),
        ));
        registry.finish_loading(vec![
            Arc::new(Installed(Source::new(1, "Web", "en"), true)),
            Arc::new(Installed(Source::new(2, "Local", ""), false)),
        ]);
        let lists = MangaSourceDb::new(&repo, Arc::clone(&registry), Arc::new(Remote));

        let mut online = lists.online_sources();
        wait_for(&mut online, vec![Source::new(1, "Web", "en")]).await;
        registry.finish_loading(vec![
            Arc::new(Installed(Source::new(1, "Web", "en"), true)),
            Arc::new(Installed(Source::new(3, "More", "fr"), true)),
        ]);
        wait_for(
            &mut online,
            vec![Source::new(1, "Web", "en"), Source::new(3, "More", "fr")],
        )
        .await;
    }
}
