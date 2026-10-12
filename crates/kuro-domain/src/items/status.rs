//! Done status of a list of items of either kind (`SetReadStatus.kt`, `SetSeenStatus.kt`; T-10a-013, T-10a-014): "read" for
//! a chapter, "seen" for an episode. One coalesce update of the rows in one transaction, then the download deletion for
//! the rows marked done when the caller asks for it. The whole operation runs under [`crate::app::rx::non_cancellable`],
//! so a closed screen does not stop it (EC-29).
//!
//! Whether the downloads of newly done items go is a plain argument of [`SetDoneStatus::run`]: the "delete after marked
//! read" setting is read in one place by the caller, and Library, Updates and the title page all pass it the same way
//! (decision 7). The download deletion is a port ([`ItemDownloads`]); `app::delete_download` implements it.

use std::sync::Arc;

use kuro_data::entry::{Entry, EntryRepo};
use kuro_data::item::{Item, ItemRepo, ItemUpdate};

use crate::app::rx::non_cancellable;

/// Outcome of a status write. `NoItems` is the "nothing to change" result; the episode screens show it as `NoEpisodes`.
/// `InternalError` carries the failure text; nothing was written (EC-28).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetDoneResult {
    Success,
    NoItems,
    InternalError(String),
}

/// Deletes the downloads of items (`DeleteChapterDownload.awaitAll`, `DeleteEpisodeDownload.awaitAll`). A missing source
/// deletes nothing (EC-27).
pub trait ItemDownloads: Send + Sync {
    fn delete(&self, e: &Entry, items: &[Item]);
}

/// The items a mark-done or mark-undone touches (`SetReadStatus.await`, step 1). Marking done takes undone rows; marking
/// undone takes rows that are done or have a position to reset.
pub fn items_to_change(done: bool, items: &[Item]) -> Vec<Item> {
    items.iter().filter(|i| if done { !i.done } else { i.done || i.progress > 0 }).cloned().collect()
}

/// The update of one item (step 2). Marking done keeps the progress (`None`); marking undone resets it to 0 (EC-26). The
/// total seconds of an episode are never touched.
pub fn done_update(done: bool, item: &Item) -> ItemUpdate {
    ItemUpdate { done: Some(done), progress: if done { None } else { Some(0) }, ..ItemUpdate::new(item.id) }
}

/// Marks items done or undone, for both kinds.
#[derive(Clone)]
pub struct SetDoneStatus {
    items: ItemRepo,
    entries: EntryRepo,
    downloads: Arc<dyn ItemDownloads>,
}

impl SetDoneStatus {
    pub fn new(items: ItemRepo, entries: EntryRepo, downloads: Arc<dyn ItemDownloads>) -> Self {
        Self { items, entries, downloads }
    }

    /// `await(done, items)`, non-cancellable. With `remove_downloads` the downloads of the rows that became done are
    /// deleted, grouped by entry.
    pub async fn run(&self, done: bool, items: Vec<Item>, remove_downloads: bool) -> SetDoneResult {
        let this = self.clone();
        non_cancellable(async move { this.apply(done, &items, remove_downloads) }).await
    }

    /// `await(entryId, done)`: every item of the entry.
    pub async fn run_entry(&self, entry_id: i64, done: bool, remove_downloads: bool) -> SetDoneResult {
        let this = self.clone();
        non_cancellable(async move {
            match this.items.by_entry_id(entry_id, false) {
                Ok(items) => this.apply(done, &items, remove_downloads),
                Err(e) => SetDoneResult::InternalError(e.to_string()),
            }
        })
        .await
    }

    fn apply(&self, done: bool, items: &[Item], remove_downloads: bool) -> SetDoneResult {
        let targets = items_to_change(done, items);
        if targets.is_empty() {
            return SetDoneResult::NoItems;
        }
        let updates: Vec<ItemUpdate> = targets.iter().map(|i| done_update(done, i)).collect();
        if let Err(e) = self.items.update_all(&updates) {
            tracing::error!(error = %e, "marking items failed");
            return SetDoneResult::InternalError(e.to_string());
        }
        if done && remove_downloads {
            self.delete_downloads(&targets);
        }
        SetDoneResult::Success
    }

    /// EC-27: grouped by entry id, one deletion call per entry. An entry that cannot be read is skipped with a log line.
    fn delete_downloads(&self, targets: &[Item]) {
        let mut ids: Vec<i64> = targets.iter().map(|i| i.entry_id).collect();
        ids.sort_unstable();
        ids.dedup();
        for entry_id in ids {
            let group: Vec<Item> = targets.iter().filter(|i| i.entry_id == entry_id).cloned().collect();
            match self.entries.by_id(entry_id) {
                Ok(entry) => self.downloads.delete(&entry, &group),
                Err(e) => tracing::warn!(error = %e, entry_id, "cannot read the entry for download deletion"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use kuro_platform::kind::Kind;

    use super::*;
    use crate::testdb::test_db;

    /// Records deletion calls as (entry id, item numbers).
    #[derive(Default)]
    struct Recording {
        calls: Mutex<Vec<(i64, Vec<f64>)>>,
    }

    impl ItemDownloads for Recording {
        fn delete(&self, e: &Entry, items: &[Item]) {
            self.calls.lock().unwrap().push((e.id, items.iter().map(|i| i.number).collect()));
        }
    }

    fn item(kind: Kind, done: bool, progress: i64, number: f64) -> Item {
        Item { done, progress, number, ..Item::create(kind) }
    }

    fn block<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(fut)
    }

    #[test]
    fn marking_done_takes_only_undone_and_keeps_the_progress() {
        for kind in [Kind::Manga, Kind::Anime] {
            let rows = [item(kind, false, 0, 1.0), item(kind, true, 0, 2.0), item(kind, false, 5, 3.0)];
            let targets = items_to_change(true, &rows);
            assert_eq!(targets.iter().map(|i| i.number).collect::<Vec<_>>(), [1.0, 3.0]);
            assert_eq!(done_update(true, &targets[1]).progress, None, "the progress is untouched");
        }
    }

    #[test]
    fn marking_undone_takes_done_rows_and_rows_with_progress_and_resets_it() {
        for kind in [Kind::Manga, Kind::Anime] {
            let rows = [item(kind, false, 0, 1.0), item(kind, true, 0, 2.0), item(kind, false, 5, 3.0)];
            let targets = items_to_change(false, &rows);
            assert_eq!(targets.iter().map(|i| i.number).collect::<Vec<_>>(), [2.0, 3.0], "EC-26");
            let update = done_update(false, &targets[1]);
            assert_eq!((update.progress, update.total_seconds), (Some(0), None), "total_seconds is never touched");
        }
    }

    fn rig(kind: Kind) -> (tempfile::TempDir, SetDoneStatus, EntryRepo, ItemRepo, Arc<Recording>) {
        let (dir, db) = test_db();
        let downloads = Arc::new(Recording::default());
        let entries = EntryRepo::new(db.clone());
        let items = ItemRepo::new(db);
        let _ = kind;
        (dir, SetDoneStatus::new(items.clone(), entries.clone(), downloads.clone()), entries, items, downloads)
    }

    #[test]
    fn a_no_op_list_is_no_items_and_writes_nothing() {
        let (_d, set, ..) = rig(Kind::Manga);
        assert_eq!(block(set.run(true, Vec::new(), true)), SetDoneResult::NoItems);
    }

    #[test]
    fn set_done_status_deletes_downloads_only_when_asked() {
        for kind in [Kind::Manga, Kind::Anime] {
            let (_d, set, entries, items, downloads) = rig(kind);
            let a = entries.insert(&Entry { url: "a".into(), ..Entry::create(kind) }).unwrap();
            let b = entries.insert(&Entry { url: "b".into(), ..Entry::create(kind) }).unwrap();
            let stored = items
                .add_all(&[
                    Item { entry_id: a, url: "a1".into(), number: 1.0, ..Item::create(kind) },
                    Item { entry_id: a, url: "a2".into(), number: 2.0, ..Item::create(kind) },
                    Item { entry_id: b, url: "b1".into(), number: 1.0, ..Item::create(kind) },
                ])
                .unwrap();
            assert_eq!(block(set.run(true, stored.clone(), true)), SetDoneResult::Success);
            assert_eq!(*downloads.calls.lock().unwrap(), vec![(a, vec![1.0, 2.0]), (b, vec![1.0])], "{kind}: grouped by entry, EC-27");
            assert!(items.by_entry_id(a, false).unwrap().iter().all(|i| i.done));

            // Undone, then done again without the flag: nothing is deleted.
            assert_eq!(block(set.run_entry(a, false, true)), SetDoneResult::Success);
            downloads.calls.lock().unwrap().clear();
            assert_eq!(block(set.run_entry(a, true, false)), SetDoneResult::Success);
            assert!(downloads.calls.lock().unwrap().is_empty(), "{kind}: the flag was off");
            // Already done: nothing to change.
            assert_eq!(block(set.run_entry(a, true, true)), SetDoneResult::NoItems);
        }
    }

    #[test]
    fn marking_undone_resets_the_progress_and_leaves_the_total() {
        let (_d, set, entries, items, _) = rig(Kind::Anime);
        let id = entries.insert(&Entry::create(Kind::Anime)).unwrap();
        let stored = items.add_all(&[Item { entry_id: id, url: "e".into(), done: true, progress: 40, total_seconds: Some(1400), ..Item::create(Kind::Anime) }]).unwrap();
        assert_eq!(block(set.run(false, stored, false)), SetDoneResult::Success);
        let row = &items.by_entry_id(id, false).unwrap()[0];
        assert_eq!((row.done, row.progress, row.total_seconds), (false, 0, Some(1400)));
    }
}
