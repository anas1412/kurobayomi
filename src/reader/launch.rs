//! Opens the reader from the app (T-43a-077): the page the navigation pushes in place of the old placeholder. The
//! root widget keeps the name `screen:reader`, so the sidebar hiding in `main.rs` still works.
//!
//! The entry screens still pass titles, not ids. [`find_ids`] resolves the manga by its title among the favourites
//! and the chapter by its name, and an unknown pair gives ids the reader reports on its failure card.
//!
//! Dependencies on other areas, recorded here so the reader's behaviour is not mistaken for a gap in it:
//! - There is no source registry in the app yet (area 17), so the source gate is open and a chapter has no source
//!   to load from: [`build_loader`] reports "Source not found" on the failure card until the registry, the local
//!   source and the download index are passed in here.
//! - The page viewers (area 44) attach through `ReaderChrome::attach_viewer` on the returned screen.

use std::rc::Rc;
use std::sync::Arc;

use gtk::glib;
use gtk::prelude::*;
use kuro_core::sources::SourceGate;
use kuro_data::chapter::ChapterRepo;
use kuro_data::manga::MangaRepo;
use kuro_db::Db;
use kuro_domain::kind::EntryKind;
use kuro_platform::toast::{Toaster, toast_channel};
use kuro_prefs::reader::ReaderPrefs;
use tokio::sync::watch;

use super::loader::{Loader, LoaderInputs, SourceKind, choose_loader};
use super::model::{LoadError, ReaderChapter};
use super::screen::{ReaderDeps, open};
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

/// Picks the loader of a chapter. Without a source registry there is nothing to pick from (see the module notes).
pub fn build_loader(chapter: &Rc<ReaderChapter>) -> Result<Loader, LoadError> {
    choose_loader(LoaderInputs { download: None, local: None, source: SourceKind::Unknown, chapter_url: chapter.chapter.url.clone() })
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

/// `reader::open(manga_id, chapter_id)`: the reader page for the manga's chapter, closing back to the page below.
pub fn open_chapter(manga_id: i64, chapter_id: i64) -> gtk::Box {
    let app = crate::data::app();
    let store = app.base.store().clone();
    let deps = ReaderDeps {
        db: app.db(EntryKind::Manga),
        manga_id,
        chapter_id,
        prefs: ReaderPrefs::new(store.clone()),
        store: Rc::new(WriterHandle::spawn(app.db(EntryKind::Manga))),
        build: Rc::new(build_loader),
        is_downloaded: Rc::new(|_| false),
        downloaded_only: app.base.downloaded_only.get(),
        incognito: app.base.incognito_mode.get(),
        prefs_store: store,
    };
    open(deps, &open_gate(), toaster(), crate::ui::pop).root
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

    #[test]
    fn the_gate_is_open_even_after_its_sender_is_gone() {
        assert!(*open_gate().ready_watch().borrow());
    }
}
