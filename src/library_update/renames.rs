//! Renames the download folder of a chapter or episode whose name changed during a sync (T-26a-086; the
//! `renameChapter` and `renameEpisode` calls of `SyncChaptersWithSource` and `SyncEpisodesWithSource`).
//!
//! The planner of the sync offers every rewritten row for a rename. The one download service decides (S-17): the folder
//! is renamed only when the old item is downloaded and its folder name differs from the one the new name gives, and the
//! download index follows. An item that is not downloaded is never touched, and no folder is created for it. A failed
//! rename is logged and never fails the sync (EC-11).
//!
//! The source name comes from the app's source directory. A source it does not know, and the local source, keep their
//! folders as they are.

use kuro_data::entry::Entry;
use kuro_data::item::Item;
use kuro_domain::items::sync::ItemRenames;
use kuro_downloads::next::runner::SourceDirectory as _;
use kuro_downloads::next::service::Downloads;
use kuro_downloads::next::unit::{Availability, EntryRef, UnitRef};

use crate::downloads::directory::AppDirectory;

/// The folder renames behind the sync port, for both kinds.
pub struct FolderRenames {
    downloads: Option<Downloads>,
    directory: AppDirectory,
}

impl FolderRenames {
    /// `downloads` is the one download service; without it (the headless pass) nothing is renamed.
    pub fn new(downloads: Option<Downloads>, directory: AppDirectory) -> Self {
        Self { downloads, directory }
    }
}

impl ItemRenames for FolderRenames {
    fn rename(&self, e: &Entry, stored: &Item, prepared: &Item) {
        let Some(downloads) = &self.downloads else { return };
        let Some(source) = self.directory.source_key(e.kind, e.source) else { return };
        if source.availability == Availability::Local {
            return;
        }
        let (entry, old, new) = (EntryRef::from(e), UnitRef::from(stored), UnitRef::from(prepared));
        // The sync runs on a runtime worker, where the series lock cannot be waited for, so the rename runs on a thread of its own.
        let renamed = std::thread::scope(|scope| scope.spawn(|| downloads.rename_unit_blocking(&entry, &source, &old, &new)).join());
        match renamed {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::error!(%error, unit = stored.id, "could not rename the downloaded unit"),
            Err(_) => tracing::error!(unit = stored.id, "the rename of the downloaded unit panicked"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use kuro_data::source::StubSourceRepo;
    use kuro_db::handle::Db;
    use kuro_db::schema_unified;
    use kuro_downloads::next::gate::Connectivity;
    use kuro_downloads::next::index::{DownloadIndex, IndexRoots};
    use kuro_downloads::next::layout::Layout;
    use kuro_downloads::next::naming::UnitName;
    use kuro_downloads::next::pending::PendingStore;
    use kuro_downloads::next::queue::{Layouts, Queue, QueueParams};
    use kuro_downloads::next::store::QueueStore;
    use kuro_platform::kind::Kind;

    use super::*;

    /// A service over a temporary folder and a database whose stub table names source 7 `Src (EN)`; the runtime that runs
    /// its queue stays alive with it.
    struct Fixture {
        _runtime: tokio::runtime::Runtime,
        _net: tokio::sync::watch::Sender<Connectivity>,
        dir: tempfile::TempDir,
        downloads: Downloads,
        directory: AppDirectory,
        layouts: Layouts,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let path = schema_unified::db_path(dir.path());
            let db = Arc::new(Db::new(&path, schema_unified::open_file(&path).expect("db file")).expect("db"));
            for kind in Kind::ALL {
                StubSourceRepo::new(Arc::clone(&db), kind).upsert(7, "en", "Src").expect("stub source");
            }
            let roots = IndexRoots { manga: dir.path().join("manga"), anime: dir.path().join("anime") };
            let cache = dir.path().join("cache");
            fs::create_dir_all(&cache).expect("cache folder");
            let index = DownloadIndex::open(roots.clone(), &cache);
            let layouts = || Layouts { manga: Layout::new(Kind::Manga, &roots.manga), anime: Layout::new(Kind::Anime, &roots.anime) };
            let directory = AppDirectory::new(Arc::clone(&db));
            let (net, connectivity) = tokio::sync::watch::channel(Connectivity { online: true, metered: false });
            let runtime = tokio::runtime::Runtime::new().expect("runtime");
            let downloads = runtime.block_on(async {
                let queue = Queue::spawn(QueueParams {
                    db: Arc::clone(&db),
                    store: Arc::new(QueueStore::new(Arc::clone(&db))),
                    runners: Vec::new(),
                    directory: Arc::new(directory.clone()),
                    layouts: layouts(),
                    index: index.clone(),
                    free_space: |_| None,
                    max_active: Arc::new(|| 1),
                    connectivity,
                    only_over_wifi: false,
                });
                Downloads::new(queue, index, layouts(), Arc::new(PendingStore::new(Arc::clone(&db))), Arc::new(directory.clone()), db)
            });
            Self { _runtime: runtime, _net: net, dir, downloads, directory, layouts: layouts() }
        }

        fn renames(&self) -> FolderRenames {
            FolderRenames::new(Some(self.downloads.clone()), self.directory.clone())
        }

        /// A finished unit of source 7 on disk and in the index.
        fn download(&self, kind: Kind, title: &str, name: &str) {
            let layout = match kind {
                Kind::Manga => &self.layouts.manga,
                Kind::Anime => &self.layouts.anime,
            };
            let source = Layout::source_dir_name("Src", "en");
            let folder = layout.unit_dir_names(UnitName::new(name, None))[0].clone();
            fs::create_dir_all(layout.entry_dir(&source, title).expect("entry folder").join(&folder)).expect("unit folder");
            self.downloads.index().add_unit(kind, 7, &source, title, &folder);
        }

        fn found(&self, kind: Kind, title: &str, name: &str) -> bool {
            let layout = match kind {
                Kind::Manga => &self.layouts.manga,
                Kind::Anime => &self.layouts.anime,
            };
            layout.find_unit(&Layout::source_dir_name("Src", "en"), title, UnitName::new(name, None)).is_some()
        }
    }

    fn pair(kind: Kind, source: i64, old: &str, new: &str) -> (Entry, Item, Item) {
        let entry = Entry { source, title: "Title".into(), ..Entry::create(kind) };
        let stored = Item { name: old.into(), ..Item::create(kind) };
        (entry, stored.clone(), Item { name: new.into(), ..stored })
    }

    #[test]
    fn s_17_a_downloaded_unit_is_renamed_on_disk_and_in_the_index_for_both_kinds() {
        for kind in Kind::ALL {
            let f = Fixture::new();
            f.download(kind, "Title", "Ch 1");
            let (entry, stored, prepared) = pair(kind, 7, "Ch 1", "Ch 1 fixed");
            ItemRenames::rename(&f.renames(), &entry, &stored, &prepared);
            assert!(f.found(kind, "Title", "Ch 1 fixed") && !f.found(kind, "Title", "Ch 1"), "{kind:?}");
            assert!(f.downloads.index().is_downloaded(kind, 7, "Title", "Ch 1 fixed", None), "the index follows ({kind:?})");
            assert!(!f.downloads.index().is_downloaded(kind, 7, "Title", "Ch 1", None), "{kind:?}");
        }
    }

    #[test]
    fn s_17_a_unit_that_is_not_downloaded_is_left_alone_and_no_folder_is_made() {
        for kind in Kind::ALL {
            let f = Fixture::new();
            let (entry, stored, prepared) = pair(kind, 7, "Ch 1", "Ch 2");
            ItemRenames::rename(&f.renames(), &entry, &stored, &prepared);
            let root = f.dir.path().join(if kind == Kind::Manga { "manga" } else { "anime" });
            assert!(!root.exists() || fs::read_dir(&root).expect("list").count() == 0, "nothing is created ({kind:?})");
        }
    }

    #[test]
    fn a_source_the_directory_does_not_know_keeps_its_folders() {
        let f = Fixture::new();
        f.download(Kind::Manga, "Title", "Ch 1");
        let (entry, stored, prepared) = pair(Kind::Manga, 8, "Ch 1", "Ch 1 v2");
        ItemRenames::rename(&f.renames(), &entry, &stored, &prepared);
        assert!(f.found(Kind::Manga, "Title", "Ch 1") && !f.found(Kind::Manga, "Title", "Ch 1 v2"));
    }

    #[test]
    fn no_service_renames_nothing() {
        let f = Fixture::new();
        f.download(Kind::Anime, "Title", "Ep 1");
        let (entry, stored, prepared) = pair(Kind::Anime, 7, "Ep 1", "Ep 1 fixed");
        ItemRenames::rename(&FolderRenames::new(None, f.directory.clone()), &entry, &stored, &prepared);
        assert!(f.found(Kind::Anime, "Title", "Ep 1"));
    }
}
