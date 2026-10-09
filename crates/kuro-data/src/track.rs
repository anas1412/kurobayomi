//! Track bindings of manga and anime (`MangaTrackRepositoryImpl.kt`, `AnimeTrackRepositoryImpl.kt`, `manga_sync.sq`,
//! `anime_sync.sq`): one row per (entry, tracker) pair. Writes are upserts that keep the row id (Q-TRK-2).
//! The domain interactors (log-and-default policy) live in `kuro-domain::track`.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::statements_anime::{self as sa, AnimeTrack as AnimeTrackRow};
use kuro_db::statements_manga::{self as sm, TrackRow};
use kuro_db::subscribe::Subscription;
use serde::{Deserialize, Serialize};

use crate::{DataResult, ENTRY_DEBOUNCE, none_if_not_found};

/// Tables a manga track listing reads.
const MANGA_SYNC: &[&str] = &["manga_sync"];

/// Tables an anime track listing reads.
const ANIME_SYNC: &[&str] = &["anime_sync"];

/// One tracker binding of a manga (`MangaTrack.kt`). `tracker_id` is the `sync_id` column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MangaTrack {
    pub id: i64,
    pub manga_id: i64,
    pub tracker_id: i64,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub last_chapter_read: f64,
    pub total_chapters: i64,
    pub status: i64,
    pub score: f64,
    pub remote_url: String,
    pub start_date: i64,
    pub finish_date: i64,
    pub private: bool,
}

/// One tracker binding of an anime (`AnimeTrack.kt`). Same as [`MangaTrack`] with episode progress.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimeTrack {
    pub id: i64,
    pub anime_id: i64,
    pub tracker_id: i64,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub last_episode_seen: f64,
    pub total_episodes: i64,
    pub status: i64,
    pub score: f64,
    pub remote_url: String,
    pub start_date: i64,
    pub finish_date: i64,
    pub private: bool,
}

/// Repository over `manga_sync`.
#[derive(Clone)]
pub struct MangaTrackRepo {
    db: Arc<Db>,
}

impl MangaTrackRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The binding with row id `id`. Named by row id on purpose (Q-TRK-1): it is never an entry id.
    pub fn by_row_id(&self, id: i64) -> DataResult<Option<MangaTrack>> {
        let row = self.db.read(|c| none_if_not_found(sm::manga_sync::get_track_by_id(c, id)))?;
        Ok(row.map(map_manga))
    }

    /// Every binding, in table order.
    pub fn all(&self) -> DataResult<Vec<MangaTrack>> {
        let rows = self.db.read(|c| Ok(sm::manga_sync::get_tracks(c)?))?;
        Ok(rows.into_iter().map(map_manga).collect())
    }

    /// Every binding of manga `manga_id`.
    pub fn by_entry(&self, manga_id: i64) -> DataResult<Vec<MangaTrack>> {
        let rows = self.db.read(|c| Ok(sm::manga_sync::get_tracks_by_manga_id(c, manga_id)?))?;
        Ok(rows.into_iter().map(map_manga).collect())
    }

    /// Upserts one binding and returns its row id. A second binding of the same (manga, tracker) pair updates the
    /// row in place, so the id does not change.
    pub fn insert(&self, t: &MangaTrack) -> DataResult<i64> {
        let row = to_manga_row(t);
        self.db.write(|tx| Ok(sm::manga_sync::upsert(tx, &row)?))
    }

    /// Upserts every binding in one transaction. A repeated pair leaves one row with the last values.
    pub fn insert_all(&self, tracks: &[MangaTrack]) -> DataResult<()> {
        self.db.write(|tx| {
            for t in tracks {
                sm::manga_sync::upsert(tx, &to_manga_row(t))?;
            }
            Ok(())
        })
    }

    /// Removes the binding of manga `manga_id` to tracker `tracker_id`. Zero rows affected is not an error.
    pub fn delete(&self, manga_id: i64, tracker_id: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(sm::manga_sync::delete(tx, manga_id, tracker_id).map(drop)?))
    }

    /// Every binding, published again after each write to `manga_sync`.
    pub fn watch_all(&self) -> Subscription<Vec<MangaTrack>> {
        self.db.subscribe(MANGA_SYNC, ENTRY_DEBOUNCE, |c| {
            Ok(sm::manga_sync::get_tracks(c)?.into_iter().map(map_manga).collect::<Vec<_>>())
        })
    }

    /// The bindings of one manga, published again after each write to `manga_sync`.
    pub fn watch_by_entry(&self, manga_id: i64) -> Subscription<Vec<MangaTrack>> {
        self.db.subscribe(MANGA_SYNC, ENTRY_DEBOUNCE, move |c| {
            Ok(sm::manga_sync::get_tracks_by_manga_id(c, manga_id)?.into_iter().map(map_manga).collect::<Vec<_>>())
        })
    }
}

/// Repository over `anime_sync`.
#[derive(Clone)]
pub struct AnimeTrackRepo {
    db: Arc<Db>,
}

impl AnimeTrackRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The binding with row id `id` (Q-TRK-1).
    pub fn by_row_id(&self, id: i64) -> DataResult<Option<AnimeTrack>> {
        let row = self.db.read(|c| Ok(sa::track_by_id(c, id)?))?;
        Ok(row.map(map_anime))
    }

    /// Every binding, in table order.
    pub fn all(&self) -> DataResult<Vec<AnimeTrack>> {
        let rows = self.db.read(|c| Ok(sa::tracks(c)?))?;
        Ok(rows.into_iter().map(map_anime).collect())
    }

    /// Every binding of anime `anime_id`.
    pub fn by_entry(&self, anime_id: i64) -> DataResult<Vec<AnimeTrack>> {
        let rows = self.db.read(|c| Ok(sa::tracks_by_anime_id(c, anime_id)?))?;
        Ok(rows.into_iter().map(map_anime).collect())
    }

    /// Upserts one binding and returns its row id (Q-TRK-2).
    pub fn insert(&self, t: &AnimeTrack) -> DataResult<i64> {
        let row = to_anime_row(t);
        self.db.write(|tx| Ok(sa::upsert_track(tx, &row)?))
    }

    /// Upserts every binding in one transaction (Q-TRK-2, TC-TRK-06).
    pub fn insert_all(&self, tracks: &[AnimeTrack]) -> DataResult<()> {
        self.db.write(|tx| {
            for t in tracks {
                sa::upsert_track(tx, &to_anime_row(t))?;
            }
            Ok(())
        })
    }

    /// Removes the binding of anime `anime_id` to tracker `tracker_id`. Zero rows affected is not an error.
    pub fn delete(&self, anime_id: i64, tracker_id: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(sa::delete_track(tx, anime_id, tracker_id).map(drop)?))
    }

    /// Every binding, published again after each write to `anime_sync`.
    pub fn watch_all(&self) -> Subscription<Vec<AnimeTrack>> {
        self.db.subscribe(ANIME_SYNC, ENTRY_DEBOUNCE, |c| {
            Ok(sa::tracks(c)?.into_iter().map(map_anime).collect::<Vec<_>>())
        })
    }

    /// The bindings of one anime, published again after each write to `anime_sync`.
    pub fn watch_by_entry(&self, anime_id: i64) -> Subscription<Vec<AnimeTrack>> {
        self.db.subscribe(ANIME_SYNC, ENTRY_DEBOUNCE, move |c| {
            Ok(sa::tracks_by_anime_id(c, anime_id)?.into_iter().map(map_anime).collect::<Vec<_>>())
        })
    }
}

fn map_manga(r: TrackRow) -> MangaTrack {
    MangaTrack {
        id: r.id,
        manga_id: r.manga_id,
        tracker_id: r.sync_id,
        remote_id: r.remote_id,
        library_id: r.library_id,
        title: r.title,
        last_chapter_read: r.last_chapter_read,
        total_chapters: r.total_chapters,
        status: r.status,
        score: r.score,
        remote_url: r.remote_url,
        start_date: r.start_date,
        finish_date: r.finish_date,
        private: r.private,
    }
}

fn to_manga_row(t: &MangaTrack) -> TrackRow {
    TrackRow {
        id: t.id,
        manga_id: t.manga_id,
        sync_id: t.tracker_id,
        remote_id: t.remote_id,
        library_id: t.library_id,
        title: t.title.clone(),
        last_chapter_read: t.last_chapter_read,
        total_chapters: t.total_chapters,
        status: t.status,
        score: t.score,
        remote_url: t.remote_url.clone(),
        start_date: t.start_date,
        finish_date: t.finish_date,
        private: t.private,
    }
}

fn map_anime(r: AnimeTrackRow) -> AnimeTrack {
    AnimeTrack {
        id: r.id,
        anime_id: r.anime_id,
        tracker_id: r.sync_id,
        remote_id: r.remote_id,
        library_id: r.library_id,
        title: r.title,
        last_episode_seen: r.last_episode_seen,
        total_episodes: r.total_episodes,
        status: r.status,
        score: r.score,
        remote_url: r.remote_url,
        start_date: r.start_date,
        finish_date: r.finish_date,
        private: r.private,
    }
}

fn to_anime_row(t: &AnimeTrack) -> AnimeTrackRow {
    AnimeTrackRow {
        id: t.id,
        anime_id: t.anime_id,
        sync_id: t.tracker_id,
        remote_id: t.remote_id,
        library_id: t.library_id,
        title: t.title.clone(),
        last_episode_seen: t.last_episode_seen,
        total_episodes: t.total_episodes,
        status: t.status,
        score: t.score,
        remote_url: t.remote_url.clone(),
        start_date: t.start_date,
        finish_date: t.finish_date,
        private: t.private,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::anime::{Anime, AnimeRepo};
    use crate::manga::{Manga, MangaRepo};
    use crate::tests::{anime_db, manga_db};

    fn manga_track(manga_id: i64, tracker: i64, title: &str, progress: f64) -> MangaTrack {
        MangaTrack {
            id: -1,
            manga_id,
            tracker_id: tracker,
            remote_id: 42,
            library_id: Some(7),
            title: title.into(),
            last_chapter_read: progress,
            total_chapters: 120,
            status: 1,
            score: 8.5,
            remote_url: "https://tracker.test/42".into(),
            start_date: 1_700_000_000_000,
            finish_date: 0,
            private: true,
        }
    }

    fn anime_track(anime_id: i64, tracker: i64, title: &str, progress: f64) -> AnimeTrack {
        AnimeTrack {
            id: -1,
            anime_id,
            tracker_id: tracker,
            remote_id: 43,
            library_id: None,
            title: title.into(),
            last_episode_seen: progress,
            total_episodes: 24,
            status: 2,
            score: 6.0,
            remote_url: "https://tracker.test/43".into(),
            start_date: 0,
            finish_date: 1,
            private: false,
        }
    }

    fn new_manga_id(db: &Arc<Db>) -> i64 {
        MangaRepo::new(db.clone())
            .insert(&Manga { url: "/m".into(), title: "M".into(), favorite: true, source: 1, ..Manga::create() })
            .expect("insert manga")
    }

    #[test]
    fn insert_then_by_entry_round_trips_every_field() {
        let (_d, db) = manga_db();
        let id = new_manga_id(&db);
        let repo = MangaTrackRepo::new(db);
        let id_inserted = repo.insert(&manga_track(id, 2, "Read", 12.5)).expect("insert");
        let got = repo.by_entry(id).expect("by entry");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], MangaTrack { id: id_inserted, ..manga_track(id, 2, "Read", 12.5) }, "TC-TRK-01");
        assert_eq!(repo.by_row_id(id_inserted).expect("by row"), Some(got[0].clone()));
    }

    #[test]
    fn a_second_track_for_the_same_pair_updates_the_row_and_keeps_its_id() {
        let (_d, db) = manga_db();
        let id = new_manga_id(&db);
        let repo = MangaTrackRepo::new(db);
        let first = repo.insert(&manga_track(id, 2, "Old", 1.0)).expect("insert");
        let second = repo.insert(&manga_track(id, 2, "New", 9.0)).expect("upsert");
        assert_eq!(first, second, "TC-TRK-02: the row id is stable (Q-TRK-2)");
        let got = repo.by_entry(id).expect("by entry");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, "New");
        assert_eq!(got[0].last_chapter_read, 9.0);
    }

    #[test]
    fn insert_all_with_a_duplicate_pair_keeps_one_row_with_the_last_values() {
        let (_d, db) = manga_db();
        let id = new_manga_id(&db);
        let repo = MangaTrackRepo::new(db);
        repo.insert_all(&[manga_track(id, 3, "first", 1.0), manga_track(id, 3, "last", 2.0)]).expect("insert all");
        let got = repo.by_entry(id).expect("by entry");
        assert_eq!(got.len(), 1, "TC-TRK-06");
        assert_eq!((got[0].title.as_str(), got[0].last_chapter_read), ("last", 2.0));
    }

    #[test]
    fn delete_of_an_absent_pair_is_ok_and_delete_removes_a_present_one() {
        let (_d, db) = manga_db();
        let id = new_manga_id(&db);
        let repo = MangaTrackRepo::new(db);
        repo.delete(id, 99).expect("absent pair is not an error (TC-TRK-03)");
        repo.insert(&manga_track(id, 4, "t", 0.0)).expect("insert");
        repo.delete(id, 4).expect("delete");
        assert!(repo.by_entry(id).expect("by entry").is_empty());
    }

    #[test]
    fn deleting_a_manga_cascades_its_tracks() {
        let (_d, db) = manga_db();
        let id = new_manga_id(&db);
        let repo = MangaTrackRepo::new(db.clone());
        repo.insert(&manga_track(id, 5, "t", 0.0)).expect("insert");
        db.write(|tx| Ok(tx.execute("DELETE FROM mangas WHERE _id = ?1", [id])?)).expect("delete manga");
        assert!(repo.by_entry(id).expect("by entry").is_empty(), "E-TRK-05: foreign_keys is on");
    }

    #[test]
    fn anime_track_round_trip_and_upsert_keep_the_row_id() {
        let (_d, db) = anime_db();
        let anime_id = AnimeRepo::new(db.clone())
            .insert(&Anime { url: "/a".into(), title: "A".into(), source: 1, ..Anime::create() })
            .expect("insert anime");
        let repo = AnimeTrackRepo::new(db);
        let id = repo.insert(&anime_track(anime_id, 2, "old", 3.0)).expect("insert");
        assert_eq!(repo.insert(&anime_track(anime_id, 2, "new", 4.5)).expect("upsert"), id);
        repo.insert(&anime_track(anime_id, 6, "other", 0.0)).expect("second tracker");
        let got = repo.by_entry(anime_id).expect("by entry");
        assert_eq!(got.len(), 2);
        let updated = got.iter().find(|t| t.tracker_id == 2).expect("tracker 2");
        assert_eq!((updated.id, updated.title.as_str(), updated.last_episode_seen), (id, "new", 4.5));
        repo.delete(anime_id, 6).expect("delete");
        assert_eq!(repo.by_entry(anime_id).expect("by entry").len(), 1);
    }

    #[test]
    fn both_models_round_trip_through_serde() {
        let m = manga_track(1, 2, "m", 1.5);
        let json = serde_json::to_string(&m).expect("encode");
        assert_eq!(serde_json::from_str::<MangaTrack>(&json).expect("decode"), m);
        let a = anime_track(1, 2, "a", 2.5);
        let json = serde_json::to_string(&a).expect("encode");
        assert_eq!(serde_json::from_str::<AnimeTrack>(&json).expect("decode"), a);
    }

    #[tokio::test]
    async fn watch_all_emits_after_insert_and_delete() {
        let (_d, db) = manga_db();
        let id = new_manga_id(&db);
        let repo = MangaTrackRepo::new(db);
        let mut sub = repo.watch_all();
        let mut wait = async |want: usize| loop {
            if sub.current().and_then(Result::ok).map(|v| v.len()) == Some(want) {
                return;
            }
            tokio::time::timeout(Duration::from_secs(5), sub.changed())
                .await
                .expect("a publication")
                .expect("alive")
                .expect("query ok");
        };
        wait(0).await;
        repo.insert(&manga_track(id, 1, "t", 0.0)).expect("insert");
        wait(1).await;
        repo.delete(id, 1).expect("delete");
        wait(0).await;
    }
}
