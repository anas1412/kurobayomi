//! The few places where background work meets the GTK main loop (T-40a-001, T-40b-003, T-40b-004): the capacity-1
//! page channel of the tab shell, the debounced search entry, the registry-ready follower, and the consumer that
//! applies a page's events on the main thread. Everything here is thin; the rules sit in `route` and `tasks`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use async_channel::{Receiver, Sender, TrySendError, bounded};
use gtk::glib;
use gtk::prelude::*;
use tokio::sync::watch;

use super::route::{BrowsePage, SEARCH_DEBOUNCE_MS, search_due, search_wait_ms};
use super::tasks::PageTasks;

/// Follows preferences while a page is open (T-40b-015): twice a second, on the GTK main loop, each `check` is asked whether its
/// preference changed (`Watch::try_changed`). When any did, `on_change` runs once. The timer stops once `alive` says the page
/// is gone. Every check runs on every tick, so one change never hides another.
pub fn poll_pref_changes(checks: Vec<Box<dyn Fn() -> bool>>, alive: impl Fn() -> bool + 'static, on_change: impl Fn() + 'static) {
    glib::timeout_add_local(Duration::from_millis(500), move || {
        if !alive() {
            return glib::ControlFlow::Break;
        }
        if checks.iter().map(|check| check()).fold(false, |any, changed| any | changed) {
            on_change();
        }
        glib::ControlFlow::Continue
    });
}

/// Opens the Categories page on its manga section (the category chooser's Edit categories, T-40b-045).
pub fn open_manga_categories() {
    let page = crate::screens::build("categories");
    crate::settings_ui::view::show_kind(&page, crate::settings_ui::library::categories_tab(kuro_domain::kind::EntryKind::Manga));
    crate::ui::push(&page);
}

/// Opens the Categories page on its anime section (the anime browse page's Edit categories).
pub fn open_anime_categories() {
    let page = crate::screens::build("categories");
    crate::settings_ui::view::show_kind(&page, crate::settings_ui::library::categories_tab(kuro_domain::kind::EntryKind::Anime));
    crate::ui::push(&page);
}

/// The tab-switch channel: capacity one, drop-oldest (E-40 R-40-08). A new request replaces one nobody has read.
#[derive(Clone)]
pub struct PageChannel {
    tx: Sender<BrowsePage>,
    rx: Receiver<BrowsePage>,
}

impl PageChannel {
    pub fn new() -> Self {
        let (tx, rx) = bounded(1);
        Self { tx, rx }
    }

    /// Asks for `page`. A request still waiting in the channel is dropped in favour of this one.
    pub fn send(&self, page: BrowsePage) {
        let mut page = page;
        loop {
            match self.tx.try_send(page) {
                Ok(()) | Err(TrySendError::Closed(_)) => return,
                Err(TrySendError::Full(back)) => {
                    page = back;
                    let _ = self.rx.try_recv();
                }
            }
        }
    }

    pub fn receiver(&self) -> Receiver<BrowsePage> {
        self.rx.clone()
    }
}

impl Default for PageChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs `on_page` on the main loop for each page request. Stops when the channel closes or `on_page` returns false.
pub fn follow_pages(channel: &PageChannel, on_page: impl Fn(BrowsePage) -> bool + 'static) {
    let rx = channel.receiver();
    glib::spawn_future_local(async move {
        while let Ok(page) = rx.recv().await {
            if !on_page(page) {
                break;
            }
        }
    });
}

/// Calls `on_text` with the entry's text once it has been still for the debounce period (B§5.1, 250 ms).
pub fn debounce_search(entry: &gtk::SearchEntry, on_text: impl Fn(String) + 'static) {
    struct State {
        origin: Instant,
        last_edit_ms: Cell<u64>,
        armed: Cell<bool>,
    }
    fn arm(state: &Rc<State>, entry: &gtk::SearchEntry, on_text: &Rc<dyn Fn(String)>, wait_ms: u64) {
        state.armed.set(true);
        let (state, entry, on_text) = (Rc::clone(state), entry.clone(), Rc::clone(on_text));
        glib::timeout_add_local_once(Duration::from_millis(wait_ms), move || {
            let now = state.origin.elapsed().as_millis() as u64;
            let last = state.last_edit_ms.get();
            if search_due(last, now) {
                state.armed.set(false);
                on_text(entry.text().to_string());
            } else {
                arm(&state, &entry, &on_text, search_wait_ms(last, now));
            }
        });
    }
    let state = Rc::new(State { origin: Instant::now(), last_edit_ms: Cell::new(0), armed: Cell::new(false) });
    let on_text: Rc<dyn Fn(String)> = Rc::new(on_text);
    // the entry's own delay would add to ours
    entry.set_search_delay(0);
    entry.connect_changed({
        let state = Rc::clone(&state);
        move |entry| {
            state.last_edit_ms.set(state.origin.elapsed().as_millis() as u64);
            if !state.armed.get() {
                arm(&state, entry, &on_text, SEARCH_DEBOUNCE_MS);
            }
        }
    });
}

/// Calls `apply` with the registry's readiness now and every time it changes, on the main loop, so an open page
/// updates without being reopened (T-40b-003). `apply` returns false once its page is gone, which ends the follower.
pub fn follow_ready(mut ready: watch::Receiver<bool>, apply: impl Fn(bool) -> bool + 'static) {
    if !apply(*ready.borrow_and_update()) {
        return;
    }
    glib::spawn_future_local(async move {
        while ready.changed().await.is_ok() {
            if !apply(*ready.borrow_and_update()) {
                break;
            }
        }
    });
}

/// Calls `apply` with the channel's value now and every time it changes, on the main loop. `apply` returns false once
/// its page is gone, which ends the follower.
pub fn follow_changes<T: Clone + Send + Sync + 'static>(mut values: watch::Receiver<T>, apply: impl Fn(T) -> bool + 'static) {
    if !apply(values.borrow_and_update().clone()) {
        return;
    }
    glib::spawn_future_local(async move {
        while values.changed().await.is_ok() {
            if !apply(values.borrow_and_update().clone()) {
                break;
            }
        }
    });
}

/// Consumes a page's event channel on the GTK main loop and passes each event of the current generation to `apply`
/// (T-40b-004). Events from an older generation are dropped. The loop holds the page's tasks weakly, so it ends when
/// the page drops them.
pub fn consume_events<E: Send + 'static>(tasks: &Rc<RefCell<PageTasks<E>>>, apply: impl Fn(E) + 'static) {
    let rx = tasks.borrow().events();
    let weak = Rc::downgrade(tasks);
    glib::spawn_future_local(async move {
        while let Ok(event) = rx.recv().await {
            let Some(tasks) = weak.upgrade() else { break };
            let accepted = tasks.borrow().accepts(&event);
            if accepted {
                apply(event.event);
            }
        }
    });
}

/// Keeps `state` alive for as long as `widget` lives. Page objects that closures reach through a `Weak` need an owner
/// when the caller keeps only the widget; the widget's destroy closure is that owner and drops it with the widget.
pub fn keep_alive<T: 'static>(widget: &impl IsA<gtk::Widget>, state: &Rc<T>) {
    let state = Rc::clone(state);
    widget.connect_destroy(move |_| drop(Rc::clone(&state)));
}

/// Ties a page's tasks to its widget: destroying the widget (popping the page) aborts every loader of the page.
pub fn own_tasks<E: Send + 'static>(page: &impl IsA<gtk::Widget>, tasks: &Rc<RefCell<PageTasks<E>>>) {
    let tasks = Rc::clone(tasks);
    page.connect_destroy(move |_| tasks.borrow_mut().abort_all());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_page_request_replaces_one_that_nobody_read() {
        let channel = PageChannel::new();
        channel.send(BrowsePage::AnimeExtensions);
        channel.send(BrowsePage::MangaExtensions);
        assert_eq!(channel.receiver().try_recv(), Ok(BrowsePage::MangaExtensions));
        assert!(channel.receiver().try_recv().is_err(), "only the newest request was kept");
    }

    #[test]
    fn the_channel_holds_one_request_at_a_time() {
        let channel = PageChannel::new();
        for page in BrowsePage::ALL {
            channel.send(page);
        }
        assert_eq!(channel.receiver().len(), 1);
        assert_eq!(channel.receiver().try_recv(), Ok(BrowsePage::MangaMigrate));
    }

    #[test]
    fn show_extension_after_show_anime_extension_wins_the_tab() {
        use crate::browse::route::BrowseKind;
        let channel = PageChannel::new();
        channel.send(BrowsePage::extensions_of(BrowseKind::Anime));
        channel.send(BrowsePage::extensions_of(BrowseKind::Manga));
        assert_eq!(channel.receiver().try_recv().map(BrowsePage::index), Ok(3));
    }

    /// A main context of its own, so the test never shares the default one.
    fn pump(context: &glib::MainContext, mut done: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            while context.iteration(false) {}
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        done()
    }

    #[test]
    fn the_ready_follower_reports_now_and_again_when_the_registry_finishes() {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let (tx, rx) = watch::channel(false);
                let seen = Rc::new(RefCell::new(Vec::new()));
                let sink = Rc::clone(&seen);
                follow_ready(rx, move |ready| {
                    sink.borrow_mut().push(ready);
                    true
                });
                assert_eq!(*seen.borrow(), [false], "the current state is reported at once");
                tx.send_replace(true);
                assert!(pump(&context, || seen.borrow().len() == 2));
                assert_eq!(*seen.borrow(), [false, true], "initialisation finishing later updates the page");
            })
            .expect("own main context");
    }

    #[test]
    fn the_ready_follower_stops_when_the_page_is_gone() {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let (tx, rx) = watch::channel(false);
                let calls = Rc::new(Cell::new(0));
                let counter = Rc::clone(&calls);
                follow_ready(rx, move |_| {
                    counter.set(counter.get() + 1);
                    counter.get() < 2
                });
                tx.send_replace(true);
                assert!(pump(&context, || calls.get() == 2));
                tx.send_replace(false);
                let _ = pump(&context, || false);
                assert_eq!(calls.get(), 2, "no call after apply returned false");
            })
            .expect("own main context");
    }

    #[test]
    fn events_of_an_old_generation_never_reach_the_page() {
        let rt = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let tasks = Rc::new(RefCell::new(PageTasks::<&'static str>::new()));
                let seen = Rc::new(RefCell::new(Vec::new()));
                let sink = Rc::clone(&seen);
                consume_events(&tasks, move |event| sink.borrow_mut().push(event));
                let (old, tx) = tasks.borrow().sender();
                tx.try_send(super::super::tasks::Tagged { generation: old, event: "late" }).expect("send");
                tasks.borrow_mut().restart();
                let (fresh, tx) = tasks.borrow().sender();
                tx.try_send(super::super::tasks::Tagged { generation: fresh, event: "fresh" }).expect("send");
                assert!(pump(&context, || !seen.borrow().is_empty()));
                let _ = pump(&context, || false);
                assert_eq!(*seen.borrow(), ["fresh"]);
            })
            .expect("own main context");
        drop(rt);
    }

    #[test]
    fn dropping_the_page_ends_its_consumer_and_aborts_its_loaders() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let tasks = Rc::new(RefCell::new(PageTasks::<()>::new()));
                consume_events(&tasks, |()| {});
                tasks.borrow_mut().spawn(async { tokio::time::sleep(Duration::from_secs(3600)).await });
                assert_eq!(tasks.borrow().live_tasks(), 1);
                let rx = tasks.borrow().events();
                drop(tasks);
                assert!(pump(&context, || rx.is_closed()), "the channel closes when the page drops its tasks");
            })
            .expect("own main context");
    }
}
