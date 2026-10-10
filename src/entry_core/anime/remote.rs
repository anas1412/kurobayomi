//! The source side of an anime refresh (T-39b-021). `RemoteAnimeBackend` runs a fetch through the domain's
//! `UpdateFromRemote`: it asks the source, writes the metadata with the cover and background rules (T-REF-5), syncs the
//! episode list and returns the episodes the sync added, which a manual refresh may queue for download (T-REF-4).
//!
//! Where a source comes from is `AnimeSourceLookup`. `LiveSources` answers for the sources this build ships
//! (`builtin_anime_sources`, none yet); an id it does not know reports that the source is not installed. A `Seasons`
//! entry syncs its season list through the domain's `SyncSeasons`, and with "update seasons on refresh" on, the episodes
//! of its seasons are fetched after it (`season_fetch`).
//!
//! Known gap: no cover cache is wired, so a changed cover is stamped but its file is not deleted.

use std::collections::HashSet;
use std::sync::Arc;

use kuro_data::anime::Anime;
use kuro_data::episode::Episode;
use kuro_db::handle::Db;
use kuro_db::schema_anime::FetchType;
use kuro_domain::error::DomainError;
use async_trait::async_trait;
use kuro_domain::items::episode_sync::{EpisodeSyncOptions, NoEpisodeRenames, SyncEpisodes, mark_duplicate_as_seen_from_pref};
use kuro_domain::items::season_sync::SyncSeasons;
use kuro_domain::items::source_sync::MangaSyncLocks;
use kuro_domain::source::remote_update::{AnimeRemote, CacheFiles, DbRemoteStore, FetchFlags, ListSync, RemoteError, RemoteStore, UpdateFromRemote};
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::store::PrefStore;
use kuro_source::SourceError;
use kuro_source::anime::registry::builtin_anime_sources;
use kuro_source::anime::{AnimeSource, SAnime, SAnimeEpisodeUpdate, SAnimeSeasonUpdate, SEpisode};
use tokio::runtime::Handle;

use super::entry_state::RefreshPlan;
use super::download_backend::AnimeDownloads;
use super::refresh::Trigger;
use super::runner::{AnimeBackend, AnimeFetch, StubAnimeBackend};
use super::season_fetch::fetch_episodes_of;
use super::store::AnimeStore;
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
    /// Whether the source is an HTTP source (`source is AnimeHttpSource`): only then does the page offer WebView.
    fn is_http(&self, _source_id: i64) -> bool {
        false
    }
    /// Whether the source is configurable (`source is ConfigurableAnimeSource`): only then does the overflow menu offer
    /// Settings (T-39a-057).
    fn is_configurable(&self, _source_id: i64) -> bool {
        false
    }
    /// Whether the source is an `UnmeteredSource`: a season refresh fetches its seasons in parallel.
    fn is_unmetered(&self, _source_id: i64) -> bool {
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

/// The lookup over live sources. The flags come from the source itself (`is_configurable`, `is_unmetered`); the trait has
/// no HTTP or torrent marker, so those two read false and the page offers neither WebView nor the torrent start.
pub struct LiveSources {
    sources: Vec<Arc<dyn AnimeSource>>,
}

impl LiveSources {
    pub fn new(sources: Vec<Arc<dyn AnimeSource>>) -> Self {
        Self { sources }
    }

    /// The sources this build ships.
    pub fn builtin() -> Self {
        Self::new(builtin_anime_sources())
    }

    fn find(&self, source_id: i64) -> Option<&Arc<dyn AnimeSource>> {
        self.sources.iter().find(|s| s.id() == source_id)
    }
}

/// A live source as the domain's remote update asks it: its own combined updates.
struct SourceRemote(Arc<dyn AnimeSource>);

#[async_trait]
impl AnimeRemote for SourceRemote {
    async fn episode_update(&self, anime: SAnime, episodes: Vec<SEpisode>, fetch_details: bool, fetch_episodes: bool) -> Result<SAnimeEpisodeUpdate, SourceError> {
        self.0.anime_episode_update(anime, episodes, fetch_details, fetch_episodes).await
    }

    async fn season_update(&self, anime: SAnime, seasons: Vec<SAnime>, fetch_details: bool, fetch_seasons: bool) -> Result<SAnimeSeasonUpdate, SourceError> {
        self.0.anime_season_update(anime, seasons, fetch_details, fetch_seasons).await
    }
}

impl AnimeSourceLookup for LiveSources {
    fn remote(&self, source_id: i64) -> Option<Arc<dyn AnimeRemote>> {
        self.find(source_id).map(|source| Arc::new(SourceRemote(Arc::clone(source))) as Arc<dyn AnimeRemote>)
    }

    fn info(&self, source_id: i64) -> Option<SourceInfo> {
        self.find(source_id).map(|source| SourceInfo { name: source.display_name(), stub: false })
    }

    fn is_configurable(&self, source_id: i64) -> bool {
        self.find(source_id).is_some_and(|source| source.is_configurable())
    }

    fn is_unmetered(&self, source_id: i64) -> bool {
        self.find(source_id).is_some_and(|source| source.is_unmetered())
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

/// The list sync of `UpdateFromRemote` over the domain's async episode sync and its season sync. It runs on a tokio
/// worker, so the wait for the episode sync is `block_in_place`.
struct EpisodeListSync {
    sync: SyncEpisodes,
    seasons: SyncSeasons,
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

    fn sync_seasons(&self, raw: &[SAnime], anime: &Anime, manual: bool, window: (i64, i64)) -> Result<Vec<Anime>, DomainError> {
        self.seasons.run(raw, anime, manual, window, chrono::Utc::now().timestamp_millis())
    }
}

/// The backend of the anime page over a source lookup.
pub struct RemoteAnimeBackend {
    stubs: StubAnimeBackend,
    lookup: Arc<dyn AnimeSourceLookup>,
    store: Arc<DbRemoteStore>,
    update: Arc<UpdateFromRemote>,
    episodes: AnimeStore,
    downloads: Option<AnimeDownloads>,
    library: LibraryPrefs,
}

impl RemoteAnimeBackend {
    /// `handle` is the runtime the fetches run on, which must be a multi-threaded one.
    pub fn new(db: Arc<Db>, prefs: &PrefStore, lookup: Arc<dyn AnimeSourceLookup>, handle: Handle) -> Self {
        let library = LibraryPrefs::new(prefs.clone());
        let mark_duplicate_as_seen = mark_duplicate_as_seen_from_pref(&library.mark_duplicate_seen_episode_as_seen.get());
        let sync = SyncEpisodes::new(Arc::clone(&db), Arc::new(MangaSyncLocks::default()), Arc::new(NoEpisodeRenames));
        let list_sync = EpisodeListSync { sync, seasons: SyncSeasons::new(Arc::clone(&db)), handle, mark_duplicate_as_seen };
        let store = Arc::new(DbRemoteStore::new(Arc::clone(&db)));
        let update = Arc::new(UpdateFromRemote::new(store.clone(), Arc::new(list_sync), Arc::new(NoCoverFiles)));
        Self { stubs: StubAnimeBackend::new(Arc::clone(&db)), lookup, store, update, episodes: AnimeStore::new(db), downloads: None, library }
    }

    /// Gives the backend the downloads of the page: the downloads folder and the app's queue.
    pub fn with_downloads(mut self, downloads: AnimeDownloads) -> Self {
        self.downloads = Some(downloads);
        self
    }

    /// The entry and the episodes with these ids (all of them when `ids` is `None`), with the source line.
    fn entry_episodes(&self, anime_id: i64, ids: Option<&[i64]>) -> Option<(Anime, Vec<Episode>, SourceInfo)> {
        let anime = self.episodes.anime(anime_id).ok().flatten()?;
        let episodes: Vec<Episode> = self.episodes.episodes(anime_id).ok()?.into_iter().filter(|e| ids.is_none_or(|ids| ids.contains(&e.id))).collect();
        let info = self.source_info(anime.source);
        Some((anime, episodes, info))
    }
}

impl AnimeBackend for RemoteAnimeBackend {
    fn source_info(&self, source_id: i64) -> SourceInfo {
        self.lookup.info(source_id).unwrap_or_else(|| self.stubs.source_info(source_id))
    }

    fn fetch(&self, anime_id: i64, plan: RefreshPlan, trigger: Trigger) -> AnimeFetch {
        let (store, update, lookup) = (Arc::clone(&self.store), Arc::clone(&self.update), Arc::clone(&self.lookup));
        let season_episodes = self.library.update_season_on_refresh.get();
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
                FetchType::Seasons => {
                    let seasons = update.seasons(&anime, &*remote, flags).await.map_err(detail_error)?.new_seasons;
                    // behaviour 6.4.1: with "update seasons on refresh" on, the episodes of the unfinished seasons follow
                    if season_episodes {
                        let episode_flags = FetchFlags { fetch_details: false, fetch_episodes: true, fetch_seasons: false, ..flags };
                        fetch_episodes_of(seasons, lookup.is_unmetered(anime.source), |season| {
                            let (update, remote) = (Arc::clone(&update), Arc::clone(&remote));
                            async move {
                                // one failing season is logged and does not stop the others; new episodes are not downloaded
                                match update.episodes(&season, &*remote, episode_flags).await {
                                    Ok(_) | Err(RemoteError::Cancelled | RemoteError::Source(SourceError::Cancelled)) => {}
                                    Err(error) => tracing::error!(season = season.id, %error, "the episodes of a season were not fetched"),
                                }
                            }
                        })
                        .await;
                    }
                    Ok(Vec::new())
                }
            }
        })
    }

    fn is_torrent_source(&self, source_id: i64) -> bool {
        self.lookup.is_torrent(source_id)
    }

    fn is_http_source(&self, source_id: i64) -> bool {
        self.lookup.is_http(source_id)
    }

    fn is_configurable_source(&self, source_id: i64) -> bool {
        self.lookup.is_configurable(source_id)
    }

    fn has_download_handler(&self) -> bool {
        self.downloads.as_ref().is_some_and(AnimeDownloads::can_enqueue)
    }

    fn downloaded_episodes(&self, anime_id: i64) -> HashSet<i64> {
        let (Some(downloads), Some((anime, episodes, info))) = (&self.downloads, self.entry_episodes(anime_id, None)) else { return HashSet::new() };
        downloads.downloaded(&anime, &episodes, &info)
    }

    fn downloaded_count(&self, anime_id: i64) -> usize {
        self.downloaded_episodes(anime_id).len()
    }

    fn enqueue_episodes(&self, anime_id: i64, episode_ids: &[i64], start_now: bool) -> bool {
        let (Some(downloads), Some((anime, episodes, info))) = (&self.downloads, self.entry_episodes(anime_id, Some(episode_ids))) else { return false };
        downloads.enqueue(&anime, &episodes, &info, start_now)
    }

    fn delete_episodes(&self, anime_id: i64, episode_ids: &[i64]) {
        let (Some(downloads), Some((anime, episodes, info))) = (&self.downloads, self.entry_episodes(anime_id, Some(episode_ids))) else { return };
        let categories = self.episodes.category_ids(anime_id).unwrap_or_default();
        downloads.delete(&anime, &episodes, categories, &info);
    }

    fn delete_downloads(&self, anime_id: i64) {
        let on_disk: Vec<i64> = self.downloaded_episodes(anime_id).into_iter().collect();
        self.delete_episodes(anime_id, &on_disk);
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
        seasons: Vec<SAnime>,
        http_failure: Option<u16>,
        asked: Mutex<Vec<(bool, bool)>>,
    }

    impl FakeSource {
        fn new(episodes: Vec<SEpisode>) -> Arc<Self> {
            Arc::new(Self { anime: SAnime { title: "Frieren".into(), ..SAnime::default() }, episodes, seasons: Vec::new(), http_failure: None, asked: Mutex::default() })
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
            if self.seasons.is_empty() {
                return Err(SourceError::Unsupported("seasons"));
            }
            Ok(SAnimeSeasonUpdate { anime: self.anime.clone(), seasons: self.seasons.clone() })
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

    fn seasons_source() -> Arc<FakeSource> {
        let seasons = vec![
            SAnime { url: "/s1".into(), title: "Frieren Season 1".into(), ..SAnime::default() },
            SAnime { url: "/s2".into(), title: "Frieren Season 2".into(), ..SAnime::default() },
        ];
        Arc::new(FakeSource { seasons, ..Arc::into_inner(FakeSource::new(vec![s_episode("/e1", 1.0)])).unwrap() })
    }

    /// Refreshes a parent that lists seasons from `source`: the temp dir to keep alive, the ids of the stored seasons and the store.
    fn season_refresh(rt: &tokio::runtime::Runtime, update_seasons: bool, source: &Arc<FakeSource>) -> (tempfile::TempDir, Vec<i64>, AnimeStore) {
        let (dir, store, id) = with_episodes(true, true, 0, 0);
        store.set_fetch_type_for_test(id, FetchType::Seasons);
        let prefs = PrefStore::in_memory();
        LibraryPrefs::new(prefs.clone()).update_season_on_refresh.set(update_seasons).unwrap();
        let backend = RemoteAnimeBackend::new(store.db(), &prefs, Arc::new(One(source.clone())), rt.handle().clone());
        let plan = RefreshPlan { info: true, episodes: false, seasons: true };
        rt.block_on(backend.fetch(id, plan, Trigger::Manual)).expect("the refresh works");
        let ids = kuro_data::anime::AnimeRepo::new(store.db()).seasons_by_parent(id).unwrap().into_iter().map(|s| s.anime.id).collect();
        (dir, ids, store)
    }

    #[test]
    fn a_seasons_refresh_stores_every_season_and_fetches_the_episodes_of_each_when_the_pref_is_on() {
        let rt = rt();
        let source = seasons_source();
        let (_dir, ids, store) = season_refresh(&rt, true, &source);
        assert_eq!(ids.len(), 2, "both source seasons are stored as children");
        for id in ids {
            assert_eq!(store.episodes(id).unwrap().len(), 1, "the episodes of the season were stored");
        }
        assert_eq!(*source.asked.lock().unwrap(), vec![(false, true), (false, true)], "one episode call per season, with no details");
    }

    #[test]
    fn a_seasons_refresh_leaves_the_episodes_alone_when_the_pref_is_off() {
        let rt = rt();
        let source = seasons_source();
        let (_dir, ids, store) = season_refresh(&rt, false, &source);
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().all(|id| store.episodes(*id).unwrap().is_empty()));
        assert!(source.asked.lock().unwrap().is_empty());
    }

    #[test]
    fn the_live_sources_know_only_their_own_ids() {
        let lookup = LiveSources::new(Vec::new());
        assert!(lookup.remote(7).is_none());
        assert!(!lookup.is_configurable(7) && !lookup.is_unmetered(7));
        assert!(LiveSources::builtin().info(7).is_none(), "no builtin source has this id");
    }

    #[test]
    fn an_error_of_each_kind_has_a_page_error() {
        assert_eq!(detail_error(RemoteError::Source(SourceError::NotInstalled(1))), DetailError::SourceNotInstalled);
        assert_eq!(detail_error(RemoteError::Cancelled), DetailError::Cancelled);
        assert_eq!(detail_error(RemoteError::Source(SourceError::UnknownHost { host: "h".into() })), DetailError::UnknownHost { online: true, host: "h".into() });
        assert!(matches!(detail_error(RemoteError::Fetch("x".into())), DetailError::Other { .. }));
    }
}
