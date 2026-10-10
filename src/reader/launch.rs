//! Opens the reader from the app (T-43a-077): the page the navigation pushes in place of the old placeholder. The
//! root widget keeps the name `screen:reader`, so the sidebar hiding in `main.rs` still works.
//!
//! The entry screens still pass titles, not ids. [`find_ids`] resolves the manga by its title among the favourites
//! and the chapter by its name, and an unknown pair gives ids the reader reports on its failure card.
//!
//! Dependencies on other areas, recorded here so the reader's behaviour is not mistaken for a gap in it:
//! - There is no source registry in the app yet (area 17), so the source gate is open and an online chapter has no
//!   source to load pages from: [`build_loader`] reports "Source not found" on the failure card for it. Real page
//!   data therefore reaches the reader only for a chapter that is on disk, which [`build_loader`] opens from the
//!   downloads folder (the one source folder that holds it), and for a local manga (`local_flow::reader`). The online
//!   path (`SourceKind::Http`, the data saver and the page-list cache) is complete in `loader` and waits for the
//!   registry to hand it a source; nothing in the reader changes when it does.
//! - The download index and the downloads folder come from the shell. A chapter counts as downloaded when the index
//!   knows it, a local manga counts every chapter as downloaded, and the transition cards use the same lookup.
//! - The page viewers (area 44) are attached by the screen itself once the chapter has loaded (`screen::install_viewer`),
//!   and swapped when the reading mode changes.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gtk::glib;
use gtk::prelude::*;
use kuro_core::sources::SourceGate;
use kuro_data::chapter::{Chapter, ChapterRepo};
use kuro_data::manga::{Manga, MangaRepo};
use kuro_db::Db;
use kuro_domain::download::prefs::DownloadPrefs;
use kuro_domain::kind::EntryKind;
use kuro_downloads::index::DownloadIndex;
use kuro_local::is_local_source;
use kuro_platform::paths::Paths;
use kuro_platform::toast::{Toaster, toast_channel};
use kuro_prefs::reader::ReaderPrefs;
use kuro_prefs::store::PrefStore;
use tokio::sync::watch;

use super::delete_disk::{DiskDeleter, Settings as DeleteSettings};
use super::delete_queue::QueueFile;
use super::download_paths::unique_download;
use super::loader::{Loader, LoaderInputs, SourceKind, choose_loader};
use super::model::{LoadError, ReaderChapter};
use super::screen::{DeleteSink, ReaderDeps, ReaderExtras, open_with};
use super::viewers::downloads::DownloadCheck;
use super::writer::WriterHandle;

/// The manga id and chapter id for a title and a chapter name. `(0, 0)` for an unknown manga, and `(manga, 0)` for
/// an unknown chapter, so the card names what was not found.
pub fn find_ids(db: &Arc<Db>, title: &str, chapter_name: &str) -> (i64, i64) {
    let Some(manga) = MangaRepo::new(Arc::clone(db)).favorites().unwrap_or_default().into_iter().find(|m| m.title == title) else {
        return (0, 0);
    };
    let chapters = ChapterRepo::new(Arc::clone(db)).by_manga_id(manga.id, false).unwrap_or_default();
    (manga.id, chapters.iter().find(|c| c.name == chapter_name).map_or(0, |c| c.id))
}

/// Picks the loader of a chapter. Without a source registry there is no source to pick, so an online chapter reports
/// "Source not found" (see the module notes); a chapter that is on disk is read from its download.
pub fn build_loader(chapter: &Rc<ReaderChapter>) -> Result<Loader, LoadError> {
    loader_for(chapter, None)
}

/// [`build_loader`] with the downloads folder and the manga's title, which locate a downloaded chapter.
fn loader_for(chapter: &Rc<ReaderChapter>, downloads: Option<(&Path, &str)>) -> Result<Loader, LoadError> {
    let download = downloads.and_then(|(root, title)| unique_download(root, title, &chapter.chapter.name, chapter.chapter.scanlator.as_deref()));
    choose_loader(LoaderInputs { download, local: None, source: SourceKind::Unknown, chapter_url: chapter.chapter.url.clone() })
}

/// A gate that is already open: the app has no source registry to wait for yet.
pub fn open_gate() -> SourceGate {
    let (_closed_sender, ready) = watch::channel(true);
    SourceGate::new(ready)
}

/// A toaster whose messages go to the window's toast overlay.
fn toaster() -> Toaster {
    let (toaster, feed) = toast_channel();
    glib::spawn_future_local(async move {
        while let Some(message) = feed.next().await {
            crate::toast::notify(message.text);
        }
    });
    toaster
}

/// The download lookup of one manga: a local manga has every chapter, and any other asks the index by the chapter's
/// name and scanlator. Without an index (before the shell exists) nothing is downloaded.
pub fn downloaded_lookup(index: Option<DownloadIndex>, manga: &Manga) -> Arc<dyn Fn(&Chapter) -> bool + Send + Sync> {
    let (source, title, local) = (manga.source, manga.title.clone(), is_local_source(manga.source));
    match index {
        Some(index) => Arc::new(crate::entry_core::downloaded::downloaded_lookup(index, source, title, local)),
        None => Arc::new(move |_| local),
    }
}

/// What closing the reader does with the chapters delete-after-read queued: a worker deletes their downloads.
fn delete_sink(db: Arc<Db>, index: DownloadIndex, data_dir: PathBuf, store: PrefStore) -> DeleteSink {
    Rc::new(move |pending| {
        let prefs = DownloadPrefs::new(&store);
        let settings = DeleteSettings {
            remove_bookmarked: prefs.remove_bookmarked.get(),
            exclude_categories: prefs.remove_exclude_manga_categories.get().0.into_iter().collect(),
        };
        let deleter = DiskDeleter { db: Arc::clone(&db), index: index.clone(), queue: QueueFile::in_dir(&data_dir), settings };
        std::thread::spawn(move || {
            deleter.run(&pending);
        });
    })
}

/// `reader::open(manga_id, chapter_id)`: the reader page for the manga's chapter, closing back to the page below. The
/// crash-restore record, the delete queue and the download index come from the platform data directory and the shell.
pub fn open_chapter(manga_id: i64, chapter_id: i64) -> gtk::Box {
    let app = crate::data::app();
    let store = app.base.store().clone();
    let db = app.db(EntryKind::Manga);
    let data_dir = kuro_platform::linux::LinuxPaths::system().ok().map(|paths| paths.data_dir());
    let index = crate::shell::host::Shell::get().map(|shell| shell.downloads().clone());
    let manga = MangaRepo::new(Arc::clone(&db)).by_id(manga_id).ok();

    let mut writer = WriterHandle::spawn(Arc::clone(&db));
    if let Some(dir) = &data_dir {
        writer = writer.with_restore_in(dir);
    }
    let (lookup, manga_is_local, title) = match &manga {
        Some(manga) => (downloaded_lookup(index.clone(), manga), is_local_source(manga.source), manga.title.clone()),
        None => (Arc::new(|_: &Chapter| false) as Arc<dyn Fn(&Chapter) -> bool + Send + Sync>, false, String::new()),
    };
    let root = index.as_ref().map(|index| index.root().to_path_buf());
    let in_reader = Arc::clone(&lookup);
    let deps = ReaderDeps {
        db: Arc::clone(&db),
        manga_id,
        chapter_id,
        prefs: ReaderPrefs::new(store.clone()),
        store: Rc::new(writer),
        build: Rc::new(move |chapter| loader_for(chapter, root.as_deref().map(|root| (root, title.as_str())))),
        is_downloaded: Rc::new(move |chapter| in_reader(chapter)),
        downloaded_only: app.base.downloaded_only.get(),
        incognito: app.base.incognito_mode.get(),
        prefs_store: store.clone(),
    };
    let extras = ReaderExtras {
        delete_sink: match (&data_dir, &index) {
            (Some(dir), Some(index)) => Some(delete_sink(db, index.clone(), dir.clone(), store)),
            _ => None,
        },
        data_dir,
        downloads: DownloadCheck::new(manga_is_local, lookup),
        // the app starts no tracker layer and no download manager yet, so these have nothing to call
        track_update: None,
        download_ahead: None,
    };
    open_with(deps, extras, &open_gate(), toaster(), crate::ui::pop).root
}

/// The reader page for the entry screens' title and chapter name.
pub fn page_for(title: &str, chapter_name: &str) -> gtk::Box {
    let (manga_id, chapter_id) = find_ids(&crate::data::app().db(EntryKind::Manga), title, chapter_name);
    open_chapter(manga_id, chapter_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::chapter::Chapter;
    use kuro_data::manga::Manga;

    fn db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let path = kuro_db::schema_manga::db_path(dir.path());
        let writer = kuro_db::schema_manga::open(&path).unwrap();
        (dir, Arc::new(Db::new(&path, writer).unwrap()))
    }

    #[test]
    fn a_title_and_chapter_name_resolve_to_their_ids() {
        let (_dir, db) = db();
        let mut manga = Manga::create();
        manga.title = "Title 1".into();
        manga.favorite = true;
        let manga_id = MangaRepo::new(Arc::clone(&db)).insert(&manga).unwrap();
        let chapters: Vec<Chapter> = ["Chapter 1", "Chapter 2"]
            .iter()
            .map(|name| {
                let mut c = Chapter::create();
                c.manga_id = manga_id;
                c.name = (*name).into();
                c.url = format!("/{name}");
                c
            })
            .collect();
        let added = ChapterRepo::new(Arc::clone(&db)).add_all(&chapters).unwrap();
        assert_eq!(find_ids(&db, "Title 1", "Chapter 2"), (manga_id, added[1].id));
        assert_eq!(find_ids(&db, "Title 1", "Chapter 9"), (manga_id, 0), "an unknown chapter keeps the manga");
        assert_eq!(find_ids(&db, "Nope", "Chapter 1"), (0, 0));
    }

    #[test]
    fn without_a_source_the_loader_choice_reports_source_not_found() {
        let chapter = ReaderChapter::new(Chapter::create());
        assert_eq!(build_loader(&chapter).err().unwrap().message, "Source not found");
    }

    fn plain(name: &str) -> Chapter {
        let mut c = Chapter::create();
        c.name = name.into();
        c.url = format!("/{name}");
        c
    }

    fn named(name: &str) -> Rc<ReaderChapter> {
        ReaderChapter::new(plain(name))
    }

    #[test]
    fn a_chapter_on_disk_opens_from_its_download_and_any_other_reports_source_not_found() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("Source (EN)/Manga Title/Chapter 1");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("001.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        let loaded = loader_for(&named("Chapter 1"), Some((root.path(), "Manga Title"))).ok().expect("the download opens");
        assert!(matches!(loaded, Loader::Download(_)));
        assert_eq!(loader_for(&named("Chapter 2"), Some((root.path(), "Manga Title"))).err().unwrap().message, "Source not found");
    }

    #[test]
    fn the_lookup_asks_the_index_and_a_local_manga_has_every_chapter() {
        let dir = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(dir.path(), dir.path());
        index.add_chapter(7, "Source (EN)", "Title 1", "Chapter 1");
        let mut manga = Manga::create();
        manga.title = "Title 1".into();
        manga.source = 7;
        let lookup = downloaded_lookup(Some(index.clone()), &manga);
        assert!(lookup(&plain("Chapter 1")));
        assert!(!lookup(&plain("Chapter 2")));
        assert!(!downloaded_lookup(None, &manga)(&plain("Chapter 1")), "no index, nothing downloaded");
        manga.source = kuro_local::LOCAL_SOURCE_ID;
        assert!(downloaded_lookup(Some(index), &manga)(&plain("Chapter 9")));
        assert!(downloaded_lookup(None, &manga)(&plain("Chapter 9")));
    }

    #[test]
    fn the_gate_is_open_even_after_its_sender_is_gone() {
        assert!(*open_gate().ready_watch().borrow());
    }
}
