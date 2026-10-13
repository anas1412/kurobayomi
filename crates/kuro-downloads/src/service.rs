//! `Downloads`: the one service every caller uses, for both kinds (docs/design/unified-downloads.md 2.11). It owns the
//! queue handle, the index, the layouts and the pending-delete store. Every delete, flush and rename of one series runs
//! under its `SeriesLocks` lock, and every disk or database call runs in `spawn_blocking`.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_data::DataError;
use kuro_data::entry::EntryRepo;
use kuro_data::item::ItemRepo;
use kuro_db::Db;
use kuro_platform::kind::Kind;
use tokio::sync::{broadcast, watch};

use crate::delete::{DeleteRules, select_for_delete};
use crate::events::DownloadEvent;
use crate::index::DownloadIndex;
use crate::layout::Layout;
use crate::locks::SeriesLocks;
use crate::naming::{UnitName, unit_dir_names};
use crate::pending::{PendingGroup, PendingStore};
use crate::queue::{CancelTarget, EnqueueRequest, Enqueued, Layouts, Queue, Snapshot};
use crate::runner::SourceDirectory;
use crate::unit::{Availability, EntryRef, SourceKey, UnitRef};
use crate::error::DownloadError;

fn join_err(e: tokio::task::JoinError) -> DownloadError {
    DownloadError::Internal(e.to_string())
}

fn store_err(e: impl std::fmt::Display) -> DownloadError {
    DownloadError::Internal(e.to_string())
}

fn source_dir(source: &SourceKey) -> String {
    Layout::source_dir_name(&source.name, &source.lang)
}

fn unit_name(unit: &UnitRef) -> UnitName<'_> {
    UnitName::new(&unit.name, unit.scanlator.as_deref())
}

/// Every name the index may hold for these units (the current and the legacy one).
fn index_names(kind: Kind, units: &[UnitRef]) -> Vec<String> {
    units.iter().flat_map(|u| unit_dir_names(kind, unit_name(u))).collect()
}

/// Removes the finished and staging folders of `units`, prunes the emptied entry and source folders and brings the
/// index along. A folder that is already gone is fine.
fn remove_units_on_disk(
    layout: &Layout,
    index: &DownloadIndex,
    entry: &EntryRef,
    source: &SourceKey,
    units: &[UnitRef],
) -> io::Result<()> {
    let dir = source_dir(source);
    let names: Vec<UnitName<'_>> = units.iter().map(unit_name).collect();
    let (_, finished, staging) = layout.find_units(&dir, &entry.title, &names);
    for path in finished.iter().chain(&staging) {
        Layout::remove_path(path)?;
    }
    let pruned = layout.prune_empty(&dir, &entry.title)?;
    index.remove_units(entry.kind, source.id, &entry.title, &index_names(entry.kind, units));
    if pruned.entry_removed {
        index.remove_entry(entry.kind, source.id, &entry.title);
    }
    if pruned.source_removed {
        index.remove_source(entry.kind, source.id);
    }
    Ok(())
}

fn remove_entry_on_disk(layout: &Layout, index: &DownloadIndex, entry: &EntryRef, source: &SourceKey) -> io::Result<()> {
    let dir = source_dir(source);
    if let Some(path) = layout.find_entry_dir(&dir, &entry.title) {
        Layout::remove_path(&path)?;
    }
    let pruned = layout.prune_empty(&dir, &entry.title)?;
    index.remove_entry(entry.kind, source.id, &entry.title);
    if pruned.source_removed {
        index.remove_source(entry.kind, source.id);
    }
    Ok(())
}

/// Renames a downloaded unit on disk and moves it in the index. The caller holds the series lock.
fn rename_unit_on_disk(
    layout: &Layout,
    index: &DownloadIndex,
    entry: &EntryRef,
    source: &SourceKey,
    old: &UnitRef,
    new: &UnitRef,
) -> Result<(), DownloadError> {
    let dir = source_dir(source);
    match layout.rename_unit(&dir, &entry.title, unit_name(old), unit_name(new)) {
        Ok(Some(new_name)) => {
            index.remove_units(entry.kind, source.id, &entry.title, &index_names(entry.kind, std::slice::from_ref(old)));
            index.add_unit(entry.kind, source.id, &dir, &entry.title, &new_name);
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(e) => {
            tracing::warn!(target: "kuro_downloads", "Could not rename downloaded unit: {}: {e}", old.name);
            Err(e)
        }
    }
}

#[derive(Clone)]
pub struct Downloads {
    queue: Queue,
    index: DownloadIndex,
    layouts: Arc<Layouts>,
    pending: Arc<PendingStore>,
    series: Arc<SeriesLocks>,
    directory: Arc<dyn SourceDirectory>,
    db: Arc<Db>,
}

impl Downloads {
    pub fn new(
        queue: Queue,
        index: DownloadIndex,
        layouts: Layouts,
        pending: Arc<PendingStore>,
        directory: Arc<dyn SourceDirectory>,
        db: Arc<Db>,
    ) -> Self {
        Self { queue, index, layouts: Arc::new(layouts), pending, series: Arc::new(SeriesLocks::default()), directory, db }
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn index(&self) -> &DownloadIndex {
        &self.index
    }

    // ----- queue control, per kind -----

    pub async fn enqueue(&self, req: EnqueueRequest) -> Result<Enqueued, DownloadError> {
        self.queue.enqueue(req).await
    }

    pub async fn start_now(&self, req: EnqueueRequest) -> Result<(), DownloadError> {
        self.queue.start_now(req).await
    }

    /// Explicit start or resume of a lane (retries ERROR units).
    pub fn start(&self, kind: Kind) {
        self.queue.start(kind);
    }

    pub fn pause(&self, kind: Kind) {
        self.queue.pause(kind);
    }

    pub fn clear(&self, kind: Kind) {
        self.queue.clear(kind);
    }

    pub fn reorder(&self, kind: Kind, unit_ids: Vec<i64>) {
        self.queue.reorder(kind, unit_ids);
    }

    pub async fn cancel(&self, kind: Kind, target: CancelTarget) -> Result<Vec<i64>, DownloadError> {
        self.queue.cancel(kind, target).await
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.queue.snapshot()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<Snapshot>> {
        self.queue.subscribe()
    }

    pub fn events(&self) -> broadcast::Receiver<DownloadEvent> {
        self.queue.events()
    }

    /// The installed sources changed: waiting units are re-evaluated.
    pub fn sources_changed(&self) {
        self.queue.sources_changed();
    }

    /// Settings > Reindex downloads: the next renew rescans `kind`, or both kinds for `None`.
    pub fn reindex(&self, kind: Option<Kind>) {
        self.index.invalidate(kind);
    }

    // ----- what is on disk -----

    /// Index lookup, no disk access.
    pub fn is_downloaded(&self, entry: &EntryRef, source: &SourceKey, unit: &UnitRef) -> bool {
        self.index.is_downloaded(entry.kind, source.id, &entry.title, &unit.name, unit.scanlator.as_deref())
    }

    /// The finished unit on disk, for the reader or the player. Stats the disk, so call it off the UI thread.
    pub fn locate(&self, entry: &EntryRef, source: &SourceKey, unit: &UnitRef) -> Option<PathBuf> {
        self.layouts.of(entry.kind).find_unit(&source_dir(source), &entry.title, unit_name(unit))
    }

    /// `(bytes, units)` of one entry. `local` is the entry folder of a local source, which the index does not hold.
    pub async fn entry_usage(&self, kind: Kind, source_id: i64, title: &str, local: Option<&Path>) -> (u64, usize) {
        match local {
            Some(dir) => {
                let dir = dir.to_path_buf();
                tokio::task::spawn_blocking(move || {
                    let count = if kind.is_anime() { crate::local::anime_episode_count(&dir) } else { crate::local::chapter_count(&dir) };
                    (crate::local::size(&dir), count.unwrap_or(0))
                })
                .await
                .unwrap_or((0, 0))
            }
            None => (
                self.index.size_entry(kind, source_id, title).await,
                self.index.unit_count_entry(kind, source_id, title),
            ),
        }
    }

    // ----- delete -----

    /// Explicit or rule-filtered delete: the units leave the queue, their folders go, emptied entry and source folders
    /// are pruned and the index follows. Returns how many units the rules selected.
    pub async fn delete_units(
        &self,
        entry: &EntryRef,
        source: &SourceKey,
        units: Vec<UnitRef>,
        rules: DeleteRules,
    ) -> Result<usize, DownloadError> {
        let selected = select_for_delete(units, rules);
        if selected.is_empty() {
            return Ok(0);
        }
        let _series = self.series.lock(entry.kind, entry.id).await;
        let ids: Vec<i64> = selected.iter().map(|u| u.id).collect();
        self.queue.cancel(entry.kind, CancelTarget::Units(ids)).await?;
        let _hold = self.queue.hold_entry(entry.kind, entry.id).await?;
        let count = selected.len();
        let (layout, index) = (self.layouts.of(entry.kind).clone(), self.index.clone());
        let (entry, source) = (entry.clone(), source.clone());
        tokio::task::spawn_blocking(move || remove_units_on_disk(&layout, &index, &entry, &source, &selected))
            .await
            .map_err(join_err)??;
        Ok(count)
    }

    /// Deletes an entry's folder (and the source folder when it is then empty). With `remove_queued` its queued units go
    /// first; without it the run of that entry is held while the folder is deleted.
    pub async fn delete_entry(&self, entry: &EntryRef, source: &SourceKey, remove_queued: bool) -> Result<(), DownloadError> {
        let _series = self.series.lock(entry.kind, entry.id).await;
        if remove_queued {
            self.queue.cancel(entry.kind, CancelTarget::Entry(entry.id)).await?;
        }
        let _hold = self.queue.hold_entry(entry.kind, entry.id).await?;
        let (layout, index) = (self.layouts.of(entry.kind).clone(), self.index.clone());
        let (entry, source) = (entry.clone(), source.clone());
        tokio::task::spawn_blocking(move || remove_entry_on_disk(&layout, &index, &entry, &source))
            .await
            .map_err(join_err)??;
        Ok(())
    }

    /// Delete-after-read: the rule runs now (`AFTER_READ`), the rows wait in `download_pending_delete`, which survives a
    /// restart. Returns how many rows were new.
    pub fn defer_delete(&self, entry: &EntryRef, units: &[UnitRef]) -> Result<usize, DownloadError> {
        let selected = select_for_delete(units.to_vec(), DeleteRules::AFTER_READ);
        if selected.is_empty() {
            return Ok(0);
        }
        self.pending.add(entry.kind, entry, &selected).map_err(store_err)
    }

    /// Runs when the reader or the player closes, and once at startup. Takes the rows whose source is known, reads the
    /// live bookmark flag of each unit, drops the rows of an entry that is gone (EC-93) and puts a failed group back.
    /// Returns how many units were deleted.
    pub async fn flush_pending_deletes(&self) -> Result<usize, DownloadError> {
        let (pending, directory) = (self.pending.clone(), self.directory.clone());
        let groups = tokio::task::spawn_blocking(move || pending.take(&|kind, id| directory.source_key(kind, id).is_some()))
            .await
            .map_err(join_err)?
            .map_err(store_err)?;
        let mut deleted = 0;
        for group in groups {
            match self.flush_group(&group).await {
                Ok(n) => deleted += n,
                Err(err) => {
                    tracing::warn!(target: "kuro_downloads", entry_id = group.entry.id, error = %err, "pending delete failed; kept for the next flush");
                    let pending = self.pending.clone();
                    let back = tokio::task::spawn_blocking(move || pending.put_back(&group)).await;
                    if !matches!(back, Ok(Ok(_))) {
                        tracing::error!(target: "kuro_downloads", "could not keep a pending delete group");
                    }
                }
            }
        }
        Ok(deleted)
    }

    async fn flush_group(&self, group: &PendingGroup) -> Result<usize, DownloadError> {
        let (db, entry_id) = (self.db.clone(), group.entry.id);
        let ids: Vec<i64> = group.units.iter().map(|u| u.id).collect();
        let live = tokio::task::spawn_blocking(move || -> Result<Option<Vec<Option<bool>>>, DownloadError> {
            match EntryRepo::new(db.clone()).by_id(entry_id) {
                Ok(_) => {}
                Err(DataError::NotFound) => return Ok(None),
                Err(e) => return Err(store_err(e)),
            }
            let items = ItemRepo::new(db);
            ids.iter()
                .map(|id| items.by_id(*id).map(|i| i.map(|i| i.bookmark)).map_err(store_err))
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        })
        .await
        .map_err(join_err)??;
        let Some(live) = live else { return Ok(0) };
        let units: Vec<UnitRef> = group
            .units
            .iter()
            .zip(live)
            .map(|(u, bookmark)| UnitRef { bookmark: bookmark.unwrap_or(u.bookmark), ..u.clone() })
            .collect();
        let source = self
            .directory
            .source_key(group.entry.kind, group.entry.source)
            .ok_or_else(|| DownloadError::Internal("the source is not available".to_owned()))?;
        if source.availability == Availability::Local {
            return Ok(0);
        }
        self.delete_units(&group.entry, &source, units, DeleteRules::AFTER_READ).await
    }

    // ----- rename -----

    /// A unit changed its name or scanlator: its folder or file follows and so does the index. `Ok` when it is not
    /// downloaded; a target that exists fails and keeps the old one.
    pub async fn rename_unit(
        &self,
        entry: &EntryRef,
        source: &SourceKey,
        old: &UnitRef,
        new: &UnitRef,
    ) -> Result<(), DownloadError> {
        let _series = self.series.lock(entry.kind, entry.id).await;
        let (layout, index) = (self.layouts.of(entry.kind).clone(), self.index.clone());
        let (entry, source, old, new) = (entry.clone(), source.clone(), old.clone(), new.clone());
        tokio::task::spawn_blocking(move || rename_unit_on_disk(&layout, &index, &entry, &source, &old, &new))
            .await
            .map_err(join_err)?
    }

    /// The same for the one synchronous caller (`FolderRenames`). Call it from `spawn_blocking` or a plain thread: the
    /// series lock panics on a runtime worker.
    pub fn rename_unit_blocking(
        &self,
        entry: &EntryRef,
        source: &SourceKey,
        old: &UnitRef,
        new: &UnitRef,
    ) -> Result<(), DownloadError> {
        let _series = self.series.lock_blocking(entry.kind, entry.id);
        rename_unit_on_disk(self.layouts.of(entry.kind), &self.index, entry, source, old, new)
    }

    /// The source changed its name: its folder follows. A case-only change goes through `<name>_tmp`; a target that
    /// exists fails and keeps the old folder.
    pub async fn rename_source(&self, kind: Kind, old: &SourceKey, new: &SourceKey) -> Result<(), DownloadError> {
        let (old_dir, new_dir) = (source_dir(old), source_dir(new));
        let layout = self.layouts.of(kind).clone();
        let renamed = tokio::task::spawn_blocking(move || layout.rename_source(&old_dir, &new_dir)).await.map_err(join_err)?;
        match renamed {
            Ok(Some(new_name)) => {
                self.index.rename_source_dir(kind, old.id, &new_name);
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(e) => {
                tracing::warn!(target: "kuro_downloads", "Failed to rename source download folder: {}: {e}", old.name);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use kuro_data::entry::Entry;
    use kuro_data::item::{Item, ItemUpdate};
    use kuro_db::schema_unified;

    use crate::gate::Connectivity;
    use crate::index::IndexRoots;
    use crate::queue::QueueParams;
    use crate::store::QueueStore;
    use super::*;

    struct KnownSources(SourceKey);

    impl SourceDirectory for KnownSources {
        fn source_key(&self, _kind: Kind, source_id: i64) -> Option<SourceKey> {
            (source_id == self.0.id).then(|| self.0.clone())
        }
    }

    struct Fixture {
        _tmp: tempfile::TempDir,
        _net: watch::Sender<Connectivity>,
        db: Arc<Db>,
        downloads: Downloads,
        entry: EntryRef,
        source: SourceKey,
        units: Vec<UnitRef>,
    }

    impl Fixture {
        /// A database with one entry and three units, a `Downloads` over it, and every unit downloaded on disk.
        fn new(kind: Kind) -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let path = schema_unified::db_path(tmp.path());
            let db = Arc::new(Db::new(&path, schema_unified::open_file(&path).unwrap()).unwrap());

            let mut e = Entry::create(kind);
            e.source = 7;
            e.url = "/e".to_owned();
            e.title = "Title".to_owned();
            e.id = EntryRepo::new(db.clone()).insert(&e).unwrap();
            let items: Vec<Item> = (1..=3)
                .map(|n| {
                    let mut i = Item::create(kind);
                    i.entry_id = e.id;
                    i.url = format!("/u{n}");
                    i.name = format!("Unit {n}");
                    i.number = n as f64;
                    i
                })
                .collect();
            let units: Vec<UnitRef> = ItemRepo::new(db.clone()).add_all(&items).unwrap().iter().map(UnitRef::from).collect();

            let source = SourceKey { id: 7, name: "Src".to_owned(), lang: "en".to_owned(), unmetered: false, availability: Availability::Live };
            let roots = IndexRoots { manga: tmp.path().join("manga"), anime: tmp.path().join("anime") };
            let cache = tmp.path().join("cache");
            fs::create_dir_all(&cache).unwrap();
            let index = DownloadIndex::open(IndexRoots { manga: roots.manga.clone(), anime: roots.anime.clone() }, &cache);
            let layouts = Layouts { manga: Layout::new(Kind::Manga, &roots.manga), anime: Layout::new(Kind::Anime, &roots.anime) };
            let directory: Arc<dyn SourceDirectory> = Arc::new(KnownSources(source.clone()));
            let (net, connectivity) = watch::channel(Connectivity { online: true, metered: false });
            let queue = Queue::spawn(QueueParams {
                db: db.clone(),
                store: Arc::new(QueueStore::new(db.clone())),
                runners: Vec::new(),
                directory: directory.clone(),
                layouts: Layouts { manga: layouts.manga.clone(), anime: layouts.anime.clone() },
                index: index.clone(),
                free_space: |_| None,
                max_active: Arc::new(|| 1),
                connectivity,
                only_over_wifi: false,
            });
            let downloads = Downloads::new(queue, index, layouts, Arc::new(PendingStore::new(db.clone())), directory, db.clone());
            let fixture = Self { _tmp: tmp, _net: net, db, downloads, entry: EntryRef::from(&e), source, units };
            for unit in &fixture.units {
                fixture.download(unit);
            }
            fixture
        }

        fn layout(&self) -> &Layout {
            self.downloads.layouts.of(self.entry.kind)
        }

        fn download(&self, unit: &UnitRef) {
            let dir = source_dir(&self.source);
            let entry_dir = self.layout().entry_dir(&dir, &self.entry.title).unwrap();
            let name = unit_dir_names(self.entry.kind, unit_name(unit))[0].clone();
            fs::create_dir_all(entry_dir.join(&name)).unwrap();
            fs::write(entry_dir.join(&name).join("1.bin"), b"x").unwrap();
            self.downloads.index.add_unit(self.entry.kind, self.source.id, &dir, &self.entry.title, &name);
        }

        fn downloaded(&self, unit: &UnitRef) -> bool {
            self.downloads.is_downloaded(&self.entry, &self.source, unit)
        }

        fn on_disk(&self, unit: &UnitRef) -> bool {
            self.downloads.locate(&self.entry, &self.source, unit).is_some()
        }

        fn bookmark(&self, unit: &UnitRef) {
            let update = ItemUpdate { bookmark: Some(true), ..ItemUpdate::new(unit.id) };
            ItemRepo::new(self.db.clone()).update(&update).unwrap();
        }
    }

    #[tokio::test]
    async fn an_explicit_delete_removes_bookmarked_units_too() {
        for kind in Kind::ALL {
            let f = Fixture::new(kind);
            let units: Vec<UnitRef> = f.units.iter().map(|u| UnitRef { bookmark: u.id == f.units[1].id, ..u.clone() }).collect();
            let n = f.downloads.delete_units(&f.entry, &f.source, units, DeleteRules::EXPLICIT).await.unwrap();
            assert_eq!(n, 3);
            for u in &f.units {
                assert!(!f.on_disk(u) && !f.downloaded(u));
            }
            assert!(f.layout().find_source_dir(&source_dir(&f.source)).is_none(), "emptied folders are pruned");
        }
    }

    #[tokio::test]
    async fn delete_after_read_keeps_bookmarked_units() {
        for kind in Kind::ALL {
            let f = Fixture::new(kind);
            let units: Vec<UnitRef> = f.units.iter().map(|u| UnitRef { bookmark: u.id == f.units[1].id, ..u.clone() }).collect();
            let n = f.downloads.delete_units(&f.entry, &f.source, units, DeleteRules::AFTER_READ).await.unwrap();
            assert_eq!(n, 2);
            assert!(!f.on_disk(&f.units[0]) && !f.downloaded(&f.units[0]));
            assert!(f.on_disk(&f.units[1]) && f.downloaded(&f.units[1]));
            assert!(!f.on_disk(&f.units[2]));
        }
    }

    #[tokio::test]
    async fn a_pending_delete_flushes_after_a_restart_and_keeps_units_bookmarked_meanwhile() {
        for kind in Kind::ALL {
            let f = Fixture::new(kind);
            assert_eq!(f.downloads.defer_delete(&f.entry, &f.units).unwrap(), 3);
            f.bookmark(&f.units[1]);

            // A restart: a new service over the same database sees the same pending rows.
            let restarted = Downloads { pending: Arc::new(PendingStore::new(f.db.clone())), ..f.downloads.clone() };
            assert_eq!(restarted.flush_pending_deletes().await.unwrap(), 2);
            assert!(!f.on_disk(&f.units[0]) && !f.downloaded(&f.units[0]));
            assert!(f.on_disk(&f.units[1]), "bookmarked after the reader closed: kept");
            assert!(!f.on_disk(&f.units[2]));
            assert_eq!(restarted.pending.len().unwrap(), 0);
            assert_eq!(restarted.flush_pending_deletes().await.unwrap(), 0);
        }
    }

    #[tokio::test]
    async fn a_second_flush_after_the_folder_was_removed_by_hand_succeeds() {
        let f = Fixture::new(Kind::Manga);
        f.downloads.defer_delete(&f.entry, &f.units[..1]).unwrap();
        let dir = f.layout().find_entry_dir(&source_dir(&f.source), &f.entry.title).unwrap();
        fs::remove_dir_all(dir).unwrap();
        assert_eq!(f.downloads.flush_pending_deletes().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn rename_updates_the_index_without_a_rescan() {
        for kind in Kind::ALL {
            let f = Fixture::new(kind);
            let old = f.units[0].clone();
            let new = UnitRef { name: "Renamed".to_owned(), ..old.clone() };
            f.downloads.rename_unit(&f.entry, &f.source, &old, &new).await.unwrap();
            assert!(f.downloaded(&new) && f.on_disk(&new));
            assert!(!f.downloaded(&old) && !f.on_disk(&old));

            // A unit that is not downloaded is fine; a target that exists fails and keeps the old folder.
            let missing = UnitRef { id: 99, name: "Missing".to_owned(), ..old.clone() };
            f.downloads.rename_unit(&f.entry, &f.source, &missing, &old).await.unwrap();
            let taken = UnitRef { name: f.units[1].name.clone(), ..new.clone() };
            assert!(f.downloads.rename_unit(&f.entry, &f.source, &new, &taken).await.is_err());
            assert!(f.downloaded(&new));
        }
    }

    #[tokio::test]
    async fn rename_unit_blocking_updates_the_index_too() {
        let f = Fixture::new(Kind::Anime);
        let (downloads, entry, source) = (f.downloads.clone(), f.entry.clone(), f.source.clone());
        let (old, new) = (f.units[0].clone(), UnitRef { name: "Renamed".to_owned(), ..f.units[0].clone() });
        let (o, n) = (old.clone(), new.clone());
        tokio::task::spawn_blocking(move || downloads.rename_unit_blocking(&entry, &source, &o, &n)).await.unwrap().unwrap();
        assert!(f.downloaded(&new) && !f.downloaded(&old));
    }

    #[tokio::test]
    async fn delete_entry_removes_the_folder_and_the_index_entry() {
        for kind in Kind::ALL {
            let f = Fixture::new(kind);
            f.downloads.delete_entry(&f.entry, &f.source, true).await.unwrap();
            assert!(f.layout().find_entry_dir(&source_dir(&f.source), &f.entry.title).is_none());
            assert!(f.units.iter().all(|u| !f.downloaded(u)));
            assert_eq!(f.downloads.entry_usage(kind, f.source.id, &f.entry.title, None).await.1, 0);
        }
    }
}
