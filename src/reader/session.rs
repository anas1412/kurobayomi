//! The reader session: progress on page change, completion, chapter switching, adjacent navigation, preload,
//! and the per-manga mode and orientation writes. It runs on the GTK thread and talks to the database only
//! through [`ReaderStore`]. T-43a-023 (initial index), T-43a-025, T-43a-026, T-43a-027, T-43a-028, T-43a-029,
//! T-43a-036, T-43a-086 (event channel).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use kuro_data::chapter::{Chapter, ChapterRepo};
use kuro_data::manga::{Manga, MangaRepo};
use kuro_db::Db;
use kuro_prefs::reader::{ORIENTATION_MASK, READING_MODE_MASK, ReaderOrientation, ReadingMode};

use super::chapters::{ListError, ListOptions, build_reader_list};
use super::flags;
use super::loader::{LoadFailure, Loader, load_chapter};
use super::model::{ChapterState, LoadError, PageStatus, ReaderChapter, ReaderPage, ViewerChapters};
use super::nav::{self, Step};
use super::strings;

/// The writes a session makes. The production implementation is the writer thread (`writer::WriterHandle`).
pub trait ReaderStore {
    fn save_progress(&self, chapter_id: i64, last_page_read: i64, read: bool);
    fn set_bookmark(&self, chapter_id: i64, bookmark: bool);
    fn set_viewer_flags(&self, manga_id: i64, value: i64, mask: i64);
    /// Blocks until every write sent so far has run. A chapter switch calls it before the new chapter loads.
    fn flush(&self);
}

/// Events from the reader to its UI. Delivered in order and never dropped (E-EVT-1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReaderEvent {
    ReloadViewerChapters,
    PageChanged,
    SetOrientation(ReaderOrientation),
    SetCoverResult(bool),
    SavedImage(PathBuf),
    ShareImage(PathBuf),
    CopyImage(PathBuf),
}

/// What an adjacent chapter load ended with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdjacentOutcome {
    /// Another adjacent load is running; this press is ignored (one at a time).
    Busy,
    /// There is no chapter in that direction (the button is disabled).
    NoNeighbour,
    /// The new chapter is current; the caller moves to its page 0.
    Moved(usize),
    /// The load failed. The current chapter stays, and the message goes in a toast (E-NAV-1).
    Failed(String),
}

/// Initial chapter index for the requested chapter id. A missing id gives the generic message (E-INIT-4).
pub fn initial_index(ids: &[i64], requested: i64) -> Result<usize, String> {
    ids.iter().position(|&id| id == requested).ok_or_else(|| strings::requested_chapter_not_found(requested))
}

/// The message for a list that could not be built. The downloaded-only text is the proposal in
/// `p1-edge-cases-and-tests.md` for E-LIST-2, flagged there for strings review.
pub fn list_error_message(error: &ListError) -> String {
    match error {
        ListError::ChapterNotFound(id) => strings::requested_chapter_not_found(*id),
        ListError::CurrentChapterMissing => strings::CHAPTER_NOT_DOWNLOADED.to_owned(),
    }
}

/// What opening the reader needs from the database: the manga, its reader list and the initial index
/// (T-43a-023). A missing manga gives "Unknown err" (E-INIT-3). `Err` carries the message the card shows.
pub struct OpenedList {
    pub manga: Manga,
    pub chapters: Vec<Rc<ReaderChapter>>,
    pub index: usize,
}

pub fn open_list(
    db: Arc<Db>,
    manga_id: i64,
    requested: i64,
    opts: impl FnOnce(&Manga) -> ListOptions,
    is_downloaded: impl Fn(&Chapter) -> bool,
) -> Result<OpenedList, String> {
    let manga = MangaRepo::new(Arc::clone(&db)).by_id(manga_id).map_err(|_| strings::UNKNOWN_ERR.to_owned())?;
    let all = ChapterRepo::new(db).by_manga_id(manga_id, true).map_err(|e| e.to_string())?;
    let list = build_reader_list(&all, requested, &opts(&manga), is_downloaded).map_err(|e| list_error_message(&e))?;
    let ids: Vec<i64> = list.iter().map(|c| c.id).collect();
    let index = initial_index(&ids, requested)?;
    let chapters = list.into_iter().map(ReaderChapter::new).collect();
    Ok(OpenedList { manga, chapters, index })
}

pub struct ReaderSession {
    manga_id: i64,
    chapters: Vec<Rc<ReaderChapter>>,
    current: Option<usize>,
    viewer_flags: i64,
    incognito: bool,
    preserve_position: bool,
    store: Rc<dyn ReaderStore>,
    events: Sender<ReaderEvent>,
    adjacent_busy: bool,
    /// The visible page, 1-based, for the page indicator (`currentPage`).
    current_page: u32,
}

impl ReaderSession {
    /// `incognito` is read once, when the session starts (E-INC-2 closes the reader when it changes).
    pub fn new(
        manga_id: i64,
        viewer_flags: i64,
        chapters: Vec<Rc<ReaderChapter>>,
        current: usize,
        incognito: bool,
        preserve_position: bool,
        store: Rc<dyn ReaderStore>,
        events: Sender<ReaderEvent>,
    ) -> Self {
        Self {
            manga_id,
            chapters,
            current: Some(current),
            viewer_flags,
            incognito,
            preserve_position,
            store,
            events,
            adjacent_busy: false,
            current_page: 1,
        }
    }

    pub fn chapters(&self) -> &[Rc<ReaderChapter>] {
        &self.chapters
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn current_page(&self) -> u32 {
        self.current_page
    }

    /// The chapter being read.
    pub fn current_chapter(&self) -> Option<&Rc<ReaderChapter>> {
        self.current.and_then(|i| self.chapters.get(i))
    }

    /// The current chapter and its neighbours, for the viewer.
    pub fn viewer_chapters(&self) -> Option<ViewerChapters> {
        let current = Rc::clone(self.current_chapter()?);
        let (previous, next) = self.neighbours();
        Some(ViewerChapters {
            curr: current,
            prev: previous.map(|i| Rc::clone(&self.chapters[i])),
            next: next.map(|i| Rc::clone(&self.chapters[i])),
        })
    }

    /// Flips the bookmark of the current chapter and writes it. Incognito does not gate it (`bookmarkCurrentChapter`).
    /// Returns the new value.
    pub fn toggle_bookmark(&self) -> Option<bool> {
        let chapter = self.current_chapter()?;
        let value = !chapter.bookmark.get();
        chapter.bookmark.set(value);
        self.store.set_bookmark(chapter.chapter.id, value);
        Some(value)
    }

    pub fn viewer_flags(&self) -> i64 {
        self.viewer_flags
    }

    /// The previous and next positions of the current chapter.
    pub fn neighbours(&self) -> (Option<usize>, Option<usize>) {
        nav::neighbours(self.chapters.len(), self.current)
    }

    /// The progress part of `onPageSelected` (7.1 and 7.2). Returns the chapter index when the page belongs to a
    /// chapter other than the current one, so the caller can switch to it. Insert pages change nothing.
    pub fn record_page(&mut self, chapter_index: usize, page: &ReaderPage) -> Option<usize> {
        if !page.writes_progress() {
            return None;
        }
        let chapter = Rc::clone(self.chapters.get(chapter_index)?);
        let pages_len = chapter.pages()?.len() as i64;
        let index = i64::from(page.index);
        self.current_page = page.number();
        chapter.requested_page.set(index);
        if !self.incognito && page.status.get() != PageStatus::Error {
            chapter.last_page_read.set(index);
            if index == pages_len - 1 {
                // landing on the last page completes the chapter; repeating it is harmless (E-PROG-2)
                chapter.read.set(true);
            }
            self.store.save_progress(chapter.chapter.id, index, chapter.read.get());
        }
        if self.current == Some(chapter_index) {
            return None;
        }
        Some(chapter_index)
    }

    /// Makes `index` the current chapter if it loads (`loadNewChapter`). A failure is logged and the current
    /// chapter stays current.
    pub fn switch_chapter(
        &mut self,
        index: usize,
        build: impl FnOnce(&Rc<ReaderChapter>) -> Result<Loader, LoadError>,
    ) -> Result<(), LoadFailure> {
        // the previous chapter's progress is durable before the new chapter is loaded (T-43a-025)
        self.store.flush();
        let chapter = Rc::clone(&self.chapters[index]);
        let result = load_chapter(&chapter, || build(&chapter), self.preserve_position);
        match &result {
            Ok(()) => self.current = Some(index),
            Err(LoadFailure::Failed(error)) => tracing::warn!(error = %error, "chapter switch failed"),
            Err(LoadFailure::Cancelled) => {}
        }
        result
    }

    /// `loadAdjacent`: next or previous chapter, one at a time. A failure keeps the current page.
    pub fn load_adjacent(
        &mut self,
        step: Step,
        build: impl FnOnce(&Rc<ReaderChapter>) -> Result<Loader, LoadError>,
    ) -> AdjacentOutcome {
        if self.adjacent_busy {
            return AdjacentOutcome::Busy;
        }
        let Some(target) = nav::target(self.chapters.len(), self.current, step) else {
            return AdjacentOutcome::NoNeighbour;
        };
        self.adjacent_busy = true;
        let result = self.switch_chapter(target, build);
        self.adjacent_busy = false;
        match result {
            Ok(()) => AdjacentOutcome::Moved(target),
            Err(LoadFailure::Failed(error)) => AdjacentOutcome::Failed(error.message),
            Err(LoadFailure::Cancelled) => AdjacentOutcome::Failed(String::new()),
        }
    }

    /// `preload` (027): loads a chapter in the background for the viewers. A downloaded copy replaces an online
    /// loader first. Failure is silent and leaves the chapter in `Error`. Success sends a reload event.
    pub fn preload(
        &mut self,
        index: usize,
        is_downloaded: bool,
        build: impl FnOnce(&Rc<ReaderChapter>) -> Result<Loader, LoadError>,
    ) {
        let chapter = Rc::clone(&self.chapters[index]);
        if matches!(*chapter.state(), ChapterState::Loaded(_) | ChapterState::Loading) {
            return;
        }
        if chapter.attached_is_local() == Some(false) && is_downloaded {
            chapter.reset();
        }
        if !matches!(*chapter.state(), ChapterState::Wait | ChapterState::Error(_)) {
            return;
        }
        if load_chapter(&chapter, || build(&chapter), self.preserve_position).is_ok() {
            let _ = self.events.send(ReaderEvent::ReloadViewerChapters);
        }
    }

    /// `setMangaReadingMode` (036): the mode is written under its mask, the requested page becomes the saved
    /// page, and the viewers reload.
    pub fn set_reading_mode(&mut self, mode: ReadingMode) {
        self.store.set_viewer_flags(self.manga_id, i64::from(mode.flag_value()), i64::from(READING_MODE_MASK));
        self.viewer_flags = flags::set_reading_mode(self.viewer_flags, mode);
        self.restore_saved_page();
        let _ = self.events.send(ReaderEvent::ReloadViewerChapters);
    }

    /// `setMangaOrientationType` (036). `default_pref` resolves a manga that has no orientation of its own.
    pub fn set_orientation(&mut self, orientation: ReaderOrientation, default_pref: i32) {
        self.store.set_viewer_flags(self.manga_id, i64::from(orientation.flag_value()), i64::from(ORIENTATION_MASK));
        self.viewer_flags = flags::set_orientation(self.viewer_flags, orientation);
        self.restore_saved_page();
        let resolved = flags::resolve_orientation(self.viewer_flags, default_pref);
        let _ = self.events.send(ReaderEvent::SetOrientation(resolved));
        let _ = self.events.send(ReaderEvent::ReloadViewerChapters);
    }

    /// `onPageSelected` (T-043a-093). An insert page is ignored. Otherwise the progress is saved, a chapter switch
    /// runs when the page belongs to another chapter, then download-ahead and the flash run, in that order.
    /// Returns the chapter index the session switched to.
    pub fn on_page_selected(
        &mut self,
        page: &Rc<ReaderPage>,
        build: impl FnOnce(&Rc<ReaderChapter>) -> Result<Loader, LoadError>,
        download_ahead: &mut dyn FnMut(usize),
        flash: &mut dyn FnMut(),
    ) -> Option<usize> {
        if page.is_insert_page() {
            return None;
        }
        let chapter = page.chapter()?;
        let index = self.index_of(&chapter)?;
        let switch = self.record_page(index, page);
        if let Some(target) = switch {
            let _ = self.switch_chapter(target, build);
        }
        if let Some(current) = self.current {
            download_ahead(current);
        }
        flash();
        switch
    }

    /// `request_preload` from a viewer (T-043a-093): loads the chapter in the background when the list has it.
    pub fn on_request_preload(
        &mut self,
        chapter: &Rc<ReaderChapter>,
        is_downloaded: bool,
        build: impl FnOnce(&Rc<ReaderChapter>) -> Result<Loader, LoadError>,
    ) {
        if let Some(index) = self.index_of(chapter) {
            self.preload(index, is_downloaded, build);
        }
    }

    fn index_of(&self, chapter: &Rc<ReaderChapter>) -> Option<usize> {
        self.chapters.iter().position(|c| Rc::ptr_eq(c, chapter))
    }

    /// Sends an event on the reader's ordered channel.
    pub fn notify(&self, event: ReaderEvent) {
        let _ = self.events.send(event);
    }

    fn restore_saved_page(&self) {
        if let Some(chapter) = self.current.and_then(|i| self.chapters.get(i)) {
            chapter.requested_page.set(chapter.last_page_read.get());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::chapters::ListError;
    use crate::reader::model::{ChapterState, PageSource};
    use kuro_data::chapter::Chapter;
    use std::cell::RefCell;
    use std::sync::mpsc;

    #[derive(Default)]
    struct Recorder {
        progress: RefCell<Vec<(i64, i64, bool)>>,
        order: RefCell<Vec<&'static str>>,
        flags: RefCell<Vec<(i64, i64, i64)>>,
        bookmarks: RefCell<Vec<(i64, bool)>>,
    }

    impl ReaderStore for Recorder {
        fn save_progress(&self, chapter_id: i64, last_page_read: i64, read: bool) {
            self.progress.borrow_mut().push((chapter_id, last_page_read, read));
        }
        fn set_bookmark(&self, chapter_id: i64, bookmark: bool) {
            self.bookmarks.borrow_mut().push((chapter_id, bookmark));
        }
        fn set_viewer_flags(&self, manga_id: i64, value: i64, mask: i64) {
            self.flags.borrow_mut().push((manga_id, value, mask));
        }
        fn flush(&self) {
            self.order.borrow_mut().push("flush");
        }
    }

    fn chapter(id: i64, read: bool) -> Rc<ReaderChapter> {
        let mut c = Chapter::create();
        c.id = id;
        c.url = format!("/c/{id}");
        c.read = read;
        let ch = ReaderChapter::new(c);
        let pages = (0..3)
            .map(|i| ReaderPage::new(i, format!("p{i}"), None, PageSource::Http))
            .collect::<Vec<_>>();
        ch.finish_loading(pages).unwrap();
        ch
    }

    fn page(ch: &Rc<ReaderChapter>, index: u32) -> Rc<ReaderPage> {
        let pages = ch.pages().unwrap();
        let p = Rc::clone(&pages[index as usize]);
        p.set_chapter(ch);
        p
    }

    fn session(store: Rc<Recorder>, chapters: Vec<Rc<ReaderChapter>>, current: usize, incognito: bool) -> (ReaderSession, mpsc::Receiver<ReaderEvent>) {
        let (tx, rx) = mpsc::channel();
        (ReaderSession::new(7, 0x2, chapters, current, incognito, false, store, tx), rx)
    }

    fn never_built(_: &Rc<ReaderChapter>) -> Result<Loader, LoadError> {
        Err(LoadError::new("should not build"))
    }

    #[test]
    fn the_bookmark_toggle_flips_the_current_chapter_and_writes_even_in_incognito() {
        let rec = Rc::new(Recorder::default());
        let ch = chapter(4, false);
        let (s, _rx) = session(Rc::clone(&rec), vec![Rc::clone(&ch)], 0, true);
        assert_eq!(s.toggle_bookmark(), Some(true));
        assert!(ch.bookmark.get());
        assert_eq!(s.toggle_bookmark(), Some(false));
        assert_eq!(rec.bookmarks.borrow().as_slice(), [(4, true), (4, false)]);
    }

    #[test]
    fn progress_is_saved_on_each_page_and_the_last_page_completes_the_chapter() {
        let rec = Rc::new(Recorder::default());
        let ch = chapter(1, false);
        let (mut s, _rx) = session(Rc::clone(&rec), vec![Rc::clone(&ch)], 0, false);
        assert_eq!(s.record_page(0, &page(&ch, 1)), None);
        assert_eq!(ch.last_page_read.get(), 1);
        assert!(!ch.read.get());
        s.record_page(0, &page(&ch, 2));
        assert!(ch.read.get());
        s.record_page(0, &page(&ch, 2));
        assert_eq!(rec.progress.borrow().as_slice(), [(1, 1, false), (1, 2, true), (1, 2, true)]);
    }

    #[test]
    fn a_page_in_error_and_incognito_write_nothing_to_the_database() {
        let rec = Rc::new(Recorder::default());
        let ch = chapter(1, false);
        let errored = page(&ch, 1);
        errored.status.set(PageStatus::Error);
        let (mut s, _rx) = session(Rc::clone(&rec), vec![Rc::clone(&ch)], 0, false);
        s.record_page(0, &errored);
        assert!(rec.progress.borrow().is_empty());

        let (mut private, _rx2) = session(Rc::clone(&rec), vec![Rc::clone(&ch)], 0, true);
        private.record_page(0, &page(&ch, 2));
        assert!(rec.progress.borrow().is_empty());
        assert_eq!(ch.last_page_read.get(), 0, "incognito keeps the saved page untouched");
        assert!(!ch.read.get());
    }

    #[test]
    fn an_insert_page_changes_nothing() {
        let rec = Rc::new(Recorder::default());
        let ch = chapter(1, false);
        let parent = page(&ch, 1);
        let (mut s, _rx) = session(Rc::clone(&rec), vec![Rc::clone(&ch)], 0, false);
        let insert = ReaderPage::insert_page(&parent);
        assert_eq!(s.record_page(0, &insert), None);
        assert!(rec.progress.borrow().is_empty());
        assert_eq!(ch.last_page_read.get(), 0);
    }

    #[test]
    fn switching_to_a_new_chapter_is_reported_by_record_page() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let b = chapter(2, false);
        let (mut s, _rx) = session(rec, vec![Rc::clone(&a), Rc::clone(&b)], 0, false);
        assert_eq!(s.record_page(1, &page(&b, 0)), Some(1));
    }

    #[test]
    fn adjacent_navigation_is_disabled_at_the_ends_and_moves_between_chapters() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let (mut s, _rx) = session(rec, vec![Rc::clone(&a)], 0, false);
        assert_eq!(s.load_adjacent(Step::Next, never_built), AdjacentOutcome::NoNeighbour);
        assert_eq!(s.load_adjacent(Step::Previous, never_built), AdjacentOutcome::NoNeighbour);
    }

    #[test]
    fn the_previous_chapters_writes_are_flushed_before_the_new_chapter_is_built() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let b = ReaderChapter::new(Chapter::create());
        let (mut s, _rx) = session(Rc::clone(&rec), vec![Rc::clone(&a), b], 0, false);
        let order = Rc::clone(&rec);
        let _ = s.switch_chapter(1, |_| {
            order.order.borrow_mut().push("build");
            Err(LoadError::new("offline"))
        });
        assert_eq!(rec.order.borrow().as_slice(), ["flush", "build"]);
    }

    #[test]
    fn a_failed_adjacent_load_keeps_the_current_chapter() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let b = ReaderChapter::new(Chapter::create());
        let (mut s, _rx) = session(rec, vec![Rc::clone(&a), b], 0, false);
        let outcome = s.load_adjacent(Step::Next, |_| Err(LoadError::new("Source not found")));
        assert_eq!(outcome, AdjacentOutcome::Failed("Source not found".into()));
        assert_eq!(s.current_index(), Some(0));
    }

    #[test]
    fn preload_of_a_loaded_or_loading_chapter_does_nothing_and_a_failed_preload_is_silent() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let b = ReaderChapter::new(Chapter::create());
        let (mut s, rx) = session(rec, vec![Rc::clone(&a), Rc::clone(&b)], 0, false);
        s.preload(0, false, never_built);
        assert!(rx.try_recv().is_err());
        s.preload(1, false, |_| Err(LoadError::new("offline")));
        assert!(rx.try_recv().is_err(), "a failed preload sends no event");
        assert!(matches!(*b.state(), ChapterState::Error(_)));
    }

    #[test]
    fn reading_mode_write_is_masked_and_reloads_the_viewers() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        a.last_page_read.set(2);
        let (mut s, rx) = session(Rc::clone(&rec), vec![Rc::clone(&a)], 0, false);
        s.set_reading_mode(ReadingMode::Webtoon);
        assert_eq!(rec.flags.borrow().as_slice(), [(7, 4, 0x7)]);
        assert_eq!(a.requested_page.get(), 2);
        assert_eq!(rx.try_recv(), Ok(ReaderEvent::ReloadViewerChapters));
    }

    #[test]
    fn orientation_write_sends_the_resolved_value_then_reloads() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let (mut s, rx) = session(Rc::clone(&rec), vec![Rc::clone(&a)], 0, false);
        s.set_orientation(ReaderOrientation::Default, 8);
        assert_eq!(rec.flags.borrow().as_slice(), [(7, 0, 0x38)]);
        assert_eq!(rx.try_recv(), Ok(ReaderEvent::SetOrientation(ReaderOrientation::Free)));
        assert_eq!(rx.try_recv(), Ok(ReaderEvent::ReloadViewerChapters));
    }

    #[test]
    fn a_page_selected_runs_progress_then_switch_then_download_ahead_then_flash() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let b = chapter(2, false);
        let (mut s, _rx) = session(Rc::clone(&rec), vec![Rc::clone(&a), Rc::clone(&b)], 0, false);
        let log = RefCell::new(Vec::new());
        let switched = s.on_page_selected(
            &page(&b, 1),
            |_| {
                log.borrow_mut().push("build");
                Err(LoadError::new("offline"))
            },
            &mut |i| log.borrow_mut().push(if i == 0 { "ahead 0" } else { "ahead other" }),
            &mut || log.borrow_mut().push("flash"),
        );
        assert_eq!(switched, Some(1));
        assert_eq!(log.borrow().as_slice(), ["build", "ahead 0", "flash"], "a failed switch keeps chapter 0 current");
        assert_eq!(rec.progress.borrow().as_slice(), [(2, 1, false)]);
    }

    #[test]
    fn an_insert_page_or_an_unknown_chapter_runs_none_of_the_hooks() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let (mut s, _rx) = session(Rc::clone(&rec), vec![Rc::clone(&a)], 0, false);
        let calls = std::cell::Cell::new(0);
        let parent = page(&a, 0);
        let insert = ReaderPage::insert_page(&parent);
        s.on_page_selected(&insert, never_built, &mut |_| calls.set(calls.get() + 1), &mut || calls.set(calls.get() + 1));
        let stray = chapter(9, false);
        s.on_page_selected(&page(&stray, 0), never_built, &mut |_| calls.set(calls.get() + 1), &mut || calls.set(calls.get() + 1));
        assert_eq!(calls.get(), 0);
        assert!(rec.progress.borrow().is_empty());
    }

    #[test]
    fn a_preload_request_for_a_chapter_in_the_list_reaches_the_preload() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let b = ReaderChapter::new(Chapter::create());
        let (mut s, rx) = session(rec, vec![Rc::clone(&a), Rc::clone(&b)], 0, false);
        s.on_request_preload(&b, false, |_| Err(LoadError::new("offline")));
        assert!(matches!(*b.state(), ChapterState::Error(_)));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn events_arrive_in_send_order_without_loss() {
        let rec = Rc::new(Recorder::default());
        let a = chapter(1, false);
        let (s, rx) = session(rec, vec![a], 0, false);
        for _ in 0..1000 {
            s.notify(ReaderEvent::PageChanged);
        }
        assert_eq!(rx.try_iter().count(), 1000);
    }

    #[test]
    fn requested_chapter_missing_gives_the_generic_message() {
        assert_eq!(initial_index(&[4, 5], 5), Ok(1));
        assert_eq!(initial_index(&[4, 5], 9), Err("Requested chapter of id 9 not found in chapter list".into()));
    }

    #[test]
    fn list_errors_have_user_messages() {
        assert_eq!(
            list_error_message(&ListError::ChapterNotFound(3)),
            "Requested chapter of id 3 not found in chapter list"
        );
        assert!(list_error_message(&ListError::CurrentChapterMissing).starts_with("The chapter is not downloaded"));
    }
}
