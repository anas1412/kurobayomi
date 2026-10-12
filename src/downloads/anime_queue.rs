//! The anime download queue at startup (lane D): the queue starts over the download tables of the anime database, the
//! saved rows come back as QUEUE, and the progress, pause and completion notices go to the desktop.
//!
//! Each saved row keeps only ids, so a restore reads the episode and its anime from the anime database. The source name
//! comes from the live source when it is installed, else from the stub table, which keeps the names of sources that were
//! listed before. A row whose episode, anime or source name is unknown is dropped; its folder stays on disk.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use kuro_data::anime::AnimeRepo;
use kuro_data::episode::EpisodeRepo;
use kuro_data::source::AnimeStubSourceRepo;
use kuro_db::handle::Db;
use kuro_domain::download::prefs::DownloadPrefs;
use kuro_domain::kind::EntryKind;
use kuro_domain::storage::StorageManager;
use kuro_db::schema_anime;
use kuro_downloads::anime::downloader::EpisodeEngine;
use kuro_downloads::anime::layout::AnimeLayout;
use kuro_downloads::anime::manager::{AnimeQueue, AnimeQueueParams, AnimeSettings, QueueEntry, spawn};
use kuro_downloads::anime::notify::AnimeNotifier;
use kuro_downloads::anime::pending::AnimePendingStore;
use kuro_downloads::anime::store::AnimeQueueStore;
use kuro_downloads::notify::{NotificationSink, NotifierSink};
use kuro_net::NetworkClient;
use kuro_platform::notifier::Notifier;
use kuro_platform::paths::Paths;
use kuro_prefs::network::NetworkPrefs;
use kuro_prefs::store::PrefStore;
use tokio::sync::broadcast::error::RecvError;

use crate::downloads::anime_io::{HttpVideoFetch, LiveHosters, live_source};
use crate::entry_core::anime::download_backend::{anime_ref, episode_ref, source_of};
use crate::entry_core::detail::SourceInfo;

/// How often the progress line is checked. The notifier itself shows at most one update a second.
const TICK: Duration = Duration::from_secs(1);

/// Starts the anime queue on the current runtime, downloading into the anime folder of `storage`. `db` is the app's anime database (the restore reads it), `prefs` the
/// store the network client reads, and `notifier` the desktop notifications.
pub fn start(
    paths: &dyn Paths,
    storage: &StorageManager,
    db: Arc<Db>,
    prefs: PrefStore,
    notifier: Arc<dyn Notifier>,
) -> Result<AnimeQueue, String> {
    let root = storage.downloads_dir(EntryKind::Anime).map_err(|e| format!("the anime downloads folder is not available: {e}"))?;
    let index = crate::updates::download_host::anime_index(storage).ok_or("the anime download index has no folder")?;
    let layout = AnimeLayout::new(Some(root));
    let store = AnimeQueueStore::open(schema_anime::open_in_data_dir(paths).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let pending = AnimePendingStore::open(schema_anime::open_in_data_dir(paths).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let restored = restore(&store, &db)?;
    // Read at every scheduling pass, so a change in Settings > Downloads applies to the running queue.
    let downloads = DownloadPrefs::new(&prefs);
    let max_active = Arc::new(move || downloads.number_of_downloads().max(1) as usize);
    let client: NetworkClient = crate::net_client::build(&NetworkPrefs::new(prefs), paths).map_err(|e| e.to_string())?;
    let runner = EpisodeEngine::new(layout.clone(), Arc::new(LiveHosters)).with_fetch(Arc::new(HttpVideoFetch::new(client)));
    let queue = spawn(AnimeQueueParams {
        layout,
        store: Arc::new(store),
        pending: Arc::new(pending),
        free_space: crate::downloads::free_space,
        runner: Arc::new(runner),
        settings: AnimeSettings::default(),
        restored,
        index,
        max_active,
    });
    notices(queue.clone(), notifier);
    Ok(queue)
}

/// The saved queue as QUEUE entries, in saved order. The store keeps only the rows whose entry could be rebuilt.
fn restore(store: &AnimeQueueStore, db: &Arc<Db>) -> Result<Vec<QueueEntry>, String> {
    let mut built: HashMap<(i64, i64), QueueEntry> = HashMap::new();
    for (episode_id, anime_id, _) in store.rows().map_err(|e| e.to_string())? {
        if let Some(entry) = restore_entry(db, anime_id, episode_id) {
            built.insert((anime_id, episode_id), entry);
        }
    }
    let kept = store
        .restore(|anime_id, episode_id| built.contains_key(&(anime_id, episode_id)))
        .map_err(|e| e.to_string())?;
    Ok(kept.into_iter().filter_map(|pair| built.remove(&pair)).collect())
}

/// One saved row as a queue entry, or `None` when its episode, anime or source name is unknown.
fn restore_entry(db: &Arc<Db>, anime_id: i64, episode_id: i64) -> Option<QueueEntry> {
    let episode = EpisodeRepo::new(Arc::clone(db)).by_id(episode_id).ok()??;
    let anime = AnimeRepo::new(Arc::clone(db)).by_id(anime_id).ok()?;
    let (name, stub) = source_name(db, anime.source)?;
    let info = SourceInfo { name, stub };
    Some(QueueEntry {
        source: source_of(&anime, &info),
        anime: anime_ref(&anime),
        episode: episode_ref(&episode),
        alt: false,
        video: None,
    })
}

/// `Name (LANG)` of a source and whether it is a stub: the live source when it is installed, else the name the stub
/// table kept (`stub: true`, the app's flag for a source that is not installed).
fn source_name(db: &Arc<Db>, source_id: i64) -> Option<(String, bool)> {
    if let Some(live) = live_source(source_id) {
        return Some((kuro_source::display_name(live.name(), live.lang()), false));
    }
    AnimeStubSourceRepo::new(Arc::clone(db)).get(source_id).ok()?.map(|stub| (stub.display_name(), true))
}

/// Forwards the queue's events to the desktop, and keeps the progress line on the downloading item while the run goes.
/// A pause shows the paused notice, and a run that ends with nothing queued withdraws the line.
fn notices(queue: AnimeQueue, notifier: Arc<dyn Notifier>) {
    let sink: Arc<dyn NotificationSink> = Arc::new(NotifierSink::new(notifier));
    let mut notice = AnimeNotifier::new(sink, Arc::new(AtomicBool::new(false)));
    let mut events = queue.events();
    let mut ticks = tokio::time::interval(TICK);
    tokio::spawn(async move {
        let mut was_running = false;
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Ok(event) => notice.dispatch(Instant::now(), &event),
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                },
                _ = ticks.tick() => {
                    let running = queue.running();
                    let snapshot = queue.snapshot();
                    // no notification while downloading (the Downloads item shows it); only the end is announced
                    // a stop with episodes left (a pause or a failure) posts nothing: the failure has its own notice
                    if !running && was_running && snapshot.is_empty() {
                        notice.complete();
                    }
                    was_running = running;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::anime::Anime;
    use kuro_data::episode::Episode;

    fn anime_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tachiyomi.animedb");
        let writer = schema_anime::open(&path).unwrap();
        (dir, Arc::new(Db::new(&path, writer).unwrap()))
    }

    /// An anime of `source` with one episode. Returns `(anime_id, episode_id)`.
    fn add(db: &Arc<Db>, url: &str, source: i64) -> (i64, i64) {
        let anime = Anime { url: url.into(), title: url.into(), source, status: 1, ..Anime::create() };
        let anime_id = AnimeRepo::new(Arc::clone(db)).insert(&anime).unwrap();
        let episode = Episode { anime_id, url: url.into(), name: format!("Episode {url}"), ..Episode::create() };
        let added = EpisodeRepo::new(Arc::clone(db)).add_all(&[episode]).unwrap();
        (anime_id, added[0].id)
    }

    #[test]
    fn a_source_with_only_a_stub_row_is_named_by_it_and_flagged_as_a_stub() {
        let (_dir, db) = anime_db();
        AnimeStubSourceRepo::new(Arc::clone(&db)).upsert(77, "en", "Site").unwrap();
        assert_eq!(source_name(&db, 77), Some(("Site (EN)".to_owned(), true)));
    }

    #[test]
    fn a_source_with_no_live_source_and_no_stub_has_no_name() {
        let (_dir, db) = anime_db();
        assert_eq!(source_name(&db, 88), None);
    }

    #[test]
    fn a_saved_row_comes_back_with_its_stub_flag_and_ids() {
        let (_dir, db) = anime_db();
        AnimeStubSourceRepo::new(Arc::clone(&db)).upsert(77, "en", "Site").unwrap();
        let (anime_id, episode_id) = add(&db, "a", 77);
        let entry = restore_entry(&db, anime_id, episode_id).expect("the row is rebuilt");
        assert_eq!((entry.anime.id, entry.episode.id), (anime_id, episode_id));
        assert_eq!(entry.source.id, 77);
        assert!(!entry.source.is_http, "a source that is not installed is a stub");
        assert!(entry.video.is_none() && !entry.alt);
    }

    #[test]
    fn a_row_is_dropped_when_its_episode_anime_or_source_name_is_unknown() {
        let (_dir, db) = anime_db();
        AnimeStubSourceRepo::new(Arc::clone(&db)).upsert(77, "en", "Site").unwrap();
        let (anime_id, episode_id) = add(&db, "a", 77);
        let (no_name_anime, no_name_episode) = add(&db, "b", 88);
        assert!(restore_entry(&db, anime_id, 9999).is_none(), "unknown episode");
        assert!(restore_entry(&db, 9999, episode_id).is_none(), "unknown anime");
        assert!(restore_entry(&db, no_name_anime, no_name_episode).is_none(), "unknown source name");
    }

    #[test]
    fn restore_keeps_the_rebuilt_rows_in_saved_order_and_drops_the_rest_from_the_store() {
        let (_dir, db) = anime_db();
        AnimeStubSourceRepo::new(Arc::clone(&db)).upsert(77, "en", "Site").unwrap();
        let (a1, e1) = add(&db, "a", 77);
        let (a2, e2) = add(&db, "b", 77);
        let (a3, e3) = add(&db, "c", 88);
        let store = AnimeQueueStore::open_in_memory().unwrap();
        store.replace_all(&[(e2, a2), (e3, a3), (e1, a1)]).unwrap();
        let kept = restore(&store, &db).unwrap();
        assert_eq!(kept.iter().map(|e| e.episode.id).collect::<Vec<_>>(), vec![e2, e1]);
        assert_eq!(store.rows().unwrap().iter().map(|r| r.0).collect::<Vec<_>>(), vec![e2, e1]);
    }
}
