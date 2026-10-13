//! Track bindings of both kinds (`manga_sync.sq`, `anime_sync.sq`): one row per (entry, tracker) pair. Writes are
//! upserts that keep the row id (Q-TRK-2). One `Track` and one `TrackRepo` over `kuro_db::tables::tracks`. The domain
//! interactors (log-and-default policy) live in `kuro-domain::track`.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::subscribe::Subscription;
use kuro_db::tables::tracks as t;
use kuro_db::tables::TrackRow;
pub use kuro_db::tables::TrackPatch;
use kuro_platform::kind::Kind;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::{DataResult, ENTRY_DEBOUNCE, none_if_not_found};

/// Tables a track listing reads.
const TRACK_TABLE: &[&str] = &["tracks"];

/// One tracker binding of an entry. `id` is `-1` until a write assigns the row id and `-1` is never persisted
/// (this type is read by `kuro-track`, `kuro-backup` and the app, so the sentinel stays and `kuro-track` keeps its own
/// `Option` model for remote results). `last_item` and `total_items` are the chapters or the episodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: i64,
    pub entry_id: i64,
    pub kind: Kind,
    pub tracker_id: i64,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub last_item: f64,
    pub total_items: i64,
    pub status: i64,
    pub score: f64,
    pub remote_url: String,
    pub start_date: i64,
    pub finish_date: i64,
    pub private: bool,
}

impl Track {
    /// An unsaved binding to `tracker_id` with the `TrackImpl` defaults. `id <= 0` means unsaved.
    pub fn new(kind: Kind, tracker_id: i64) -> Self {
        Self {
            id: -1,
            entry_id: 0,
            kind,
            tracker_id,
            remote_id: 0,
            library_id: None,
            title: String::new(),
            last_item: 0.0,
            total_items: 0,
            status: 0,
            score: 0.0,
            remote_url: String::new(),
            start_date: 0,
            finish_date: 0,
            private: false,
        }
    }

    /// Copies exactly the six personal fields of `other`: progress, score, status, start and finish dates, and
    /// `private` (`copyPersonalFrom`). Identity fields stay.
    pub fn copy_personal_from(&mut self, other: &Track) {
        self.copy_personal_keeping_private(other);
        self.private = other.private;
    }

    /// [`Self::copy_personal_from`] without `private` (`copyRemotePrivate = false`).
    pub fn copy_personal_keeping_private(&mut self, other: &Track) {
        self.last_item = other.last_item;
        self.score = other.score;
        self.status = other.status;
        self.start_date = other.start_date;
        self.finish_date = other.finish_date;
    }
}

impl From<TrackRow> for Track {
    fn from(r: TrackRow) -> Self {
        Self {
            id: r.id,
            entry_id: r.entry_id,
            kind: r.kind,
            tracker_id: r.tracker_id,
            remote_id: r.remote_id,
            library_id: r.library_id,
            title: r.title,
            last_item: r.last_item,
            total_items: r.total_items,
            status: r.status,
            score: r.score,
            remote_url: r.remote_url,
            start_date: r.start_date,
            finish_date: r.finish_date,
            private: r.private,
        }
    }
}

impl From<&Track> for TrackRow {
    fn from(t: &Track) -> Self {
        Self {
            id: t.id,
            entry_id: t.entry_id,
            kind: t.kind,
            tracker_id: t.tracker_id,
            remote_id: t.remote_id,
            library_id: t.library_id,
            title: t.title.clone(),
            last_item: t.last_item,
            total_items: t.total_items,
            status: t.status,
            score: t.score,
            remote_url: t.remote_url.clone(),
            start_date: t.start_date,
            finish_date: t.finish_date,
            private: t.private,
        }
    }
}

/// Updates the non-`None` columns of one `tracks` row inside the caller's connection or transaction (the restorer's
/// coalesced update). `None` keeps the stored value, so a patch with every field `None` changes nothing. Zero rows
/// matched is not an error.
pub fn update_coalesced(c: &Connection, p: &TrackPatch) -> DataResult<()> {
    t::update_coalesced(c, p)?;
    Ok(())
}

/// Track access for both kinds.
#[derive(Clone)]
pub struct TrackRepo {
    db: Arc<Db>,
}

impl TrackRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The binding with row id `id` (`_id`, Q-TRK-1). It is never an entry id.
    pub fn by_id(&self, id: i64) -> DataResult<Option<Track>> {
        Ok(self.db.read(|c| none_if_not_found(t::by_id(c, id)))?.map(Track::from))
    }

    /// Every binding of the kind, in table order.
    pub fn all(&self, kind: Kind) -> DataResult<Vec<Track>> {
        Ok(self.db.read(|c| Ok(t::all(c, kind)?))?.into_iter().map(Track::from).collect())
    }

    /// Every binding of one entry.
    pub fn by_entry(&self, entry_id: i64) -> DataResult<Vec<Track>> {
        Ok(self.db.read(|c| Ok(t::by_entry(c, entry_id)?))?.into_iter().map(Track::from).collect())
    }

    /// Upserts one binding and returns its row id. A second binding of the same (entry, tracker) pair updates the row
    /// in place, so the id does not change.
    pub fn insert(&self, track: &Track) -> DataResult<i64> {
        let row = TrackRow::from(track);
        self.db.write(|tx| Ok(t::insert(tx, &row)?))
    }

    /// Upserts every binding in one transaction. A repeated pair leaves one row with the last values.
    pub fn insert_all(&self, tracks: &[Track]) -> DataResult<()> {
        self.db.write(|tx| {
            for track in tracks {
                t::insert(tx, &TrackRow::from(track))?;
            }
            Ok(())
        })
    }

    /// Removes the binding of the entry to the tracker. Zero rows affected is not an error.
    pub fn delete(&self, entry_id: i64, tracker_id: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(t::delete(tx, entry_id, tracker_id).map(drop)?))
    }

    /// Every binding of the kind, published again after each write to `tracks`.
    pub fn watch_all(&self, kind: Kind) -> Subscription<Vec<Track>> {
        self.db.subscribe(TRACK_TABLE, ENTRY_DEBOUNCE, move |c| Ok(t::all(c, kind)?.into_iter().map(Track::from).collect::<Vec<_>>()))
    }

    /// The bindings of one entry, published again after each write to `tracks`.
    pub fn watch_by_entry(&self, entry_id: i64) -> Subscription<Vec<Track>> {
        self.db.subscribe(TRACK_TABLE, ENTRY_DEBOUNCE, move |c| Ok(t::by_entry(c, entry_id)?.into_iter().map(Track::from).collect::<Vec<_>>()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::support::{insert_anime, insert_manga, test_db};

    fn track(kind: Kind, entry_id: i64, tracker: i64, title: &str, progress: f64) -> Track {
        Track {
            entry_id,
            remote_id: 42,
            library_id: Some(7),
            title: title.into(),
            last_item: progress,
            total_items: 120,
            status: 1,
            score: 8.5,
            remote_url: "https://tracker.test/42".into(),
            start_date: 1_700_000_000_000,
            private: true,
            ..Track::new(kind, tracker)
        }
    }

    fn full(entry_id: i64) -> Track {
        Track { remote_id: 42, library_id: Some(7), title: "Stored".into(), last_item: 4.0, total_items: 120, status: 1, score: 8.0, remote_url: "https://t/42".into(), start_date: 10, finish_date: 20, entry_id, ..Track::new(Kind::Manga, 2) }
    }

    #[test]
    fn insert_then_by_entry_round_trips_every_field() {
        let (_d, db) = test_db();
        let id = insert_manga(&db, "/m", true);
        let repo = TrackRepo::new(db);
        let id_inserted = repo.insert(&track(Kind::Manga, id, 2, "Read", 12.5)).expect("insert");
        let got = repo.by_entry(id).expect("by entry");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], Track { id: id_inserted, ..track(Kind::Manga, id, 2, "Read", 12.5) }, "TC-TRK-01");
        assert_eq!(repo.by_id(id_inserted).expect("by row"), Some(got[0].clone()));
    }

    #[test]
    // Decision (T-06a-030 T1 vs area 09 Q-TRK-2): the row id is kept. Area 06 T1 asks for the Kotlin REPLACE (new `_id`,
    // `by_id(old)` None); area 09 T-09a-012 and `kuro-track` rely on a stable id, so the upsert stands and T1 is not met.
    fn a_second_track_for_the_same_pair_updates_the_row_and_keeps_its_id_for_both_kinds() {
        let (_d, db) = test_db();
        let manga = insert_manga(&db, "/m", true);
        let anime = insert_anime(&db, "/a", true);
        let repo = TrackRepo::new(db);
        for (kind, id) in [(Kind::Manga, manga), (Kind::Anime, anime)] {
            let first = repo.insert(&track(kind, id, 2, "Old", 1.0)).expect("insert");
            let second = repo.insert(&track(kind, id, 2, "New", 9.0)).expect("upsert");
            assert_eq!(first, second, "TC-TRK-02: the row id is stable (Q-TRK-2)");
            repo.insert(&track(kind, id, 6, "other", 0.0)).expect("second tracker");
            let got = repo.by_entry(id).expect("by entry");
            assert_eq!(got.len(), 2);
            let updated = got.iter().find(|t| t.tracker_id == 2).expect("tracker 2");
            assert_eq!((updated.id, updated.title.as_str(), updated.last_item, updated.kind), (first, "New", 9.0, kind));
            repo.delete(id, 6).expect("delete");
            assert_eq!(repo.by_entry(id).expect("by entry").len(), 1);
        }
    }

    #[test]
    fn insert_all_with_a_duplicate_pair_keeps_one_row_with_the_last_values() {
        let (_d, db) = test_db();
        let id = insert_manga(&db, "/m", true);
        let repo = TrackRepo::new(db);
        repo.insert_all(&[track(Kind::Manga, id, 3, "first", 1.0), track(Kind::Manga, id, 3, "last", 2.0)]).expect("insert all");
        let got = repo.by_entry(id).expect("by entry");
        assert_eq!(got.len(), 1, "TC-TRK-06");
        assert_eq!((got[0].title.as_str(), got[0].last_item), ("last", 2.0));
    }

    #[test]
    fn delete_of_an_absent_pair_is_ok_and_delete_removes_a_present_one() {
        let (_d, db) = test_db();
        let id = insert_manga(&db, "/m", true);
        let repo = TrackRepo::new(db);
        repo.delete(id, 99).expect("absent pair is not an error (TC-TRK-03)");
        repo.insert(&track(Kind::Manga, id, 4, "t", 0.0)).expect("insert");
        repo.delete(id, 4).expect("delete");
        assert!(repo.by_entry(id).expect("by entry").is_empty());
    }

    #[tokio::test]
    async fn deleting_an_entry_cascades_its_tracks_and_notifies_watch_all() {
        let (_d, db) = test_db();
        let id = insert_manga(&db, "/m", true);
        let repo = TrackRepo::new(db.clone());
        repo.insert(&track(Kind::Manga, id, 5, "t", 0.0)).expect("insert");
        let mut sub = repo.watch_all(Kind::Manga);
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
        db.write(|tx| Ok(tx.execute("DELETE FROM entries WHERE _id = ?1", [id])?)).expect("delete manga");
        wait(0).await;
        assert!(repo.by_entry(id).expect("by entry").is_empty(), "E-TRK-05: foreign_keys is on");
    }

    #[test]
    fn the_model_round_trips_through_serde() {
        let m = track(Kind::Manga, 1, 2, "m", 1.5);
        let json = serde_json::to_string(&m).expect("encode");
        assert_eq!(serde_json::from_str::<Track>(&json).expect("decode"), m);
    }

    #[tokio::test]
    async fn watch_all_emits_after_insert_and_delete() {
        let (_d, db) = test_db();
        let id = insert_manga(&db, "/m", true);
        let repo = TrackRepo::new(db);
        let mut sub = repo.watch_all(Kind::Manga);
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
        repo.insert(&track(Kind::Manga, id, 1, "t", 0.0)).expect("insert");
        wait(1).await;
        repo.delete(id, 1).expect("delete");
        wait(0).await;
    }

    #[test]
    fn a_patch_with_every_field_none_leaves_the_row_unchanged_and_one_field_changes_only_itself() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "/m", true);
        let repo = TrackRepo::new(db.clone());
        let id = repo.insert(&full(m)).unwrap();
        let before = repo.by_id(id).unwrap().unwrap();
        db.write(|tx| update_coalesced(tx, &TrackPatch { id, ..Default::default() })).unwrap();
        assert_eq!(repo.by_id(id).unwrap().unwrap(), before, "T5: all-None patch is a no-op");
        db.write(|tx| update_coalesced(tx, &TrackPatch { id, last_item: Some(9.5), ..Default::default() })).unwrap();
        assert_eq!(repo.by_id(id).unwrap().unwrap(), Track { last_item: 9.5, ..before });
    }

    #[test]
    fn a_patch_sets_every_column_for_anime() {
        let (_d, db) = test_db();
        let a = insert_anime(&db, "/a", false);
        let repo = TrackRepo::new(db.clone());
        let base = Track { entry_id: a, remote_id: 5, title: "t".into(), last_item: 1.0, total_items: 12, status: 1, score: 2.0, remote_url: "u".into(), start_date: 3, finish_date: 4, ..Track::new(Kind::Anime, 3) };
        let id = repo.insert(&base).unwrap();
        let all = TrackPatch {
            id,
            entry_id: Some(a),
            tracker_id: Some(3),
            remote_id: Some(6),
            library_id: Some(9),
            title: Some("new".into()),
            last_item: Some(2.5),
            total_items: Some(24),
            status: Some(2),
            score: Some(7.5),
            remote_url: Some("v".into()),
            start_date: Some(30),
            finish_date: Some(40),
            private: Some(true),
        };
        db.write(|tx| update_coalesced(tx, &all)).unwrap();
        assert_eq!(
            repo.by_id(id).unwrap().unwrap(),
            Track {
                id,
                entry_id: a,
                kind: Kind::Anime,
                tracker_id: 3,
                remote_id: 6,
                library_id: Some(9),
                title: "new".into(),
                last_item: 2.5,
                total_items: 24,
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
        let (_d, db) = test_db();
        let m = insert_manga(&db, "/m", true);
        let repo = TrackRepo::new(db.clone());
        let missing = Track { tracker_id: 9, ..full(999_999) };
        assert!(repo.insert_all(&[full(m), missing]).is_err(), "foreign key to a missing entry");
        assert!(repo.by_entry(m).unwrap().is_empty(), "the first row is rolled back with the failing one");
    }

    #[test]
    fn fractional_progress_round_trips_exactly_and_an_anime_delete_cascades() {
        let (_d, db) = test_db();
        let repo = TrackRepo::new(db.clone());
        let m = insert_manga(&db, "/m", true);
        let a = insert_anime(&db, "/a", true);
        let id = repo.insert(&Track { entry_id: m, last_item: 1.0 / 3.0, ..Track::new(Kind::Manga, 1) }).unwrap();
        assert_eq!(repo.by_id(id).unwrap().unwrap().last_item, 1.0 / 3.0);
        let aid = repo.insert(&Track { entry_id: a, last_item: 2.0 / 3.0, ..Track::new(Kind::Anime, 1) }).unwrap();
        assert_eq!(repo.by_id(aid).unwrap().unwrap().last_item, 2.0 / 3.0);
        db.write(|tx| Ok(tx.execute("DELETE FROM entries WHERE _id = ?1", [a])?)).unwrap();
        assert!(repo.by_entry(a).unwrap().is_empty());
        assert_eq!(repo.all(Kind::Manga).unwrap().len(), 1);
        assert!(repo.all(Kind::Anime).unwrap().is_empty());
    }

    #[test]
    fn new_gives_the_documented_defaults_and_an_unsaved_id() {
        let m = Track::new(Kind::Manga, 5);
        assert_eq!((m.id, m.tracker_id, m.entry_id, m.last_item, m.title.as_str(), m.private), (-1, 5, 0, 0.0, "", false));
        assert_eq!((m.library_id, m.remote_id, m.total_items, m.start_date, m.finish_date), (None, 0, 0, 0, 0));
    }

    #[test]
    fn copy_personal_from_copies_private_and_the_keeping_form_does_not() {
        let src = Track { private: true, last_item: 7.0, score: 9.0, status: 3, start_date: 1, finish_date: 2, ..Track::new(Kind::Manga, 1) };
        let mut kept = Track { id: 77, entry_id: 5, title: "keep".into(), ..Track::new(Kind::Manga, 1) };
        kept.copy_personal_keeping_private(&src);
        assert!(!kept.private, "T6: private unchanged with false");
        assert_eq!((kept.last_item, kept.score, kept.status, kept.start_date, kept.finish_date), (7.0, 9.0, 3, 1, 2));
        assert_eq!((kept.id, kept.entry_id, kept.title.as_str()), (77, 5, "keep"), "identity fields are not copied");
        kept.copy_personal_from(&src);
        assert!(kept.private, "T6: private copied with true");
    }
}
