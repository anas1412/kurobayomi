//! Read and seen status of a list (`SetReadStatus.kt`, `SetSeenStatus.kt`; T-10a-013, T-10a-014). One coalesce update
//! of the rows in one transaction, then the download deletion for marked-read rows when the preference is on. The whole
//! operation runs under [`crate::app::rx::non_cancellable`], so a closed screen does not stop it (EC-29).
//!
//! The download deletion is a port ([`ChapterDownloads`], [`EpisodeDownloads`]): `DeleteChapterDownload` and
//! `DeleteEpisodeDownload` (`app::delete_download`, T-10a-015) implement it.

use kuro_data::anime::{Anime, AnimeRepo};
use kuro_data::chapter::{Chapter, ChapterRepo, ChapterUpdate};
use kuro_data::episode::{Episode, EpisodeRepo, EpisodeUpdate};
use kuro_data::manga::{Manga, MangaRepo};
use kuro_prefs::pref::Pref;

use crate::app::rx::non_cancellable;

/// Outcome of a status write (`Result` upstream). `NoItems` is the "nothing to change" result; the episode screens show it
/// as `NoEpisodes`. `InternalError` carries the failure text; nothing was written (EC-28).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetStatusResult {
    Success,
    NoItems,
    InternalError(String),
}

/// Outcome of [`SetSeenStatus`] for the episode screens: the empty change of [`SetStatusResult::NoItems`] is `NoEpisodes`
/// (T-10a-014).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetSeenResult {
    Success,
    NoEpisodes,
    InternalError(String),
}

impl From<SetStatusResult> for SetSeenResult {
    fn from(result: SetStatusResult) -> Self {
        match result {
            SetStatusResult::Success => Self::Success,
            SetStatusResult::NoItems => Self::NoEpisodes,
            SetStatusResult::InternalError(e) => Self::InternalError(e),
        }
    }
}

/// Deletes the downloads of chapters (`DeleteChapterDownload.awaitAll`). A missing source deletes nothing (EC-27).
pub trait ChapterDownloads: Send + Sync {
    fn delete_chapters(&self, manga: &Manga, chapters: &[Chapter]);
}

/// Deletes the downloads of episodes (`DeleteEpisodeDownload.awaitAll`).
pub trait EpisodeDownloads: Send + Sync {
    fn delete_episodes(&self, anime: &Anime, episodes: &[Episode]);
}

/// The chapters a mark-read or mark-unread touches (`SetReadStatus.await`, step 1). Marking read takes unread rows;
/// marking unread takes rows that are read or have a page position to reset.
pub fn chapters_to_change(read: bool, chapters: &[Chapter]) -> Vec<Chapter> {
    chapters
        .iter()
        .filter(|c| if read { !c.read } else { c.read || c.last_page_read > 0 })
        .cloned()
        .collect()
}

/// The update of one chapter (step 2). Marking read keeps `last_page_read` (None); marking unread resets it to 0 (EC-26).
pub fn chapter_status_update(read: bool, chapter: &Chapter) -> ChapterUpdate {
    ChapterUpdate {
        read: Some(read),
        last_page_read: if read { None } else { Some(0) },
        ..ChapterUpdate::new(chapter.id)
    }
}

/// The episodes a mark-seen or mark-unseen touches. Same rule as chapters, on `seen` and `last_second_seen`.
pub fn episodes_to_change(seen: bool, episodes: &[Episode]) -> Vec<Episode> {
    episodes
        .iter()
        .filter(|e| if seen { !e.seen } else { e.seen || e.last_second_seen > 0 })
        .cloned()
        .collect()
}

/// The update of one episode. Marking unseen resets `last_second_seen` to 0; `total_seconds` is never touched.
pub fn episode_status_update(seen: bool, episode: &Episode) -> EpisodeUpdate {
    EpisodeUpdate {
        seen: Some(seen),
        last_second_seen: if seen { None } else { Some(0) },
        ..EpisodeUpdate::new(episode.id)
    }
}

/// `SetReadStatus`: marks chapters read or unread.
#[derive(Clone)]
pub struct SetReadStatus {
    chapters: ChapterRepo,
    mangas: MangaRepo,
    remove_after_marked_as_read: Pref<bool>,
    downloads: std::sync::Arc<dyn ChapterDownloads>,
}

impl SetReadStatus {
    pub fn new(
        chapters: ChapterRepo,
        mangas: MangaRepo,
        remove_after_marked_as_read: Pref<bool>,
        downloads: std::sync::Arc<dyn ChapterDownloads>,
    ) -> Self {
        Self { chapters, mangas, remove_after_marked_as_read, downloads }
    }

    /// `await(read, chapters)`, non-cancellable.
    pub async fn run(&self, read: bool, chapters: Vec<Chapter>) -> SetStatusResult {
        let this = self.clone();
        non_cancellable(async move { this.apply(read, &chapters) }).await
    }

    /// `await(mangaId, read)`: every chapter of the manga.
    pub async fn run_manga(&self, manga_id: i64, read: bool) -> SetStatusResult {
        let this = self.clone();
        non_cancellable(async move {
            match this.chapters.by_manga_id(manga_id, false) {
                Ok(chapters) => this.apply(read, &chapters),
                Err(e) => SetStatusResult::InternalError(e.to_string()),
            }
        })
        .await
    }

    /// `await(manga, read)`: the overload that takes the entry itself.
    pub async fn run_for_manga(&self, manga: &Manga, read: bool) -> SetStatusResult {
        self.run_manga(manga.id, read).await
    }

    fn apply(&self, read: bool, chapters: &[Chapter]) -> SetStatusResult {
        let targets = chapters_to_change(read, chapters);
        if targets.is_empty() {
            return SetStatusResult::NoItems;
        }
        let updates: Vec<ChapterUpdate> = targets.iter().map(|c| chapter_status_update(read, c)).collect();
        if let Err(e) = self.chapters.update_all(&updates) {
            tracing::error!(error = %e, "marking chapters failed");
            return SetStatusResult::InternalError(e.to_string());
        }
        if read && self.remove_after_marked_as_read.get() {
            self.delete_downloads(&targets);
        }
        SetStatusResult::Success
    }

    /// EC-27: grouped by manga id, one deletion call per manga. A manga that cannot be read is skipped with a log line.
    fn delete_downloads(&self, targets: &[Chapter]) {
        let mut ids: Vec<i64> = targets.iter().map(|c| c.manga_id).collect();
        ids.sort_unstable();
        ids.dedup();
        for manga_id in ids {
            let group: Vec<Chapter> = targets.iter().filter(|c| c.manga_id == manga_id).cloned().collect();
            match self.mangas.by_id(manga_id) {
                Ok(manga) => self.downloads.delete_chapters(&manga, &group),
                Err(e) => tracing::warn!(error = %e, manga_id, "cannot read the manga for download deletion"),
            }
        }
    }
}

/// `SetSeenStatus`: marks episodes seen or unseen.
#[derive(Clone)]
pub struct SetSeenStatus {
    episodes: EpisodeRepo,
    animes: AnimeRepo,
    remove_after_marked_as_seen: Pref<bool>,
    downloads: std::sync::Arc<dyn EpisodeDownloads>,
}

impl SetSeenStatus {
    pub fn new(
        episodes: EpisodeRepo,
        animes: AnimeRepo,
        remove_after_marked_as_seen: Pref<bool>,
        downloads: std::sync::Arc<dyn EpisodeDownloads>,
    ) -> Self {
        Self { episodes, animes, remove_after_marked_as_seen, downloads }
    }

    /// `await(seen, episodes)`, non-cancellable. The result of an empty change is `NoEpisodes`.
    pub async fn run(&self, seen: bool, episodes: Vec<Episode>) -> SetSeenResult {
        let this = self.clone();
        non_cancellable(async move { this.apply(seen, &episodes) }).await.into()
    }

    /// `await(animeId, seen)`: every episode of the anime.
    pub async fn run_anime(&self, anime_id: i64, seen: bool) -> SetSeenResult {
        let this = self.clone();
        non_cancellable(async move {
            match this.episodes.by_anime_id(anime_id) {
                Ok(episodes) => this.apply(seen, &episodes),
                Err(e) => SetStatusResult::InternalError(e.to_string()),
            }
        })
        .await
        .into()
    }

    /// `await(anime, seen)`: the overload that takes the entry itself.
    pub async fn run_for_anime(&self, anime: &Anime, seen: bool) -> SetSeenResult {
        self.run_anime(anime.id, seen).await
    }

    fn apply(&self, seen: bool, episodes: &[Episode]) -> SetStatusResult {
        let targets = episodes_to_change(seen, episodes);
        if targets.is_empty() {
            return SetStatusResult::NoItems;
        }
        let updates: Vec<EpisodeUpdate> = targets.iter().map(|e| episode_status_update(seen, e)).collect();
        if let Err(e) = self.episodes.update_all(&updates) {
            tracing::error!(error = %e, "marking episodes failed");
            return SetStatusResult::InternalError(e.to_string());
        }
        if seen && self.remove_after_marked_as_seen.get() {
            self.delete_downloads(&targets);
        }
        SetStatusResult::Success
    }

    fn delete_downloads(&self, targets: &[Episode]) {
        let mut ids: Vec<i64> = targets.iter().map(|e| e.anime_id).collect();
        ids.sort_unstable();
        ids.dedup();
        for anime_id in ids {
            let group: Vec<Episode> = targets.iter().filter(|e| e.anime_id == anime_id).cloned().collect();
            match self.animes.by_id(anime_id) {
                Ok(anime) => self.downloads.delete_episodes(&anime, &group),
                Err(e) => tracing::warn!(error = %e, anime_id, "cannot read the anime for download deletion"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::testdb::{anime_db, manga_db};
    use kuro_db::Db;
    use kuro_prefs::store::PrefStore;
    use kuro_testkit::InMemorySettings;

    /// Inserts a chapter row through the database layer and returns its id.
    fn insert_chapter(db: &Db, c: &Chapter) -> i64 {
        let row = kuro_db::statements_manga::ChapterRow {
            id: c.id,
            manga_id: c.manga_id,
            url: c.url.clone(),
            name: c.name.clone(),
            scanlator: c.scanlator.clone(),
            read: c.read,
            bookmark: c.bookmark,
            last_page_read: c.last_page_read,
            chapter_number: c.chapter_number,
            source_order: c.source_order,
            date_fetch: c.date_fetch,
            date_upload: c.date_upload,
            last_modified_at: 0,
            version: 1,
            is_syncing: false,
        };
        db.write(|tx| Ok(kuro_db::statements_manga::chapters::insert(tx, &row)?)).expect("insert chapter")
    }

    /// Inserts an episode row through the database layer and returns its id.
    fn insert_episode(db: &Db, e: &Episode) -> i64 {
        let row = kuro_db::statements_anime::EpisodeRow {
            id: e.id,
            anime_id: e.anime_id,
            url: e.url.clone(),
            name: e.name.clone(),
            scanlator: e.scanlator.clone(),
            seen: e.seen,
            bookmark: e.bookmark,
            last_second_seen: e.last_second_seen,
            total_seconds: e.total_seconds,
            episode_number: e.episode_number,
            source_order: e.source_order,
            date_fetch: e.date_fetch,
            date_upload: e.date_upload,
            last_modified_at: 0,
            version: 1,
            is_syncing: 0,
            summary: e.summary.clone(),
            preview_url: e.preview_url.clone(),
            fillermark: e.fillermark,
            memo: kuro_db::schema_anime::Memo(e.memo.clone()),
        };
        db.write(|tx| Ok(kuro_db::statements_anime::insert_episode(tx, &row)?)).expect("insert episode")
    }

    /// Records deletion calls as (manga id, chapter numbers).
    #[derive(Default)]
    struct RecordingChapterDownloads {
        calls: Mutex<Vec<(i64, Vec<f64>)>>,
    }

    impl ChapterDownloads for RecordingChapterDownloads {
        fn delete_chapters(&self, manga: &Manga, chapters: &[Chapter]) {
            self.calls.lock().expect("lock").push((manga.id, chapters.iter().map(|c| c.chapter_number).collect()));
        }
    }

    #[derive(Default)]
    struct RecordingEpisodeDownloads {
        calls: Mutex<Vec<(i64, usize)>>,
    }

    impl EpisodeDownloads for RecordingEpisodeDownloads {
        fn delete_episodes(&self, anime: &Anime, episodes: &[Episode]) {
            self.calls.lock().expect("lock").push((anime.id, episodes.len()));
        }
    }

    fn chapter(read: bool, last_page_read: i64, number: f64) -> Chapter {
        Chapter { read, last_page_read, chapter_number: number, ..Chapter::create() }
    }

    fn episode(seen: bool, last_second_seen: i64, number: f64) -> Episode {
        Episode { seen, last_second_seen, episode_number: number, ..Episode::create() }
    }

    fn pref(on: bool) -> Pref<bool> {
        let settings = InMemorySettings::new();
        let store: PrefStore = PrefStore::clone(&settings);
        let pref = store.pref("pref_remove_after_marked_as_read_key", false);
        pref.set(on);
        pref
    }

    #[test]
    fn marking_read_takes_only_unread_and_keeps_the_page() {
        let rows = [chapter(false, 0, 1.0), chapter(true, 0, 2.0), chapter(false, 5, 3.0)];
        let targets = chapters_to_change(true, &rows);
        assert_eq!(targets.iter().map(|c| c.chapter_number).collect::<Vec<_>>(), [1.0, 3.0]);
        assert_eq!(chapter_status_update(true, &targets[1]).last_page_read, None, "last_page_read untouched");
    }

    #[test]
    fn marking_unread_takes_read_rows_and_rows_with_a_page_and_resets_the_page() {
        let rows = [chapter(false, 0, 1.0), chapter(true, 0, 2.0), chapter(false, 5, 3.0)];
        let targets = chapters_to_change(false, &rows);
        assert_eq!(targets.iter().map(|c| c.chapter_number).collect::<Vec<_>>(), [2.0, 3.0], "EC-26");
        assert_eq!(chapter_status_update(false, &targets[1]).last_page_read, Some(0));
    }

    #[test]
    fn a_no_op_list_is_no_items_and_writes_nothing() {
        let (_dir, db) = manga_db();
        let chapters = ChapterRepo::new(db.clone());
        let mangas = MangaRepo::new(db);
        let id = mangas.insert(&Manga::create()).expect("manga");
        let cid = chapters.by_manga_id(id, false).expect("none");
        assert!(cid.is_empty());
        let read_all = SetReadStatus::new(chapters, mangas, pref(false), std::sync::Arc::new(RecordingChapterDownloads::default()));
        let result = futures_block(read_all.run(true, Vec::new()));
        assert_eq!(result, SetStatusResult::NoItems);
    }

    #[test]
    fn downloads_are_deleted_only_for_read_rows_with_the_preference_grouped_by_manga() {
        let (_dir, db) = manga_db();
        let chapters = ChapterRepo::new(db.clone());
        let mangas = MangaRepo::new(db.clone());
        let a = mangas.insert(&Manga::create()).expect("manga a");
        let b = mangas.insert(&Manga::create()).expect("manga b");
        let db_chapters: Vec<Chapter> = vec![
            Chapter { manga_id: a, url: "a1".into(), chapter_number: 1.0, ..Chapter::create() },
            Chapter { manga_id: a, url: "a2".into(), chapter_number: 2.0, ..Chapter::create() },
            Chapter { manga_id: b, url: "b1".into(), chapter_number: 1.0, ..Chapter::create() },
        ];
        let ids: Vec<i64> = db_chapters.iter().map(|c| insert_chapter(&db, c)).collect();
        let stored: Vec<Chapter> = db_chapters
            .iter()
            .zip(ids)
            .map(|(c, id)| Chapter { id, ..c.clone() })
            .collect();
        let downloads = std::sync::Arc::new(RecordingChapterDownloads::default());
        let set = SetReadStatus::new(chapters.clone(), mangas.clone(), pref(true), downloads.clone());
        assert_eq!(futures_block(set.run(true, stored.clone())), SetStatusResult::Success);
        let calls = downloads.calls.lock().expect("lock").clone();
        assert_eq!(calls, vec![(a, vec![1.0, 2.0]), (b, vec![1.0])], "grouped by manga id, EC-27");

        let off = std::sync::Arc::new(RecordingChapterDownloads::default());
        let set = SetReadStatus::new(chapters, mangas, pref(false), off.clone());
        let already_read: Vec<Chapter> = stored.iter().map(|c| Chapter { read: true, ..c.clone() }).collect();
        assert_eq!(futures_block(set.run(true, already_read)), SetStatusResult::NoItems, "already read");
        assert!(off.calls.lock().expect("lock").is_empty());
    }

    #[test]
    fn marking_unseen_resets_the_last_second_and_leaves_the_total() {
        let rows = [episode(true, 0, 1.0), episode(false, 40, 2.0), episode(false, 0, 3.0)];
        let targets = episodes_to_change(false, &rows);
        assert_eq!(targets.iter().map(|e| e.episode_number).collect::<Vec<_>>(), [1.0, 2.0]);
        let update = episode_status_update(false, &targets[1]);
        assert_eq!(update.last_second_seen, Some(0));
        assert_eq!(update.total_seconds, None, "total_seconds is never touched");
        assert_eq!(episode_status_update(true, &targets[0]).last_second_seen, None);
    }

    #[test]
    fn a_seen_run_deletes_episode_downloads_per_anime_when_asked() {
        let (_dir, db) = anime_db();
        let episodes = EpisodeRepo::new(db.clone());
        let animes = AnimeRepo::new(db.clone());
        let anime = animes.insert(&Anime::create()).expect("anime");
        let row = Episode { anime_id: anime, url: "e1".into(), ..Episode::create() };
        let id = insert_episode(&db, &row);
        let stored = vec![Episode { id, ..row }];
        let downloads = std::sync::Arc::new(RecordingEpisodeDownloads::default());
        let set = SetSeenStatus::new(episodes.clone(), animes.clone(), pref(true), downloads.clone());
        assert_eq!(futures_block(set.run(true, stored.clone())), SetSeenResult::Success);
        assert_eq!(*downloads.calls.lock().expect("lock"), vec![(anime, 1)]);

        let already_seen: Vec<Episode> = stored.iter().map(|e| Episode { seen: true, ..e.clone() }).collect();
        assert_eq!(futures_block(set.run(true, already_seen)), SetSeenResult::NoEpisodes, "empty change is NoEpisodes");
        assert_eq!(futures_block(set.run(true, Vec::new())), SetSeenResult::NoEpisodes);
    }

    #[test]
    fn no_items_maps_to_no_episodes_and_the_rest_is_kept() {
        assert_eq!(SetSeenResult::from(SetStatusResult::NoItems), SetSeenResult::NoEpisodes);
        assert_eq!(SetSeenResult::from(SetStatusResult::Success), SetSeenResult::Success);
        assert_eq!(SetSeenResult::from(SetStatusResult::InternalError("x".into())), SetSeenResult::InternalError("x".into()));
    }

    /// Drives a future on a current-thread runtime, so the status tests stay synchronous to read.
    fn futures_block<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(fut)
    }
}
