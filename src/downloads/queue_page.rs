//! The download queue page (lane D): the manga queue and the anime queue as one list, with Pause or Resume for both,
//! Clear, and Cancel on each row. The commands go through `kuro_downloads::engine::QueueEngine`, and the rows come from
//! its models. The list is redrawn once a second, only when a row or a state changed.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib::{self, prelude::*};
use gtk::prelude::*;
use kuro_downloads::DownloadState;
use kuro_downloads::engine::{AnimeEngine, MangaEngine, QueueEngine, anime_model, manga_model};
use kuro_downloads::queue_model::{EntryKind, QueueItem};

use crate::downloads::Queues;
use crate::downloads::manga_io::Waiting;

/// How often the page checks the queues.
const REFRESH: Duration = Duration::from_secs(1);

/// The queue page, or `None` when the app runs no download manager (the Download tab then shows its empty state).
pub fn page() -> Option<gtk::Widget> {
    let queues = crate::shell::host::Shell::get()?.download_queues()?;
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_margin_top(16);
    column.set_margin_bottom(16);
    column.set_margin_start(16);
    column.set_margin_end(16);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let toggle = gtk::Button::new();
    let clear = gtk::Button::with_label("Clear");
    actions.append(&toggle);
    actions.append(&clear);
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 4);
    column.append(&actions);
    column.append(&rows);

    let page = Page { queues: queues.clone(), toggle: toggle.clone(), rows, shown: Rc::new(RefCell::new(None)) };
    let on_toggle = page.clone();
    toggle.connect_clicked(move |_| {
        let resume = !running(&on_toggle.queues);
        for_each_engine(&on_toggle.queues, |engine| {
            if resume {
                engine.start();
            } else {
                engine.stop();
            }
        });
        on_toggle.draw();
    });
    let on_clear = queues.clone();
    clear.connect_clicked(move |_| for_each_engine(&on_clear, |engine| engine.clear()));

    page.draw();
    let weak = column.downgrade();
    let refresh = page.clone();
    glib::timeout_add_local(REFRESH, move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        refresh.draw();
        glib::ControlFlow::Continue
    });
    Some(column.upcast())
}

/// The state the page draws: the queues and the widgets that change.
#[derive(Clone)]
struct Page {
    queues: Queues,
    toggle: gtk::Button,
    rows: gtk::Box,
    /// The key of the rows last drawn. The list is rebuilt only when it changes.
    shown: Rc<RefCell<Option<String>>>,
}

impl Page {
    fn draw(&self) {
        self.toggle.set_label(if running(&self.queues) { "Pause" } else { "Resume" });
        let snapshot = self.queues.manga.snapshot();
        let manga = manga_model(&snapshot);
        let anime = self.queues.anime.as_ref().map(|queue| anime_model(&queue.snapshot()));
        let mut items: Vec<&QueueItem> = manga.flat().iter().collect();
        if let Some(anime) = &anime {
            items.extend(anime.flat().iter());
        }
        // A waiting row that was queued again (enqueued after its source came back) shows once, as the queued row.
        let queued: Vec<i64> = snapshot.iter().map(|d| d.chapter.id).collect();
        let waiting: Vec<Waiting> = self
            .queues
            .waiting
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|w| !queued.contains(&w.row.chapter_id))
            .cloned()
            .collect();
        let key: String = items.iter().map(|item| row_key(item)).chain(waiting.iter().map(|w| format!("w{}", w.row.chapter_id))).collect::<Vec<_>>().join("|");
        if self.shown.borrow().as_deref() == Some(key.as_str()) {
            return;
        }
        *self.shown.borrow_mut() = Some(key);
        while let Some(child) = self.rows.first_child() {
            self.rows.remove(&child);
        }
        if items.is_empty() && waiting.is_empty() {
            self.rows.append(&gtk::Label::new(Some("No downloads are queued.")));
        }
        for item in items {
            self.rows.append(&self.row(item));
        }
        for line in &waiting {
            self.rows.append(&self.waiting_row(line));
        }
    }

    /// A saved download whose source is not installed. It is queued again when the source comes back. Cancel removes it.
    fn waiting_row(&self, line: &Waiting) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let text = gtk::Label::new(Some(&format!("{} · {}", line.title, line.chapter)));
        text.set_xalign(0.0);
        text.set_hexpand(true);
        let status = gtk::Label::new(Some("Waiting for source"));
        let cancel = gtk::Button::with_label("Cancel");
        let (queues, chapter_id) = (self.queues.clone(), line.row.chapter_id);
        cancel.connect_clicked(move |_| {
            let _ = queues.cancel_waiting.send(chapter_id);
        });
        row.append(&text);
        row.append(&status);
        row.append(&cancel);
        row
    }

    fn row(&self, item: &QueueItem) -> gtk::Box {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let text = gtk::Label::new(Some(&format!("{} · {}", item.title, item.item_name)));
        text.set_xalign(0.0);
        text.set_hexpand(true);
        let status = gtk::Label::new(Some(&status_line(item)));
        let cancel = gtk::Button::with_label("Cancel");
        let (queues, kind, id) = (self.queues.clone(), item.kind, item.item_id);
        cancel.connect_clicked(move |_| remove_row(&queues, kind, id));
        line.append(&text);
        line.append(&status);
        line.append(&cancel);
        line
    }
}

/// Whether either queue is running.
fn running(queues: &Queues) -> bool {
    queues.manga.running() || queues.anime.as_ref().is_some_and(|queue| queue.running())
}

/// Calls `f` on the engine of each queue that is up.
fn for_each_engine(queues: &Queues, mut f: impl FnMut(&dyn QueueEngine)) {
    f(&MangaEngine::new(queues.manga.clone()));
    if let Some(anime) = &queues.anime {
        f(&AnimeEngine::new(anime.clone()));
    }
}

/// Cancels one queued item of its kind. Downloaded files are not touched.
fn remove_row(queues: &Queues, kind: EntryKind, item_id: i64) {
    match kind {
        EntryKind::Manga => MangaEngine::new(queues.manga.clone()).remove(&[item_id]),
        EntryKind::Anime => {
            if let Some(queue) = &queues.anime {
                AnimeEngine::new(queue.clone()).remove(&[item_id]);
            }
        }
    }
}

/// What one row shows, as a key: a change of any of these rebuilds the list.
fn row_key(item: &QueueItem) -> String {
    format!("{:?}:{}:{}:{}", item.kind, item.item_id, item.state as u8, item.progress)
}

/// The status text beside a row while it downloads: pages done of the list for a manga (its progress is a sum of page
/// percents, so it is not a percent), the percent for an episode, else "Downloading…". Otherwise the state.
fn status_line(item: &QueueItem) -> String {
    match item.state {
        DownloadState::Downloading => match (item.kind, item.pages_total) {
            (EntryKind::Manga, Some(total)) => format!("{} of {total} pages", item.pages_done),
            (EntryKind::Anime, _) if item.progress > 0 => format!("{}%", item.progress),
            _ => "Downloading…".to_owned(),
        },
        DownloadState::Queue => "Queued".to_owned(),
        DownloadState::Error => "Failed".to_owned(),
        DownloadState::Downloaded => "Downloaded".to_owned(),
        DownloadState::NotDownloaded => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: EntryKind, state: DownloadState, progress: i64, pages: Option<(u32, u32)>) -> QueueItem {
        QueueItem {
            kind,
            item_id: 1,
            parent_id: 1,
            source_id: 1,
            source_name: "Source".into(),
            title: "Title".into(),
            item_name: "Name".into(),
            date_upload: 0,
            number: 1.0,
            state,
            pages_done: pages.map_or(0, |(done, _)| done),
            pages_total: pages.map(|(_, total)| total),
            progress,
        }
    }

    #[test]
    fn a_row_shows_its_progress_while_it_downloads_and_its_state_otherwise() {
        assert_eq!(status_line(&item(EntryKind::Anime, DownloadState::Downloading, 40, None)), "40%");
        assert_eq!(status_line(&item(EntryKind::Anime, DownloadState::Downloading, 0, None)), "Downloading…");
        assert_eq!(status_line(&item(EntryKind::Manga, DownloadState::Downloading, 250, Some((2, 5)))), "2 of 5 pages", "a manga row counts pages, not the page percent sum");
        assert_eq!(status_line(&item(EntryKind::Manga, DownloadState::Downloading, 0, None)), "Downloading…");
        assert_eq!(status_line(&item(EntryKind::Manga, DownloadState::Queue, 0, None)), "Queued");
        assert_eq!(status_line(&item(EntryKind::Anime, DownloadState::Error, 0, None)), "Failed");
        assert_eq!(status_line(&item(EntryKind::Manga, DownloadState::Downloaded, 0, None)), "Downloaded");
    }
}
