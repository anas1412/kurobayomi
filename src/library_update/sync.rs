//! The database side of the item sync of a pass (behaviour 5.4 and 5.5): the `ItemSync` the pass calls for each
//! entry. The diff is the domain's (`kuro_domain::items::chapter_sync`, `episode_sync` and `sync_plan`). This file
//! turns the pass's `SourceItem`s into the source models, runs the domain sync on the entry's row and maps the new
//! rows back to `SourceItem`.

use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;
use kuro_data::manga::{Manga, MangaRepo};
use kuro_db::handle::Db;
use kuro_domain::items::chapter_sync::{ChapterRenames, ChapterSyncError, ChapterSyncOptions, NoRenames, SyncChapters};
use kuro_domain::items::episode_sync::{EpisodeRenames, EpisodeSyncError, EpisodeSyncOptions, NoEpisodeRenames, SyncEpisodes};
use kuro_domain::items::source_sync::MangaSyncLocks;
use kuro_platform::kind::Kind;
use kuro_source::anime::SEpisode;
use kuro_source::local::LOCAL_SOURCE_ID;
use kuro_source::model::chapter::SChapter;

use crate::library_update::error::UpdateError;
use crate::library_update::fetch_interval::Window;
use crate::library_update::pass::ItemSync;
use crate::library_update::queue::LibraryEntry;
use crate::library_update::source::{Fetched, SourceItem};

/// The item sync over the app database. `chapter_locks` and `episode_locks` are separate because the domain keys its
/// locks by manga id and anime id, which are different id spaces. The caller shares them with other writers of the
/// same rows, so a pass and a manual refresh never interleave.
pub struct DbItemSync {
    db: Arc<Db>,
    chapter_locks: Arc<MangaSyncLocks>,
    episode_locks: Arc<MangaSyncLocks>,
    chapters: SyncChapters,
    episodes: SyncEpisodes,
    mark_duplicate_read: bool,
    mark_duplicate_seen: bool,
}

impl DbItemSync {
    pub fn new(
        db: Arc<Db>,
        chapter_locks: Arc<MangaSyncLocks>,
        episode_locks: Arc<MangaSyncLocks>,
        mark_duplicate_read: bool,
        mark_duplicate_seen: bool,
    ) -> Self {
        let chapters = SyncChapters::new(db.clone(), chapter_locks.clone(), Arc::new(NoRenames));
        let episodes = SyncEpisodes::new(db.clone(), episode_locks.clone(), Arc::new(NoEpisodeRenames));
        Self { db, chapter_locks, episode_locks, chapters, episodes, mark_duplicate_read, mark_duplicate_seen }
    }

    /// Takes the folder renames of downloaded chapters and episodes instead of the no-op ones (T-26a-086).
    pub fn with_renames(mut self, chapter_renames: Arc<dyn ChapterRenames>, episode_renames: Arc<dyn EpisodeRenames>) -> Self {
        self.chapters = SyncChapters::new(self.db.clone(), self.chapter_locks.clone(), chapter_renames);
        self.episodes = SyncEpisodes::new(self.db.clone(), self.episode_locks.clone(), episode_renames);
        self
    }
}

/// The source chapter a pass item stands for.
pub(crate) fn to_source_chapter(item: &SourceItem) -> SChapter {
    SChapter {
        url: item.url.clone(),
        name: item.name.clone(),
        date_upload: item.date_upload_ms,
        chapter_number: item.number as f32,
        scanlator: item.scanlator.clone(),
    }
}

/// The source episode a pass item stands for. Fields the pass does not carry keep the source defaults.
pub(crate) fn to_source_episode(item: &SourceItem) -> SEpisode {
    SEpisode {
        url: item.url.clone(),
        name: item.name.clone(),
        episode_number: item.number as f32,
        date_upload: item.date_upload_ms,
        scanlator: item.scanlator.clone(),
        ..SEpisode::default()
    }
}

/// The pass item of a stored chapter (the rows the domain returns as new).
pub(crate) fn chapter_item(c: &Chapter) -> SourceItem {
    SourceItem {
        url: c.url.clone(),
        name: c.name.clone(),
        number: c.chapter_number,
        date_upload_ms: c.date_upload,
        source_order: c.source_order,
        scanlator: c.scanlator.clone(),
    }
}

/// The pass item of a stored episode.
pub(crate) fn episode_item(e: &Episode) -> SourceItem {
    SourceItem {
        url: e.url.clone(),
        name: e.name.clone(),
        number: e.episode_number,
        date_upload_ms: e.date_upload,
        source_order: e.source_order,
        scanlator: e.scanlator.clone(),
    }
}

fn window_pair(window: Window) -> (i64, i64) {
    (window.lower_ms, window.upper_ms)
}

fn chapter_error(error: ChapterSyncError) -> UpdateError {
    match error {
        ChapterSyncError::NoChapters(_) => UpdateError::NoChapters,
        ChapterSyncError::Db(e) => UpdateError::from(e),
    }
}

fn episode_error(error: EpisodeSyncError) -> UpdateError {
    match error {
        EpisodeSyncError::NoEpisodes(_) => UpdateError::NoEpisodes,
        EpisodeSyncError::Db(e) => UpdateError::from(e),
    }
}

/// Reads the manga row off the async worker: the repository call blocks.
async fn load_manga(db: Arc<Db>, id: i64) -> Result<Manga, UpdateError> {
    tokio::task::spawn_blocking(move || MangaRepo::new(db).by_id(id))
        .await
        .map_err(|e| UpdateError::Other(e.to_string()))?
        .map_err(|e| UpdateError::Other(e.to_string()))
}

async fn load_anime(db: Arc<Db>, id: i64) -> Result<kuro_data::anime::Anime, UpdateError> {
    tokio::task::spawn_blocking(move || AnimeRepo::new(db).by_id(id))
        .await
        .map_err(|e| UpdateError::Other(e.to_string()))?
        .map_err(|e| UpdateError::Other(e.to_string()))
}

impl ItemSync for DbItemSync {
    async fn sync(
        &self,
        entry: &LibraryEntry,
        fetched: &Fetched,
        window: Window,
        now_ms: i64,
    ) -> Result<Vec<SourceItem>, UpdateError> {
        // The pass does not say whether the refresh was manual; an automatic pass is the default (see DECISIONS).
        let is_local = entry.source == LOCAL_SOURCE_ID;
        let prepare = |_: &mut SChapter| {};
        match entry.kind {
            Kind::Manga => {
                let manga = load_manga(self.db.clone(), entry.id).await?;
                let raw: Vec<SChapter> = fetched.items.iter().map(to_source_chapter).collect();
                let opts = ChapterSyncOptions {
                    now_ms,
                    manual: false,
                    window: window_pair(window),
                    mark_duplicate_as_read: self.mark_duplicate_read,
                    is_local,
                };
                let new = self.chapters.run(raw, &manga, opts, &prepare).await.map_err(chapter_error)?;
                Ok(new.iter().map(chapter_item).collect())
            }
            Kind::Anime => {
                let anime = load_anime(self.db.clone(), entry.id).await?;
                let raw: Vec<SEpisode> = fetched.items.iter().map(to_source_episode).collect();
                let opts = EpisodeSyncOptions {
                    now_ms,
                    manual: false,
                    window: window_pair(window),
                    mark_duplicate_as_seen: self.mark_duplicate_seen,
                    is_local,
                };
                let new = self.episodes.run(raw, &anime, opts).await.map_err(episode_error)?;
                Ok(new.iter().map(episode_item).collect())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_domain::error::NoEpisodes;
    use kuro_domain::error::NoChapters;

    fn item(url: &str, number: f64) -> SourceItem {
        SourceItem {
            url: url.into(),
            name: format!("Chapter {number}"),
            number,
            date_upload_ms: 42,
            source_order: 3,
            scanlator: Some("group".into()),
        }
    }

    #[test]
    fn a_pass_item_becomes_a_source_chapter_with_its_fields() {
        let chapter = to_source_chapter(&item("/c/1", 1.5));
        assert_eq!(chapter.url, "/c/1");
        assert_eq!(chapter.name, "Chapter 1.5");
        assert_eq!(chapter.chapter_number, 1.5);
        assert_eq!(chapter.date_upload, 42);
        assert_eq!(chapter.scanlator.as_deref(), Some("group"));
    }

    #[test]
    fn a_pass_item_becomes_a_source_episode_and_keeps_source_defaults_for_the_rest() {
        let episode = to_source_episode(&item("/e/2", -1.0));
        assert_eq!(episode.url, "/e/2");
        assert_eq!(episode.episode_number, -1.0, "an unrecognised number stays negative");
        assert_eq!(episode.date_upload, 42);
        assert!(!episode.fillermark);
        assert_eq!(episode.summary, None);
        assert_eq!(episode.preview_url, None);
        assert!(episode.memo.is_empty());
    }

    #[test]
    fn stored_rows_map_back_to_pass_items() {
        let chapter = Chapter {
            url: "/c/9".into(),
            name: "Nine".into(),
            chapter_number: 9.0,
            date_upload: 7,
            source_order: 4,
            scanlator: None,
            ..Chapter::create()
        };
        let back = chapter_item(&chapter);
        assert_eq!(back, SourceItem { url: "/c/9".into(), name: "Nine".into(), number: 9.0, date_upload_ms: 7, source_order: 4, scanlator: None });
    }

    #[test]
    fn the_domain_errors_map_to_the_pass_errors() {
        assert_eq!(chapter_error(ChapterSyncError::NoChapters(NoChapters)), UpdateError::NoChapters);
        assert_eq!(episode_error(EpisodeSyncError::NoEpisodes(NoEpisodes)), UpdateError::NoEpisodes);
    }

    fn test_db(dir: &std::path::Path) -> Arc<Db> {
        let path = kuro_db::schema_unified::db_path(dir);
        let writer = kuro_db::schema_unified::open_file(&path).expect("open manga db");
        Arc::new(Db::new(&path, writer).expect("manga db handle"))
    }

    fn sync_for(manga: Arc<Db>) -> DbItemSync {
        DbItemSync::new(manga, Arc::new(MangaSyncLocks::default()), Arc::new(MangaSyncLocks::default()), false, false)
    }

    fn entry(id: i64) -> LibraryEntry {
        crate::library_update::fake::entry(id, 7, "Berserk")
    }

    fn window() -> Window {
        Window { lower_ms: 0, upper_ms: 0 }
    }

    fn fetched(items: Vec<SourceItem>) -> Fetched {
        Fetched { details: None, items }
    }

    #[tokio::test]
    async fn the_second_sync_of_the_same_list_finds_nothing_new() {
        let dir = tempfile::tempdir().expect("temp dir");
        let manga = test_db(dir.path());
        let id = MangaRepo::new(manga.clone())
            .insert(&Manga { favorite: true, source: 7, url: "/m".into(), title: "Berserk".into(), ..Manga::create() })
            .expect("insert manga");
        let sync = sync_for(manga);
        let list = fetched(vec![item("/c/1", 1.0), item("/c/2", 2.0)]);
        let first = sync.sync(&entry(id), &list, window(), 1_000).await.expect("first sync");
        assert_eq!(first.len(), 2, "both chapters are new on the first sync");
        let second = sync.sync(&entry(id), &list, window(), 2_000).await.expect("second sync");
        assert!(second.is_empty(), "nothing is new the second time");
    }

    #[tokio::test]
    #[ignore = "slow: run on demand"]
    async fn perf_02_a_500_item_sync_is_fast() {
        // The plan's bound is 100 ms in a release build; a debug build is held to a loose bound.
        let dir = tempfile::tempdir().expect("temp dir");
        let manga = test_db(dir.path());
        let id = MangaRepo::new(manga.clone())
            .insert(&Manga { favorite: true, source: 7, url: "/m".into(), title: "Berserk".into(), ..Manga::create() })
            .expect("insert manga");
        let sync = sync_for(manga);
        let items: Vec<SourceItem> = (0..500).map(|n| SourceItem { source_order: n, ..item(&format!("/c/{n}"), f64::from(n as i32)) }).collect();
        let list = fetched(items);
        let limit = if cfg!(debug_assertions) { std::time::Duration::from_secs(5) } else { std::time::Duration::from_millis(100) };
        let started = std::time::Instant::now();
        let first = sync.sync(&entry(id), &list, window(), 1_000).await.expect("500 new chapters");
        assert_eq!(first.len(), 500);
        let again = sync.sync(&entry(id), &list, window(), 2_000).await.expect("unchanged list");
        assert!(again.is_empty());
        assert!(started.elapsed() < limit, "two 500-item syncs took {:?}", started.elapsed());
    }

    #[derive(Default)]
    struct RecordingRenames(std::sync::Mutex<Vec<(String, String)>>);

    impl ChapterRenames for RecordingRenames {
        fn rename(&self, _manga: &Manga, stored: &Chapter, prepared: &Chapter) {
            self.0.lock().unwrap().push((stored.name.clone(), prepared.name.clone()));
        }
    }

    #[tokio::test]
    async fn a_pass_sync_hands_a_renamed_chapter_to_the_rename_port_it_was_given() {
        // T-26a-086: the pass takes the real port in place of `NoRenames`.
        let dir = tempfile::tempdir().expect("temp dir");
        let manga = test_db(dir.path());
        let id = MangaRepo::new(manga.clone())
            .insert(&Manga { favorite: true, source: 7, url: "/m".into(), title: "Berserk".into(), ..Manga::create() })
            .expect("insert manga");
        let renames = Arc::new(RecordingRenames::default());
        let sync = sync_for(manga).with_renames(renames.clone(), Arc::new(NoEpisodeRenames));
        sync.sync(&entry(id), &fetched(vec![item("/c/1", 1.0)]), window(), 1_000).await.expect("first sync");
        assert!(renames.0.lock().unwrap().is_empty(), "a new chapter has nothing to rename");
        let renamed = SourceItem { name: "Chapter 1 v2".into(), ..item("/c/1", 1.0) };
        sync.sync(&entry(id), &fetched(vec![renamed]), window(), 2_000).await.expect("second sync");
        assert_eq!(renames.0.lock().unwrap().as_slice(), &[("Chapter 1".to_owned(), "Chapter 1 v2".to_owned())]);
    }

    #[tokio::test]
    async fn an_empty_list_from_a_remote_source_is_no_chapters_and_writes_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let manga = test_db(dir.path());
        let id = MangaRepo::new(manga.clone())
            .insert(&Manga { favorite: true, source: 7, url: "/m".into(), title: "Berserk".into(), ..Manga::create() })
            .expect("insert manga");
        let sync = sync_for(manga);
        let result = sync.sync(&entry(id), &fetched(Vec::new()), window(), 1_000).await;
        assert_eq!(result, Err(UpdateError::NoChapters));
    }
}
