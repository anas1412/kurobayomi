//! Repository seams for the tracking flows. The database implementations are the small adapters over the shared
//! `kuro-data` repos at the bottom of this file (one struct per repo, the seam of each kind picks the `Kind`); tests
//! use the in-memory fakes in `fakes.rs`.

use std::sync::Arc;

use async_trait::async_trait;
use kuro_data::entry_history::HistoryRepo;
use kuro_data::item::{ItemRepo, ItemUpdate};
use kuro_data::track::{Track, TrackRepo};
use kuro_db::handle::Db;
use kuro_platform::kind::Kind;
use tokio::sync::watch;

use crate::error::TrackError;
use crate::model::{AnimeTrack, MangaTrack, TrackerId};

/// Live view of the bindings of one entry. The first value is the rows at subscribe time (empty until the first
/// query finishes); a failed query is sent as `Err` and the last good value stays in the channel's history.
pub type RowsWatch<T> = watch::Receiver<Result<Vec<T>, String>>;

/// Writes and reads of anime bindings.
#[async_trait]
pub trait AnimeTrackRepo: Send + Sync {
    /// Upserts by `(anime_id, tracker_id)` and returns the row id, which does not change on update.
    async fn upsert(&self, t: &AnimeTrack) -> Result<i64, TrackError>;
    /// Upserts every binding in one transaction.
    async fn upsert_all(&self, ts: &[AnimeTrack]) -> Result<(), TrackError>;
    /// The binding with row id `id`, if any.
    async fn get_by_id(&self, id: i64) -> Result<Option<AnimeTrack>, TrackError>;
    /// Every binding, unfiltered.
    async fn list_all(&self) -> Result<Vec<AnimeTrack>, TrackError>;
    /// Every binding of the entry.
    async fn list_for_entry(&self, anime_id: i64) -> Result<Vec<AnimeTrack>, TrackError>;
    /// Removes the binding of `(anime_id, tracker_id)` locally. Other trackers stay.
    async fn delete(&self, anime_id: i64, tracker_id: TrackerId) -> Result<(), TrackError>;
    /// The bindings of the entry, published again after each write to the table.
    fn watch_for_entry(&self, anime_id: i64) -> RowsWatch<AnimeTrack>;
}

/// Writes and reads of manga bindings.
#[async_trait]
pub trait MangaTrackRepo: Send + Sync {
    /// Upserts by `(manga_id, tracker_id)` and returns the row id, which does not change on update.
    async fn upsert(&self, t: &MangaTrack) -> Result<i64, TrackError>;
    /// Upserts every binding in one transaction.
    async fn upsert_all(&self, ts: &[MangaTrack]) -> Result<(), TrackError>;
    /// The binding with row id `id`, if any.
    async fn get_by_id(&self, id: i64) -> Result<Option<MangaTrack>, TrackError>;
    /// Every binding, unfiltered.
    async fn list_all(&self) -> Result<Vec<MangaTrack>, TrackError>;
    /// Every binding of the entry.
    async fn list_for_entry(&self, manga_id: i64) -> Result<Vec<MangaTrack>, TrackError>;
    /// Removes the binding of `(manga_id, tracker_id)` locally. Other trackers stay.
    async fn delete(&self, manga_id: i64, tracker_id: TrackerId) -> Result<(), TrackError>;
    /// The bindings of the entry, published again after each write to the table.
    fn watch_for_entry(&self, manga_id: i64) -> RowsWatch<MangaTrack>;
}

/// One episode or chapter as the tracking flows read it. `number` is the unit's number, `-1.0` when it
/// was not recognised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitRef {
    pub id: i64,
    pub number: f64,
    pub seen: bool,
}

/// Episodes of an anime (`GetEpisodesByAnimeId`, `UpdateEpisode`).
#[async_trait]
pub trait AnimeEpisodeRepo: Send + Sync {
    /// Every episode of the entry, unfiltered.
    async fn list(&self, anime_id: i64) -> Result<Vec<UnitRef>, TrackError>;
    /// Marks the episodes with `ids` as seen.
    async fn mark_seen(&self, ids: &[i64]) -> Result<(), TrackError>;
}

/// Chapters of a manga (`GetChaptersByMangaId`, `UpdateChapter`).
#[async_trait]
pub trait MangaChapterRepo: Send + Sync {
    /// Every chapter of the entry, unfiltered.
    async fn list(&self, manga_id: i64) -> Result<Vec<UnitRef>, TrackError>;
    /// Marks the chapters with `ids` as read.
    async fn mark_read(&self, ids: &[i64]) -> Result<(), TrackError>;
}

/// Episode history of an anime (`GetAnimeHistory`).
#[async_trait]
pub trait AnimeHistoryRepo: Send + Sync {
    /// The `seen` time of every history row of the entry, epoch milliseconds. `None` for a reset row.
    async fn seen_times(&self, anime_id: i64) -> Result<Vec<Option<i64>>, TrackError>;
}

/// Chapter history of a manga (`GetMangaHistory`).
#[async_trait]
pub trait MangaHistoryRepo: Send + Sync {
    /// The `read` time of every history row of the entry, epoch milliseconds. `None` for a reset row.
    async fn read_times(&self, manga_id: i64) -> Result<Vec<Option<i64>>, TrackError>;
}

/// Forwards a database subscription into a [`RowsWatch`], mapping each published result. The forwarding task ends
/// when the last receiver is dropped or the database handle goes away.
fn bridge<D, T, F>(mut sub: kuro_db::subscribe::Subscription<Vec<D>>, map: F) -> RowsWatch<T>
where
    D: Send + Sync + 'static,
    T: Send + Sync + 'static,
    F: Fn(&[D]) -> Vec<T> + Send + 'static,
{
    let (tx, rx) = watch::channel(Ok(Vec::new()));
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tx.closed() => break,
                next = sub.changed() => {
                    let Some(published) = next else { break };
                    let value = match published {
                        Ok(rows) => Ok(map(&rows)),
                        Err(e) => Err(e),
                    };
                    if tx.send(value).is_err() {
                        break;
                    }
                }
            }
        }
    });
    rx
}

/// Runs a blocking `kuro-data` call on tokio's blocking pool, so SQLite never stalls an async worker.
async fn blocking<T, F>(f: F) -> Result<T, TrackError>
where
    T: Send + 'static,
    F: FnOnce() -> kuro_data::DataResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| kuro_db::DbError::Task(e.to_string()))?
        .map_err(TrackError::from)
}

/// The shared [`TrackRepo`] as both track seams. The kind of a call is the seam it comes through.
#[derive(Clone)]
pub struct SharedTracks(TrackRepo);

impl SharedTracks {
    pub fn new(db: Arc<Db>) -> Self {
        Self(TrackRepo::new(db))
    }

    async fn upsert_row(&self, row: Track) -> Result<i64, TrackError> {
        let repo = self.0.clone();
        blocking(move || repo.insert(&row)).await
    }

    async fn upsert_rows(&self, rows: Vec<Track>) -> Result<(), TrackError> {
        let repo = self.0.clone();
        blocking(move || repo.insert_all(&rows)).await
    }

    /// The binding with row id `id`; a binding of the other kind is `None`.
    async fn row(&self, kind: Kind, id: i64) -> Result<Option<Track>, TrackError> {
        let repo = self.0.clone();
        Ok(blocking(move || repo.by_id(id)).await?.filter(|t| t.kind == kind))
    }

    async fn rows(&self, kind: Kind) -> Result<Vec<Track>, TrackError> {
        let repo = self.0.clone();
        blocking(move || repo.all(kind)).await
    }

    async fn rows_for_entry(&self, entry_id: i64) -> Result<Vec<Track>, TrackError> {
        let repo = self.0.clone();
        blocking(move || repo.by_entry(entry_id)).await
    }

    async fn delete_row(&self, entry_id: i64, tracker_id: TrackerId) -> Result<(), TrackError> {
        let repo = self.0.clone();
        blocking(move || repo.delete(entry_id, tracker_id.0)).await
    }
}

#[async_trait]
impl AnimeTrackRepo for SharedTracks {
    async fn upsert(&self, t: &AnimeTrack) -> Result<i64, TrackError> {
        self.upsert_row(t.to_db(false)?).await
    }

    async fn upsert_all(&self, ts: &[AnimeTrack]) -> Result<(), TrackError> {
        self.upsert_rows(ts.iter().map(|t| t.to_db(false)).collect::<Result<_, _>>()?).await
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<AnimeTrack>, TrackError> {
        Ok(self.row(Kind::Anime, id).await?.as_ref().map(AnimeTrack::from_db))
    }

    async fn list_all(&self) -> Result<Vec<AnimeTrack>, TrackError> {
        Ok(self.rows(Kind::Anime).await?.iter().map(AnimeTrack::from_db).collect())
    }

    async fn list_for_entry(&self, anime_id: i64) -> Result<Vec<AnimeTrack>, TrackError> {
        Ok(self.rows_for_entry(anime_id).await?.iter().map(AnimeTrack::from_db).collect())
    }

    fn watch_for_entry(&self, anime_id: i64) -> RowsWatch<AnimeTrack> {
        bridge(self.0.watch_by_entry(anime_id), |rows| rows.iter().map(AnimeTrack::from_db).collect())
    }

    async fn delete(&self, anime_id: i64, tracker_id: TrackerId) -> Result<(), TrackError> {
        self.delete_row(anime_id, tracker_id).await
    }
}

#[async_trait]
impl MangaTrackRepo for SharedTracks {
    async fn upsert(&self, t: &MangaTrack) -> Result<i64, TrackError> {
        self.upsert_row(t.to_db(false)?).await
    }

    async fn upsert_all(&self, ts: &[MangaTrack]) -> Result<(), TrackError> {
        self.upsert_rows(ts.iter().map(|t| t.to_db(false)).collect::<Result<_, _>>()?).await
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<MangaTrack>, TrackError> {
        Ok(self.row(Kind::Manga, id).await?.as_ref().map(MangaTrack::from_db))
    }

    async fn list_all(&self) -> Result<Vec<MangaTrack>, TrackError> {
        Ok(self.rows(Kind::Manga).await?.iter().map(MangaTrack::from_db).collect())
    }

    async fn list_for_entry(&self, manga_id: i64) -> Result<Vec<MangaTrack>, TrackError> {
        Ok(self.rows_for_entry(manga_id).await?.iter().map(MangaTrack::from_db).collect())
    }

    fn watch_for_entry(&self, manga_id: i64) -> RowsWatch<MangaTrack> {
        bridge(self.0.watch_by_entry(manga_id), |rows| rows.iter().map(MangaTrack::from_db).collect())
    }

    async fn delete(&self, manga_id: i64, tracker_id: TrackerId) -> Result<(), TrackError> {
        self.delete_row(manga_id, tracker_id).await
    }
}

/// The shared [`ItemRepo`] as both unit seams (episodes and chapters).
#[derive(Clone)]
pub struct SharedItems(ItemRepo);

impl SharedItems {
    pub fn new(db: Arc<Db>) -> Self {
        Self(ItemRepo::new(db))
    }

    async fn units(&self, entry_id: i64) -> Result<Vec<UnitRef>, TrackError> {
        let repo = self.0.clone();
        let rows = blocking(move || repo.by_entry_id(entry_id, false)).await?;
        Ok(rows.into_iter().map(|i| UnitRef { id: i.id, number: i.number, seen: i.done }).collect())
    }

    async fn mark_done(&self, ids: &[i64]) -> Result<(), TrackError> {
        if ids.is_empty() {
            return Ok(());
        }
        let updates: Vec<_> = ids.iter().map(|&id| ItemUpdate { done: Some(true), ..ItemUpdate::new(id) }).collect();
        let repo = self.0.clone();
        blocking(move || repo.update_all(&updates)).await
    }
}

#[async_trait]
impl AnimeEpisodeRepo for SharedItems {
    async fn list(&self, anime_id: i64) -> Result<Vec<UnitRef>, TrackError> {
        self.units(anime_id).await
    }

    async fn mark_seen(&self, ids: &[i64]) -> Result<(), TrackError> {
        self.mark_done(ids).await
    }
}

#[async_trait]
impl MangaChapterRepo for SharedItems {
    async fn list(&self, manga_id: i64) -> Result<Vec<UnitRef>, TrackError> {
        self.units(manga_id).await
    }

    async fn mark_read(&self, ids: &[i64]) -> Result<(), TrackError> {
        self.mark_done(ids).await
    }
}

/// The shared [`HistoryRepo`] as both history seams.
#[derive(Clone)]
pub struct SharedHistory(HistoryRepo);

impl SharedHistory {
    pub fn new(db: Arc<Db>) -> Self {
        Self(HistoryRepo::new(db))
    }

    async fn times(&self, entry_id: i64) -> Result<Vec<Option<i64>>, TrackError> {
        let repo = self.0.clone();
        let rows = blocking(move || repo.by_entry_id(entry_id)).await?;
        Ok(rows.iter().map(|h| Some(h.last_at)).collect())
    }
}

#[async_trait]
impl AnimeHistoryRepo for SharedHistory {
    async fn seen_times(&self, anime_id: i64) -> Result<Vec<Option<i64>>, TrackError> {
        self.times(anime_id).await
    }
}

#[async_trait]
impl MangaHistoryRepo for SharedHistory {
    async fn read_times(&self, manga_id: i64) -> Result<Vec<Option<i64>>, TrackError> {
        self.times(manga_id).await
    }
}

// The impls below serve the per-kind `kuro-data` repos that `src/settings_ui/tracker_host.rs` still hands to the
// context. Phase E: switch that file to `SharedTracks`, `SharedItems` and `SharedHistory`, then delete this section.

use kuro_data::track::{LegacyTrack, LegacyTrackRepo};

use crate::model::{AnimeTrack as AnimeModel, MangaTrack as MangaModel};

/// `upsert`, `upsert_all`, `get_by_id`, `list_all`, `list_for_entry` and `delete` of a legacy track repo, in terms of
/// the shared [`Track`].
struct Legacy<'a, T: LegacyTrack>(&'a LegacyTrackRepo<T>);

impl<T: LegacyTrack> Legacy<'_, T> {
    async fn upsert(&self, row: Track) -> Result<i64, TrackError> {
        let repo = self.0.clone();
        blocking(move || repo.insert(&T::from_track(row))).await
    }

    async fn upsert_all(&self, rows: Vec<Track>) -> Result<(), TrackError> {
        let repo = self.0.clone();
        blocking(move || repo.insert_all(&rows.into_iter().map(T::from_track).collect::<Vec<_>>())).await
    }

    async fn by_id(&self, id: i64) -> Result<Option<Track>, TrackError> {
        let repo = self.0.clone();
        Ok(blocking(move || repo.by_id(id)).await?.map(|t| t.to_track()))
    }

    async fn all(&self) -> Result<Vec<Track>, TrackError> {
        let repo = self.0.clone();
        Ok(blocking(move || repo.all()).await?.iter().map(T::to_track).collect())
    }

    async fn by_entry(&self, entry_id: i64) -> Result<Vec<Track>, TrackError> {
        let repo = self.0.clone();
        Ok(blocking(move || repo.by_entry(entry_id)).await?.iter().map(T::to_track).collect())
    }

    async fn delete(&self, entry_id: i64, tracker_id: TrackerId) -> Result<(), TrackError> {
        let repo = self.0.clone();
        blocking(move || repo.delete(entry_id, tracker_id.0)).await
    }
}

#[async_trait]
impl AnimeTrackRepo for kuro_data::track::AnimeTrackRepo {
    async fn upsert(&self, t: &AnimeModel) -> Result<i64, TrackError> {
        Legacy(self).upsert(t.to_db(false)?).await
    }

    async fn upsert_all(&self, ts: &[AnimeModel]) -> Result<(), TrackError> {
        Legacy(self).upsert_all(ts.iter().map(|t| t.to_db(false)).collect::<Result<_, _>>()?).await
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<AnimeModel>, TrackError> {
        Ok(Legacy(self).by_id(id).await?.as_ref().map(AnimeModel::from_db))
    }

    async fn list_all(&self) -> Result<Vec<AnimeModel>, TrackError> {
        Ok(Legacy(self).all().await?.iter().map(AnimeModel::from_db).collect())
    }

    async fn list_for_entry(&self, anime_id: i64) -> Result<Vec<AnimeModel>, TrackError> {
        Ok(Legacy(self).by_entry(anime_id).await?.iter().map(AnimeModel::from_db).collect())
    }

    fn watch_for_entry(&self, anime_id: i64) -> RowsWatch<AnimeModel> {
        bridge(self.watch_by_entry(anime_id), |rows| rows.iter().map(|t| AnimeModel::from_db(&t.to_track())).collect())
    }

    async fn delete(&self, anime_id: i64, tracker_id: TrackerId) -> Result<(), TrackError> {
        Legacy(self).delete(anime_id, tracker_id).await
    }
}

#[async_trait]
impl MangaTrackRepo for kuro_data::track::MangaTrackRepo {
    async fn upsert(&self, t: &MangaModel) -> Result<i64, TrackError> {
        Legacy(self).upsert(t.to_db(false)?).await
    }

    async fn upsert_all(&self, ts: &[MangaModel]) -> Result<(), TrackError> {
        Legacy(self).upsert_all(ts.iter().map(|t| t.to_db(false)).collect::<Result<_, _>>()?).await
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<MangaModel>, TrackError> {
        Ok(Legacy(self).by_id(id).await?.as_ref().map(MangaModel::from_db))
    }

    async fn list_all(&self) -> Result<Vec<MangaModel>, TrackError> {
        Ok(Legacy(self).all().await?.iter().map(MangaModel::from_db).collect())
    }

    async fn list_for_entry(&self, manga_id: i64) -> Result<Vec<MangaModel>, TrackError> {
        Ok(Legacy(self).by_entry(manga_id).await?.iter().map(MangaModel::from_db).collect())
    }

    fn watch_for_entry(&self, manga_id: i64) -> RowsWatch<MangaModel> {
        bridge(self.watch_by_entry(manga_id), |rows| rows.iter().map(|t| MangaModel::from_db(&t.to_track())).collect())
    }

    async fn delete(&self, manga_id: i64, tracker_id: TrackerId) -> Result<(), TrackError> {
        Legacy(self).delete(manga_id, tracker_id).await
    }
}

#[async_trait]
impl AnimeEpisodeRepo for kuro_data::episode::EpisodeRepo {
    async fn list(&self, anime_id: i64) -> Result<Vec<UnitRef>, TrackError> {
        let repo = self.clone();
        let rows =
            blocking(move || kuro_data::episode::EpisodeRepo::by_anime_id(&repo, anime_id)).await?;
        Ok(rows
            .into_iter()
            .map(|e| UnitRef {
                id: e.id,
                number: e.episode_number,
                seen: e.seen,
            })
            .collect())
    }

    async fn mark_seen(&self, ids: &[i64]) -> Result<(), TrackError> {
        if ids.is_empty() {
            return Ok(());
        }
        let updates: Vec<_> = ids
            .iter()
            .map(|&id| kuro_data::episode::EpisodeUpdate {
                seen: Some(true),
                ..kuro_data::episode::EpisodeUpdate::new(id)
            })
            .collect();
        let repo = self.clone();
        blocking(move || kuro_data::episode::EpisodeRepo::update_all(&repo, &updates)).await
    }
}

#[async_trait]
impl MangaChapterRepo for kuro_data::chapter::ChapterRepo {
    async fn list(&self, manga_id: i64) -> Result<Vec<UnitRef>, TrackError> {
        let repo = self.clone();
        let rows =
            blocking(move || kuro_data::chapter::ChapterRepo::by_manga_id(&repo, manga_id, false))
                .await?;
        Ok(rows
            .into_iter()
            .map(|c| UnitRef {
                id: c.id,
                number: c.chapter_number,
                seen: c.read,
            })
            .collect())
    }

    async fn mark_read(&self, ids: &[i64]) -> Result<(), TrackError> {
        if ids.is_empty() {
            return Ok(());
        }
        let updates: Vec<_> = ids
            .iter()
            .map(|&id| kuro_data::chapter::ChapterUpdate {
                read: Some(true),
                ..kuro_data::chapter::ChapterUpdate::new(id)
            })
            .collect();
        let repo = self.clone();
        blocking(move || kuro_data::chapter::ChapterRepo::update_all(&repo, &updates)).await
    }
}

#[async_trait]
impl AnimeHistoryRepo for kuro_data::history::AnimeHistoryRepo {
    async fn seen_times(&self, anime_id: i64) -> Result<Vec<Option<i64>>, TrackError> {
        let repo = self.clone();
        let rows =
            blocking(move || kuro_data::history::AnimeHistoryRepo::by_anime_id(&repo, anime_id))
                .await?;
        Ok(rows.iter().map(|h| h.last_seen).collect())
    }
}

#[async_trait]
impl MangaHistoryRepo for kuro_data::history::MangaHistoryRepo {
    async fn read_times(&self, manga_id: i64) -> Result<Vec<Option<i64>>, TrackError> {
        let repo = self.clone();
        let rows =
            blocking(move || kuro_data::history::MangaHistoryRepo::by_manga_id(&repo, manga_id))
                .await?;
        Ok(rows.iter().map(|h| Some(h.last_read)).collect())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_db::handle::Db;
    use kuro_testkit::{AnimeBuilder, EpisodeBuilder, TempDir, TestDb};

    use super::*;
    use crate::model::{AnimeTrack, TrackerId};

    fn track(anime_id: i64, score: f64) -> AnimeTrack {
        AnimeTrack {
            id: None,
            anime_id,
            tracker_id: TrackerId::ANILIST,
            remote_id: 10,
            library_id: None,
            title: "Show".into(),
            last_episode_seen: 2.0,
            total_episodes: 12,
            status: 1,
            score,
            remote_url: "https://example.test/10".into(),
            start_date: 0,
            finish_date: 0,
            private: false,
        }
    }

    #[tokio::test]
    async fn tr_27_d01_upsert_keeps_the_row_id_and_returns_it() {
        let dir = TempDir::new().unwrap();
        let db: Arc<Db> = TestDb::handle(&dir);
        let anime_id = db.write(|tx| Ok(AnimeBuilder::new().insert(tx))).unwrap();
        let repo = SharedTracks::new(db.clone());

        let first = AnimeTrackRepo::upsert(&repo, &track(anime_id, 1.0))
            .await
            .unwrap();
        let second = AnimeTrackRepo::upsert(&repo, &track(anime_id, 9.0))
            .await
            .unwrap();
        assert_eq!(first, second, "the same (anime, tracker) keeps its _id");

        let rows = AnimeTrackRepo::list_for_entry(&repo, anime_id).await.unwrap();
        assert_eq!(kuro_data::track::TrackRepo::new(db.clone()).by_entry(anime_id).unwrap().len(), 1);
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].id, rows[0].score), (Some(first), 9.0));
    }

    #[tokio::test]
    async fn tr_27_015_a_second_tracker_gets_its_own_row() {
        let dir = TempDir::new().unwrap();
        let db: Arc<Db> = TestDb::handle(&dir);
        let anime_id = db.write(|tx| Ok(AnimeBuilder::new().insert(tx))).unwrap();
        let repo = SharedTracks::new(db.clone());
        let a = AnimeTrackRepo::upsert(&repo, &track(anime_id, 1.0))
            .await
            .unwrap();
        let mut other = track(anime_id, 1.0);
        other.tracker_id = TrackerId::KITSU;
        let b = AnimeTrackRepo::upsert(&repo, &other).await.unwrap();
        assert_ne!(a, b);
        assert_eq!(kuro_data::track::TrackRepo::new(db.clone()).by_entry(anime_id).unwrap().len(), 2);
        assert_eq!(
            AnimeTrackRepo::list_for_entry(&repo, anime_id)
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn tr_27_027_episode_repo_reads_numbers_and_marks_seen() {
        let dir = TempDir::new().unwrap();
        let db: Arc<Db> = TestDb::handle(&dir);
        let anime_id = db.write(|tx| Ok(AnimeBuilder::new().insert(tx))).unwrap();
        db.write(|tx| {
            EpisodeBuilder::new(anime_id).episode_number(1.0).insert(tx);
            EpisodeBuilder::new(anime_id).episode_number(2.5).insert(tx);
            Ok(())
        })
        .unwrap();
        let repo = SharedItems::new(db.clone());
        let rows = AnimeEpisodeRepo::list(&repo, anime_id).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.number == 2.5 && !r.seen));

        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        AnimeEpisodeRepo::mark_seen(&repo, &ids).await.unwrap();
        assert!(
            AnimeEpisodeRepo::list(&repo, anime_id)
                .await
                .unwrap()
                .iter()
                .all(|r| r.seen)
        );
    }
}
