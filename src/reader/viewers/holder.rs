//! Rows of a viewer and their live state (T-44a-006, B§8.1, B§11.3, E-44). A page row follows its page's status and
//! download progress through `watch` receivers; a transition row follows the destination chapter's load state. A
//! row's subscriptions end when the row is dropped.
//!
//! The row content is a label until page decoding exists; the state it shows comes from the pure functions here.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use tokio::sync::watch;

use super::status::{PageView, StatusBranch};
use crate::reader::model::{ChapterState, ReaderChapter, ReaderPage, ViewerChapters};
use crate::reader::strings;

/// Running subscriptions. Dropping the guard stops them.
#[derive(Default)]
pub struct Tasks(Vec<glib::JoinHandle<()>>);

impl Drop for Tasks {
    fn drop(&mut self) {
        self.0.drain(..).for_each(|task| task.abort());
    }
}

impl Tasks {
    /// Runs `on_change` on the main loop for every value `rx` takes after this call.
    fn follow<T: 'static>(&mut self, mut rx: watch::Receiver<T>, on_change: impl Fn(&T) + 'static) {
        rx.borrow_and_update();
        self.0.push(glib::spawn_future_local(async move {
            while rx.changed().await.is_ok() {
                on_change(&rx.borrow_and_update());
            }
        }));
    }
}

/// A realised row: the widget in the strip and the subscriptions that keep it current.
pub struct Row {
    pub widget: gtk::Widget,
    _tasks: Tasks,
}

fn label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_valign(gtk::Align::Center);
    label.set_justify(gtk::Justification::Center);
    label
}

/// A row with no state to follow.
pub fn plain_row(text: &str) -> Row {
    Row { widget: label(text).upcast(), _tasks: Tasks::default() }
}

/// The line a page row shows for its holder view.
pub fn page_text(base: &str, view: PageView) -> String {
    match view {
        PageView::Ready => base.to_owned(),
        PageView::Spinner => format!("{base}\n{}", strings::LOADING),
        PageView::Progress(percent) => format!("{base}\n{percent}%"),
        PageView::Error => format!("{base}\n{}", strings::DOWNLOAD_NOTIFIER_TITLE_ERROR),
    }
}

/// The line a transition card adds for the destination chapter's state, if any. `Wait` and `Loading` show the
/// spinner, `Error` the message, and `Loaded` nothing.
pub fn chapter_text(state: &ChapterState) -> Option<String> {
    match state {
        ChapterState::Wait | ChapterState::Loading => Some(strings::LOADING.to_owned()),
        ChapterState::Error(e) if e.message.trim().is_empty() => Some(strings::UNKNOWN_ERROR.to_owned()),
        ChapterState::Error(e) => Some(e.message.clone()),
        ChapterState::Loaded(_) => None,
    }
}

/// The card text for a destination chapter's state: the card line, then the state line when there is one.
pub fn transition_text(base: &str, state: Option<&ChapterState>) -> String {
    match state.and_then(chapter_text) {
        Some(line) => format!("{base}\n{line}"),
        None => base.to_owned(),
    }
}

/// A page row. The row follows `page` (the parent of an insert page) when there is one.
pub fn page_row(base: String, page: Option<Rc<ReaderPage>>) -> Row {
    let Some(page) = page else { return plain_row(&base) };
    let text = label(&base);
    let branch = Rc::new(RefCell::new(StatusBranch::default()));
    let show = {
        let (weak, base) = (text.downgrade(), base.clone());
        move |view: PageView| {
            if let Some(text) = weak.upgrade() {
                text.set_text(&page_text(&base, view));
            }
        }
    };
    let show = Rc::new(show);
    let mut tasks = Tasks::default();
    {
        let (branch, show) = (Rc::clone(&branch), Rc::clone(&show));
        let update = move |status: &_| show(branch.borrow_mut().on_status(*status));
        update(&page.status.get());
        tasks.follow(page.status.subscribe(), update);
    }
    // Progress applies only while the download branch is the latest status (collectLatest, E-44).
    let (progress_branch, progress_show) = (Rc::clone(&branch), Rc::clone(&show));
    let on_progress = move |value: &i32| {
        let view = progress_branch.borrow().on_progress(*value);
        if let Some(view) = view {
            progress_show(view);
        }
    };
    on_progress(&page.progress.get());
    tasks.follow(page.progress.subscribe(), on_progress);
    Row { widget: text.upcast(), _tasks: tasks }
}

/// A transition card. The row follows the destination chapter's state when there is one.
pub fn transition_row(base: String, to: Option<Rc<ReaderChapter>>) -> Row {
    let Some(to) = to else { return plain_row(&base) };
    let text = label(&transition_text(&base, Some(&to.state())));
    let weak = text.downgrade();
    let mut tasks = Tasks::default();
    tasks.follow(to.subscribe_state(), move |state: &ChapterState| {
        if let Some(text) = weak.upgrade() {
            text.set_text(&transition_text(&base, Some(state)));
        }
    });
    Row { widget: text.upcast(), _tasks: tasks }
}

/// Calls `on_change` on the main loop whenever the load state of the current chapter or a neighbour changes
/// (B§11.3), so the viewer can hand its chapters over again. Dropping the guard stops the watch.
pub fn watch_chapters(chapters: &ViewerChapters, on_change: Rc<dyn Fn()>) -> Tasks {
    let mut tasks = Tasks::default();
    for chapter in std::iter::once(&chapters.curr).chain(chapters.prev.iter()).chain(chapters.next.iter()) {
        let on_change = Rc::clone(&on_change);
        tasks.follow(chapter.subscribe_state(), move |_: &ChapterState| on_change());
    }
    tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::model::LoadError;

    #[test]
    fn a_page_row_shows_each_holder_view() {
        assert_eq!(page_text("Page 1", PageView::Ready), "Page 1");
        assert_eq!(page_text("Page 1", PageView::Spinner), "Page 1\nLoading…");
        assert_eq!(page_text("Page 1", PageView::Progress(40)), "Page 1\n40%");
        assert_eq!(page_text("Page 1", PageView::Error), "Page 1\nError");
    }

    #[test]
    fn a_transition_card_follows_the_destination_state() {
        let error = ChapterState::Error(Rc::new(LoadError::new("No pages found")));
        let blank = ChapterState::Error(Rc::new(LoadError::new("  ")));
        assert_eq!(transition_text("Next", Some(&ChapterState::Wait)), "Next\nLoading…");
        assert_eq!(transition_text("Next", Some(&ChapterState::Loading)), "Next\nLoading…");
        assert_eq!(transition_text("Next", Some(&error)), "Next\nNo pages found");
        assert_eq!(transition_text("Next", Some(&blank)), "Next\nUnknown error");
        assert_eq!(transition_text("Next", Some(&ChapterState::Loaded(Rc::default()))), "Next");
        assert_eq!(transition_text("Next", None), "Next");
    }
}
