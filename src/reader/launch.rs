//! Opens the reader from the app (T-43a-077): the page the navigation pushes in place of the old placeholder. The
//! root widget keeps the name `screen:reader`, so the sidebar hiding in `main.rs` still works.
//!
//! The entry screens still pass titles, not ids. [`find_ids`] resolves the manga by its title among the favourites,
//! then the read mangas outside the library, and the chapter by its name or its id (the library's Continue passes ids);
//! an unknown pair gives ids the reader reports on its failure card.
//!
//! Dependencies on other areas, recorded here so the reader's behaviour is not mistaken for a gap in it:
//! - An online chapter reads its pages from the manga's source when the source directory knows it (the recipe
//!   sources and the installed packages): `source_pages` gives the HTTP loader the page list and the images, and the
//!   chapter cache keeps both. Page lists are fetched on
//!   threads (`source_pages::Prefetch`): the gate waits for the opened chapter's list, Retry waits for a new fetch, and
//!   each chapter that loads starts its neighbours' lists. A chapter whose list is not cached yet does not ask the
//!   source on the GTK thread: it reports "still loading" (a wait the reader shows as a spinner, see
//!   `source_pages::is_pending`) or the fetch's error. A chapter on disk still opens from the downloads
//!   folder first, a local manga goes through `local_flow::reader`, and a source the directory does not know reports
//!   "Source not found".
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
use kuro_data::chapter::Chapter;
use kuro_data::entry::{Entry, EntryRepo};
use kuro_data::item::{Item, ItemRepo};
use kuro_data::manga::Manga;
use kuro_db::Db;
use kuro_platform::kind::Kind;
use kuro_downloads::index::DownloadIndex;
use kuro_local::is_local_source;
use kuro_platform::paths::Paths;
use kuro_platform::toast::{Toaster, toast_channel};
use kuro_prefs::reader::ReaderPrefs;
use tokio::sync::watch;

use super::delete_disk::{DiskDeleter, Settings as DeleteSettings};
use super::delete_queue::QueueFile;
use super::download_paths::unique_download;
use super::loader::{Loader, LoaderInputs, SourceKind, choose_loader};
use super::model::{LoadError, ReaderChapter};
use super::screen::{DeleteSink, ReaderDeps, ReaderExtras, open_with};
use super::source_pages::Prefetch;
use super::viewers::downloads::DownloadCheck;
use super::writer::WriterHandle;

/// The manga id and chapter id for a title and a chapter name. `(0, 0)` for an unknown manga, and `(manga, 0)` for
/// an unknown chapter, so the card names what was not found.
/// The manga is a favourite with that title, else a read manga outside the library (History opens those); the chapter
/// is the one with that name, else the one with that id.
pub fn find_ids(db: &Arc<Db>, title: &str, chapter_name: &str) -> (i64, i64) {
    let repo = EntryRepo::new(Arc::clone(db));
    let titled = |list: Vec<Entry>| list.into_iter().find(|m| m.title == title);
    let Some(manga) = titled(repo.favorites(Kind::Manga).unwrap_or_default())
        .or_else(|| titled(repo.watched_not_in_library(Kind::Manga).unwrap_or_default()))
    else {
        return (0, 0);
    };
    let chapters = ItemRepo::new(Arc::clone(db)).by_entry_id(manga.id, false).unwrap_or_default();
    let by_id = chapter_name.parse::<i64>().ok().and_then(|id| chapters.iter().find(|c| c.id == id));
    (manga.id, chapters.iter().find(|c| c.name == chapter_name).or(by_id).map_or(0, |c| c.id))
}

/// Builds the loader with the downloads folder and the manga's title, which locate a downloaded chapter, and the manga's
/// online source, which a chapter that is not on disk is read from once its page list is cached.
fn loader_for(chapter: &Rc<ReaderChapter>, downloads: Option<(&Path, &str)>, online: Option<&Arc<Prefetch>>) -> Result<Loader, LoadError> {
    let download = downloads.and_then(|(root, title)| unique_download(root, title, &chapter.chapter.name, chapter.chapter.scanlator.as_deref()));
    let url = &chapter.chapter.url;
    let source = match online {
        // asking the source here would wait on the network with the window, so a list that is not cached yet fails fast
        Some(prefetch) if download.is_none() && !prefetch.is_cached(url) => return Err(LoadError::new(prefetch.missing(url))),
        Some(prefetch) => SourceKind::Http { source: prefetch.pages(), manga_id: chapter.chapter.entry_id, cache: prefetch.cache() },
        None => SourceKind::Unknown,
    };
    choose_loader(LoaderInputs { download, local: None, source, chapter_url: chapter.chapter.url.clone() })
}

/// Starts fetching the page lists of the chapters next to `chapter` in source order that are not on disk, so moving
/// to them reads the list from the cache.
// ponytail: neighbours in source order, not in the reader's filtered list; a chapter the filter skips over is not
// fetched ahead and takes the fail-fast path. Ask the session for its neighbours if that shows.
fn warm_neighbours(db: &Arc<Db>, prefetch: &Arc<Prefetch>, chapter: &Item, on_disk: &dyn Fn(&Item) -> bool) {
    let mut chapters = ItemRepo::new(Arc::clone(db)).by_entry_id(chapter.entry_id, false).unwrap_or_default();
    chapters.sort_by_key(|c| c.source_order);
    let Some(at) = chapters.iter().position(|c| c.id == chapter.id) else { return };
    for next in [at.checked_sub(1), Some(at + 1)].into_iter().flatten().filter_map(|i| chapters.get(i)) {
        if !on_disk(next) {
            prefetch.fetch(&next.url);
        }
    }
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
pub fn downloaded_items(index: Option<DownloadIndex>, manga: &Entry) -> Arc<dyn Fn(&Item) -> bool + Send + Sync> {
    let (source, title, local) = (manga.source, manga.title.clone(), is_local_source(manga.source));
    match index {
        Some(index) => Arc::new(crate::entry_core::downloaded::downloaded_lookup(index, source, title, local)),
        None => Arc::new(move |_| local),
    }
}

/// [`downloaded_items`] for a caller outside the entry flow that still holds the legacy shapes (`library::mount`).
// seam: deleted in phase E with `Manga` and `Chapter`
pub fn downloaded_lookup(index: Option<DownloadIndex>, manga: &Manga) -> Arc<dyn Fn(&Chapter) -> bool + Send + Sync> {
    let lookup = downloaded_items(index, &Entry::from(manga));
    Arc::new(move |chapter| lookup(&Item::from(chapter)))
}

/// What closing the reader does with the chapters delete-after-read queued: a worker deletes their downloads.
fn delete_sink(db: Arc<Db>, index: DownloadIndex, data_dir: PathBuf) -> DeleteSink {
    Rc::new(move |pending| {
        // The settings diet cut the bookmark and category exclusions of delete-after-read: nothing is kept back.
        let settings = DeleteSettings::default();
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
    let db = app.db.clone();
    let data_dir = kuro_platform::linux::LinuxPaths::system().ok().map(|paths| paths.data_dir());
    let index = crate::shell::host::Shell::get().map(|shell| shell.downloads().clone());
    let manga = EntryRepo::new(Arc::clone(&db)).by_id(manga_id).ok();

    let mut writer = WriterHandle::spawn(Arc::clone(&db));
    if let Some(dir) = &data_dir {
        writer = writer.with_restore_in(dir);
    }
    let (lookup, manga_is_local, title) = match &manga {
        Some(manga) => (downloaded_items(index.clone(), manga), is_local_source(manga.source), manga.title.clone()),
        None => (Arc::new(|_: &Item| false) as Arc<dyn Fn(&Item) -> bool + Send + Sync>, false, String::new()),
    };
    let root = index.as_ref().map(|index| index.root().to_path_buf());
    let online = manga.as_ref().filter(|_| !manga_is_local).and_then(|m| super::source_pages::online(m.source, manga_id));
    // the opened chapter's page list is fetched before the reader asks for it, unless the chapter is on disk; Retry on
    // the failure card fetches it again and waits
    let opened = ItemRepo::new(Arc::clone(&db)).by_id(chapter_id).ok().flatten().filter(|c| !lookup(c));
    let retry_gate: Option<Rc<dyn Fn() -> SourceGate>> = match (&online, opened) {
        (Some(prefetch), Some(chapter)) => {
            let prefetch = Arc::clone(prefetch);
            Some(Rc::new(move || prefetch.fetch(&chapter.url)))
        }
        _ => None,
    };
    let gate = retry_gate.as_ref().map_or_else(open_gate, |fetch| fetch());
    let in_reader = Arc::clone(&lookup);
    let (neighbours_db, on_disk) = (Arc::clone(&db), Arc::clone(&lookup));
    let incognito = app.base.incognito_mode.get();
    let deps = ReaderDeps {
        db: Arc::clone(&db),
        manga_id,
        chapter_id,
        prefs: ReaderPrefs::new(store.clone()),
        store: Rc::new(writer),
        build: Rc::new(move |chapter| {
            let loaded = loader_for(chapter, root.as_deref().map(|root| (root, title.as_str())), online.as_ref());
            if let (Ok(Loader::Http(_)), Some(prefetch)) = (&loaded, &online) {
                warm_neighbours(&neighbours_db, prefetch, &chapter.chapter, &*on_disk);
            }
            loaded
        }),
        is_downloaded: Rc::new(move |chapter| in_reader(chapter)),
        downloaded_only: app.base.downloaded_only.get(),
        incognito,
        prefs_store: store.clone(),
    };
    let extras = ReaderExtras {
        delete_sink: match (&data_dir, &index) {
            (Some(dir), Some(index)) => Some(delete_sink(db, index.clone(), dir.clone())),
            _ => None,
        },
        data_dir,
        downloads: DownloadCheck::new(manga_is_local, lookup),
        // the app starts no download manager yet, so the download-ahead hook has nothing to call
        track_update: Some(Rc::new(move |manga_id, number| {
            crate::entry_core::tracking::progress::item_finished(Kind::Manga, manga_id, number, incognito);
        })),
        download_ahead: None,
        retry_gate,
    };
    open_with(deps, extras, &gate, toaster(), crate::ui::pop).root
}

/// The reader page for the entry screens' title and chapter name.
pub fn page_for(title: &str, chapter_name: &str) -> gtk::Box {
    let (manga_id, chapter_id) = find_ids(&crate::data::app().db.clone(), title, chapter_name);
    open_chapter(manga_id, chapter_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::super::cache::DiskCache;
    use super::super::loader::http::HttpPageSource;

    fn db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let path = kuro_db::schema_unified::db_path(dir.path());
        let writer = kuro_db::schema_unified::open_file(&path).unwrap();
        (dir, Arc::new(Db::new(&path, writer).unwrap()))
    }

    #[test]
    fn a_title_and_chapter_name_resolve_to_their_ids() {
        let (_dir, db) = db();
        let mut manga = Entry::create(Kind::Manga);
        manga.title = "Title 1".into();
        manga.favorite = true;
        let manga_id = EntryRepo::new(Arc::clone(&db)).insert(&manga).unwrap();
        let chapters: Vec<Item> = ["Chapter 1", "Chapter 2"]
            .iter()
            .map(|name| {
                let mut c = Item::create(Kind::Manga);
                c.entry_id = manga_id;
                c.name = (*name).into();
                c.url = format!("/{name}");
                c
            })
            .collect();
        let added = ItemRepo::new(Arc::clone(&db)).add_all(&chapters).unwrap();
        assert_eq!(find_ids(&db, "Title 1", "Chapter 2"), (manga_id, added[1].id));
        assert_eq!(find_ids(&db, "Title 1", "Chapter 9"), (manga_id, 0), "an unknown chapter keeps the manga");
        assert_eq!(find_ids(&db, "Nope", "Chapter 1"), (0, 0));
    }

    fn plain(name: &str) -> Item {
        let mut c = Item::create(Kind::Manga);
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
        let loaded = loader_for(&named("Chapter 1"), Some((root.path(), "Manga Title")), None).ok().expect("the download opens");
        assert!(matches!(loaded, Loader::Download(_)));
        assert_eq!(loader_for(&named("Chapter 2"), Some((root.path(), "Manga Title")), None).err().unwrap().message, "Source not found");
    }

    /// A source whose chapters have one page; it counts the page lists it was asked for.
    struct NoPages(AtomicUsize);

    impl HttpPageSource for NoPages {
        fn page_list(&self, _: &str) -> Result<Vec<super::super::loader::PageEntry>, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(vec![super::super::loader::PageEntry { url: "p0".into(), image_url: None }])
        }
        fn image_url(&self, _: &str) -> Result<String, String> {
            Err("none".into())
        }
        fn image(&self, _: &str) -> Result<Vec<u8>, String> {
            Err("none".into())
        }
    }

    /// A chapter that is not on disk opens through its source's pages once its list is cached. Before that it fails at
    /// once, without asking the source on this thread, and a fetch starts on a thread.
    #[test]
    fn a_chapter_of_an_online_source_opens_through_its_cached_list_unless_it_is_on_disk() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("Source (EN)/Manga Title/Chapter 1")).unwrap();
        std::fs::write(root.path().join("Source (EN)/Manga Title/Chapter 1/001.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(DiskCache::open(dir.path(), super::super::cache::CACHE_LIMIT_BYTES).unwrap());
        let source = Arc::new(NoPages(AtomicUsize::new(0)));
        let online = Prefetch::new(source.clone(), Arc::clone(&cache), 0);
        let downloads = Some((root.path(), "Manga Title"));
        assert!(matches!(loader_for(&named("Chapter 1"), downloads, Some(&online)), Ok(Loader::Download(_))), "the download wins");
        assert_eq!(source.0.load(Ordering::SeqCst), 0, "a download asks the source for nothing");
        assert_eq!(loader_for(&named("Chapter 2"), downloads, Some(&online)).err().unwrap().message, super::super::source_pages::STILL_LOADING);
        let entry = super::super::loader::PageEntry { url: "p0".into(), image_url: None };
        cache.put_page_list(0, "/Chapter 2", &[entry]).unwrap();
        assert!(matches!(loader_for(&named("Chapter 2"), downloads, Some(&online)), Ok(Loader::Http(_))));
    }

    /// The neighbours in source order that are not on disk get their lists fetched.
    #[test]
    fn a_loaded_chapter_fetches_its_neighbours_lists_ahead() {
        let (_dir, db) = db();
        let manga_id = EntryRepo::new(Arc::clone(&db)).insert(&Entry::create(Kind::Manga)).unwrap();
        let chapters: Vec<Item> = (0..4)
            .map(|n| {
                let mut c = plain(&format!("Chapter {n}"));
                c.entry_id = manga_id;
                c.source_order = n;
                c
            })
            .collect();
        let added = ItemRepo::new(Arc::clone(&db)).add_all(&chapters).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(DiskCache::open(dir.path(), super::super::cache::CACHE_LIMIT_BYTES).unwrap());
        let source = Arc::new(NoPages(AtomicUsize::new(0)));
        let prefetch = Prefetch::new(source.clone(), Arc::clone(&cache), manga_id);
        warm_neighbours(&db, &prefetch, &added[1], &|c: &Item| c.name == "Chapter 0");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !prefetch.is_cached("/Chapter 2") {
            assert!(std::time::Instant::now() < deadline, "the next chapter's list was never fetched");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(source.0.load(Ordering::SeqCst), 1, "the previous chapter is on disk and the others are not next to it");
    }

    /// History and the library's Continue reach the reader by title: a read manga outside the library is found, and a
    /// chapter id works in place of the name.
    #[test]
    fn a_non_favourite_read_manga_and_a_chapter_id_resolve() {
        let (_dir, db) = db();
        let mut manga = Entry::create(Kind::Manga);
        manga.title = "Browsed".into();
        let manga_id = EntryRepo::new(Arc::clone(&db)).insert(&manga).unwrap();
        let mut chapter = plain("Chapter 1");
        chapter.entry_id = manga_id;
        chapter.progress = 3;
        let added = ItemRepo::new(Arc::clone(&db)).add_all(&[chapter]).unwrap();
        assert_eq!(find_ids(&db, "Browsed", "Chapter 1"), (manga_id, added[0].id), "History opens a read manga outside the library");
        assert_eq!(find_ids(&db, "Browsed", &added[0].id.to_string()), (manga_id, added[0].id), "Continue passes the chapter id");
        let mut favourite = Entry::create(Kind::Manga);
        favourite.title = "Browsed".into();
        favourite.url = "/favourite".into();
        favourite.favorite = true;
        let favourite_id = EntryRepo::new(Arc::clone(&db)).insert(&favourite).unwrap();
        assert_eq!(find_ids(&db, "Browsed", "Chapter 1"), (favourite_id, 0), "a favourite with the title comes first");
    }

    #[test]
    fn the_lookup_asks_the_index_and_a_local_manga_has_every_chapter() {
        let dir = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(dir.path(), dir.path());
        index.add_chapter(7, "Source (EN)", "Title 1", "Chapter 1");
        let mut manga = Entry::create(Kind::Manga);
        manga.title = "Title 1".into();
        manga.source = 7;
        let lookup = downloaded_items(Some(index.clone()), &manga);
        assert!(lookup(&plain("Chapter 1")));
        assert!(!lookup(&plain("Chapter 2")));
        assert!(!downloaded_items(None, &manga)(&plain("Chapter 1")), "no index, nothing downloaded");
        manga.source = kuro_local::LOCAL_SOURCE_ID;
        assert!(downloaded_items(Some(index), &manga)(&plain("Chapter 9")));
        assert!(downloaded_items(None, &manga)(&plain("Chapter 9")));
        let legacy = downloaded_lookup(None, &Manga::from(manga));
        assert!(legacy(&Chapter::from(plain("Chapter 9"))), "the seam for the library answers the same");
    }

    #[test]
    fn the_gate_is_open_even_after_its_sender_is_gone() {
        assert!(*open_gate().ready_watch().borrow());
    }
}
