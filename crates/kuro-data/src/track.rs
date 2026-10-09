//! Track bindings of manga and anime (`MangaTrackRepositoryImpl.kt`, `AnimeTrackRepositoryImpl.kt`, `manga_sync.sq`,
//! `anime_sync.sq`): one row per (entry, tracker) pair. Writes are upserts that keep the row id (Q-TRK-2).
//! The domain interactors (log-and-default policy) live in `kuro-domain::track`.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::statements_anime::{self as sa, AnimeTrack as AnimeTrackRow};
use kuro_db::statements_manga::{self as sm, TrackRow};
use kuro_db::subscribe::Subscription;
use rusqlite::{Connection, named_params};
use serde::{Deserialize, Serialize};

use crate::{DataResult, ENTRY_DEBOUNCE, none_if_not_found};

/// Tables a manga track listing reads.
const MANGA_SYNC: &[&str] = &["manga_sync"];

/// Tables an anime track listing reads.
const ANIME_SYNC: &[&str] = &["anime_sync"];

/// One tracker binding of a manga (`MangaTrack.kt`). `tracker_id` is the `sync_id` column. `id` is `-1` until a write
/// assigns the row id and `-1` is never persisted (decision for T-10a-027: the plan asks for `Option<i64>`, but this
/// type is read by `kuro-track`, `kuro-backup` and the app, so the sentinel stays and `kuro-track` keeps its own
/// `Option` model for remote results).
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

impl MangaTrack {
    /// An unsaved binding to `tracker_id` with the `MangaTrackImpl` defaults (`MangaTrack.create`). `id <= 0` means
    /// unsaved, so the struct carries `-1` until a write assigns the row id.
    pub fn new(tracker_id: i64) -> Self {
        Self {
            id: -1,
            manga_id: 0,
            tracker_id,
            remote_id: 0,
            library_id: None,
            title: String::new(),
            last_chapter_read: 0.0,
            total_chapters: 0,
            status: 0,
            score: 0.0,
            remote_url: String::new(),
            start_date: 0,
            finish_date: 0,
            private: false,
        }
    }

    /// Copies exactly the six personal fields of `other`: progress, score, status, start and finish dates, and
    /// `private` (`MangaTrack.copyPersonalFrom`). Identity fields stay.
    pub fn copy_personal_from(&mut self, other: &MangaTrack) {
        self.copy_personal_keeping_private(other);
        self.private = other.private;
    }

    /// [`Self::copy_personal_from`] without `private` (`copyRemotePrivate = false`).
    pub fn copy_personal_keeping_private(&mut self, other: &MangaTrack) {
        self.last_chapter_read = other.last_chapter_read;
        self.score = other.score;
        self.status = other.status;
        self.start_date = other.start_date;
        self.finish_date = other.finish_date;
    }
}

impl AnimeTrack {
    /// An unsaved binding to `tracker_id` with the `AnimeTrackImpl` defaults. `id <= 0` means unsaved.
    pub fn new(tracker_id: i64) -> Self {
        Self {
            id: -1,
            anime_id: 0,
            tracker_id,
            remote_id: 0,
            library_id: None,
            title: String::new(),
            last_episode_seen: 0.0,
            total_episodes: 0,
            status: 0,
            score: 0.0,
            remote_url: String::new(),
            start_date: 0,
            finish_date: 0,
            private: false,
        }
    }

    /// Copies exactly the six personal fields of `other`: progress, score, status, watch dates and `private`
    /// (`AnimeTrack.copyPersonalFrom`). Identity fields stay.
    pub fn copy_personal_from(&mut self, other: &AnimeTrack) {
        self.copy_personal_keeping_private(other);
        self.private = other.private;
    }

    /// [`Self::copy_personal_from`] without `private` (`copyRemotePrivate = false`).
    pub fn copy_personal_keeping_private(&mut self, other: &AnimeTrack) {
        self.last_episode_seen = other.last_episode_seen;
        self.score = other.score;
        self.status = other.status;
        self.start_date = other.start_date;
        self.finish_date = other.finish_date;
    }
}

/// Column values of a coalesced manga track update (`update_coalesced`). `None` keeps the stored value, so a patch
/// with every field `None` changes nothing. The 13 data columns plus the row id make the 14 parameters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MangaTrackPatch {
    pub id: i64,
    pub manga_id: Option<i64>,
    pub tracker_id: Option<i64>,
    pub remote_id: Option<i64>,
    pub library_id: Option<i64>,
    pub title: Option<String>,
    pub last_chapter_read: Option<f64>,
    pub total_chapters: Option<i64>,
    pub status: Option<i64>,
    pub score: Option<f64>,
    pub remote_url: Option<String>,
    pub start_date: Option<i64>,
    pub finish_date: Option<i64>,
    pub private: Option<bool>,
}

/// Column values of a coalesced anime track update (`update_coalesced`). Same contract as [`MangaTrackPatch`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimeTrackPatch {
    pub id: i64,
    pub anime_id: Option<i64>,
    pub tracker_id: Option<i64>,
    pub remote_id: Option<i64>,
    pub library_id: Option<i64>,
    pub title: Option<String>,
    pub last_episode_seen: Option<f64>,
    pub total_episodes: Option<i64>,
    pub status: Option<i64>,
    pub score: Option<f64>,
    pub remote_url: Option<String>,
    pub start_date: Option<i64>,
    pub finish_date: Option<i64>,
    pub private: Option<bool>,
}

const MANGA_UPDATE_COALESCED: &str = "UPDATE manga_sync SET \
    manga_id = coalesce(:mangaId, manga_id), sync_id = coalesce(:syncId, sync_id), \
    remote_id = coalesce(:remoteId, remote_id), library_id = coalesce(:libraryId, library_id), \
    title = coalesce(:title, title), last_chapter_read = coalesce(:lastChapterRead, last_chapter_read), \
    total_chapters = coalesce(:totalChapters, total_chapters), status = coalesce(:status, status), \
    score = coalesce(:score, score), remote_url = coalesce(:remoteUrl, remote_url), \
    start_date = coalesce(:startDate, start_date), finish_date = coalesce(:finishDate, finish_date), \
    private = coalesce(:private, private) WHERE _id = :id";

const ANIME_UPDATE_COALESCED: &str = "UPDATE anime_sync SET \
    anime_id = coalesce(:animeId, anime_id), sync_id = coalesce(:syncId, sync_id), \
    remote_id = coalesce(:remoteId, remote_id), library_id = coalesce(:libraryId, library_id), \
    title = coalesce(:title, title), last_episode_seen = coalesce(:lastEpisodeSeen, last_episode_seen), \
    total_episodes = coalesce(:totalEpisodes, total_episodes), status = coalesce(:status, status), \
    score = coalesce(:score, score), remote_url = coalesce(:remoteUrl, remote_url), \
    start_date = coalesce(:startDate, start_date), finish_date = coalesce(:finishDate, finish_date), \
    private = coalesce(:private, private) WHERE _id = :id";

/// Updates the non-`None` columns of one `manga_sync` row inside the caller's connection or transaction
/// (`MangaRestorer` coalesced update). Zero rows matched is not an error.
pub fn update_manga_coalesced(c: &Connection, p: &MangaTrackPatch) -> DataResult<()> {
    c.execute(
        MANGA_UPDATE_COALESCED,
        named_params! {
            ":mangaId": p.manga_id, ":syncId": p.tracker_id, ":remoteId": p.remote_id, ":libraryId": p.library_id,
            ":title": p.title, ":lastChapterRead": p.last_chapter_read, ":totalChapters": p.total_chapters,
            ":status": p.status, ":score": p.score, ":remoteUrl": p.remote_url, ":startDate": p.start_date,
            ":finishDate": p.finish_date, ":private": p.private, ":id": p.id,
        },
    )?;
    Ok(())
}

/// Updates the non-`None` columns of one `anime_sync` row. Same contract as [`update_manga_coalesced`].
pub fn update_anime_coalesced(c: &Connection, p: &AnimeTrackPatch) -> DataResult<()> {
    c.execute(
        ANIME_UPDATE_COALESCED,
        named_params! {
            ":animeId": p.anime_id, ":syncId": p.tracker_id, ":remoteId": p.remote_id, ":libraryId": p.library_id,
            ":title": p.title, ":lastEpisodeSeen": p.last_episode_seen, ":totalEpisodes": p.total_episodes,
            ":status": p.status, ":score": p.score, ":remoteUrl": p.remote_url, ":startDate": p.start_date,
            ":finishDate": p.finish_date, ":private": p.private, ":id": p.id,
        },
    )?;
    Ok(())
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

    /// The binding with row id `id` (`_id`, Q-TRK-1). It is never an entry id.
    pub fn by_id(&self, id: i64) -> DataResult<Option<MangaTrack>> {
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

    /// The binding with row id `id` (`_id`, Q-TRK-1). It is never an entry id.
    pub fn by_id(&self, id: i64) -> DataResult<Option<AnimeTrack>> {
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
        assert_eq!(repo.by_id(id_inserted).expect("by row"), Some(got[0].clone()));
    }

    #[test]
    // Decision (T-06a-030 T1 vs area 09 Q-TRK-2): the row id is kept. Area 06 T1 asks for the Kotlin REPLACE (new `_id`,
    // `by_id(old)` None); area 09 T-09a-012 and `kuro-track` rely on a stable id, so the upsert stands and T1 is not met.
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

    #[tokio::test]
    async fn deleting_a_manga_cascades_its_tracks_and_notifies_watch_all() {
        let (_d, db) = manga_db();
        let id = new_manga_id(&db);
        let repo = MangaTrackRepo::new(db.clone());
        repo.insert(&manga_track(id, 5, "t", 0.0)).expect("insert");
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
        wait(1).await;
        db.write(|tx| Ok(tx.execute("DELETE FROM mangas WHERE _id = ?1", [id])?)).expect("delete manga");
        wait(0).await;
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

#[cfg(test)]
mod patch_and_model_tests {
    use super::*;
    use crate::anime::{Anime, AnimeRepo};
    use crate::manga::{Manga, MangaRepo};
    use crate::tests::{anime_db, manga_db};

    fn new_manga(db: &Arc<Db>) -> i64 {
        MangaRepo::new(db.clone())
            .insert(&Manga { url: "/m".into(), title: "M".into(), favorite: true, source: 1, ..Manga::create() })
            .expect("insert manga")
    }

    fn new_anime(db: &Arc<Db>) -> i64 {
        AnimeRepo::new(db.clone())
            .insert(&Anime { url: "/a".into(), title: "A".into(), source: 1, ..Anime::create() })
            .expect("insert anime")
    }

    fn full_manga(manga_id: i64) -> MangaTrack {
        MangaTrack {
            id: -1,
            manga_id,
            tracker_id: 2,
            remote_id: 42,
            library_id: Some(7),
            title: "Stored".into(),
            last_chapter_read: 4.0,
            total_chapters: 120,
            status: 1,
            score: 8.0,
            remote_url: "https://t/42".into(),
            start_date: 10,
            finish_date: 20,
            private: false,
        }
    }

    #[test]
    fn a_patch_with_every_field_none_leaves_the_manga_row_unchanged() {
        let (_d, db) = manga_db();
        let m = new_manga(&db);
        let repo = MangaTrackRepo::new(db.clone());
        let id = repo.insert(&full_manga(m)).unwrap();
        let before = repo.by_id(id).unwrap();
        let patch = MangaTrackPatch { id, ..Default::default() };
        db.write(|tx| update_manga_coalesced(tx, &patch)).unwrap();
        assert_eq!(repo.by_id(id).unwrap(), before, "T5: all-None patch is a no-op");
    }

    #[test]
    fn a_manga_patch_with_only_last_chapter_read_changes_only_that_column() {
        let (_d, db) = manga_db();
        let m = new_manga(&db);
        let repo = MangaTrackRepo::new(db.clone());
        let id = repo.insert(&full_manga(m)).unwrap();
        let before = repo.by_id(id).unwrap().unwrap();
        let patch = MangaTrackPatch { id, last_chapter_read: Some(9.5), ..Default::default() };
        db.write(|tx| update_manga_coalesced(tx, &patch)).unwrap();
        let after = repo.by_id(id).unwrap().unwrap();
        assert_eq!(after, MangaTrack { last_chapter_read: 9.5, ..before });
    }

    #[test]
    fn an_anime_patch_sets_every_column_and_all_none_changes_nothing() {
        let (_d, db) = anime_db();
        let a = new_anime(&db);
        let repo = AnimeTrackRepo::new(db.clone());
        let base = AnimeTrack {
            id: -1,
            anime_id: a,
            tracker_id: 3,
            remote_id: 5,
            library_id: None,
            title: "t".into(),
            last_episode_seen: 1.0,
            total_episodes: 12,
            status: 1,
            score: 2.0,
            remote_url: "u".into(),
            start_date: 3,
            finish_date: 4,
            private: false,
        };
        let id = repo.insert(&base).unwrap();
        db.write(|tx| update_anime_coalesced(tx, &AnimeTrackPatch { id, ..Default::default() })).unwrap();
        assert_eq!(repo.by_id(id).unwrap().unwrap(), AnimeTrack { id, ..base.clone() });
        let all = AnimeTrackPatch {
            id,
            anime_id: Some(a),
            tracker_id: Some(3),
            remote_id: Some(6),
            library_id: Some(9),
            title: Some("new".into()),
            last_episode_seen: Some(2.5),
            total_episodes: Some(24),
            status: Some(2),
            score: Some(7.5),
            remote_url: Some("v".into()),
            start_date: Some(30),
            finish_date: Some(40),
            private: Some(true),
        };
        db.write(|tx| update_anime_coalesced(tx, &all)).unwrap();
        let got = repo.by_id(id).unwrap().unwrap();
        assert_eq!(
            got,
            AnimeTrack {
                id,
                anime_id: a,
                tracker_id: 3,
                remote_id: 6,
                library_id: Some(9),
                title: "new".into(),
                last_episode_seen: 2.5,
                total_episodes: 24,
                status: 2,
                score: 7.5,
                remote_url: "v".into(),
                start_date: 30,
                finish_date: 40,
                private: true,
            }
        );
    }

    #[test]
    fn insert_all_is_one_transaction_and_a_failing_row_rolls_back_the_rest() {
        let (_d, db) = manga_db();
        let m = new_manga(&db);
        let repo = MangaTrackRepo::new(db.clone());
        let mut missing = full_manga(999_999);
        missing.tracker_id = 9;
        assert!(repo.insert_all(&[full_manga(m), missing]).is_err(), "foreign key to a missing manga");
        assert!(repo.by_entry(m).unwrap().is_empty(), "the first row is rolled back with the failing one");
    }

    #[test]
    fn fractional_progress_round_trips_exactly_for_both_kinds() {
        let (_d, db) = manga_db();
        let m = new_manga(&db);
        let repo = MangaTrackRepo::new(db.clone());
        let mut t = full_manga(m);
        t.last_chapter_read = 1.0 / 3.0;
        let id = repo.insert(&t).unwrap();
        assert_eq!(repo.by_id(id).unwrap().unwrap().last_chapter_read, 1.0 / 3.0);

        let (_d2, adb) = anime_db();
        let a = new_anime(&adb);
        let arepo = AnimeTrackRepo::new(adb);
        let at = AnimeTrack {
            id: -1,
            anime_id: a,
            tracker_id: 1,
            remote_id: 1,
            library_id: None,
            title: String::new(),
            last_episode_seen: 2.0 / 3.0,
            total_episodes: 0,
            status: 0,
            score: 0.0,
            remote_url: String::new(),
            start_date: 0,
            finish_date: 0,
            private: false,
        };
        let aid = arepo.insert(&at).unwrap();
        assert_eq!(arepo.by_id(aid).unwrap().unwrap().last_episode_seen, 2.0 / 3.0);
    }

    #[test]
    fn deleting_an_anime_cascades_its_tracks() {
        let (_d, db) = anime_db();
        let a = new_anime(&db);
        let repo = AnimeTrackRepo::new(db.clone());
        let mut t = AnimeTrack::new(4);
        t.anime_id = a;
        repo.insert(&t).unwrap();
        db.write(|tx| Ok(tx.execute("DELETE FROM animes WHERE _id = ?1", [a])?)).unwrap();
        assert!(repo.by_entry(a).unwrap().is_empty());
    }

    #[test]
    fn new_gives_the_documented_defaults_and_an_unsaved_id() {
        let m = MangaTrack::new(5);
        assert_eq!((m.id, m.tracker_id, m.manga_id, m.last_chapter_read, m.title.as_str(), m.private), (-1, 5, 0, 0.0, "", false));
        assert_eq!((m.library_id, m.remote_id, m.total_chapters, m.start_date, m.finish_date), (None, 0, 0, 0, 0));
        let a = AnimeTrack::new(6);
        assert_eq!((a.id, a.tracker_id, a.last_episode_seen, a.remote_url.as_str()), (-1, 6, 0.0, ""));
    }

    #[test]
    fn copy_personal_from_copies_private_and_the_keeping_form_does_not() {
        let src = MangaTrack { private: true, last_chapter_read: 7.0, score: 9.0, status: 3, start_date: 1, finish_date: 2, ..MangaTrack::new(1) };
        let mut kept = MangaTrack { id: 77, manga_id: 5, title: "keep".into(), ..MangaTrack::new(1) };
        kept.copy_personal_keeping_private(&src);
        assert!(!kept.private, "T6: private unchanged with false");
        assert_eq!((kept.last_chapter_read, kept.score, kept.status, kept.start_date, kept.finish_date), (7.0, 9.0, 3, 1, 2));
        assert_eq!((kept.id, kept.manga_id, kept.title.as_str()), (77, 5, "keep"), "identity fields are not copied");
        kept.copy_personal_from(&src);
        assert!(kept.private, "T6: private copied with true");

        let asrc = AnimeTrack { private: true, last_episode_seen: 4.0, ..AnimeTrack::new(2) };
        let mut adst = AnimeTrack::new(2);
        adst.copy_personal_keeping_private(&asrc);
        assert_eq!((adst.last_episode_seen, adst.private), (4.0, false));
        adst.copy_personal_from(&asrc);
        assert!(adst.private);
    }
}
