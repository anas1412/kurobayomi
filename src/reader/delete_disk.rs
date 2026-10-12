//! The disk side of delete-after-read (T-43a-032). A closing reader hands over the chapters its queue holds
//! ([`DeleteQueue`](super::delete_queue::DeleteQueue)); this resolves each one in the database, applies the download
//! delete rules (bookmarked chapters stay unless the setting allows removing them, and so do read chapters of an
//! excluded category) and removes the chapter's folder or `.cbz` from the downloads folder.
//!
//! The app has no source registry yet, so a source's folder name is not known. The chapter is looked for under every
//! source folder instead, and it is removed only when exactly one folder holds it, so a same-named chapter of another
//! source is never deleted by mistake. A chapter that is not on disk, protected by the rules or of a local manga
//! counts as dealt with. One that could not be removed stays in the queue file for the next reader that closes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_data::category::CategoryRepo;
use kuro_data::entry::EntryRepo;
use kuro_data::item::ItemRepo;
use kuro_platform::kind::Kind;
use kuro_db::Db;
use kuro_downloads::index::DownloadIndex;
use kuro_downloads::manager::{DeleteRules, chapters_to_delete};
use kuro_local::is_local_source;

use super::delete_queue::{Pending, QueueFile};
use crate::entry_core::downloaded::chapter_ref;
use super::download_paths::find_downloads;

/// The delete settings read when the reader closes.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub remove_bookmarked: bool,
    pub exclude_categories: Vec<i64>,
}

/// Everything a flush needs. All of it is `Send`, so a flush runs on a worker thread.
pub struct DiskDeleter {
    pub db: Arc<Db>,
    pub index: DownloadIndex,
    pub queue: QueueFile,
    pub settings: Settings,
}

fn remove(path: &Path) -> std::io::Result<()> {
    if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) }
}

impl DiskDeleter {
    /// Deletes what `pending` names and takes the dealt-with chapters out of the queue file. Blocking: run it on a
    /// worker thread. Returns how many downloads were removed.
    pub fn run(&self, pending: &[Pending]) -> usize {
        let (mut removed, mut done) = (0, Vec::new());
        for entry in pending {
            match self.delete_one(entry) {
                Ok(true) => {
                    removed += 1;
                    done.push(*entry);
                }
                Ok(false) => done.push(*entry),
                Err(error) => tracing::warn!(chapter_id = entry.chapter_id, error = %error, "a read chapter could not be deleted; kept for the next reader"),
            }
        }
        if let Err(error) = self.queue.remove(&done) {
            tracing::warn!(error = %error, "the delete queue could not be updated");
        }
        removed
    }

    /// `Ok(true)` when a download was removed, `Ok(false)` when there was nothing to remove.
    fn delete_one(&self, entry: &Pending) -> Result<bool, String> {
        let Some(chapter) = ItemRepo::new(Arc::clone(&self.db)).by_id(entry.chapter_id).map_err(|e| e.to_string())? else {
            return Ok(false);
        };
        let manga = EntryRepo::new(Arc::clone(&self.db)).by_id(entry.manga_id).map_err(|e| e.to_string())?;
        if is_local_source(manga.source) {
            return Ok(false);
        }
        let categories: Vec<i64> = CategoryRepo::new(Arc::clone(&self.db), Kind::Manga)
            .by_entry(manga.id)
            .map_err(|e| e.to_string())?
            .iter()
            .map(|c| c.id)
            .collect();
        let rules = DeleteRules {
            exclude_categories: self.settings.exclude_categories.clone(),
            manga_categories: categories,
            remove_bookmarked: self.settings.remove_bookmarked,
        };
        if chapters_to_delete(vec![chapter_ref(&chapter)], &rules).is_empty() {
            return Ok(false);
        }
        let found = find_downloads(self.index.root(), &manga.title, &chapter.name, chapter.scanlator.as_deref());
        let [path] = found.as_slice() else {
            if found.len() > 1 {
                tracing::warn!(chapter_id = chapter.id, "several source folders hold this chapter; none was deleted");
            }
            return Ok(false);
        };
        remove(path).map_err(|e| e.to_string())?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.index.remove_chapter(manga.source, &manga.title, &[name]);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::entry::Entry;
    use kuro_data::item::Item;

    struct Fixture {
        dir: tempfile::TempDir,
        db: Arc<Db>,
        index: DownloadIndex,
        manga_id: i64,
    }

    fn fixture(source: i64) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let path = kuro_db::schema_unified::db_path(&dir.path().join("data"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let writer = kuro_db::schema_unified::open_file(&path).unwrap();
        let db = Arc::new(Db::new(&path, writer).unwrap());
        let mut manga = Entry::create(Kind::Manga);
        manga.title = "Manga Title".into();
        manga.source = source;
        let manga_id = EntryRepo::new(Arc::clone(&db)).insert(&manga).unwrap();
        let downloads = dir.path().join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        let index = DownloadIndex::open(&downloads, dir.path());
        Fixture { dir, db, index, manga_id }
    }

    fn add_chapter(f: &Fixture, name: &str, bookmark: bool) -> i64 {
        let mut c = Item::create(Kind::Manga);
        c.entry_id = f.manga_id;
        c.name = name.into();
        c.url = format!("/{name}");
        c.done = true;
        c.bookmark = bookmark;
        ItemRepo::new(Arc::clone(&f.db)).add_all(&[c]).unwrap()[0].id
    }

    fn download(f: &Fixture, source_dir: &str, chapter: &str) -> PathBuf {
        let path = f.index.root().join(source_dir).join("Manga Title").join(chapter);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("001.png"), b"x").unwrap();
        f.index.add_chapter(7, source_dir, "Manga Title", chapter);
        path
    }

    fn deleter(f: &Fixture, settings: Settings) -> DiskDeleter {
        DiskDeleter { db: Arc::clone(&f.db), index: f.index.clone(), queue: QueueFile::in_dir(f.dir.path()), settings }
    }

    fn pending(f: &Fixture, chapter_id: i64) -> Pending {
        Pending { manga_id: f.manga_id, chapter_id }
    }

    #[test]
    fn a_queued_chapter_loses_its_download_and_leaves_the_index_and_the_queue_file() {
        let f = fixture(7);
        let id = add_chapter(&f, "Chapter 1", false);
        let kept = download(&f, "Source (EN)", "Chapter 2");
        let path = download(&f, "Source (EN)", "Chapter 1");
        let queue = QueueFile::in_dir(f.dir.path());
        queue.add(pending(&f, id)).unwrap();
        assert!(f.index.is_chapter_downloaded(7, "Manga Title", "Chapter 1", None));

        assert_eq!(deleter(&f, Settings::default()).run(&queue.list()), 1);
        assert!(!path.exists());
        assert!(kept.exists(), "only the queued chapter goes");
        assert!(!f.index.is_chapter_downloaded(7, "Manga Title", "Chapter 1", None));
        assert!(queue.list().is_empty());
    }

    #[test]
    fn a_cbz_download_is_removed_too() {
        let f = fixture(7);
        let id = add_chapter(&f, "Chapter 1", false);
        let source = f.index.root().join("Source (EN)").join("Manga Title");
        std::fs::create_dir_all(&source).unwrap();
        let cbz = source.join("Chapter 1.cbz");
        std::fs::write(&cbz, b"zip").unwrap();
        assert_eq!(deleter(&f, Settings::default()).run(&[pending(&f, id)]), 1);
        assert!(!cbz.exists());
    }

    #[test]
    fn a_bookmarked_chapter_stays_unless_the_setting_allows_removing_it() {
        let f = fixture(7);
        let id = add_chapter(&f, "Chapter 1", true);
        let path = download(&f, "Source (EN)", "Chapter 1");
        assert_eq!(deleter(&f, Settings::default()).run(&[pending(&f, id)]), 0);
        assert!(path.exists());
        let allowed = Settings { remove_bookmarked: true, ..Settings::default() };
        assert_eq!(deleter(&f, allowed).run(&[pending(&f, id)]), 1);
        assert!(!path.exists());
    }

    #[test]
    fn a_local_manga_and_a_chapter_held_by_two_source_folders_are_never_touched() {
        let local = fixture(kuro_local::LOCAL_SOURCE_ID);
        let id = add_chapter(&local, "Chapter 1", false);
        let path = download(&local, "Source (EN)", "Chapter 1");
        assert_eq!(deleter(&local, Settings::default()).run(&[pending(&local, id)]), 0);
        assert!(path.exists());

        let f = fixture(7);
        let id = add_chapter(&f, "Chapter 1", false);
        let one = download(&f, "Source A (EN)", "Chapter 1");
        let two = download(&f, "Source B (EN)", "Chapter 1");
        assert_eq!(deleter(&f, Settings::default()).run(&[pending(&f, id)]), 0);
        assert!(one.exists() && two.exists());
    }

    #[test]
    fn a_chapter_that_is_not_on_disk_or_is_gone_from_the_database_is_dealt_with() {
        let f = fixture(7);
        let id = add_chapter(&f, "Chapter 1", false);
        let queue = QueueFile::in_dir(f.dir.path());
        queue.add(pending(&f, id)).unwrap();
        queue.add(Pending { manga_id: f.manga_id, chapter_id: 9999 }).unwrap();
        assert_eq!(deleter(&f, Settings::default()).run(&queue.list()), 0);
        assert!(queue.list().is_empty(), "nothing on disk and a chapter that is gone are both dealt with");
    }
}
