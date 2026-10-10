//! The manga migration engine (`migrateMangaInternal` of `MigrateMangaDialog.kt`; T-40a-016, T-40a-041). Pure over the
//! [`MigrateSteps`] seam: the database steps run in one transaction, the file steps after it commits, and a missing new
//! source or a failed chapter fetch stops the run before anything is written.
//!
//! Step order (B section 15): sync the new manga's chapters (an error there is ignored), copy the chapter state (flag 1),
//! the categories (flag 2) and the tracks (always), flip the new manga to a favourite with the old one's flags and then,
//! for a replace, un-favourite the old one (E-40-86). After the commit: delete the old downloads (flag 16, only when the
//! old source is still installed) and copy the custom cover (flag 8, only when the old manga has one).

use std::future::Future;
use std::pin::Pin;

use kuro_data::chapter::{Chapter, ChapterUpdate};
use kuro_data::manga::{Manga, MangaUpdate};
use kuro_data::track::MangaTrack;
use kuro_source::model::SChapter;

use super::migrate_flags::{CATEGORIES, CUSTOM_COVER, DELETE_DOWNLOADED, MANGA_CHAPTERS};
use super::migrate_merge::merge_chapter_state;

/// A boxed future of the engine's seam. The engine runs on the GTK main context, so it need not be `Send`.
pub type StepFuture<T> = Pin<Box<dyn Future<Output = T>>>;

/// What a migrate action asks for: whether the old manga leaves the library (Migrate) or stays (Copy), and the option bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigratePlan {
    pub replace: bool,
    pub flags: i32,
}

impl MigratePlan {
    fn has(self, bit: u32) -> bool {
        (self.flags as u32) & bit != 0
    }
}

/// Why a migration did not run to the end. Nothing was written for the first two.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrateError {
    /// The old and the new manga are the same entry (`same_manga`, E-40-88). Nothing is fetched or written.
    SameManga,
    /// The new manga's source is not installed (E-40-74).
    NewSourceMissing,
    /// The new source could not list the chapters (TC-40-037).
    FetchChapters(String),
    /// A database step failed: the transaction rolled back.
    Step(String),
}

/// What a finished migration reports besides success.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrateOutcome {
    /// File steps that failed after the commit (the downloads delete, the cover copy). The migration still stands.
    pub warnings: Vec<String>,
}

/// The database steps. They all run inside the one transaction `MigrateSteps::transact` opens.
pub trait MigrateTx {
    fn chapters(&mut self, manga_id: i64) -> Result<Vec<Chapter>, String>;
    fn update_chapters(&mut self, updates: &[ChapterUpdate]) -> Result<(), String>;
    fn category_ids(&mut self, manga_id: i64) -> Result<Vec<i64>, String>;
    fn set_categories(&mut self, manga_id: i64, ids: &[i64]) -> Result<(), String>;
    fn tracks(&mut self, manga_id: i64) -> Result<Vec<MangaTrack>, String>;
    fn insert_tracks(&mut self, tracks: &[MangaTrack]) -> Result<(), String>;
    fn update_manga(&mut self, update: &MangaUpdate) -> Result<(), String>;
}

/// The one body of work of a transaction.
pub type TxWork<'a> = &'a mut dyn FnMut(&mut dyn MigrateTx) -> Result<(), String>;

/// What the engine needs of the app.
pub trait MigrateSteps {
    /// Whether the source of this id is installed.
    fn source_present(&self, source_id: i64) -> bool;
    /// The chapter list of the new manga from its source. `None` when the source cannot list chapters.
    fn fetch_chapters(&self, new: &Manga) -> Option<StepFuture<Result<Vec<SChapter>, String>>>;
    /// `syncChaptersWithSource` for the new manga.
    fn sync_chapters(&self, new: &Manga, fetched: Vec<SChapter>) -> StepFuture<Result<(), String>>;
    /// Runs `work` in one transaction: it commits when `work` returns `Ok` and rolls back otherwise.
    fn transact(&self, work: TxWork<'_>) -> Result<(), String>;
    fn now_ms(&self) -> i64;
    fn delete_downloads(&self, old: &Manga) -> Result<(), String>;
    fn old_has_custom_cover(&self, old: &Manga) -> bool;
    fn copy_custom_cover(&self, old: &Manga, new: &Manga) -> Result<(), String>;
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

/// The old manga's tracks, bound to the new manga as rows of their own (enhanced trackers are not re-resolved).
fn moved_tracks(tracks: Vec<MangaTrack>, new_id: i64) -> Vec<MangaTrack> {
    tracks.into_iter().map(|track| MangaTrack { id: -1, manga_id: new_id, ..track }).collect()
}

/// The database half of the migration (everything but the chapter sync).
fn database_steps(tx: &mut dyn MigrateTx, old: &Manga, new: &Manga, plan: MigratePlan, now_ms: i64) -> Result<(), String> {
    if plan.has(MANGA_CHAPTERS) {
        let (prev, current) = (tx.chapters(old.id)?, tx.chapters(new.id)?);
        let updates = chapter_updates(&current, &merge_chapter_state(&prev, &current));
        if !updates.is_empty() {
            tx.update_chapters(&updates)?;
        }
    }
    if plan.has(CATEGORIES) {
        let ids = tx.category_ids(old.id)?;
        tx.set_categories(new.id, &ids)?;
    }
    let tracks = moved_tracks(tx.tracks(old.id)?, new.id);
    if !tracks.is_empty() {
        tx.insert_tracks(&tracks)?;
    }
    // the new manga becomes a favourite before the old one stops being one (E-40-86)
    tx.update_manga(&MangaUpdate {
        favorite: Some(true),
        chapter_flags: Some(old.chapter_flags),
        viewer_flags: Some(old.viewer_flags),
        date_added: Some(if plan.replace { old.date_added } else { now_ms }),
        ..MangaUpdate::new(new.id)
    })?;
    if plan.replace {
        tx.update_manga(&MangaUpdate { favorite: Some(false), ..MangaUpdate::new(old.id) })?;
    }
    Ok(())
}

/// Moves `old` to `new`. A missing new source and a failed chapter fetch return before any write.
pub async fn migrate_manga(steps: &dyn MigrateSteps, old: &Manga, new: &Manga, plan: MigratePlan) -> Result<MigrateOutcome, MigrateError> {
    if old.id == new.id {
        return Err(MigrateError::SameManga);
    }
    if !steps.source_present(new.source) {
        return Err(MigrateError::NewSourceMissing);
    }
    let Some(fetch) = steps.fetch_chapters(new) else { return Err(MigrateError::FetchChapters("the source cannot list chapters".to_owned())) };
    let fetched = fetch.await.map_err(MigrateError::FetchChapters)?;

    if let Err(error) = steps.sync_chapters(new, fetched).await {
        tracing::warn!(%error, "the chapters of the new manga were not synced, the migration goes on without them");
    }
    let now_ms = steps.now_ms();
    steps.transact(&mut |tx| database_steps(tx, old, new, plan, now_ms)).map_err(MigrateError::Step)?;

    let mut warnings = Vec::new();
    if plan.has(DELETE_DOWNLOADED) && steps.source_present(old.source) {
        if let Err(error) = steps.delete_downloads(old) {
            tracing::warn!(%error, "the downloads of the old manga were not deleted");
            warnings.push(error);
        }
    }
    if plan.has(CUSTOM_COVER) && steps.old_has_custom_cover(old) {
        if let Err(error) = steps.copy_custom_cover(old, new) {
            tracing::warn!(%error, "the custom cover was not copied");
            warnings.push(error);
        }
    }
    Ok(MigrateOutcome { warnings })
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;
    use std::sync::Mutex;

    use super::*;

    /// Everything the fake saw, in order.
    type Log = Mutex<Vec<String>>;

    /// A scripted app: which sources exist, what the fetch answers, the old state to copy, and what fails.
    pub(crate) struct Fake {
        pub log: Log,
        pub installed: Vec<i64>,
        pub fetch: Option<Result<Vec<SChapter>, String>>,
        pub sync_fails: bool,
        pub transact_fails: bool,
        pub downloads_fail: bool,
        pub old_has_cover: bool,
        pub old_chapters: Vec<Chapter>,
        pub new_chapters: Vec<Chapter>,
        pub old_categories: Vec<i64>,
        pub old_tracks: Vec<MangaTrack>,
    }

    impl Fake {
        pub(crate) fn new() -> Self {
            Self {
                log: Mutex::default(),
                installed: vec![1, 2],
                fetch: Some(Ok(vec![SChapter::new("/c1", "Chapter 1")])),
                sync_fails: false,
                transact_fails: false,
                downloads_fail: false,
                old_has_cover: true,
                old_chapters: Vec::new(),
                new_chapters: Vec::new(),
                old_categories: vec![5, 6],
                old_tracks: Vec::new(),
            }
        }

        fn note(&self, line: impl Into<String>) {
            self.log.lock().expect("log").push(line.into());
        }

        pub(crate) fn lines(&self) -> Vec<String> {
            self.log.lock().expect("log").clone()
        }
    }

    struct Tx<'a> {
        fake: &'a Fake,
        writes: RefCell<Vec<String>>,
    }

    impl MigrateTx for Tx<'_> {
        fn chapters(&mut self, manga_id: i64) -> Result<Vec<Chapter>, String> {
            Ok(if manga_id == 10 { self.fake.old_chapters.clone() } else { self.fake.new_chapters.clone() })
        }
        fn update_chapters(&mut self, updates: &[ChapterUpdate]) -> Result<(), String> {
            self.writes.borrow_mut().push(format!("chapters {}", updates.len()));
            Ok(())
        }
        fn category_ids(&mut self, _: i64) -> Result<Vec<i64>, String> {
            Ok(self.fake.old_categories.clone())
        }
        fn set_categories(&mut self, manga_id: i64, ids: &[i64]) -> Result<(), String> {
            self.writes.borrow_mut().push(format!("categories {manga_id} {ids:?}"));
            Ok(())
        }
        fn tracks(&mut self, _: i64) -> Result<Vec<MangaTrack>, String> {
            Ok(self.fake.old_tracks.clone())
        }
        fn insert_tracks(&mut self, tracks: &[MangaTrack]) -> Result<(), String> {
            self.writes.borrow_mut().push(format!("tracks {:?}", tracks.iter().map(|t| (t.manga_id, t.id)).collect::<Vec<_>>()));
            Ok(())
        }
        fn update_manga(&mut self, update: &MangaUpdate) -> Result<(), String> {
            self.writes.borrow_mut().push(format!(
                "manga {} favorite={:?} chapter_flags={:?} viewer_flags={:?} date_added={:?}",
                update.id, update.favorite, update.chapter_flags, update.viewer_flags, update.date_added
            ));
            Ok(())
        }
    }

    impl MigrateSteps for Fake {
        fn source_present(&self, source_id: i64) -> bool {
            self.installed.contains(&source_id)
        }
        fn fetch_chapters(&self, _: &Manga) -> Option<StepFuture<Result<Vec<SChapter>, String>>> {
            self.note("fetch");
            let answer = self.fetch.clone()?;
            Some(Box::pin(async move { answer }))
        }
        fn sync_chapters(&self, _: &Manga, _: Vec<SChapter>) -> StepFuture<Result<(), String>> {
            self.note("sync");
            let failed = self.sync_fails;
            Box::pin(async move { if failed { Err("sync failed".to_owned()) } else { Ok(()) } })
        }
        fn transact(&self, work: TxWork<'_>) -> Result<(), String> {
            let mut tx = Tx { fake: self, writes: RefCell::default() };
            let result = work(&mut tx).and_then(|()| if self.transact_fails { Err("disk full".to_owned()) } else { Ok(()) });
            // a failed transaction rolls everything back: only a commit keeps the writes
            if result.is_ok() {
                for write in tx.writes.take() {
                    self.note(format!("write {write}"));
                }
                self.note("commit");
            } else {
                self.note("rollback");
            }
            result
        }
        fn now_ms(&self) -> i64 {
            777
        }
        fn delete_downloads(&self, _: &Manga) -> Result<(), String> {
            self.note("delete downloads");
            if self.downloads_fail { Err("busy".to_owned()) } else { Ok(()) }
        }
        fn old_has_custom_cover(&self, _: &Manga) -> bool {
            self.old_has_cover
        }
        fn copy_custom_cover(&self, _: &Manga, _: &Manga) -> Result<(), String> {
            self.note("copy cover");
            Ok(())
        }
    }

    fn manga(id: i64, source: i64) -> Manga {
        Manga { id, source, chapter_flags: 11, viewer_flags: 22, date_added: 100, ..Manga::create() }
    }

    fn chapter(id: i64, number: f64, read: bool) -> Chapter {
        Chapter { id, chapter_number: number, read, ..Chapter::create() }
    }

    fn run(fake: &Fake, replace: bool, flags: i32) -> Result<MigrateOutcome, MigrateError> {
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        runtime.block_on(migrate_manga(fake, &manga(10, 1), &manga(20, 2), MigratePlan { replace, flags }))
    }

    #[test]
    fn migrating_a_manga_onto_itself_is_rejected_before_anything_is_fetched_or_written() {
        let fake = Fake::new();
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        let same = manga(10, 1);
        assert_eq!(runtime.block_on(migrate_manga(&fake, &same, &same, MigratePlan { replace: true, flags: i32::MAX })), Err(MigrateError::SameManga), "E-40-88");
        assert!(fake.lines().is_empty(), "no fetch, no sync, no write: {:?}", fake.lines());
    }

    #[test]
    fn a_missing_new_source_returns_before_anything_is_fetched_or_written() {
        let fake = Fake { installed: vec![1], ..Fake::new() };
        assert_eq!(run(&fake, true, i32::MAX), Err(MigrateError::NewSourceMissing), "TC-40-036");
        assert!(fake.lines().is_empty(), "no fetch, no sync, no write: {:?}", fake.lines());
    }

    #[test]
    fn a_failed_chapter_fetch_aborts_before_any_write() {
        let fake = Fake { fetch: Some(Err("offline".to_owned())), ..Fake::new() };
        assert_eq!(run(&fake, true, i32::MAX), Err(MigrateError::FetchChapters("offline".to_owned())), "TC-40-037");
        assert_eq!(fake.lines(), ["fetch"], "nothing was synced or written");
        let cannot = Fake { fetch: None, ..Fake::new() };
        assert!(matches!(run(&cannot, true, i32::MAX), Err(MigrateError::FetchChapters(_))), "a source that cannot list chapters stops too");
    }

    #[test]
    fn the_steps_run_in_the_documented_order_with_the_favourite_flip_before_the_unfavourite() {
        let fake = Fake {
            old_chapters: vec![chapter(1, 3.0, true)],
            new_chapters: vec![chapter(2, 2.0, false), chapter(3, 4.0, false)],
            old_tracks: vec![MangaTrack { manga_id: 10, id: 9, ..MangaTrack::new(1) }],
            ..Fake::new()
        };
        assert_eq!(run(&fake, true, i32::MAX), Ok(MigrateOutcome::default()));
        assert_eq!(
            fake.lines(),
            [
                "fetch",
                "sync",
                "write chapters 1",
                "write categories 20 [5, 6]",
                "write tracks [(20, -1)]",
                "write manga 20 favorite=Some(true) chapter_flags=Some(11) viewer_flags=Some(22) date_added=Some(100)",
                "write manga 10 favorite=Some(false) chapter_flags=None viewer_flags=None date_added=None",
                "commit",
                "delete downloads",
                "copy cover",
            ],
            "T-40a-041: one transaction, then the file steps"
        );
    }

    #[test]
    fn copy_keeps_the_old_manga_and_stamps_the_new_one_with_now() {
        let fake = Fake::new();
        run(&fake, false, MANGA_CHAPTERS as i32).expect("copy");
        let lines = fake.lines();
        assert!(lines.contains(&"write manga 20 favorite=Some(true) chapter_flags=Some(11) viewer_flags=Some(22) date_added=Some(777)".to_owned()), "{lines:?}");
        assert!(!lines.iter().any(|l| l.starts_with("write manga 10")), "the old manga stays a favourite");
        assert!(!lines.iter().any(|l| l.starts_with("write categories")), "no categories without flag 2");
        assert!(!lines.contains(&"delete downloads".to_owned()) && !lines.contains(&"copy cover".to_owned()), "no file steps without their flags");
    }

    #[test]
    fn a_failed_transaction_writes_nothing_and_runs_no_file_step() {
        let fake = Fake { transact_fails: true, ..Fake::new() };
        assert_eq!(run(&fake, true, i32::MAX), Err(MigrateError::Step("disk full".to_owned())));
        assert_eq!(fake.lines(), ["fetch", "sync", "rollback"], "E-40-85: all or nothing, and the files are untouched");
    }

    #[test]
    fn a_chapter_sync_error_is_ignored() {
        let fake = Fake { sync_fails: true, ..Fake::new() };
        assert!(run(&fake, false, i32::MAX).is_ok(), "worst case the chapters are not synced");
        assert!(fake.lines().contains(&"commit".to_owned()));
    }

    #[test]
    fn a_failing_file_step_is_a_warning_after_the_commit() {
        let fake = Fake { downloads_fail: true, ..Fake::new() };
        let outcome = run(&fake, true, i32::MAX).expect("the migration stands");
        assert_eq!(outcome.warnings, ["busy"]);
        assert!(fake.lines().contains(&"copy cover".to_owned()), "the next file step still runs");
    }

    #[test]
    fn downloads_are_deleted_only_while_the_old_source_is_installed_and_the_cover_only_when_there_is_one() {
        let no_old_source = Fake { installed: vec![2], ..Fake::new() };
        run(&no_old_source, true, i32::MAX).expect("migrates");
        assert!(!no_old_source.lines().contains(&"delete downloads".to_owned()), "no old source, no delete");
        let no_cover = Fake { old_has_cover: false, ..Fake::new() };
        run(&no_cover, true, i32::MAX).expect("migrates");
        assert!(!no_cover.lines().contains(&"copy cover".to_owned()), "flag 8 needs a custom cover on the old manga");
    }

    #[test]
    fn the_chapter_state_is_merged_and_only_changed_rows_are_written() {
        let current = [chapter(2, 2.0, false), chapter(3, 4.0, false)];
        let merged = merge_chapter_state(&[chapter(1, 3.0, true)], &current);
        let updates = chapter_updates(&current, &merged);
        assert_eq!(updates.len(), 1, "chapter 2 is below the read point, chapter 4 is not");
        assert_eq!((updates[0].id, updates[0].read), (2, Some(true)));
    }
}
