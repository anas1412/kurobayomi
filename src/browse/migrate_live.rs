//! The app side of the migration engine for both kinds (T-40a-016, T-40a-041, T-41a-034): the steps of `MigrateSteps` over
//! the kind's database, its source directory, the manga download index and the custom image files. The new entry's
//! chapters or episodes are fetched and stored by the same backend its entry page refreshes with. The database steps
//! share one transaction, so a failed step rolls all of them back; the file steps run after it commits.
//!
//! Downloads: this app publishes no anime download index, so an anime never offers Delete downloaded and has nothing to
//! delete.

use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::chapter::{Chapter, ChapterUpdate, map_chapter};
use kuro_data::episode::{Episode, EpisodeUpdate, map_episode};
use kuro_data::manga::MangaRepo;
use kuro_db::sql::{categories, chapters, manga_sync, mangas, mangas_categories};
use kuro_db::statements_anime as sa;
use kuro_db::statements_manga::{ChapterPatch, MangaPatch, TrackRow};
use kuro_db::{Db, DbError, Transaction};
use kuro_downloads::index::DownloadIndex;
use kuro_downloads::layout::DownloadLayout;
use kuro_images::cover_paths::{Family, custom_file};
use kuro_platform::linux::LinuxPaths;
use kuro_prefs::store::PrefStore;
use kuro_source::anime::model::FetchType;
use tokio::runtime::Handle;

use super::directory::SourceDirectory;
use super::migrate_engine::{CustomImage, Entry, MigrateSteps, MigrateTx, StepFuture, TxWork};
use super::migrate_merge::merge_chapter_state;
use kuro_platform::kind::Kind;
use crate::entry_core::anime::entry_state::RefreshPlan;
use crate::entry_core::anime::refresh::Trigger;
use crate::entry_core::anime::remote::{LiveSources, RemoteAnimeBackend};
use crate::entry_core::anime::runner::AnimeBackend;
use crate::entry_core::backend::{Backend, SourceBackend};

pub struct LiveSteps {
    kind: Kind,
    db: Arc<Db>,
    directory: Arc<dyn SourceDirectory>,
    /// The manga download index; `None` when the app runs none.
    downloads: Option<DownloadIndex>,
    prefs: PrefStore,
}

impl LiveSteps {
    /// The fetch runs on the app runtime: the one entered on the calling thread, or the shell's.
    pub fn new(kind: Kind, db: Arc<Db>, directory: Arc<dyn SourceDirectory>, downloads: Option<DownloadIndex>, prefs: PrefStore) -> Self {
        Self { kind, db, directory, downloads, prefs }
    }

    /// The library entry of `id` as the engine reads it. `None` when it is not in the library.
    pub fn entry(&self, id: i64) -> Option<Entry> {
        match self.kind {
            Kind::Manga => MangaRepo::new(Arc::clone(&self.db)).by_id(id).ok().map(|manga| Entry {
                id: manga.id,
                source: manga.source,
                title: manga.title,
                item_flags: manga.chapter_flags,
                viewer_flags: manga.viewer_flags,
                date_added: manga.date_added,
            }),
            Kind::Anime => AnimeRepo::new(Arc::clone(&self.db)).by_id(id).ok().map(|anime| Entry {
                id: anime.id,
                source: anime.source,
                title: anime.title,
                item_flags: anime.episode_flags,
                viewer_flags: anime.viewer_flags,
                date_added: anime.date_added,
            }),
        }
    }

    /// How the entry lists its items, for the dialog's rules: a manga, like an anime that is not a seasons title, lists
    /// episodes (chapters). The library stores the anime fetch type as the database's enum; the rules take the source API's.
    pub fn fetch_type(&self, id: i64) -> FetchType {
        match self.kind {
            Kind::Manga => FetchType::Episodes,
            Kind::Anime => AnimeRepo::new(Arc::clone(&self.db)).by_id(id).map_or(FetchType::Episodes, |anime| FetchType::from(anime.fetch_type as i64)),
        }
    }

    /// Whether the old entry has downloaded chapters on disk (the dialog offers Delete downloaded only then).
    pub fn has_downloads(&self, entry: &Entry) -> bool {
        self.kind == Kind::Manga && self.downloads.as_ref().is_some_and(|index| index.chapter_count_manga(entry.source, &entry.title) > 0)
    }

    /// The image files of a custom image of this kind; a manga has no background.
    fn family(&self, image: CustomImage) -> Option<Family> {
        match (self.kind, image) {
            (Kind::Manga, CustomImage::Cover) => Some(Family::MangaCover),
            (Kind::Manga, CustomImage::Background) => None,
            (Kind::Anime, CustomImage::Cover) => Some(Family::AnimeCover),
            (Kind::Anime, CustomImage::Background) => Some(Family::AnimeBackground),
        }
    }
}

/// The app runtime: the one entered on this thread, or the shell's.
fn runtime() -> Option<Handle> {
    Handle::try_current().ok().or_else(|| crate::shell::host::Shell::get().map(|shell| shell.runtime().clone()))
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// The chapter updates that carry the old state over: only the rows whose read flag, bookmark or fetch date changed.
fn chapter_updates(current: &[Chapter], merged: &[Chapter]) -> Vec<ChapterUpdate> {
    current
        .iter()
        .zip(merged)
        .filter(|(before, after)| (before.read, before.bookmark, before.date_fetch) != (after.read, after.bookmark, after.date_fetch))
        .map(|(_, after)| ChapterUpdate { read: Some(after.read), bookmark: Some(after.bookmark), date_fetch: Some(after.date_fetch), ..ChapterUpdate::new(after.id) })
        .collect()
}

fn recognized(episode: &Episode) -> bool {
    episode.episode_number >= 0.0
}

/// The episode rows of the new anime with the old anime's state carried over. A recognised number takes the date
/// fetched, bookmark, last second seen and filler mark of the old episode with the same number (E20). It is seen when
/// the old anime had seen an episode at or above its number (the max-seen rule); `seen` is never taken from the matched
/// episode. Unrecognised episodes keep their state. Every row is written, as the Kotlin code does.
fn episode_updates(prev: &[Episode], current: &[Episode]) -> Vec<EpisodeUpdate> {
    let max_seen = prev.iter().filter(|e| e.seen).map(|e| e.episode_number).reduce(f64::max);
    current
        .iter()
        .map(|episode| {
            let mut state = episode.clone();
            if recognized(episode) {
                if let Some(old) = prev.iter().find(|p| recognized(p) && p.episode_number == episode.episode_number) {
                    state.date_fetch = old.date_fetch;
                    state.bookmark = old.bookmark;
                    state.last_second_seen = old.last_second_seen;
                    state.fillermark = old.fillermark;
                }
                if max_seen.is_some_and(|max| episode.episode_number <= max) {
                    state.seen = true;
                }
            }
            EpisodeUpdate {
                id: state.id,
                seen: Some(state.seen),
                bookmark: Some(state.bookmark),
                fillermark: Some(state.fillermark),
                last_second_seen: Some(state.last_second_seen),
                date_fetch: Some(state.date_fetch),
                ..EpisodeUpdate::default()
            }
        })
        .collect()
}

fn chapter_patch(update: &ChapterUpdate) -> ChapterPatch {
    ChapterPatch {
        id: update.id,
        manga_id: None,
        url: None,
        name: None,
        scanlator: None,
        read: update.read,
        bookmark: update.bookmark,
        last_page_read: None,
        chapter_number: None,
        source_order: None,
        date_fetch: update.date_fetch,
        date_upload: None,
        version: None,
        is_syncing: None,
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

/// The columns a migration writes on a manga: the favourite flag, the date added and, when given, the chapter and viewer
/// flags.
fn manga_patch(id: i64, favorite: bool, date_added: i64, flags: Option<(i64, i64)>) -> MangaPatch {
    MangaPatch {
        id,
        source: None,
        url: None,
        artist: None,
        author: None,
        description: None,
        genre: None,
        title: None,
        status: None,
        thumbnail_url: None,
        favorite: Some(favorite),
        last_update: None,
        next_update: None,
        initialized: None,
        viewer: flags.map(|(_, viewer)| viewer),
        chapter_flags: flags.map(|(items, _)| items),
        cover_last_modified: None,
        date_added: Some(date_added),
        update_strategy: None,
        calculate_interval: None,
        version: None,
        is_syncing: None,
    }
}

struct LiveTx<'a, 'c> {
    kind: Kind,
    tx: &'a Transaction<'c>,
}

impl MigrateTx for LiveTx<'_, '_> {
    fn copy_item_state(&mut self, old: i64, new: i64) -> Result<(), String> {
        let tx = self.tx;
        match self.kind {
            Kind::Manga => {
                let read = |id: i64| chapters::get_by_manga_id(tx, id, false).map(|rows| rows.iter().map(map_chapter).collect::<Vec<Chapter>>()).map_err(text);
                let (prev, current) = (read(old)?, read(new)?);
                let updates = chapter_updates(&current, &merge_chapter_state(&prev, &current));
                updates.iter().try_for_each(|update| chapters::update(tx, &chapter_patch(update)).map(drop)).map_err(text)
            }
            Kind::Anime => {
                let read = |id: i64| sa::episodes_by_anime_id(tx, id).map(|rows| rows.iter().map(map_episode).collect::<Vec<Episode>>()).map_err(text);
                let (prev, current) = (read(old)?, read(new)?);
                episode_updates(&prev, &current).iter().try_for_each(|update| sa::update_episode(tx, &episode_patch(update)).map(drop)).map_err(text)
            }
        }
    }

    fn category_ids(&mut self, id: i64) -> Result<Vec<i64>, String> {
        match self.kind {
            Kind::Manga => categories::get_categories_by_manga_id(self.tx, id).map(|rows| rows.into_iter().map(|row| row.id).collect()).map_err(text),
            Kind::Anime => sa::categories_by_anime_id(self.tx, id).map(|rows| rows.into_iter().map(|row| row.id).collect()).map_err(text),
        }
    }

    /// The anime links are replaced with plain statements: `set_anime_categories` opens a transaction of its own, which
    /// cannot nest inside this one.
    fn set_categories(&mut self, id: i64, ids: &[i64]) -> Result<(), String> {
        match self.kind {
            Kind::Manga => mangas_categories::set_categories(self.tx, id, ids).map_err(text),
            Kind::Anime => {
                self.tx.execute("DELETE FROM entry_categories WHERE entry_id = ?1 AND kind = 'anime'", [id]).map_err(text)?;
                let mut unique = ids.to_vec();
                unique.sort_unstable();
                unique.dedup();
                unique
                    .into_iter()
                    .try_for_each(|category| self.tx.execute("INSERT INTO entry_categories(entry_id, kind, category_id) VALUES (?1, 'anime', ?2)", (id, category)).map(drop))
                    .map_err(text)
            }
        }
    }

    /// Each binding becomes a new row (`id` -1) of the new entry; the old one keeps its own.
    fn move_tracks(&mut self, old: i64, new: i64) -> Result<(), String> {
        match self.kind {
            Kind::Manga => {
                let rows = manga_sync::get_tracks_by_manga_id(self.tx, old).map_err(text)?;
                rows.into_iter().try_for_each(|row| manga_sync::upsert(self.tx, &TrackRow { id: -1, manga_id: new, ..row }).map(drop)).map_err(text)
            }
            Kind::Anime => {
                let rows = sa::tracks_by_anime_id(self.tx, old).map_err(text)?;
                rows.into_iter().try_for_each(|row| sa::upsert_track(self.tx, &sa::AnimeTrack { id: -1, anime_id: new, ..row }).map(drop)).map_err(text)
            }
        }
    }

    fn set_favorite(&mut self, id: i64, favorite: bool, date_added: i64, flags: Option<(i64, i64)>) -> Result<(), String> {
        match self.kind {
            Kind::Manga => mangas::update(self.tx, &manga_patch(id, favorite, date_added, flags)).map(drop).map_err(text),
            Kind::Anime => {
                let update = sa::AnimeUpdate {
                    id,
                    favorite: Some(favorite),
                    episode_flags: flags.map(|(items, _)| items),
                    viewer_flags: flags.map(|(_, viewer)| viewer),
                    date_added: Some(date_added),
                    ..sa::AnimeUpdate::default()
                };
                sa::update_anime(self.tx, &update).map(drop).map_err(text)
            }
        }
    }
}

impl MigrateSteps for LiveSteps {
    fn source_present(&self, source_id: i64) -> bool {
        self.directory.catalogue(source_id).is_some()
    }

    /// The chapters or episodes of the new entry, fetched and stored by its entry page's backend on the app runtime.
    fn fetch_items(&self, new: &Entry) -> StepFuture<Result<(), String>> {
        let Some(handle) = runtime() else {
            return Box::pin(async { Err("the app runtime is not running".to_owned()) });
        };
        match self.kind {
            Kind::Manga => {
                let directory = Arc::clone(&self.directory);
                let lookup = Arc::new(move |id: i64| directory.catalogue(id)?.manga());
                let backend = SourceBackend::new(Arc::clone(&self.db), lookup, crate::misc::diet_defaults::LIBRARY_MARK_DUPLICATE_CHAPTER_READ);
                let task = backend.fetch_chapters(new.id);
                Box::pin(async move {
                    match handle.spawn(task).await {
                        Ok(Ok(())) => Ok(()),
                        Ok(Err(error)) => Err(error.user_message()),
                        Err(error) => Err(error.to_string()),
                    }
                })
            }
            Kind::Anime => {
                let Some(source) = self.directory.catalogue(new.source).and_then(|catalogue| catalogue.anime()) else {
                    return Box::pin(async { Err("the source of the new entry is not installed".to_owned()) });
                };
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
        }
    }

    fn transact(&self, work: TxWork<'_>) -> Result<(), String> {
        self.db
            .write(|tx| work(&mut LiveTx { kind: self.kind, tx }).map_err(DbError::Constraint))
            .map_err(|error| match error {
                DbError::Constraint(message) => message,
                other => other.to_string(),
            })
    }

    fn now_ms(&self) -> i64 {
        super::browse_screen::now_ms()
    }

    /// Removes the old manga's folder. The source folder a manga lives in is not known without a source registry, so the
    /// folder is deleted only when exactly one source folder holds one for this title; two are ambiguous and left alone.
    fn delete_downloads(&self, old: &Entry) -> Result<(), String> {
        let (Kind::Manga, Some(index)) = (self.kind, &self.downloads) else { return Ok(()) };
        let manga_dir = DownloadLayout::manga_dir_name(&old.title);
        let found: Vec<_> = std::fs::read_dir(index.root())
            .map_err(text)?
            .filter_map(Result::ok)
            .map(|source| source.path().join(&manga_dir))
            .filter(|path| path.is_dir())
            .collect();
        match found.as_slice() {
            [] => {}
            [only] => std::fs::remove_dir_all(only).map_err(text)?,
            _ => return Err("more than one source folder holds downloads for this title".to_owned()),
        }
        index.remove_manga(old.source, &old.title);
        Ok(())
    }

    fn has_custom(&self, image: CustomImage, old: &Entry) -> bool {
        self.family(image).is_some_and(|family| LinuxPaths::system().is_ok_and(|paths| custom_file(&paths, family, old.id).is_file()))
    }

    fn copy_custom(&self, image: CustomImage, old: &Entry, new: &Entry) -> Result<(), String> {
        let Some(family) = self.family(image) else { return Ok(()) };
        let paths = LinuxPaths::system().map_err(text)?;
        if kuro_images::cover_file::copy_custom(&paths, family, old.id, new.id) { Ok(()) } else { Err(format!("the custom {image:?} could not be copied")) }
    }
}

#[cfg(test)]
mod tests {
    use kuro_data::anime::Anime;
    use kuro_data::category::{CategoryRepo, create_anime_category};
    use kuro_data::chapter::ChapterRepo;
    use kuro_data::manga::Manga;
    use kuro_data::track::{AnimeTrack, AnimeTrackRepo, MangaTrack, MangaTrackRepo};

    use super::*;
    use crate::browse::directory::{Catalogues, DirectorySource};
    use crate::browse::fake_catalogue::{FakeAnime, FakeCatalogue};
    use crate::browse::loader::Catalogue;
    use crate::browse::migrate_engine::{MigrateError, MigratePlan, migrate};
    use crate::browse::migrate_flags::{CATEGORIES, ITEMS};
    use kuro_domain::source::model::Source;

    fn runtime() -> &'static tokio::runtime::Runtime {
        Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().expect("runtime")))
    }

    fn db(_kind: Kind) -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("library.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("schema");
        (dir, Arc::new(Db::new(&path, writer).expect("db")))
    }

    /// A directory whose sources 1 and 2 are runnable.
    fn directory(sources: Vec<Catalogue>) -> Arc<dyn SourceDirectory> {
        let rows = sources
            .into_iter()
            .map(|source| {
                let row = DirectorySource { source: Source::new(source.id(), "S", "en"), package: String::new(), home_url: None };
                (source, row)
            })
            .collect();
        Arc::new(Catalogues::with_store(rows, Arc::new(|| None)))
    }

    fn add_manga(repo: &MangaRepo, source: i64, url: &str, favorite: bool) -> i64 {
        repo.insert(&Manga { source, url: url.into(), title: format!("t{url}"), favorite, chapter_flags: 5, viewer_flags: 6, date_added: 42, ..Manga::create() }).expect("insert")
    }

    fn add_anime(repo: &AnimeRepo, source: i64, url: &str, favorite: bool) -> i64 {
        repo.insert(&Anime { source, url: url.into(), title: format!("t{url}"), favorite, episode_flags: 5, viewer_flags: 6, date_added: 42, ..Anime::create() }).expect("insert")
    }

    #[test]
    fn a_manga_migration_moves_the_chapter_state_the_categories_the_tracks_and_the_favourite_in_one_go() {
        let rt = runtime();
        let _entered = rt.enter();
        let (_dir, db) = db(Kind::Manga);
        let repo = MangaRepo::new(Arc::clone(&db));
        let (old_id, new_id) = (add_manga(&repo, 1, "/old", true), add_manga(&repo, 2, "/new", false));
        let chapter_repo = ChapterRepo::new(Arc::clone(&db));
        chapter_repo.add_all(&[Chapter { manga_id: old_id, url: "/old1".into(), name: "Chapter 1".into(), chapter_number: 1.0, read: true, bookmark: true, date_fetch: 9, ..Chapter::create() }]).expect("old chapters");
        MangaTrackRepo::new(Arc::clone(&db)).insert(&MangaTrack { manga_id: old_id, remote_id: 77, title: "Tracked".into(), ..MangaTrack::new(1) }).expect("track");
        let category = kuro_data::category::create_manga_category(&db, "Reading", 0).expect("category");
        repo.set_categories(old_id, &[category]).expect("categories");

        let steps = LiveSteps::new(Kind::Manga, Arc::clone(&db), directory(vec![Catalogue::Manga(FakeCatalogue::new(1)), Catalogue::Manga(FakeCatalogue::new(2))]), None, PrefStore::in_memory());
        let (old, new) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
        let flags = (ITEMS | CATEGORIES) as i32;
        let outcome = rt.block_on(migrate(&steps, &old, &new, MigratePlan { replace: true, flags })).expect("migrates");
        assert!(outcome.warnings.is_empty());

        let moved = chapter_repo.by_manga_id(new_id, false).expect("new chapters");
        let first = moved.iter().find(|c| c.chapter_number == 1.0).expect("chapter 1");
        assert!(first.read && first.bookmark && first.date_fetch == 9, "the old state carried over: {first:?}");
        assert_eq!(MangaTrackRepo::new(Arc::clone(&db)).by_entry(new_id).expect("tracks").len(), 1, "the track follows");
        assert_eq!(CategoryRepo::new(Arc::clone(&db), Kind::Manga).by_entry(new_id).expect("categories").len(), 1);
        let (old_after, new_after) = (repo.by_id(old_id).expect("old"), repo.by_id(new_id).expect("new"));
        assert!(!old_after.favorite && new_after.favorite, "Migrate swaps the favourite");
        assert_eq!((new_after.chapter_flags, new_after.viewer_flags, new_after.date_added), (5, 6, 42), "flags copied, date added from the old manga");
    }

    #[test]
    fn a_failed_fetch_of_either_kind_stops_before_any_write() {
        let rt = runtime();
        let _entered = rt.enter();
        // the manga source answers HTTP 503; the anime source lists no episodes
        let failing = FakeCatalogue::new(2);
        failing.fail.store(true, std::sync::atomic::Ordering::SeqCst);
        let kinds = [(Kind::Manga, Catalogue::Manga(failing)), (Kind::Anime, Catalogue::Anime(FakeAnime::new(2, Vec::new())))];
        for (kind, source) in kinds {
            let (_dir, db) = db(kind);
            let (old_id, new_id) = match kind {
                Kind::Manga => {
                    let repo = MangaRepo::new(Arc::clone(&db));
                    (add_manga(&repo, 1, "/old", true), add_manga(&repo, 2, "/new", false))
                }
                Kind::Anime => {
                    let repo = AnimeRepo::new(Arc::clone(&db));
                    (add_anime(&repo, 1, "/old", true), add_anime(&repo, 2, "/new", false))
                }
            };
            let steps = LiveSteps::new(kind, Arc::clone(&db), directory(vec![source]), None, PrefStore::in_memory());
            let (old, new) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
            let result = rt.block_on(migrate(&steps, &old, &new, MigratePlan { replace: true, flags: i32::MAX }));
            assert!(matches!(result, Err(MigrateError::FetchItems(_))), "{kind:?}: {result:?}");
            let (old_after, new_after) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
            assert_eq!((old_after.date_added, new_after.date_added), (42, 42), "{kind:?}: nothing was written");
        }
    }

    #[test]
    fn with_no_installed_source_the_new_source_is_missing() {
        let rt = runtime();
        let _entered = rt.enter();
        let (_dir, db) = db(Kind::Manga);
        let repo = MangaRepo::new(Arc::clone(&db));
        let (old_id, new_id) = (add_manga(&repo, 1, "/old", true), add_manga(&repo, 2, "/new", false));
        let steps = LiveSteps::new(Kind::Manga, db, directory(Vec::new()), None, PrefStore::in_memory());
        let (old, new) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
        assert_eq!(rt.block_on(migrate(&steps, &old, &new, MigratePlan { replace: false, flags: 0 })), Err(MigrateError::NewSourceMissing));
    }

    /// The anime transaction itself: every database step through the trait, committed together.
    #[test]
    fn the_anime_database_steps_move_categories_and_tracks_and_flip_the_favourites_in_one_transaction() {
        let (_dir, db) = db(Kind::Anime);
        let (repo, categories, tracks) = (AnimeRepo::new(Arc::clone(&db)), CategoryRepo::new(Arc::clone(&db), Kind::Anime), AnimeTrackRepo::new(Arc::clone(&db)));
        let category = create_anime_category(&db, "Watching", 0).expect("category");
        let (old_id, new_id) = (add_anime(&repo, 1, "/old", true), add_anime(&repo, 2, "/new", false));
        repo.set_categories(old_id, &[category]).expect("link");
        tracks.insert(&AnimeTrack { anime_id: old_id, tracker_id: 7, remote_id: 99, title: "t".into(), ..AnimeTrack::new(7) }).expect("track");
        let steps = LiveSteps::new(Kind::Anime, Arc::clone(&db), directory(Vec::new()), None, PrefStore::in_memory());
        steps
            .transact(&mut |tx| {
                tx.copy_item_state(old_id, new_id)?;
                let ids = tx.category_ids(old_id)?;
                tx.set_categories(new_id, &ids)?;
                tx.move_tracks(old_id, new_id)?;
                tx.set_favorite(new_id, true, 42, Some((5, 6)))?;
                tx.set_favorite(old_id, false, 0, None)
            })
            .expect("commit");
        assert_eq!(categories.by_entry(new_id).expect("categories").into_iter().map(|c| c.id).collect::<Vec<_>>(), [category]);
        assert_eq!(tracks.by_entry(new_id).expect("tracks").len(), 1, "the binding moved");
        let (old_row, new_row) = (repo.by_id(old_id).expect("old"), repo.by_id(new_id).expect("new"));
        assert!(!old_row.favorite && new_row.favorite);
        assert_eq!((new_row.episode_flags, new_row.viewer_flags, new_row.date_added), (5, 6, 42));
    }

    #[test]
    fn a_failing_step_rolls_every_earlier_write_back() {
        for kind in [Kind::Manga, Kind::Anime] {
            let (_dir, db) = db(kind);
            let steps = LiveSteps::new(kind, Arc::clone(&db), directory(Vec::new()), None, PrefStore::in_memory());
            let (old_id, new_id) = match kind {
                Kind::Manga => {
                    let repo = MangaRepo::new(Arc::clone(&db));
                    (add_manga(&repo, 1, "/old", true), add_manga(&repo, 2, "/new", false))
                }
                Kind::Anime => {
                    let repo = AnimeRepo::new(Arc::clone(&db));
                    (add_anime(&repo, 1, "/old", true), add_anime(&repo, 2, "/new", false))
                }
            };
            let result = steps.transact(&mut |tx| {
                tx.set_favorite(new_id, true, 1, None)?;
                tx.set_favorite(old_id, false, 0, None)?;
                Err("disk full".to_owned())
            });
            assert_eq!(result, Err("disk full".to_owned()));
            let (old, new) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
            assert_eq!((old.date_added, new.date_added), (42, 42), "{kind:?}: both flips rolled back");
        }
    }

    #[test]
    fn the_chapter_state_is_merged_and_only_changed_rows_are_written() {
        let chapter = |id: i64, number: f64, read: bool| Chapter { id, chapter_number: number, read, ..Chapter::create() };
        let current = [chapter(2, 2.0, false), chapter(3, 4.0, false)];
        let merged = merge_chapter_state(&[chapter(1, 3.0, true)], &current);
        let updates = chapter_updates(&current, &merged);
        assert_eq!(updates.len(), 1, "chapter 2 is below the read point, chapter 4 is not");
        assert_eq!((updates[0].id, updates[0].read), (2, Some(true)));
    }

    #[test]
    fn the_seen_rule_and_the_matched_state_carry_over_from_the_old_episodes() {
        let episode = |id: i64, number: f64, seen: bool| Episode { id, episode_number: number, seen, ..Episode::create() };
        let prev = [
            Episode { seen: true, bookmark: true, date_fetch: 500, last_second_seen: 90, fillermark: true, ..episode(1, 1.0, true) },
            episode(2, 2.0, false),
            episode(3, 5.0, true),
        ];
        let current = [episode(11, 1.0, false), episode(12, 2.0, false), episode(13, 4.0, false), episode(14, -1.0, false)];
        let updates = episode_updates(&prev, &current);
        let row = |id: i64| updates.iter().find(|u| u.id == id).expect("row");
        let first = row(11);
        assert_eq!((first.seen, first.bookmark, first.date_fetch), (Some(true), Some(true), Some(500)), "matched by number, seen by max-seen");
        assert_eq!((first.last_second_seen, first.fillermark), (Some(90), Some(true)), "E20: copied with the match");
        assert_eq!(row(12).seen, Some(true), "2.0 is under the max seen 5.0");
        assert_eq!(row(13).seen, Some(true), "4.0 is under 5.0 even with no matching episode");
        let unrecognised = row(14);
        assert_eq!((unrecognised.seen, unrecognised.bookmark, unrecognised.date_fetch), (Some(false), Some(false), Some(0)), "unrecognised numbers are copied unchanged");
        assert_eq!(episode_updates(&[episode(1, 1.0, false)], &[episode(2, 1.0, false)])[0].seen, Some(false), "no max-seen when nothing was seen");
    }

    /// T-40b-085 `migrate_counts_query_orders_by_source`: the migrate tab's count query of `mangas.sq` on an in-memory
    /// database, then through the repository over the real schema. Sources come back in order, favourites only.
    #[test]
    fn migrate_counts_query_orders_by_source() {
        let memory = rusqlite::Connection::open_in_memory().expect("in-memory database");
        memory.execute_batch("CREATE TABLE entries(_id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, source INTEGER NOT NULL, favorite INTEGER NOT NULL);").expect("table");
        for (source, favorite) in [(9, 1), (3, 1), (9, 1), (5, 0), (5, 1), (7, 0)] {
            memory.execute("INSERT INTO entries(kind, source, favorite) VALUES ('manga', ?1, ?2)", (source, favorite)).expect("row");
        }
        assert!(kuro_db::sql::mangas::GET_SOURCE_ID_WITH_FAVORITE_COUNT.contains("ORDER BY source"));
        assert_eq!(kuro_db::sql::mangas::get_source_id_with_favorite_count(&memory).expect("counts"), [(3, 1), (5, 1), (9, 2)], "ordered by source; source 7 has no favourite");

        let (_dir, db) = db(Kind::Manga);
        let repo = MangaRepo::new(db);
        for (source, url, favorite) in [(9, "/a", true), (3, "/b", true), (9, "/c", true), (7, "/d", false)] {
            add_manga(&repo, source, url, favorite);
        }
        assert_eq!(repo.favorite_counts_by_source().expect("counts"), [(3, 1), (9, 2)]);
    }

    #[test]
    fn a_manga_has_no_background_and_reads_as_an_episodes_title() {
        let (_dir, db) = db(Kind::Manga);
        let steps = LiveSteps::new(Kind::Manga, db, directory(Vec::new()), None, PrefStore::in_memory());
        let entry = Entry { id: 1, source: 1, title: "t".into(), item_flags: 0, viewer_flags: 0, date_added: 0 };
        assert!(!steps.has_custom(CustomImage::Background, &entry));
        assert_eq!(steps.copy_custom(CustomImage::Background, &entry, &entry), Ok(()));
        assert_eq!(steps.fetch_type(1), FetchType::Episodes);
        assert!(!steps.has_downloads(&entry), "no download index, no downloads");
    }
}
