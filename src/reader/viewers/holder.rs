//! Rows of a viewer and their live state (T-44a-006, T-44a-062, T-44a-064, T-44a-065, B§8.1, B§11.3, E-44). A page row
//! follows its page's status and download progress through `watch` receivers; a transition row follows the destination
//! chapter's load state. A row's subscriptions end when the row is dropped.
//!
//! A page whose bytes can be read (a file or an archive entry) shows its status as a spinner or a determinate ring, then
//! a `PageView` once its image is ready, fitted by the viewer's settings, and the error card when it fails. Any other
//! page keeps the text label. The state the label shows comes from the pure functions here.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use tokio::sync::watch;

use super::page_bytes::{self, ByteSource};
use super::page_opts::{PageOpts, background_rgb};
use super::status::{PageView, StatusBranch};
use crate::reader::error_card;
use crate::reader::model::{ChapterState, ReaderChapter, ReaderPage, ViewerChapters};
use crate::reader::page_view::PageView as PageWidget;
use crate::reader::progress::LoadingIndicator;
use crate::reader::strings;

/// What a viewer hands a page row: the options it shows with, what a long press does (position in the page view) and
/// what a zoom does (the menu hides on any scale change, T-44a-071).
#[derive(Clone)]
pub struct PageHost {
    pub opts: PageOpts,
    pub on_long_tap: Rc<dyn Fn(Rc<ReaderPage>, f64, f64)>,
    pub on_zoom: Rc<dyn Fn()>,
}

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
    page: Option<(PageWidget, Rc<Cell<PageOpts>>)>,
    _tasks: Tasks,
}

impl Row {
    /// Applies changed settings to the shown page without reloading it (D-11): only what differs from the options the
    /// page holds is applied, so an unrelated change does not reset the user's zoom.
    pub fn apply(&self, new: &PageOpts) {
        let Some((view, held)) = &self.page else { return };
        let old = held.replace(*new);
        if old.config != new.config {
            view.set_config(new.config);
        }
        if old.background != new.background {
            view.set_page_background(Some(rgba(*new)));
        }
        view.set_double_tap_anim_ms(new.double_tap_ms);
        view.set_long_tap_enabled(new.long_tap);
    }
}

fn rgba(opts: PageOpts) -> gtk::gdk::RGBA {
    let (r, g, b) = background_rgb(opts.background);
    gtk::gdk::RGBA::new(r, g, b, 1.0)
}

fn label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_valign(gtk::Align::Center);
    label.set_justify(gtk::Justification::Center);
    label
}

/// A row with no state to follow.
pub fn plain_row(text: &str) -> Row {
    Row { widget: label(text).upcast(), page: None, _tasks: Tasks::default() }
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

/// The widgets of an image row and the handlers that drive them.
struct ImageRow {
    stack: gtk::Stack,
    slot: (PageWidget, Rc<Cell<PageOpts>>),
    /// Shows a status: the indicator for a queued, loading or downloading page, the card for an error, and nothing
    /// for a ready page (the image is then on its way).
    show: Rc<dyn Fn(PageView)>,
}

impl ImageRow {
    /// Builds the stack (status, page and error card) and the load of the page's bytes. A ready page loads once; the
    /// card's Retry loads it again (T-44a-065).
    fn build(base: &str, page: &Rc<ReaderPage>, host: &PageHost, src: ByteSource) -> Self {
        let stack = gtk::Stack::new();
        let status_text = label(base);
        let indicator = Rc::new(LoadingIndicator::new());
        let status = gtk::Box::new(gtk::Orientation::Vertical, 8);
        status.append(&status_text);
        status.append(indicator.widget());
        stack.add_named(&status, Some("status"));

        let view = PageWidget::new();
        let opts = Rc::new(Cell::new(host.opts));
        view.set_page_background(Some(rgba(host.opts)));
        view.set_double_tap_anim_ms(host.opts.double_tap_ms);
        view.set_long_tap_enabled(host.opts.long_tap);
        view.set_decode_to_view(host.opts.webtoon);
        stack.add_named(&view, Some("page"));

        let (on_long_tap, on_zoom, shown) = (Rc::clone(&host.on_long_tap), Rc::clone(&host.on_zoom), Rc::clone(page));
        view.connect_long_tap(move |x, y| on_long_tap(Rc::clone(&shown), x, y));
        let zoomed = Rc::clone(&on_zoom);
        view.connect_scale_changed(move |_| zoomed());

        let load: Rc<dyn Fn()> = {
            let (weak_stack, weak_view, indicator) = (stack.downgrade(), view.downgrade(), Rc::clone(&indicator));
            let opts = Rc::clone(&opts);
            Rc::new(move || {
                let Some(stack) = weak_stack.upgrade() else { return };
                stack.set_visible_child_name("status");
                indicator.show_spinner();
                let (weak_stack, weak_view, src) = (stack.downgrade(), weak_view.clone(), src.clone());
                let config = opts.get().config;
                glib::spawn_future_local(async move {
                    let bytes = gtk::gio::spawn_blocking(move || page_bytes::read(&src)).await;
                    let (Some(stack), Some(view)) = (weak_stack.upgrade(), weak_view.upgrade()) else { return };
                    match bytes {
                        Ok(Ok(bytes)) => view.set_image(bytes, config),
                        _ => stack.set_visible_child_name("error"),
                    }
                });
            })
        };
        let started = Rc::new(Cell::new(false));
        let auto_load = {
            let (load, started) = (Rc::clone(&load), Rc::clone(&started));
            move || {
                if !started.replace(true) {
                    load();
                }
            }
        };

        let card_load = Rc::clone(&load);
        let card = error_card::card(move || card_load(), page.image_url.borrow().clone());
        stack.add_named(&card, Some("error"));

        let weak_stack = stack.downgrade();
        view.connect_image_loaded({
            let weak_stack = weak_stack.clone();
            move || {
                if let Some(stack) = weak_stack.upgrade() {
                    stack.set_visible_child_name("page");
                }
            }
        });
        view.connect_image_load_error({
            let weak_stack = weak_stack.clone();
            move || {
                if let Some(stack) = weak_stack.upgrade() {
                    stack.set_visible_child_name("error");
                }
            }
        });
        let indicator = Rc::clone(&indicator);
        let show: Rc<dyn Fn(PageView)> = Rc::new(move |state: PageView| {
            let Some(stack) = weak_stack.upgrade() else { return };
            match state {
                PageView::Spinner => {
                    stack.set_visible_child_name("status");
                    indicator.show_spinner();
                }
                PageView::Progress(percent) => {
                    stack.set_visible_child_name("status");
                    indicator.show();
                    indicator.set_progress(percent);
                }
                PageView::Ready => {
                    indicator.hide();
                    auto_load();
                }
                PageView::Error => {
                    indicator.hide();
                    stack.set_visible_child_name("error");
                }
            }
        });
        Self { stack, slot: (view, opts), show }
    }
}

/// A page row. The row follows `page` (the parent of an insert page) when there is one. With a `host` and a page
/// whose bytes can be read, the page is shown as an image (T-43b-014).
pub fn page_row(base: String, page: Option<Rc<ReaderPage>>, host: Option<&PageHost>) -> Row {
    let Some(page) = page else { return plain_row(&base) };
    let shown = host.and_then(|h| page_bytes::byte_source(&page.source).map(|src| (h.clone(), src)));
    let (widget, slot, show): (gtk::Widget, Option<(PageWidget, Rc<Cell<PageOpts>>)>, Rc<dyn Fn(PageView)>) = match shown {
        Some((host, src)) => {
            let row = ImageRow::build(&base, &page, &host, src);
            (row.stack.clone().upcast(), Some(row.slot.clone()), row.show)
        }
        None => {
            // A page whose chapter has no loader stays a plain label with no status (E-40): it shows the page name only.
            let text = label(&base);
            let weak = text.downgrade();
            let base = base.clone();
            let page = Rc::clone(&page);
            let show: Rc<dyn Fn(PageView)> = Rc::new(move |view| {
                let loader = page.chapter().is_some_and(|c| c.has_loader());
                if let Some(text) = weak.upgrade() {
                    text.set_text(&if loader { page_text(&base, view) } else { base.clone() });
                }
            });
            (text.upcast(), None, show)
        }
    };
    let branch = Rc::new(RefCell::new(StatusBranch::default()));
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
    Row { widget, page: slot, _tasks: tasks }
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
    Row { widget: text.upcast(), page: None, _tasks: tasks }
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
