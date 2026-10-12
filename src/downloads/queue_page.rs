//! The download queue page (lane D): one kind's queue as a list, with Pause or Resume for it,
//! Clear, and the shared download button on each row (a click cancels, or retries a failed row). The commands go through
//! `kuro_downloads::engine::QueueEngine`, and the rows come from its models. A row's right-click menu offers Cancel, Retry
//! (a failed row), Move to top and Move to bottom. The list is rebuilt once a second, only
//! when its rows or their states changed; the buttons follow the progress live (`downloads::live`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib::{self, prelude::*};
use gtk::prelude::*;
use kuro_platform::kind::Kind;
use kuro_downloads::DownloadState;
use kuro_downloads::engine::{AnimeEngine, MangaEngine, QueueEngine, anime_model, manga_model};
use kuro_downloads::queue_model::{QueueItem, QueueModel};
use kuro_i18n::{MsgKey, tr};

use crate::browse::look;
use crate::downloads::Queues;
use crate::downloads::live;
use crate::downloads::manga_io::Waiting;
use crate::widgets::context_menu::{self, Item};
use crate::widgets::download_button::{DownloadButton, Request};

/// How often the page checks the queues.
const REFRESH: Duration = Duration::from_secs(1);

/// The queue page of one kind and its title search for the page header, or `None` when the app runs no download
/// manager (the Download tab then shows its empty state).
pub fn page(kind: Kind) -> Option<(gtk::Widget, gtk::SearchEntry)> {
    let queues = crate::shell::host::Shell::get()?.download_queues()?;
    let column = crate::ui::column();
    // the shared top row of a page (`ui::pill_row`), as the library's categories
    let actions = crate::ui::pill_row();
    let toggle = gtk::Button::new();
    let clear = gtk::Button::with_label("Clear");
    actions.append(&toggle);
    actions.append(&clear);
    let list = crate::ui::boxed_list();
    // the app's empty state (icon, line), as the library and history show theirs
    let (empty, _) = look::empty_state("folder-download-symbolic", "No downloads are queued.", None);
    empty.set_vexpand(true);
    column.append(&actions);
    column.append(&list);
    column.append(&empty);
    let search = crate::ui::list_search(&list);

    let page = Page { kind, queues: queues.clone(), toggle: toggle.clone(), actions: actions.clone(), list, empty, shown: Rc::new(RefCell::new(None)), buttons: Rc::default() };
    let on_toggle = page.clone();
    toggle.connect_clicked(move |_| {
        let resume = !running(&on_toggle.queues, kind);
        for_each_engine_of(&on_toggle.queues, kind, |engine| {
            if resume {
                engine.start();
            } else {
                engine.stop();
            }
        });
        on_toggle.draw();
    });
    let on_clear = queues.clone();
    clear.connect_clicked(move |_| for_each_engine_of(&on_clear, kind, |engine| engine.clear()));

    page.draw();
    live::follow(&column, {
        let buttons = Rc::clone(&page.buttons);
        move |change| {
            if let Some(button) = buttons.borrow().get(&(change.kind, change.id)) {
                button.set(change.state, change.percent);
            }
        }
    });
    let weak = column.downgrade();
    let refresh = page.clone();
    glib::timeout_add_local(REFRESH, move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        refresh.draw();
        glib::ControlFlow::Continue
    });
    Some((column.upcast(), search))
}

/// The rows of `kind`'s queue: the manga model for manga, the anime model (when that queue is up) for anime.
fn items_of<'a>(kind: Kind, manga: &'a QueueModel, anime: Option<&'a QueueModel>) -> Vec<&'a QueueItem> {
    match kind {
        Kind::Manga => manga.flat().iter().collect(),
        Kind::Anime => anime.map_or_else(Vec::new, |model| model.flat().iter().collect()),
    }
}

/// The state the page draws: the queues and the widgets that change.
#[derive(Clone)]
struct Page {
    kind: Kind,
    queues: Queues,
    toggle: gtk::Button,
    /// Resume or Pause and Clear: hidden while nothing is queued.
    actions: gtk::Box,
    list: gtk::ListBox,
    empty: gtk::Box,
    /// The key of the rows last drawn. The list is rebuilt only when it changes.
    shown: Rc<RefCell<Option<String>>>,
    /// The download button of each row drawn, by kind and item id, so the live progress reaches it.
    buttons: Rc<RefCell<HashMap<(Kind, i64), DownloadButton>>>,
}

impl Page {
    fn draw(&self) {
        self.toggle.set_label(if running(&self.queues, self.kind) { "Pause" } else { "Resume" });
        let snapshot = self.queues.manga.snapshot();
        let manga = manga_model(&snapshot);
        let anime = self.queues.anime.as_ref().map(|queue| anime_model(&queue.snapshot()));
        let items = items_of(self.kind, &manga, anime.as_ref());
        // A waiting row that was queued again (enqueued after its source came back) shows once, as the queued row.
        // Waiting rows are saved chapters, so they belong to the manga page.
        let queued: Vec<i64> = snapshot.iter().map(|d| d.chapter.id).collect();
        let waiting: Vec<Waiting> = if self.kind == Kind::Manga {
            self.queues.waiting.read().unwrap_or_else(|e| e.into_inner()).iter().filter(|w| !queued.contains(&w.row.chapter_id)).cloned().collect()
        } else {
            Vec::new()
        };
        let key: String = items.iter().map(|item| row_key(item)).chain(waiting.iter().map(|w| format!("w{}", w.row.chapter_id))).collect::<Vec<_>>().join("|");
        if self.shown.borrow().as_deref() == Some(key.as_str()) {
            return;
        }
        *self.shown.borrow_mut() = Some(key);
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.buttons.borrow_mut().clear();
        let empty = items.is_empty() && waiting.is_empty();
        self.actions.set_visible(!empty);
        self.empty.set_visible(empty);
        self.list.set_visible(!empty);
        for item in items {
            self.list.append(&self.row(item));
        }
        for line in &waiting {
            self.list.append(&self.waiting_row(line));
        }
    }

    /// A saved download whose source is not installed. It is queued again when the source comes back. Cancel removes it.
    fn waiting_row(&self, line: &Waiting) -> gtk::ListBoxRow {
        let cancel = gtk::Button::with_label(&tr(MsgKey::action_cancel));
        let (queues, chapter_id) = (self.queues.clone(), line.row.chapter_id);
        let on_menu = queues.clone();
        cancel.connect_clicked(move |_| {
            let _ = queues.cancel_waiting.send(chapter_id);
        });
        let row = look::list_row(None, &format!("{} · {}", line.title, line.chapter), Some("Waiting for source"), Some(cancel.upcast()));
        row.set_activatable(false);
        context_menu::attach(&row, move |_, _| {
            let queues = on_menu.clone();
            vec![Item::new(tr(MsgKey::action_cancel), 0, move || {
                let _ = queues.cancel_waiting.send(chapter_id);
            })]
        });
        row
    }

    fn row(&self, item: &QueueItem) -> gtk::ListBoxRow {
        let (queues, kind, id) = (self.queues.clone(), item.kind, item.item_id);
        // a click on a queued or running row cancels it, and on a failed row starts its queue again
        let button = DownloadButton::new(move |request| match request {
            Request::Cancel => live::cancel(kind, id),
            Request::Start => for_each_engine_of(&queues, kind, |engine| engine.start()),
            Request::Delete => {}
        });
        let (state, percent) = live::queued(&self.queues, kind, id).unwrap_or((item.state, 0));
        button.set(state, percent);
        let line = look::list_row(None, &format!("{} · {}", item.title, item.item_name), Some(&status_line(item)), Some(button.widget.clone().upcast()));
        line.set_activatable(false);
        let (on_menu, drawn) = (self.queues.clone(), item.state);
        context_menu::attach(&line, move |_, _| {
            // the state now, not the one the row was drawn with
            let failed = live::queued(&on_menu, kind, id).map_or(drawn, |(state, _)| state) == DownloadState::Error;
            row_menu(&on_menu, kind, id, failed)
        });
        self.buttons.borrow_mut().insert((kind, id), button);
        line
    }
}

/// The right-click menu of a queued row: Cancel, Retry when it failed, Move to top and Move to bottom.
fn row_menu(queues: &Queues, kind: Kind, id: i64, failed: bool) -> Vec<Item> {
    let mut items = vec![Item::new(tr(MsgKey::action_cancel), 0, move || live::cancel(kind, id))];
    if failed {
        // as the row's button does on a failed row: the queue starts again
        let queues = queues.clone();
        items.push(Item::new(tr(MsgKey::action_retry), 0, move || for_each_engine_of(&queues, kind, |engine| engine.start())));
    }
    for (label, top) in [(MsgKey::action_move_to_top, true), (MsgKey::action_move_to_bottom, false)] {
        let queues = queues.clone();
        items.push(Item::new(tr(label), 1, move || move_item(&queues, kind, id, top)));
    }
    items
}

/// Moves one item to the top or the bottom of its source's section (`QueueModel::move_to_top`) and sends the new order.
fn move_item(queues: &Queues, kind: Kind, id: i64, top: bool) {
    let mut model: QueueModel = match kind {
        Kind::Manga => manga_model(&queues.manga.snapshot()),
        Kind::Anime => match &queues.anime {
            Some(queue) => anime_model(&queue.snapshot()),
            None => return,
        },
    };
    let moved = if top { model.move_to_top(id) } else { model.move_to_bottom(id) };
    if moved {
        let order: Vec<i64> = model.flat().iter().map(|item| item.item_id).collect();
        for_each_engine_of(queues, kind, |engine| engine.reorder(&order));
    }
}

/// Whether `kind`'s queue is running.
fn running(queues: &Queues, kind: Kind) -> bool {
    match kind {
        Kind::Manga => queues.manga.running(),
        Kind::Anime => queues.anime.as_ref().is_some_and(|queue| queue.running()),
    }
}

/// Calls `f` on the engine of one kind's queue, when it is up.
fn for_each_engine_of(queues: &Queues, kind: Kind, mut f: impl FnMut(&dyn QueueEngine)) {
    match kind {
        Kind::Manga => f(&MangaEngine::new(queues.manga.clone())),
        Kind::Anime => {
            if let Some(anime) = &queues.anime {
                f(&AnimeEngine::new(anime.clone()));
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
            (Kind::Manga, Some(total)) => format!("{} of {total} pages", item.pages_done),
            (Kind::Anime, _) if item.progress > 0 => format!("{}%", item.progress),
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
    use kuro_platform::kind::Kind;

    use super::*;

    fn item(kind: Kind, state: DownloadState, progress: i64, pages: Option<(u32, u32)>) -> QueueItem {
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
    fn each_kind_page_lists_only_its_own_queue() {
        let manga = QueueModel::from_flat(vec![item(Kind::Manga, DownloadState::Queue, 0, None)]);
        let anime = QueueModel::from_flat(vec![item(Kind::Anime, DownloadState::Queue, 0, None), item(Kind::Anime, DownloadState::Error, 0, None)]);
        assert!(items_of(Kind::Manga, &manga, Some(&anime)).iter().all(|i| i.kind == Kind::Manga));
        assert_eq!(items_of(Kind::Manga, &manga, Some(&anime)).len(), 1);
        assert!(items_of(Kind::Anime, &manga, Some(&anime)).iter().all(|i| i.kind == Kind::Anime));
        assert_eq!(items_of(Kind::Anime, &manga, Some(&anime)).len(), 2);
        assert!(items_of(Kind::Anime, &manga, None).is_empty(), "no anime queue, no anime rows");
    }

    #[test]
    fn a_row_shows_its_progress_while_it_downloads_and_its_state_otherwise() {
        assert_eq!(status_line(&item(Kind::Anime, DownloadState::Downloading, 40, None)), "40%");
        assert_eq!(status_line(&item(Kind::Anime, DownloadState::Downloading, 0, None)), "Downloading…");
        assert_eq!(status_line(&item(Kind::Manga, DownloadState::Downloading, 250, Some((2, 5)))), "2 of 5 pages", "a manga row counts pages, not the page percent sum");
        assert_eq!(status_line(&item(Kind::Manga, DownloadState::Downloading, 0, None)), "Downloading…");
        assert_eq!(status_line(&item(Kind::Manga, DownloadState::Queue, 0, None)), "Queued");
        assert_eq!(status_line(&item(Kind::Anime, DownloadState::Error, 0, None)), "Failed");
        assert_eq!(status_line(&item(Kind::Manga, DownloadState::Downloaded, 0, None)), "Downloaded");
    }
}
