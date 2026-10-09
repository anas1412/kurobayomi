//! The source side of an anime refresh (T-39b-021). `RemoteAnimeBackend` runs a fetch through the domain's
//! `UpdateFromRemote`: it asks the source, writes the metadata with the cover and background rules (T-REF-5), syncs the
//! episode list and returns the episodes the sync added, which a manual refresh may queue for download (T-REF-4).
//!
//! Where a source comes from is `AnimeSourceLookup`. The app has no registry of live anime sources yet, so the page
//! passes `NoSources` and every fetch reports that the source is not installed, as the stub did; a lookup that hands
//! out sources is the only thing missing for a real fetch.
//!
//! Known gaps: the list sync of seasons does not exist in the domain layer, so a `Seasons` entry reports an error
//! instead of syncing, and no cover cache is wired, so a changed cover is stamped but its file is not deleted.

use std::sync::Arc;

use kuro_data::anime::Anime;
use kuro_data::episode::Episode;
use kuro_db::handle::Db;
use kuro_db::schema_anime::FetchType;
use kuro_domain::error::DomainError;
use kuro_domain::items::episode_sync::{EpisodeSyncOptions, NoEpisodeRenames, SyncEpisodes, mark_duplicate_as_seen_from_pref};
use kuro_domain::items::source_sync::MangaSyncLocks;
use kuro_domain::source::remote_update::{AnimeRemote, CacheFiles, DbRemoteStore, FetchFlags, ListSync, RemoteError, RemoteStore, UpdateFromRemote};
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::store::PrefStore;
use kuro_source::SourceError;
use kuro_source::anime::{SAnime, SEpisode};
use tokio::runtime::Handle;

use super::entry_state::RefreshPlan;
use super::refresh::Trigger;
use super::runner::{AnimeBackend, AnimeFetch, StubAnimeBackend};
use crate::entry_core::detail::SourceInfo;
use crate::entry_core::errors::DetailError;

/// Finds the live anime source of an id.
pub trait AnimeSourceLookup: Send + Sync + 'static {
    /// The source, or `None` when its extension is not installed.
    fn remote(&self, source_id: i64) -> Option<Arc<dyn AnimeRemote>>;
    /// The source line of the info box for an installed source. `None` falls back to the stub reading.
    fn info(&self, _source_id: i64) -> Option<SourceInfo> {
        None
    }
    /// Whether the source serves torrents (`isSourceForTorrents`).
    fn is_torrent(&self, _source_id: i64) -> bool {
        false
    }
}

/// The lookup of an app with no source registry: every source reads as not installed.
pub struct NoSources;

impl AnimeSourceLookup for NoSources {
    fn remote(&self, _source_id: i64) -> Option<Arc<dyn AnimeRemote>> {
        None
    }
}

/// No cover cache is wired: an entry never has a custom cover file, and no file is deleted.
struct NoCoverFiles;

impl CacheFiles for NoCoverFiles {
    fn has_custom_cover(&self, _anime: &Anime) -> bool {
        false
    }
    fn has_custom_background(&self, _anime: &Anime) -> bool {
        false
    }
    fn delete_cover(&self, _anime: &Anime) {}
    fn delete_background(&self, _anime: &Anime) {}
}

/// The list sync of `UpdateFromRemote` over the domain's async episode sync. It runs on a tokio worker, so the wait
/// for the sync is `block_in_place`.
struct EpisodeListSync {
    sync: SyncEpisodes,
    handle: Handle,
    mark_duplicate_as_seen: bool,
}

impl ListSync for EpisodeListSync {
    fn sync_episodes(&self, raw: &[SEpisode], anime: &Anime, manual: bool, window: (i64, i64)) -> Result<Vec<Episode>, DomainError> {
        let opts = EpisodeSyncOptions {
            now_ms: chrono::Utc::now().timestamp_millis(),
            manual,
            window,
            mark_duplicate_as_seen: self.mark_duplicate_as_seen,
            is_local: anime.source == kuro_source::local::LOCAL_SOURCE_ID,
        };
        let run = self.sync.run(raw.to_vec(), anime, opts);
        tokio::task::block_in_place(|| self.handle.block_on(run)).map_err(|e| match e {
            kuro_domain::items::episode_sync::EpisodeSyncError::Db(e) => DomainError::Db(e),
            kuro_domain::items::episode_sync::EpisodeSyncError::NoEpisodes(_) => DomainError::Invalid("the source returned no episodes"),
        })
    }

    fn sync_seasons(&self, _raw: &[SAnime], _anime: &Anime, _manual: bool, _window: (i64, i64)) -> Result<Vec<Anime>, DomainError> {
        Err(DomainError::Invalid("the season list sync is not available yet"))
    }
}

/// The backend of the anime page over a source lookup.
pub struct RemoteAnimeBackend {
    stubs: StubAnimeBackend,
    lookup: Arc<dyn AnimeSourceLookup>,
    store: Arc<DbRemoteStore>,
    update: Arc<UpdateFromRemote>,
}

impl RemoteAnimeBackend {
    /// `handle` is the runtime the fetches run on, which must be a multi-threaded one.
    pub fn new(db: Arc<Db>, prefs: &PrefStore, lookup: Arc<dyn AnimeSourceLookup>, handle: Handle) -> Self {
        let mark_duplicate_as_seen = mark_duplicate_as_seen_from_pref(&LibraryPrefs::new(prefs.clone()).mark_duplicate_seen_episode_as_seen.get());
        let sync = SyncEpisodes::new(Arc::clone(&db), Arc::new(MangaSyncLocks::default()), Arc::new(NoEpisodeRenames));
        let list_sync = EpisodeListSync { sync, handle, mark_duplicate_as_seen };
        let store = Arc::new(DbRemoteStore::new(Arc::clone(&db)));
        let update = Arc::new(UpdateFromRemote::new(store.clone(), Arc::new(list_sync), Arc::new(NoCoverFiles)));
        Self { stubs: StubAnimeBackend::new(db), lookup, store, update }
    }
}

impl AnimeBackend for RemoteAnimeBackend {
    fn source_info(&self, source_id: i64) -> SourceInfo {
        self.lookup.info(source_id).unwrap_or_else(|| self.stubs.source_info(source_id))
    }

    fn fetch(&self, anime_id: i64, plan: RefreshPlan, trigger: Trigger) -> AnimeFetch {
        let (store, update, lookup) = (Arc::clone(&self.store), Arc::clone(&self.update), Arc::clone(&self.lookup));
        Box::pin(async move {
            let anime = store.anime(anime_id).map_err(domain_error)?;
            let Some(remote) = lookup.remote(anime.source) else {
                return Err(DetailError::SourceNotInstalled);
            };
            let flags = FetchFlags {
                fetch_details: plan.info,
                fetch_episodes: plan.episodes,
                fetch_seasons: plan.seasons,
                manual_fetch: trigger == Trigger::Manual,
                fetch_window: (0, 0),
            };
            match anime.fetch_type {
                FetchType::Episodes => update.episodes(&anime, &*remote, flags).await.map(|r| r.new_episodes).map_err(detail_error),
                FetchType::Seasons => update.seasons(&anime, &*remote, flags).await.map(|_| Vec::new()).map_err(detail_error),
            }
        })
    }

    fn is_torrent_source(&self, source_id: i64) -> bool {
        self.lookup.is_torrent(source_id)
    }
}

fn domain_error(e: DomainError) -> DetailError {
    DetailError::Other { class: "SQLiteException".to_owned(), message: Some(e.to_string()) }
}

/// The page's error for a failed remote update.
fn detail_error(e: RemoteError) -> DetailError {
    match e {
        RemoteError::Source(SourceError::NotInstalled(_)) => DetailError::SourceNotInstalled,
        RemoteError::Source(SourceError::Http(code)) => DetailError::Http(i64::from(code)),
        RemoteError::Source(SourceError::UnknownHost { host }) => DetailError::UnknownHost { online: true, host },
        RemoteError::Source(SourceError::Cancelled) | RemoteError::Cancelled => DetailError::Cancelled,
        RemoteError::Source(other) => DetailError::Other { class: "IOException".to_owned(), message: Some(other.to_string()) },
        RemoteError::Db(e) => domain_error(e),
        RemoteError::Fetch(message) => DetailError::Other { class: "Exception".to_owned(), message: Some(message) },
        RemoteError::NoEpisodes(_) => DetailError::NoEpisodes,
        RemoteError::NoSeasons(_) => DetailError::NoSeasons,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use kuro_source::anime::{SAnimeEpisodeUpdate, SAnimeSeasonUpdate};

    use super::super::store::fixture::with_episodes;
    use super::*;

    /// A source that answers with canned data, records the `(fetch_details, fetch_list)` flags of each call and can
    /// fail with an HTTP status.
    struct FakeSource {
        anime: SAnime,
        episodes: Vec<SEpisode>,
        http_failure: Option<u16>,
        asked: Mutex<Vec<(bool, bool)>>,
    }

    impl FakeSource {
        fn new(episodes: Vec<SEpisode>) -> Arc<Self> {
            Arc::new(Self { anime: SAnime { title: "Frieren".into(), ..SAnime::default() }, episodes, http_failure: None, asked: Mutex::default() })
        }
    }

    #[async_trait]
    impl AnimeRemote for FakeSource {
        async fn episode_update(&self, _: SAnime, _: Vec<SEpisode>, fetch_details: bool, fetch_episodes: bool) -> Result<SAnimeEpisodeUpdate, SourceError> {
            self.asked.lock().unwrap().push((fetch_details, fetch_episodes));
            if let Some(code) = self.http_failure {
                return Err(SourceError::Http(code));
            }
            Ok(SAnimeEpisodeUpdate { anime: self.anime.clone(), episodes: self.episodes.clone() })
        }

        async fn season_update(&self, _: SAnime, _: Vec<SAnime>, _: bool, _: bool) -> Result<SAnimeSeasonUpdate, SourceError> {
            Err(SourceError::Unsupported("seasons"))
        }
    }

    struct One(Arc<FakeSource>);

    impl AnimeSourceLookup for One {
        fn remote(&self, _: i64) -> Option<Arc<dyn AnimeRemote>> {
            Some(self.0.clone())
        }
    }

    fn s_episode(url: &str, number: f32) -> SEpisode {
        SEpisode { url: url.into(), name: format!("Episode {number}"), episode_number: number, ..SEpisode::default() }
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap()
    }

    fn backend(db: Arc<Db>, source: Arc<FakeSource>, handle: Handle) -> RemoteAnimeBackend {
        RemoteAnimeBackend::new(db, &PrefStore::in_memory(), Arc::new(One(source)), handle)
    }

    const PLAN: RefreshPlan = RefreshPlan { info: true, episodes: true, seasons: false };

    #[test]
    fn a_fetch_stores_what_the_source_lists_and_returns_the_new_episodes() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, false, 1, 0);
        let backend = backend(store.db(), FakeSource::new(vec![s_episode("/e1", 1.0), s_episode("/e2", 2.0)]), rt.handle().clone());
        let new = rt.block_on(backend.fetch(id, PLAN, Trigger::Manual)).expect("the fetch works");
        assert_eq!(new.iter().map(|e| e.episode_number).collect::<Vec<_>>(), [2.0], "the one that was not stored before");
        assert_eq!(store.episodes(id).unwrap().len(), 2);
        assert!(store.anime(id).unwrap().unwrap().initialized, "the metadata write marks it initialized");
    }

    #[test]
    fn an_unknown_source_and_the_source_errors_map_to_the_page_errors() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 1, 0);
        let none = RemoteAnimeBackend::new(store.db(), &PrefStore::in_memory(), Arc::new(NoSources), rt.handle().clone());
        assert_eq!(rt.block_on(none.fetch(id, PLAN, Trigger::Open)), Err(DetailError::SourceNotInstalled));
        assert!(matches!(rt.block_on(none.fetch(4242, PLAN, Trigger::Open)), Err(DetailError::Other { .. })), "a missing row is a store error");
        let failing = Arc::new(FakeSource { http_failure: Some(503), ..Arc::into_inner(FakeSource::new(vec![])).unwrap() });
        let backend = backend(store.db(), failing, rt.handle().clone());
        assert_eq!(rt.block_on(backend.fetch(id, PLAN, Trigger::Manual)), Err(DetailError::Http(503)));
        let backend = self::backend(store.db(), FakeSource::new(vec![]), rt.handle().clone());
        assert_eq!(rt.block_on(backend.fetch(id, PLAN, Trigger::Manual)), Err(DetailError::NoEpisodes), "a source with no episodes");
    }

    #[test]
    fn the_plan_reaches_the_source_call() {
        let rt = rt();
        let (_dir, store, id) = with_episodes(true, true, 1, 0);
        let source = FakeSource::new(vec![s_episode("/e1", 1.0)]);
        let backend = backend(store.db(), source.clone(), rt.handle().clone());
        let plan = RefreshPlan { info: false, episodes: true, seasons: false };
        rt.block_on(backend.fetch(id, plan, Trigger::Open)).unwrap();
        assert_eq!(*source.asked.lock().unwrap(), vec![(false, true)], "details are not asked for");
    }

    #[test]
    fn an_error_of_each_kind_has_a_page_error() {
        assert_eq!(detail_error(RemoteError::Source(SourceError::NotInstalled(1))), DetailError::SourceNotInstalled);
        assert_eq!(detail_error(RemoteError::Cancelled), DetailError::Cancelled);
        assert_eq!(detail_error(RemoteError::Source(SourceError::UnknownHost { host: "h".into() })), DetailError::UnknownHost { online: true, host: "h".into() });
        assert!(matches!(detail_error(RemoteError::Fetch("x".into())), DetailError::Other { .. }));
    }
}
