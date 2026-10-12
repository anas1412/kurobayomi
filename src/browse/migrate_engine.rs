//! The migration engine of both kinds (`migrateMangaInternal` of `MigrateMangaDialog.kt`, `migrateAnimeInternal` of
//! `MigrateAnimeDialog.kt`; T-40a-016, T-40a-041, T-41a-034). Pure over the [`MigrateSteps`] seam.
//!
//! Step order (B section 15, behavior 12): the new entry's chapters or episodes are fetched and stored first, outside the
//! transaction, so a missing new source or a failed fetch stops the run before anything is written. Then, in one
//! transaction: the item state (flag 1), the categories (flag 2) and the tracks (always), and the new entry becomes a
//! favourite with the old one's flags before, for a replace, the old one stops being one (E-40-86). After the commit:
//! delete the old downloads (flag 16, only while the old source is installed) and copy the custom cover (flag 8) and the
//! custom background (flag 4, anime only), each only when the old entry has one. A file step that fails after the commit
//! is a warning; the migration stands.

use std::future::Future;
use std::pin::Pin;

use super::migrate_flags::{CATEGORIES, CUSTOM_BACKGROUND, CUSTOM_COVER, DELETE_DOWNLOADED, ITEMS};

/// A boxed future of the engine's seam. The engine runs on the GTK main context, so it need not be `Send`.
pub type StepFuture<T> = Pin<Box<dyn Future<Output = T>>>;

/// What a migrate action asks for: whether the old entry leaves the library (Migrate) or stays (Copy), and the option bits.
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

/// What the engine reads of the old or the new library entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub source: i64,
    pub title: String,
    /// The chapter or episode flags, carried over to the new entry.
    pub item_flags: i64,
    pub viewer_flags: i64,
    pub date_added: i64,
}

/// Why a migration did not run to the end. Nothing was written for the first three.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrateError {
    /// The old and the new entry are the same (`same_manga`, E-40-88). Nothing is fetched or written.
    SameEntry,
    /// The new entry's source is not installed (E-40-74).
    NewSourceMissing,
    /// The new entry's chapters or episodes could not be fetched (TC-40-037).
    FetchItems(String),
    /// A database step failed: the transaction rolled back.
    Step(String),
}

impl std::fmt::Display for MigrateError {
    /// The reason the dialog shows after "Migration failed: ".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrateError::SameEntry => f.write_str("the old and the new entry are the same"),
            MigrateError::NewSourceMissing => f.write_str("the source of the new entry is not installed"),
            MigrateError::FetchItems(reason) => write!(f, "the chapters or episodes could not be fetched ({reason})"),
            MigrateError::Step(reason) => f.write_str(reason),
        }
    }
}

/// What a finished migration reports besides success.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrateOutcome {
    /// File steps that failed after the commit (the downloads delete, an image copy). The migration still stands.
    pub warnings: Vec<String>,
}

/// A custom image of an entry that a migration copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomImage {
    Cover,
    /// Anime only.
    Background,
}

/// The database steps. They all run inside the one transaction `MigrateSteps::transact` opens.
pub trait MigrateTx {
    /// Carries the old entry's chapter or episode state over to the new entry's items with the same number.
    fn copy_item_state(&mut self, old: i64, new: i64) -> Result<(), String>;
    fn category_ids(&mut self, id: i64) -> Result<Vec<i64>, String>;
    fn set_categories(&mut self, id: i64, ids: &[i64]) -> Result<(), String>;
    /// Binds the old entry's tracks to the new entry as rows of their own. Enhanced trackers are not re-resolved: the
    /// tracker page repairs a binding on its next refresh.
    fn move_tracks(&mut self, old: i64, new: i64) -> Result<(), String>;
    /// Writes the favourite flag and the date added, and the item and viewer flags when given.
    fn set_favorite(&mut self, id: i64, favorite: bool, date_added: i64, flags: Option<(i64, i64)>) -> Result<(), String>;
}

/// The one body of work of a transaction.
pub type TxWork<'a> = &'a mut dyn FnMut(&mut dyn MigrateTx) -> Result<(), String>;

/// What the engine needs of the app.
pub trait MigrateSteps {
    /// Whether the source of this id is installed.
    fn source_present(&self, source_id: i64) -> bool;
    /// Fetches the new entry's chapters or episodes from its source and stores them.
    fn fetch_items(&self, new: &Entry) -> StepFuture<Result<(), String>>;
    /// Runs `work` in one transaction: it commits when `work` returns `Ok` and rolls back otherwise.
    fn transact(&self, work: TxWork<'_>) -> Result<(), String>;
    fn now_ms(&self) -> i64;
    fn delete_downloads(&self, old: &Entry) -> Result<(), String>;
    fn has_custom(&self, image: CustomImage, old: &Entry) -> bool;
    fn copy_custom(&self, image: CustomImage, old: &Entry, new: &Entry) -> Result<(), String>;
}

/// The database half of the migration.
fn database_steps(tx: &mut dyn MigrateTx, old: &Entry, new: &Entry, plan: MigratePlan, now_ms: i64) -> Result<(), String> {
    if plan.has(ITEMS) {
        tx.copy_item_state(old.id, new.id)?;
    }
    if plan.has(CATEGORIES) {
        let ids = tx.category_ids(old.id)?;
        tx.set_categories(new.id, &ids)?;
    }
    // the tracks move whatever the flags say
    tx.move_tracks(old.id, new.id)?;
    // the new entry becomes a favourite before the old one stops being one (E-40-86)
    let added = if plan.replace { old.date_added } else { now_ms };
    tx.set_favorite(new.id, true, added, Some((old.item_flags, old.viewer_flags)))?;
    if plan.replace {
        tx.set_favorite(old.id, false, 0, None)?;
    }
    Ok(())
}

/// Moves `old` to `new`. A missing new source and a failed fetch return before any write.
pub async fn migrate(steps: &dyn MigrateSteps, old: &Entry, new: &Entry, plan: MigratePlan) -> Result<MigrateOutcome, MigrateError> {
    if old.id == new.id {
        return Err(MigrateError::SameEntry);
    }
    if !steps.source_present(new.source) {
        return Err(MigrateError::NewSourceMissing);
    }
    steps.fetch_items(new).await.map_err(MigrateError::FetchItems)?;
    let now_ms = steps.now_ms();
    steps.transact(&mut |tx| database_steps(tx, old, new, plan, now_ms)).map_err(MigrateError::Step)?;

    let mut warnings = Vec::new();
    if plan.has(DELETE_DOWNLOADED) && steps.source_present(old.source) {
        if let Err(error) = steps.delete_downloads(old) {
            tracing::warn!(%error, "the downloads of the old entry were not deleted");
            warnings.push(error);
        }
    }
    for (bit, image) in [(CUSTOM_COVER, CustomImage::Cover), (CUSTOM_BACKGROUND, CustomImage::Background)] {
        if plan.has(bit) && steps.has_custom(image, old) {
            if let Err(error) = steps.copy_custom(image, old, new) {
                tracing::warn!(%error, ?image, "a custom image was not copied");
                warnings.push(error);
            }
        }
    }
    Ok(MigrateOutcome { warnings })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// A scripted app: which sources exist, whether the fetch fails, which images the old entry has, and what fails.
    struct Fake {
        log: RefCell<Vec<String>>,
        installed: Vec<i64>,
        fetch_fails: Option<String>,
        transact_fails: bool,
        downloads_fail: bool,
        old_cover: bool,
        old_background: bool,
    }

    impl Fake {
        fn new() -> Self {
            Self { log: RefCell::default(), installed: vec![1, 2], fetch_fails: None, transact_fails: false, downloads_fail: false, old_cover: true, old_background: true }
        }

        fn note(&self, line: impl Into<String>) {
            self.log.borrow_mut().push(line.into());
        }

        fn lines(&self) -> Vec<String> {
            self.log.borrow().clone()
        }
    }

    /// The writes of one transaction; they reach the log only when it commits.
    struct Tx {
        writes: Vec<String>,
    }

    impl MigrateTx for Tx {
        fn copy_item_state(&mut self, old: i64, new: i64) -> Result<(), String> {
            self.writes.push(format!("items {old} {new}"));
            Ok(())
        }
        fn category_ids(&mut self, _: i64) -> Result<Vec<i64>, String> {
            Ok(vec![5, 6])
        }
        fn set_categories(&mut self, id: i64, ids: &[i64]) -> Result<(), String> {
            self.writes.push(format!("categories {id} {ids:?}"));
            Ok(())
        }
        fn move_tracks(&mut self, old: i64, new: i64) -> Result<(), String> {
            self.writes.push(format!("tracks {old} {new}"));
            Ok(())
        }
        fn set_favorite(&mut self, id: i64, favorite: bool, date_added: i64, flags: Option<(i64, i64)>) -> Result<(), String> {
            self.writes.push(format!("favorite {id} {favorite} {date_added} {flags:?}"));
            Ok(())
        }
    }

    impl MigrateSteps for Fake {
        fn source_present(&self, source_id: i64) -> bool {
            self.installed.contains(&source_id)
        }
        fn fetch_items(&self, _: &Entry) -> StepFuture<Result<(), String>> {
            self.note("fetch");
            let answer = self.fetch_fails.clone().map_or(Ok(()), Err);
            Box::pin(async move { answer })
        }
        fn transact(&self, work: TxWork<'_>) -> Result<(), String> {
            let mut tx = Tx { writes: Vec::new() };
            let result = work(&mut tx).and_then(|()| if self.transact_fails { Err("disk full".to_owned()) } else { Ok(()) });
            // a failed transaction rolls everything back: only a commit keeps the writes
            if result.is_ok() {
                for write in tx.writes {
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
        fn delete_downloads(&self, _: &Entry) -> Result<(), String> {
            self.note("delete downloads");
            if self.downloads_fail { Err("busy".to_owned()) } else { Ok(()) }
        }
        fn has_custom(&self, image: CustomImage, _: &Entry) -> bool {
            match image {
                CustomImage::Cover => self.old_cover,
                CustomImage::Background => self.old_background,
            }
        }
        fn copy_custom(&self, image: CustomImage, _: &Entry, _: &Entry) -> Result<(), String> {
            self.note(format!("copy {image:?}"));
            Ok(())
        }
    }

    fn entry(id: i64, source: i64) -> Entry {
        Entry { id, source, title: format!("t{id}"), item_flags: 11, viewer_flags: 22, date_added: 100 }
    }

    fn run(fake: &Fake, replace: bool, flags: i32) -> Result<MigrateOutcome, MigrateError> {
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        runtime.block_on(migrate(fake, &entry(10, 1), &entry(20, 2), MigratePlan { replace, flags }))
    }

    #[test]
    fn migrating_an_entry_onto_itself_is_rejected_before_anything_is_fetched_or_written() {
        let fake = Fake::new();
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        let same = entry(10, 1);
        assert_eq!(runtime.block_on(migrate(&fake, &same, &same, MigratePlan { replace: true, flags: i32::MAX })), Err(MigrateError::SameEntry), "E-40-88");
        assert!(fake.lines().is_empty(), "no fetch, no write: {:?}", fake.lines());
    }

    #[test]
    fn a_missing_new_source_returns_before_anything_is_fetched_or_written() {
        let fake = Fake { installed: vec![1], ..Fake::new() };
        assert_eq!(run(&fake, true, i32::MAX), Err(MigrateError::NewSourceMissing), "TC-40-036");
        assert!(fake.lines().is_empty(), "{:?}", fake.lines());
    }

    #[test]
    fn a_failed_fetch_stops_before_any_write() {
        let fake = Fake { fetch_fails: Some("offline".to_owned()), ..Fake::new() };
        assert_eq!(run(&fake, true, i32::MAX), Err(MigrateError::FetchItems("offline".to_owned())), "TC-40-037");
        assert_eq!(fake.lines(), ["fetch"], "nothing was written");
    }

    #[test]
    fn each_error_has_the_reason_the_dialog_shows() {
        assert_eq!(MigrateError::NewSourceMissing.to_string(), "the source of the new entry is not installed");
        assert_eq!(MigrateError::FetchItems("timeout".into()).to_string(), "the chapters or episodes could not be fetched (timeout)");
        assert_eq!(MigrateError::Step("disk full".into()).to_string(), "disk full");
    }

    #[test]
    fn the_steps_run_in_order_with_the_favourite_flip_before_the_unfavourite_and_the_file_steps_after_the_commit() {
        let fake = Fake::new();
        assert_eq!(run(&fake, true, i32::MAX), Ok(MigrateOutcome::default()));
        assert_eq!(
            fake.lines(),
            [
                "fetch",
                "write items 10 20",
                "write categories 20 [5, 6]",
                "write tracks 10 20",
                "write favorite 20 true 100 Some((11, 22))",
                "write favorite 10 false 0 None",
                "commit",
                "delete downloads",
                "copy Cover",
                "copy Background",
            ],
            "T-40a-041, behavior 12: one transaction, then the file steps"
        );
    }

    #[test]
    fn zero_flags_still_fetch_move_the_tracks_and_flip_the_favourite_and_copy_stamps_now() {
        let fake = Fake::new();
        run(&fake, false, 0).expect("copy with no options");
        assert_eq!(fake.lines(), ["fetch", "write tracks 10 20", "write favorite 20 true 777 Some((11, 22))", "commit"], "T35: the old entry stays a favourite");
    }

    #[test]
    fn a_failed_transaction_writes_nothing_and_runs_no_file_step() {
        let fake = Fake { transact_fails: true, ..Fake::new() };
        assert_eq!(run(&fake, true, i32::MAX), Err(MigrateError::Step("disk full".to_owned())));
        assert_eq!(fake.lines(), ["fetch", "rollback"], "E-40-85, T30: all or nothing, and the files are untouched");
    }

    #[test]
    fn a_failing_file_step_is_a_warning_and_the_next_file_steps_still_run() {
        let fake = Fake { downloads_fail: true, ..Fake::new() };
        let outcome = run(&fake, true, i32::MAX).expect("the migration stands");
        assert_eq!(outcome.warnings, ["busy"]);
        assert!(fake.lines().contains(&"copy Cover".to_owned()) && fake.lines().contains(&"copy Background".to_owned()));
    }

    #[test]
    fn downloads_go_only_while_the_old_source_is_installed_and_images_are_copied_only_when_flagged_and_present() {
        let no_old_source = Fake { installed: vec![2], ..Fake::new() };
        run(&no_old_source, true, i32::MAX).expect("migrates");
        assert!(!no_old_source.lines().contains(&"delete downloads".to_owned()), "no old source, no delete");
        let no_cover = Fake { old_cover: false, ..Fake::new() };
        run(&no_cover, true, i32::MAX).expect("migrates");
        assert!(!no_cover.lines().contains(&"copy Cover".to_owned()) && no_cover.lines().contains(&"copy Background".to_owned()));
        let unflagged = Fake::new();
        run(&unflagged, true, (ITEMS | CATEGORIES) as i32).expect("migrates");
        assert!(!unflagged.lines().iter().any(|l| l.starts_with("copy") || l.starts_with("delete")));
    }
}
