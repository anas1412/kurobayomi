//! Startup restore of one lane (docs/design/unified-downloads.md 2.8; merges the app's `spawn_restore` and
//! `anime_queue::restore`). The rows come from [`QueueStore`], the items and entries from the repos, the source from the
//! [`SourceDirectory`].
//!
//! A row whose item or entry is gone is dropped. A row whose source cannot be named at all is kept as a stub unit that
//! waits (D5), for both kinds, so removing a source never loses the user's queue. Every restored unit is QUEUE (B7).
//! The caller sweeps staging with [`Restored::keep`] after the restore, so a half-downloaded chapter keeps its pages (B20).
//!
//! Blocking (SQLite and a stat per unit): the actor runs it in `spawn_blocking`.

use std::path::PathBuf;
use std::sync::Arc;

use kuro_data::entry::EntryRepo;
use kuro_data::item::ItemRepo;
use kuro_db::{Db, DbError};
use kuro_platform::kind::Kind;

use super::layout::Layout;
use super::runner::{SourceDirectory, UnitRunner};
use super::store::{QueueStore, StoreError};
use super::unit::{Availability, EntryRef, QueuedUnit, SourceKey, UnitRef};

/// The restored lane and the staging folders its units may be resuming.
pub struct Restored {
    /// In queue order. Hand them to `Lane::restored`.
    pub units: Vec<QueuedUnit>,
    /// The staging folders of the restored units, for `Layout::sweep_staging`. `source_dir` names a unit's source folder
    /// (for a source nobody can name, the index remembers it), so a waiting unit keeps its pages too.
    pub keep: Vec<PathBuf>,
}

/// Restores the saved queue of `kind`. A row that cannot be read (as opposed to one that is gone) fails the whole
/// restore and sets the store's guard for `kind`, so the next save cannot erase the saved rows.
pub fn restore(
    db: &Arc<Db>,
    store: &QueueStore,
    kind: Kind,
    directory: &dyn SourceDirectory,
    runner: &dyn UnitRunner,
    layout: &Layout,
    source_dir: &dyn Fn(&QueuedUnit) -> String,
) -> Result<Restored, StoreError> {
    let rows = store.load(kind)?;
    let entries = EntryRepo::new(db.clone());
    let items = ItemRepo::new(db.clone());
    let mut units = Vec::new();
    let mut keep = Vec::new();
    for row in rows {
        let unreadable = |err: DbError| {
            store.mark_unreadable(kind);
            StoreError::from(err)
        };
        let Some(item) = items.by_id(row.unit_id).map_err(unreadable)? else { continue };
        let entry = match entries.by_id(row.entry_id) {
            Ok(entry) => entry,
            Err(DbError::NotFound) => continue,
            Err(err) => return Err(unreadable(err)),
        };
        if entry.kind != kind {
            continue;
        }
        let (source, waiting) = match directory.source_key(kind, row.source) {
            Some(source) => {
                let waiting = !runner.can_run(&source);
                (source, waiting)
            }
            None => (stub_source(row.source), true),
        };
        let mut unit = QueuedUnit::new(kind, EntryRef::from(&entry), UnitRef::from(&item), source);
        unit.waiting = waiting;
        keep.extend(staging_paths(layout, runner, &unit, &source_dir(&unit)));
        units.push(unit);
    }
    Ok(Restored { units, keep })
}

/// A source nobody can name: no name, no language, not installed.
fn stub_source(id: i64) -> SourceKey {
    SourceKey { id, name: String::new(), lang: String::new(), unmetered: false, availability: Availability::Stub }
}

/// The staging folders of `unit` that exist under its entry folder.
fn staging_paths(layout: &Layout, runner: &dyn UnitRunner, unit: &QueuedUnit, source_dir: &str) -> Vec<PathBuf> {
    let Some(entry_dir) = layout.find_entry_dir(source_dir, &unit.entry.title) else { return Vec::new() };
    runner.staging_names(&unit.unit).into_iter().map(|name| entry_dir.join(name)).collect()
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::super::runner::{RunCtx, UnitDone, UnitJob};
    use super::super::store::testing::{row, seed, test_db};
    use super::super::state::Lane;
    use super::*;
    use crate::error::DownloadError;

    struct FakeRunner {
        kind: Kind,
        can_run: bool,
    }

    #[async_trait]
    impl UnitRunner for FakeRunner {
        fn kind(&self) -> Kind {
            self.kind
        }
        fn can_run(&self, _source: &SourceKey) -> bool {
            self.can_run
        }
        fn staging_names(&self, unit: &UnitRef) -> Vec<String> {
            vec![format!("{}_tmp", unit.name)]
        }
        async fn run(&self, _job: UnitJob, _ctx: RunCtx) -> Result<UnitDone, DownloadError> {
            Err(DownloadError::Cancelled)
        }
    }

    /// Names every source of `named`, installed.
    struct FakeDirectory {
        named: Vec<i64>,
    }

    impl SourceDirectory for FakeDirectory {
        fn source_key(&self, _kind: Kind, source_id: i64) -> Option<SourceKey> {
            self.named.contains(&source_id).then(|| SourceKey {
                id: source_id,
                name: format!("Source {source_id}"),
                lang: "en".to_owned(),
                unmetered: false,
                availability: Availability::Live,
            })
        }
    }

    fn ids(units: &[QueuedUnit]) -> Vec<i64> {
        units.iter().map(|u| u.unit.id).collect()
    }

    fn run_restore(db: &Arc<Db>, store: &QueueStore, kind: Kind, named: &[i64], can_run: bool, root: &std::path::Path) -> Result<Restored, StoreError> {
        let directory = FakeDirectory { named: named.to_vec() };
        let runner = FakeRunner { kind, can_run };
        restore(db, store, kind, &directory, &runner, &Layout::new(kind, root), &|u| Layout::source_dir_name(&u.source.name, &u.source.lang))
    }

    #[test]
    fn a_restart_keeps_rows_of_unnamed_sources_as_waiting_for_both_kinds() {
        let (dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        for kind in Kind::ALL {
            let (e, u) = seed(&db, kind, 7, "/a", 3);
            store.replace(kind, &[row(u[2], e, 7), row(u[0], e, 7), row(u[1], e, 7)]).unwrap();
            let restored = run_restore(&db, &store, kind, &[], true, dir.path()).unwrap();
            assert_eq!(ids(&restored.units), vec![u[2], u[0], u[1]], "queue order is kept");
            for unit in &restored.units {
                assert!(unit.waiting, "never run until the source is back");
                assert_eq!(unit.source.availability, Availability::Stub);
                assert_eq!(unit.source.id, 7);
                assert_eq!(unit.state, crate::model::DownloadState::Queue);
            }
            // The lane saves what it holds: the waiting rows survive the next write.
            let lane = Lane::restored(kind, restored.units);
            let saved: Vec<_> = lane.items().iter().map(|q| row(q.unit.id, q.entry.id, q.source.id)).collect();
            store.replace(kind, &saved).unwrap();
            assert_eq!(store.load(kind).unwrap().len(), 3);
        }
    }

    #[test]
    fn a_named_source_runs_unless_the_runner_cannot_reach_it() {
        let (dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        for kind in Kind::ALL {
            let (e, u) = seed(&db, kind, 7, "/a", 2);
            store.replace(kind, &[row(u[0], e, 7), row(u[1], e, 7)]).unwrap();
            let live = run_restore(&db, &store, kind, &[7], true, dir.path()).unwrap();
            assert!(live.units.iter().all(|q| !q.waiting && q.source.availability == Availability::Live));
            let blocked = run_restore(&db, &store, kind, &[7], false, dir.path()).unwrap();
            assert!(blocked.units.iter().all(|q| q.waiting), "can_run false: waiting");
        }
    }

    #[test]
    fn a_row_whose_item_is_gone_is_dropped() {
        let (dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        let (e, u) = seed(&db, Kind::Anime, 7, "/a", 2);
        store.replace(Kind::Anime, &[row(u[0], e, 7), row(u[1], e, 7)]).unwrap();
        ItemRepo::new(db.clone()).remove_with_ids(&[u[0]]).unwrap();
        let restored = run_restore(&db, &store, Kind::Anime, &[7], true, dir.path()).unwrap();
        assert_eq!(ids(&restored.units), vec![u[1]]);
    }

    #[test]
    fn the_staging_folder_of_a_restored_unit_survives_the_sweep_and_a_stray_one_does_not() {
        let (_dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        let (e, u) = seed(&db, Kind::Manga, 7, "/a", 1);
        store.replace(Kind::Manga, &[row(u[0], e, 7)]).unwrap();
        let root = tempfile::tempdir().unwrap();
        let layout = Layout::new(Kind::Manga, root.path());
        let restored = run_restore(&db, &store, Kind::Manga, &[7], true, root.path()).unwrap();
        let queued = &restored.units[0];
        let source_dir = Layout::source_dir_name(&queued.source.name, &queued.source.lang);
        let entry_dir = layout.entry_dir(&source_dir, &queued.entry.title).unwrap();
        let resuming = entry_dir.join(format!("{}_tmp", queued.unit.name));
        let stray = entry_dir.join("Other chapter_tmp");
        for folder in [&resuming, &stray] {
            std::fs::create_dir_all(folder).unwrap();
            std::fs::write(folder.join("001.jpg"), b"page").unwrap();
        }
        // Restore again now that the folders exist: `keep` only lists folders under a found entry folder.
        let restored = run_restore(&db, &store, Kind::Manga, &[7], true, root.path()).unwrap();
        assert_eq!(restored.keep, vec![resuming.clone()]);
        assert_eq!(layout.sweep_staging(&restored.keep), 1);
        assert!(resuming.join("001.jpg").exists(), "B20: the half-downloaded chapter keeps its pages");
        assert!(!stray.exists());
    }

    #[test]
    fn an_unreadable_queue_fails_the_restore_and_blocks_the_next_save() {
        let (dir, db) = test_db();
        let store = QueueStore::new(db.clone());
        let (e, u) = seed(&db, Kind::Anime, 7, "/a", 1);
        store.replace(Kind::Anime, &[row(u[0], e, 7)]).unwrap();
        db.write(|tx| Ok(tx.execute("UPDATE download_queue SET status = 'bad'", [])?)).unwrap();
        assert!(run_restore(&db, &store, Kind::Anime, &[7], true, dir.path()).is_err());
        assert!(matches!(store.replace(Kind::Anime, &[]), Err(StoreError::Unreadable)));
    }
}
