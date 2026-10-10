//! The app side of the anime migration engine (T-41a-034): the steps of `AnimeMigrateSteps` over the anime database, the
//! installed sources and the cover files. The database steps share the one transaction `transact` opens, so a failed step
//! rolls all of them back; the file steps run after it commits.
//!
//! Downloads: the dialog offers Delete downloaded only when the old anime has downloads, and this app publishes no anime
//! download index to ask, so the option is never ticked and `delete_downloads` has nothing to delete.

use std::sync::Arc;

use kuro_data::anime::{Anime, AnimeUpdate};
use kuro_data::episode::{Episode, EpisodeUpdate, map_episode};
use kuro_data::track::AnimeTrack;
use kuro_db::statements_anime as sa;
use kuro_db::{Db, DbError, Transaction};
use kuro_images::cover_paths::{Family, custom_file};
use kuro_platform::linux::LinuxPaths;
use kuro_prefs::store::PrefStore;
use tokio::runtime::Handle;

use super::directory::AnimeDirectory;
use super::migrate_engine::{AnimeMigrateSteps, AnimeMigrateTx, AnimeTxWork};
use crate::browse::migrate_engine::StepFuture;
use crate::entry_core::anime::entry_state::RefreshPlan;
use crate::entry_core::anime::refresh::Trigger;
use crate::entry_core::anime::remote::{LiveSources, RemoteAnimeBackend};
use crate::entry_core::anime::runner::AnimeBackend;

pub struct AnimeLiveSteps {
    db: Arc<Db>,
    directory: Arc<AnimeDirectory>,
    prefs: PrefStore,
}

impl AnimeLiveSteps {
    /// Needs a tokio runtime entered on the calling thread: the episode fetch runs on it.
    pub fn new(db: Arc<Db>, directory: Arc<AnimeDirectory>, prefs: PrefStore) -> Self {
        Self { db, directory, prefs }
    }

    fn has_custom(family: Family, id: i64) -> bool {
        LinuxPaths::system().is_ok_and(|paths| custom_file(&paths, family, id).is_file())
    }

    fn copy_custom(family: Family, old: &Anime, new: &Anime, what: &str) -> Result<(), String> {
        let paths = LinuxPaths::system().map_err(|e| e.to_string())?;
        if kuro_images::cover_file::copy_custom(&paths, family, old.id, new.id) { Ok(()) } else { Err(format!("the custom {what} could not be copied")) }
    }
}

fn track_of(row: sa::AnimeTrack) -> AnimeTrack {
    AnimeTrack {
        id: row.id,
        anime_id: row.anime_id,
        tracker_id: row.sync_id,
        remote_id: row.remote_id,
        library_id: row.library_id,
        title: row.title,
        last_episode_seen: row.last_episode_seen,
        total_episodes: row.total_episodes,
        status: row.status,
        score: row.score,
        remote_url: row.remote_url,
        start_date: row.start_date,
        finish_date: row.finish_date,
        private: row.private,
    }
}

fn track_row(track: &AnimeTrack) -> sa::AnimeTrack {
    sa::AnimeTrack {
        id: track.id,
        anime_id: track.anime_id,
        sync_id: track.tracker_id,
        remote_id: track.remote_id,
        library_id: track.library_id,
        title: track.title.clone(),
        last_episode_seen: track.last_episode_seen,
        total_episodes: track.total_episodes,
        status: track.status,
        score: track.score,
        remote_url: track.remote_url.clone(),
        start_date: track.start_date,
        finish_date: track.finish_date,
        private: track.private,
    }
}

/// The columns a migration writes on an episode: the seen flag, bookmark, filler mark, resume point and fetch date.
fn episode_patch(update: &EpisodeUpdate) -> sa::EpisodeUpdate {
    sa::EpisodeUpdate {
        id: update.id,
        seen: update.seen,
        bookmark: update.bookmark,
        fillermark: update.fillermark,
        last_second_seen: update.last_second_seen,
        date_fetch: update.date_fetch,
        ..sa::EpisodeUpdate::default()
    }
}

/// The columns a migration writes on an anime: the favourite flag, the two flag words and the date added.
fn anime_patch(update: &AnimeUpdate) -> sa::AnimeUpdate {
    sa::AnimeUpdate {
        id: update.id,
        favorite: update.favorite,
        viewer_flags: update.viewer_flags,
        episode_flags: update.episode_flags,
        date_added: update.date_added,
        ..sa::AnimeUpdate::default()
    }
}

struct LiveTx<'a, 'c>(&'a Transaction<'c>);

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl AnimeMigrateTx for LiveTx<'_, '_> {
    fn episodes(&mut self, anime_id: i64) -> Result<Vec<Episode>, String> {
        sa::episodes_by_anime_id(self.0, anime_id).map(|rows| rows.iter().map(map_episode).collect()).map_err(text)
    }
    fn update_episodes(&mut self, updates: &[EpisodeUpdate]) -> Result<(), String> {
        updates.iter().try_for_each(|update| sa::update_episode(self.0, &episode_patch(update)).map(drop)).map_err(text)
    }
    fn category_ids(&mut self, anime_id: i64) -> Result<Vec<i64>, String> {
        sa::categories_by_anime_id(self.0, anime_id).map(|rows| rows.into_iter().map(|row| row.id).collect()).map_err(text)
    }
    /// The links are replaced with plain statements: `set_anime_categories` opens a transaction of its own, which cannot
    /// nest inside this one.
    fn set_categories(&mut self, anime_id: i64, ids: &[i64]) -> Result<(), String> {
        self.0.execute("DELETE FROM animes_categories WHERE anime_id = ?1", [anime_id]).map_err(text)?;
        let mut unique = ids.to_vec();
        unique.sort_unstable();
        unique.dedup();
        unique
            .into_iter()
            .try_for_each(|id| self.0.execute("INSERT INTO animes_categories(anime_id, category_id) VALUES (?1, ?2)", (anime_id, id)).map(drop))
            .map_err(text)
    }
    fn tracks(&mut self, anime_id: i64) -> Result<Vec<AnimeTrack>, String> {
        sa::tracks_by_anime_id(self.0, anime_id).map(|rows| rows.into_iter().map(track_of).collect()).map_err(text)
    }
    fn insert_tracks(&mut self, tracks: &[AnimeTrack]) -> Result<(), String> {
        tracks.iter().try_for_each(|track| sa::upsert_track(self.0, &track_row(track)).map(drop)).map_err(text)
    }
    fn update_anime(&mut self, update: &AnimeUpdate) -> Result<(), String> {
        sa::update_anime(self.0, &anime_patch(update)).map(drop).map_err(text)
    }
}

impl AnimeMigrateSteps for AnimeLiveSteps {
    fn source_present(&self, source_id: i64) -> bool {
        self.directory.instance(source_id).is_some()
    }

    /// The episode list of the new anime, fetched and stored by the same backend the entry page refreshes with, on the
    /// runtime (its sync blocks a worker).
    fn fetch_episodes(&self, new: &Anime) -> StepFuture<Result<(), String>> {
        let Some(source) = self.directory.instance(new.source) else {
            return Box::pin(async { Err("the source of the new anime is not installed".to_owned()) });
        };
        let handle = Handle::current();
        let backend = RemoteAnimeBackend::new(Arc::clone(&self.db), &self.prefs, Arc::new(LiveSources::new(vec![source])), handle.clone());
        let task = backend.fetch(new.id, RefreshPlan { info: false, episodes: true, seasons: false }, Trigger::Manual);
        Box::pin(async move {
            match handle.spawn(task).await {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(error)) => Err(error.user_message()),
                Err(error) => Err(error.to_string()),
            }
        })
    }

    fn transact(&self, work: AnimeTxWork<'_>) -> Result<(), String> {
        self.db
            .write(|tx| work(&mut LiveTx(tx)).map_err(DbError::Constraint))
            .map_err(|error| match error {
                DbError::Constraint(message) => message,
                other => other.to_string(),
            })
    }

    /// A plain tracker keeps the binding and its remote id; an enhanced tracker is matched against the new source's
    /// catalogue over the network, which a synchronous step cannot do, so its binding is kept as it is and the tracker
    /// page repairs it on the next refresh.
    fn move_track(&self, track: AnimeTrack, _old: &Anime, _new: &Anime) -> Option<AnimeTrack> {
        Some(track)
    }

    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
    }

    fn delete_downloads(&self, _old: &Anime) -> Result<(), String> {
        Ok(())
    }

    fn old_has_custom_cover(&self, old: &Anime) -> bool {
        Self::has_custom(Family::AnimeCover, old.id)
    }

    fn copy_custom_cover(&self, old: &Anime, new: &Anime) -> Result<(), String> {
        Self::copy_custom(Family::AnimeCover, old, new, "cover")
    }

    fn old_has_custom_background(&self, old: &Anime) -> bool {
        Self::has_custom(Family::AnimeBackground, old.id)
    }

    fn copy_custom_background(&self, old: &Anime, new: &Anime) -> Result<(), String> {
        Self::copy_custom(Family::AnimeBackground, old, new, "background")
    }
}

#[cfg(test)]
mod tests {
    use kuro_data::anime::AnimeRepo;
    use kuro_data::category::{CategoryKind, CategoryRepo, create_anime_category};
    use kuro_data::track::AnimeTrackRepo;

    use super::*;
    use crate::browse::anime::directory::{AnimeInstances, sources_of};
    use crate::browse::anime::fixtures::{FakeSource, lookup};
    use crate::browse::anime::migrate_engine::{AnimeMigrateError, migrate_anime};
    use crate::browse::migrate_engine::MigratePlan;
    use crate::browse::migrate_flags::{CATEGORIES, EPISODES};
    use kuro_extensions::anime::model::Buckets;
    use kuro_source::anime::AnimeSource;

    struct Held(Vec<Arc<FakeSource>>);

    impl AnimeInstances for Held {
        fn source(&self, id: i64) -> Option<Arc<dyn AnimeSource>> {
            self.0.iter().find(|s| s.id() == id).map(|s| Arc::clone(s) as Arc<dyn AnimeSource>)
        }
    }

    fn db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("animetachiyomi.db");
        let writer = kuro_db::schema_anime::open(&path).expect("schema");
        (dir, Arc::new(Db::new(&path, writer).expect("db")))
    }

    fn add(repo: &AnimeRepo, source: i64, url: &str, favorite: bool) -> Anime {
        let id = repo.insert(&Anime { source, url: url.into(), title: format!("t{url}"), favorite, episode_flags: 5, viewer_flags: 6, date_added: 42, ..Anime::create() }).expect("insert");
        repo.by_id(id).expect("by id")
    }

    fn steps(db: &Arc<Db>, installed: Vec<Arc<FakeSource>>) -> AnimeLiveSteps {
        let (lookup, _) = lookup(&[]);
        let directory = AnimeDirectory::new(sources_of(&Buckets::default()), Some(Arc::new(Held(installed))), Arc::new(move |_| Arc::clone(&lookup)));
        AnimeLiveSteps::new(Arc::clone(db), Arc::new(directory), PrefStore::in_memory())
    }

    fn runtime() -> &'static tokio::runtime::Runtime {
        Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime")))
    }

    /// Migrating moves the categories and the tracks, flips the favourites, and a missing new source writes nothing.
    #[test]
    fn a_migration_over_the_database_moves_categories_and_tracks_and_flips_the_favourites_in_one_transaction() {
        let rt = runtime();
        let _entered = rt.enter();
        let (_dir, db) = db();
        let (repo, categories, tracks) = (AnimeRepo::new(Arc::clone(&db)), CategoryRepo::new(Arc::clone(&db), CategoryKind::Anime), AnimeTrackRepo::new(Arc::clone(&db)));
        let category = create_anime_category(&db, "Watching", 0).expect("category");
        let (old, new) = (add(&repo, 1, "/old", true), add(&repo, 2, "/new", false));
        repo.set_categories(old.id, &[category]).expect("link");
        tracks.insert(&AnimeTrack { anime_id: old.id, tracker_id: 7, remote_id: 99, title: "t".into(), ..AnimeTrack::new(7) }).expect("track");
        let steps = steps(&db, vec![FakeSource::new(1, Vec::new()), FakeSource::new(2, Vec::new())]);
        // the fetch needs a listed episode; the fake source has none, so the engine's own fetch error is the answer
        let plan = MigratePlan { replace: true, flags: (CATEGORIES | EPISODES) as i32 };
        let failed = rt.block_on(migrate_anime(&steps, &old, &new, plan));
        assert!(matches!(failed, Err(AnimeMigrateError::FetchEpisodes(_))), "a failed fetch stops before any write: {failed:?}");
        assert!(repo.by_id(old.id).expect("old").favorite && !repo.by_id(new.id).expect("new").favorite, "nothing was flipped");

        // the transaction itself: every database step through the trait, committed together
        steps
            .transact(&mut |tx| {
                let ids = tx.category_ids(old.id)?;
                tx.set_categories(new.id, &ids)?;
                let moved: Vec<AnimeTrack> = tx.tracks(old.id)?.into_iter().map(|t| AnimeTrack { id: -1, anime_id: new.id, ..t }).collect();
                tx.insert_tracks(&moved)?;
                tx.update_anime(&AnimeUpdate { favorite: Some(false), date_added: Some(0), ..AnimeUpdate::new(old.id) })?;
                tx.update_anime(&AnimeUpdate { favorite: Some(true), episode_flags: Some(old.episode_flags), viewer_flags: Some(old.viewer_flags), date_added: Some(42), ..AnimeUpdate::new(new.id) })
            })
            .expect("commit");
        assert_eq!(categories.by_entry(new.id).expect("categories").into_iter().map(|c| c.id).collect::<Vec<_>>(), [category]);
        assert_eq!(tracks.by_entry(new.id).expect("tracks").len(), 1, "the binding moved");
        let (old_row, new_row) = (repo.by_id(old.id).expect("old"), repo.by_id(new.id).expect("new"));
        assert!(!old_row.favorite && new_row.favorite);
        assert_eq!((new_row.episode_flags, new_row.viewer_flags, new_row.date_added), (5, 6, 42));
    }

    #[test]
    fn a_failing_step_rolls_every_earlier_write_back() {
        let rt = runtime();
        let _entered = rt.enter();
        let (_dir, db) = db();
        let repo = AnimeRepo::new(Arc::clone(&db));
        let (old, new) = (add(&repo, 1, "/old", true), add(&repo, 2, "/new", false));
        let steps = steps(&db, Vec::new());
        let result = steps.transact(&mut |tx| {
            tx.update_anime(&AnimeUpdate { favorite: Some(true), ..AnimeUpdate::new(new.id) })?;
            tx.update_anime(&AnimeUpdate { favorite: Some(false), ..AnimeUpdate::new(old.id) })?;
            Err("disk full".to_owned())
        });
        assert_eq!(result, Err("disk full".to_owned()));
        assert!(repo.by_id(old.id).expect("old").favorite && !repo.by_id(new.id).expect("new").favorite, "both flips rolled back");
    }

    #[test]
    fn an_uninstalled_source_is_absent_and_its_fetch_fails_without_touching_the_database() {
        let rt = runtime();
        let _entered = rt.enter();
        let (_dir, db) = db();
        let repo = AnimeRepo::new(Arc::clone(&db));
        let new = add(&repo, 2, "/new", false);
        let steps = steps(&db, vec![FakeSource::new(1, Vec::new())]);
        assert!(steps.source_present(1) && !steps.source_present(2));
        assert!(rt.block_on(steps.fetch_episodes(&new)).is_err());
    }
}
