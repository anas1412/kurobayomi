//! The app side of the manga migration engine (T-40a-016, T-40a-041): the steps of `MigrateSteps` over the manga database,
//! the source directory, the download index and the cover files. The database steps share one transaction; the file steps
//! run after it commits.

use std::sync::Arc;

use kuro_data::chapter::{Chapter, ChapterUpdate, map_chapter};
use kuro_data::manga::{Manga, MangaUpdate};
use kuro_data::track::MangaTrack;
use kuro_db::statements_manga::{ChapterPatch, MangaPatch, TrackRow};
use kuro_db::sql::{categories, chapters, manga_sync, mangas, mangas_categories};
use kuro_db::{Db, DbError, Transaction};
use kuro_domain::items::chapter_sync::{ChapterSyncOptions, NoRenames, SyncChapters};
use kuro_domain::items::source_sync::MangaSyncLocks;
use kuro_downloads::index::DownloadIndex;
use kuro_downloads::layout::DownloadLayout;
use kuro_images::cover_paths::{Family, custom_file};
use kuro_platform::linux::LinuxPaths;
use kuro_source::model::SChapter;

use super::directory::SourceDirectory;
use super::migrate_engine::{MigrateSteps, MigrateTx, StepFuture, TxWork};

pub struct LiveSteps {
    db: Arc<Db>,
    directory: Option<Arc<dyn SourceDirectory>>,
    downloads: Option<DownloadIndex>,
    locks: Arc<MangaSyncLocks>,
}

impl LiveSteps {
    pub fn new(db: Arc<Db>, directory: Option<Arc<dyn SourceDirectory>>, downloads: Option<DownloadIndex>) -> Self {
        Self { db, directory, downloads, locks: Arc::new(MangaSyncLocks::default()) }
    }

    /// Whether the old manga has downloaded chapters on disk (the dialog offers Delete downloaded only then).
    pub fn has_downloads(&self, manga: &Manga) -> bool {
        self.downloads.as_ref().is_some_and(|index| index.chapter_count_manga(manga.source, &manga.title) > 0)
    }

    /// Whether the manga has a custom cover file.
    pub fn has_custom_cover(manga: &Manga) -> bool {
        LinuxPaths::system().is_ok_and(|paths| custom_file(&paths, Family::MangaCover, manga.id).is_file())
    }
}

fn track_of(row: TrackRow) -> MangaTrack {
    MangaTrack {
        id: row.id,
        manga_id: row.manga_id,
        tracker_id: row.sync_id,
        remote_id: row.remote_id,
        library_id: row.library_id,
        title: row.title,
        last_chapter_read: row.last_chapter_read,
        total_chapters: row.total_chapters,
        status: row.status,
        score: row.score,
        remote_url: row.remote_url,
        start_date: row.start_date,
        finish_date: row.finish_date,
        private: row.private,
    }
}

fn track_row(track: &MangaTrack) -> TrackRow {
    TrackRow {
        id: track.id,
        manga_id: track.manga_id,
        sync_id: track.tracker_id,
        remote_id: track.remote_id,
        library_id: track.library_id,
        title: track.title.clone(),
        last_chapter_read: track.last_chapter_read,
        total_chapters: track.total_chapters,
        status: track.status,
        score: track.score,
        remote_url: track.remote_url.clone(),
        start_date: track.start_date,
        finish_date: track.finish_date,
        private: track.private,
    }
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

/// The columns a migration writes on a manga: the favourite flag, the two flag words and the date added.
fn manga_patch(update: &MangaUpdate) -> MangaPatch {
    MangaPatch {
        id: update.id,
        source: None,
        url: None,
        artist: None,
        author: None,
        description: None,
        genre: None,
        title: None,
        status: None,
        thumbnail_url: None,
        favorite: update.favorite,
        last_update: None,
        next_update: None,
        initialized: None,
        viewer: update.viewer_flags,
        chapter_flags: update.chapter_flags,
        cover_last_modified: None,
        date_added: update.date_added,
        update_strategy: None,
        calculate_interval: None,
        version: None,
        is_syncing: None,
    }
}

struct LiveTx<'a, 'c>(&'a Transaction<'c>);

impl MigrateTx for LiveTx<'_, '_> {
    fn chapters(&mut self, manga_id: i64) -> Result<Vec<Chapter>, String> {
        chapters::get_by_manga_id(self.0, manga_id, false).map(|rows| rows.iter().map(map_chapter).collect()).map_err(|e| e.to_string())
    }
    fn update_chapters(&mut self, updates: &[ChapterUpdate]) -> Result<(), String> {
        updates.iter().try_for_each(|update| chapters::update(self.0, &chapter_patch(update)).map(drop)).map_err(|e| e.to_string())
    }
    fn category_ids(&mut self, manga_id: i64) -> Result<Vec<i64>, String> {
        categories::get_categories_by_manga_id(self.0, manga_id).map(|rows| rows.into_iter().map(|row| row.id).collect()).map_err(|e| e.to_string())
    }
    fn set_categories(&mut self, manga_id: i64, ids: &[i64]) -> Result<(), String> {
        mangas_categories::set_categories(self.0, manga_id, ids).map_err(|e| e.to_string())
    }
    fn tracks(&mut self, manga_id: i64) -> Result<Vec<MangaTrack>, String> {
        manga_sync::get_tracks_by_manga_id(self.0, manga_id).map(|rows| rows.into_iter().map(track_of).collect()).map_err(|e| e.to_string())
    }
    fn insert_tracks(&mut self, tracks: &[MangaTrack]) -> Result<(), String> {
        tracks.iter().try_for_each(|track| manga_sync::upsert(self.0, &track_row(track)).map(drop)).map_err(|e| e.to_string())
    }
    fn update_manga(&mut self, update: &MangaUpdate) -> Result<(), String> {
        mangas::update(self.0, &manga_patch(update)).map(drop).map_err(|e| e.to_string())
    }
}

impl MigrateSteps for LiveSteps {
    fn source_present(&self, source_id: i64) -> bool {
        self.directory.as_ref().is_some_and(|directory| directory.loader(source_id).is_some())
    }

    fn fetch_chapters(&self, new: &Manga) -> Option<StepFuture<Result<Vec<SChapter>, String>>> {
        let fetch = self.directory.as_ref()?.fetch_chapters(new.source, new)?;
        Some(fetch)
    }

    fn sync_chapters(&self, new: &Manga, fetched: Vec<SChapter>) -> StepFuture<Result<(), String>> {
        let sync = SyncChapters::new(Arc::clone(&self.db), Arc::clone(&self.locks), Arc::new(NoRenames));
        let (manga, now_ms) = (new.clone(), self.now_ms());
        Box::pin(async move {
            let options = ChapterSyncOptions { now_ms, manual: true, window: (0, 0), mark_duplicate_as_read: false, is_local: kuro_source::local::LOCAL_SOURCE_ID == manga.source };
            sync.run(fetched, &manga, options, &|_| {}).await.map(drop).map_err(|e| e.to_string())
        })
    }

    fn transact(&self, work: TxWork<'_>) -> Result<(), String> {
        self.db
            .write(|tx| work(&mut LiveTx(tx)).map_err(DbError::Constraint))
            .map_err(|error| match error {
                DbError::Constraint(message) => message,
                other => other.to_string(),
            })
    }

    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
    }

    /// Removes the old manga's folder. The source folder a manga lives in is not known without a source registry, so the
    /// folder is deleted only when exactly one source folder holds one for this title; two are ambiguous and left alone.
    fn delete_downloads(&self, old: &Manga) -> Result<(), String> {
        let Some(index) = &self.downloads else { return Ok(()) };
        let manga_dir = DownloadLayout::manga_dir_name(&old.title);
        let found: Vec<_> = std::fs::read_dir(index.root())
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .map(|source| source.path().join(&manga_dir))
            .filter(|path| path.is_dir())
            .collect();
        match found.as_slice() {
            [] => {}
            [only] => std::fs::remove_dir_all(only).map_err(|e| e.to_string())?,
            _ => return Err("more than one source folder holds downloads for this title".to_owned()),
        }
        index.remove_manga(old.source, &old.title);
        Ok(())
    }

    fn old_has_custom_cover(&self, old: &Manga) -> bool {
        Self::has_custom_cover(old)
    }

    fn copy_custom_cover(&self, old: &Manga, new: &Manga) -> Result<(), String> {
        let paths = LinuxPaths::system().map_err(|e| e.to_string())?;
        if kuro_images::cover_file::copy_custom(&paths, Family::MangaCover, old.id, new.id) {
            Ok(())
        } else {
            Err("the custom cover could not be copied".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use kuro_data::chapter::ChapterRepo;
    use kuro_data::manga::MangaRepo;
    use kuro_data::track::MangaTrackRepo;

    use super::*;
    use crate::browse::migrate_engine::{MigrateError, MigratePlan, migrate_manga};
    use crate::browse::migrate_flags::{CATEGORIES, MANGA_CHAPTERS};

    fn db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("schema");
        (dir, Arc::new(Db::new(&path, writer).expect("db")))
    }

    fn add_manga(repo: &MangaRepo, source: i64, url: &str, favorite: bool) -> Manga {
        let id = repo.insert(&Manga { source, url: url.into(), title: format!("t{url}"), favorite, chapter_flags: 5, viewer_flags: 6, date_added: 42, ..Manga::create() }).expect("insert");
        repo.by_id(id).expect("by id")
    }

    /// A directory whose source 2 lists one chapter; source 1 is installed too.
    struct Dir;

    impl SourceDirectory for Dir {
        fn sources(&self) -> Vec<crate::browse::directory::DirectorySource> {
            Vec::new()
        }
        fn loader(&self, source_id: i64) -> Option<Arc<dyn crate::browse::listing::PageLoader>> {
            struct Never;
            impl crate::browse::listing::PageLoader for Never {
                fn load(&self, _: kuro_domain::source::browse::BrowseQuery, _: kuro_source::filter_list::FilterList, _: u32) -> crate::browse::listing::LoadFuture {
                    Box::pin(async { Err("unused".to_owned()) })
                }
            }
            [1, 2].contains(&source_id).then(|| Arc::new(Never) as Arc<dyn crate::browse::listing::PageLoader>)
        }
        fn filters(&self, _: i64) -> Option<kuro_source::filter_list::FilterList> {
            None
        }
        fn fetch_chapters(&self, _: i64, _: &Manga) -> Option<crate::browse::directory::ChaptersFuture> {
            Some(Box::pin(async { Ok(vec![SChapter { chapter_number: 1.0, ..SChapter::new("/new1", "Chapter 1") }, SChapter { chapter_number: 2.0, ..SChapter::new("/new2", "Chapter 2") }]) }))
        }
    }

    #[test]
    fn a_migration_moves_the_chapter_state_the_categories_the_tracks_and_the_favourite_in_one_go() {
        let (_dir, db) = db();
        let repo = MangaRepo::new(Arc::clone(&db));
        let (old, new) = (add_manga(&repo, 1, "/old", true), add_manga(&repo, 2, "/new", false));
        let chapter_repo = ChapterRepo::new(Arc::clone(&db));
        chapter_repo.add_all(&[Chapter { manga_id: old.id, url: "/old1".into(), name: "Chapter 1".into(), chapter_number: 1.0, read: true, bookmark: true, date_fetch: 9, ..Chapter::create() }]).expect("old chapters");
        let track = MangaTrack { manga_id: old.id, remote_id: 77, title: "Tracked".into(), ..MangaTrack::new(1) };
        MangaTrackRepo::new(Arc::clone(&db)).insert(&track).expect("track");
        let category = kuro_data::category::create_manga_category(&db, "Reading", 0).expect("category");
        repo.set_categories(old.id, &[category]).expect("categories");

        let steps = LiveSteps::new(Arc::clone(&db), Some(Arc::new(Dir)), None);
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        let flags = (MANGA_CHAPTERS | CATEGORIES) as i32;
        let outcome = runtime.block_on(migrate_manga(&steps, &old, &new, MigratePlan { replace: true, flags })).expect("migrates");
        assert!(outcome.warnings.is_empty());

        let moved = chapter_repo.by_manga_id(new.id, false).expect("new chapters");
        let first = moved.iter().find(|c| c.chapter_number == 1.0).expect("chapter 1");
        assert!(first.read && first.bookmark && first.date_fetch == 9, "the old state carried over: {first:?}");
        assert_eq!(MangaTrackRepo::new(Arc::clone(&db)).by_entry(new.id).expect("tracks").len(), 1, "the track follows");
        assert_eq!(kuro_data::category::CategoryRepo::new(Arc::clone(&db), kuro_data::category::CategoryKind::Manga).by_entry(new.id).expect("categories").len(), 1);
        let (old_after, new_after) = (repo.by_id(old.id).expect("old"), repo.by_id(new.id).expect("new"));
        assert!(!old_after.favorite && new_after.favorite, "Migrate swaps the favourite");
        assert_eq!((new_after.chapter_flags, new_after.viewer_flags, new_after.date_added), (5, 6, 42), "flags copied, date added from the old manga");
    }

    #[test]
    fn a_directory_that_cannot_list_chapters_stops_before_any_write() {
        struct Quiet;
        impl SourceDirectory for Quiet {
            fn sources(&self) -> Vec<crate::browse::directory::DirectorySource> {
                Vec::new()
            }
            fn loader(&self, source_id: i64) -> Option<Arc<dyn crate::browse::listing::PageLoader>> {
                Dir.loader(source_id)
            }
            fn filters(&self, _: i64) -> Option<kuro_source::filter_list::FilterList> {
                None
            }
        }
        let (_dir, db) = db();
        let repo = MangaRepo::new(Arc::clone(&db));
        let (old, new) = (add_manga(&repo, 1, "/old", true), add_manga(&repo, 2, "/new", false));
        let steps = LiveSteps::new(Arc::clone(&db), Some(Arc::new(Quiet)), None);
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        let result = runtime.block_on(migrate_manga(&steps, &old, &new, MigratePlan { replace: true, flags: i32::MAX }));
        assert!(matches!(result, Err(MigrateError::FetchChapters(_))));
        assert!(repo.by_id(old.id).expect("old").favorite && !repo.by_id(new.id).expect("new").favorite, "nothing was written");
    }

    /// T-40b-085 `migrate_counts_query_orders_by_source`: the migrate tab's count query of `mangas.sq` on an in-memory
    /// database, then through the repository over the real schema. Sources come back in order, favourites only.
    #[test]
    fn migrate_counts_query_orders_by_source() {
        let memory = rusqlite::Connection::open_in_memory().expect("in-memory database");
        memory.execute_batch("CREATE TABLE mangas(_id INTEGER PRIMARY KEY AUTOINCREMENT, source INTEGER NOT NULL, favorite INTEGER NOT NULL);").expect("table");
        for (source, favorite) in [(9, 1), (3, 1), (9, 1), (5, 0), (5, 1), (7, 0)] {
            memory.execute("INSERT INTO mangas(source, favorite) VALUES (?1, ?2)", (source, favorite)).expect("row");
        }
        assert!(kuro_db::sql::mangas::GET_SOURCE_ID_WITH_FAVORITE_COUNT.contains("ORDER BY source"));
        assert_eq!(kuro_db::sql::mangas::get_source_id_with_favorite_count(&memory).expect("counts"), [(3, 1), (5, 1), (9, 2)], "ordered by source; source 7 has no favourite");

        let (_dir, db) = db();
        let repo = MangaRepo::new(db);
        for (source, url, favorite) in [(9, "/a", true), (3, "/b", true), (9, "/c", true), (7, "/d", false)] {
            add_manga(&repo, source, url, favorite);
        }
        assert_eq!(repo.favorite_counts_by_source().expect("counts"), [(3, 1), (9, 2)]);
    }

    #[test]
    fn with_no_directory_the_new_source_is_missing() {
        let (_dir, db) = db();
        let repo = MangaRepo::new(Arc::clone(&db));
        let (old, new) = (add_manga(&repo, 1, "/old", true), add_manga(&repo, 2, "/new", false));
        let steps = LiveSteps::new(db, None, None);
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        assert_eq!(runtime.block_on(migrate_manga(&steps, &old, &new, MigratePlan { replace: false, flags: 0 })), Err(MigrateError::NewSourceMissing));
    }
}
