//! Mounts the History and Updates tabs (T-42a-021, T-42a-025): one screen per kind under an Anime | Manga switcher,
//! the header actions, and the reselect of the sidebar item. The screens are the live ones of [`history_screen`] and
//! [`updates_screen`]; the pages call [`history`] and [`updates`], and the sidebar calls [`reselect`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Orientation as O};
use kuro_domain::kind::EntryKind;
use gtk::glib;
use kuro_i18n::{MsgKey, tr};

use crate::screens::Built;
use crate::data;
use crate::updates::download_host;
use crate::updates::library_trigger::{self, Outcome};
use crate::updates::history_screen::HistoryScreen;
use crate::updates::nav::{Reselect, reselect_action};
use crate::updates::qualities;
use crate::updates::stats_data;
use crate::updates::stats_screen::StatsScreen;
use crate::updates::storage::{self, StorageRow};
use crate::updates::storage_data;
use crate::updates::storage_screen::StorageScreen;
use crate::updates::updates_screen::UpdatesScreen;

/// The stack page names of the kind switcher; anime comes first.
const ANIME: &str = "anime";
const MANGA: &str = "manga";

thread_local! {
    static HISTORY: RefCell<Option<(gtk::Stack, Rc<HistoryScreen>, Rc<HistoryScreen>)>> = const { RefCell::new(None) };
    /// The Updates screens; their widgets only hold weak references, so this keeps them alive.
    static UPDATES: RefCell<Vec<Rc<UpdatesScreen>>> = const { RefCell::new(Vec::new()) };
    /// The Statistics and Storage screens of the last page built, with the tasks that feed them.
    static STATS: RefCell<Vec<(Rc<StatsScreen>, glib::JoinHandle<()>)>> = const { RefCell::new(Vec::new()) };
    static STORAGE: RefCell<Vec<(Rc<StorageScreen>, Vec<glib::JoinHandle<()>>)>> = const { RefCell::new(Vec::new()) };
}

fn kind_of(name: Option<&str>) -> EntryKind {
    if name == Some(MANGA) { EntryKind::Manga } else { EntryKind::Anime }
}

/// An Anime | Manga switcher over two pages.
fn kind_pages(anime: &gtk::Widget, manga: &gtk::Widget) -> (GBox, gtk::Stack) {
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.add_titled(anime, Some(ANIME), "Anime");
    stack.add_titled(manga, Some(MANGA), "Manga");
    stack.set_vexpand(true);
    let switcher = gtk::StackSwitcher::builder().stack(&stack).halign(Align::Center).margin_top(6).build();
    let page = GBox::new(O::Vertical, 6);
    page.append(&switcher);
    page.append(&stack);
    (page, stack)
}

/// The History page: both kinds, and the Clear history button for the visible kind. `None` before the app data is open.
pub fn history() -> Option<Built> {
    if !crate::data::is_installed() {
        return None;
    }
    let (anime, manga) = (HistoryScreen::new(EntryKind::Anime), HistoryScreen::new(EntryKind::Manga));
    let (page, stack) = kind_pages(anime.widget().upcast_ref(), manga.widget().upcast_ref());
    let clear = crate::ui::icon_button("edit-clear-all-symbolic", &tr(MsgKey::pref_clear_history));
    let (clear_anime, clear_manga, visible) = (anime.clone(), manga.clone(), stack.clone());
    clear.connect_clicked(move |_| match kind_of(visible.visible_child_name().as_deref()) {
        EntryKind::Anime => clear_anime.ask_clear(),
        EntryKind::Manga => clear_manga.ask_clear(),
    });
    HISTORY.with(|h| *h.borrow_mut() = Some((stack, anime, manga)));
    Some(Built { content: page.upcast(), end: vec![clear.upcast()] })
}

/// The spec label of the toolbar button that starts a library update.
const UPDATE_LIBRARY: &str = "Update library";
/// The spec label of the toolbar button that opens the Upcoming page.
const VIEW_UPCOMING: &str = "View Upcoming Updates";

thread_local! {
    /// The kind whose Upcoming page the toolbar button last asked for; `screens::build("upcoming")` reads it.
    static UPCOMING_KIND: std::cell::Cell<EntryKind> = const { std::cell::Cell::new(EntryKind::Anime) };
}

/// The Upcoming page of the kind the Updates toolbar asked for (anime until it asks). `None` before the app data is open.
/// The page is built by `ui::open("upcoming")` through `screens::build`.
pub fn upcoming() -> Option<GBox> {
    crate::updates::upcoming::page::build(UPCOMING_KIND.with(|k| k.get()))
}

/// The Updates page: both kinds under the spec toolbar. There is no search field. `None` before the app data is open.
/// The new-updates counters reset when the page is shown and when it is left (T-42b-020).
pub fn updates() -> Option<Built> {
    if !crate::data::is_installed() {
        return None;
    }
    // before the screens: their rows offer Show qualities only while a sheet is registered
    qualities::install_episode_options();
    let (anime, manga) = (UpdatesScreen::new(EntryKind::Anime), UpdatesScreen::new(EntryKind::Manga));
    let (page, stack) = kind_pages(anime.widget().upcast_ref(), manga.widget().upcast_ref());
    page.connect_map(|_| reset_new_updates());
    page.connect_unmap(|_| reset_new_updates());
    let spec = crate::ui::screen("updates");
    let mut end: Vec<gtk::Widget> = Vec::new();
    for item in &spec.toolbar {
        let button = crate::ui::icon_button(&item.icon, &item.label);
        if item.label == UPDATE_LIBRARY {
            let stack = stack.clone();
            button.connect_clicked(move |button| update_library(button, &stack));
        } else if item.label == VIEW_UPCOMING {
            // each kind has its own Upcoming page, as in the source (the anime tab opens the anime one)
            let stack = stack.clone();
            button.connect_clicked(move |_| {
                UPCOMING_KIND.with(|k| k.set(kind_of(stack.visible_child_name().as_deref())));
                crate::ui::open("upcoming");
            });
        } else {
            let label = item.label.clone();
            button.connect_clicked(move |_| {
                crate::ui::resolve("updates", &label);
            });
        }
        end.push(button.upcast());
    }
    end.reverse();
    UPDATES.with(|u| *u.borrow_mut() = vec![anime, manga]);
    Some(Built { content: page.upcast(), end })
}

/// Clears the two new-updates counters (T-42b-020).
fn reset_new_updates() {
    let library = &data::app().library;
    let _ = library.new_manga_updates_count.set(0);
    let _ = library.new_anime_updates_count.set(0);
}

/// The refresh button: starts a library pass for the visible kind, shows the outcome on that kind's page, and spins
/// until the pass ends (T-42a-033). A refused start shows its toast and no spinner.
fn update_library(button: &gtk::Button, stack: &gtk::Stack) {
    let kind = kind_of(stack.visible_child_name().as_deref());
    let outcome = library_trigger::trigger(kind);
    UPDATES.with(|u| {
        if let Some(screen) = u.borrow().iter().find(|screen| screen.kind() == kind) {
            screen.show_toast(library_trigger::message(outcome));
        }
    });
    if outcome == Outcome::Started {
        spin_until_done(button, kind);
    }
}

/// Puts a spinner on the button until the library pass of `kind` is no longer running.
fn spin_until_done(button: &gtk::Button, kind: EntryKind) {
    button.set_child(Some(&gtk::Spinner::builder().spinning(true).build()));
    let weak = button.downgrade();
    glib::timeout_add_local(Duration::from_millis(500), move || {
        let Some(button) = weak.upgrade() else { return glib::ControlFlow::Break };
        if library_trigger::running(kind) {
            return glib::ControlFlow::Continue;
        }
        button.set_child(Some(&gtk::Image::from_icon_name("view-refresh-symbolic")));
        glib::ControlFlow::Break
    });
}

/// The Statistics page: one page per kind under the Anime | Manga switcher (sidebar item "stats"). `None` before the
/// app data is open.
pub fn stats() -> Option<Built> {
    if !crate::data::is_installed() {
        return None;
    }
    let screens = [EntryKind::Anime, EntryKind::Manga].map(|kind| {
        let screen = Rc::new(StatsScreen::new());
        let feed = Rc::clone(&screen);
        let task = glib::spawn_future_local(async move { feed.set_loaded(stats_of(kind).await) });
        (screen, task)
    });
    let (page, _) = kind_pages(screens[0].0.widget().upcast_ref(), screens[1].0.widget().upcast_ref());
    STATS.with(|s| *s.borrow_mut() = screens.into());
    Some(Built { content: page.upcast(), end: vec![] })
}

/// Aggregates the statistics of one kind off the GTK thread.
async fn stats_of(kind: EntryKind) -> Option<stats_data::Loaded> {
    let db = crate::data::app().db(kind);
    let index = crate::shell::host::Shell::get().map(|shell| shell.downloads().clone());
    let rules = stats_data::GlobalUpdateRules::from_library_prefs(
        &kuro_prefs::library::LibraryPrefs::new(crate::data::app().base.store().clone()),
        kind,
    );
    download_host::blocking(move || {
        let downloaded = match kind {
            EntryKind::Manga => index.map_or(0, |i| i.chapter_count_total() as i64),
            EntryKind::Anime => download_host::anime_index().map_or(0, |i| i.episode_count_total() as i64),
        };
        // a store that cannot be opened reads as nobody logged in (the tracker section then shows N/A)
        let logged_in = match kuro_platform::linux::paths::LinuxPaths::system() {
            Ok(paths) => crate::library::trackers::logged_in(&kuro_platform::linux::secrets::open(&paths), kind),
            Err(_) => Default::default(),
        };
        stats_data::load(&db, kind, downloaded, &logged_in, &rules)
    })
    .await
}

/// The Storage page: one page per kind under the same switcher (sidebar index 8). It follows the download cache: the
/// first snapshot comes at once and a new one after each change, with `initializing` taken from the cache (T-42a-054).
pub fn storage() -> Option<Built> {
    if !crate::data::is_installed() {
        return None;
    }
    let screens = [EntryKind::Anime, EntryKind::Manga].map(|kind| storage_screen(kind));
    let widgets = screens.each_ref().map(|(screen, _)| crate::ui::scrolled(screen.widget()).upcast::<gtk::Widget>());
    let (page, _) = kind_pages(&widgets[0], &widgets[1]);
    STORAGE.with(|s| *s.borrow_mut() = screens.into());
    Some(Built { content: page.upcast(), end: vec![] })
}

/// One kind's Storage page and its watch on the download cache.
fn storage_screen(kind: EntryKind) -> (Rc<StorageScreen>, Vec<glib::JoinHandle<()>>) {
    let screen = StorageScreen::new(kind == EntryKind::Anime);
    // entry id -> source id of the rows on screen, for the delete hook
    let sources: Rc<RefCell<HashMap<i64, i64>>> = Rc::default();
    let known = Rc::clone(&sources);
    screen.connect_delete(move |row: &StorageRow| {
        let backend = download_host::current();
        let installed = known.borrow().get(&row.entry_id).is_some_and(|source| backend.source_installed(kind, *source));
        let outcome = storage::delete_outcome(installed);
        if installed {
            let entry_id = row.entry_id;
            download_host::spawn_detached(move || {
                backend.delete_entry(kind, entry_id);
            });
        }
        outcome
    });
    let reload = {
        let (weak, sources) = (Rc::downgrade(&screen), sources);
        move || {
            let (weak, sources) = (weak.clone(), Rc::clone(&sources));
            glib::spawn_future_local(async move {
                let Some(screen) = weak.upgrade() else { return };
                let db = crate::data::app().db(kind);
                let hide_hidden = crate::data::app().library.hide_hidden_categories_settings.get();
                let (backend, initializing) = (download_host::current(), *download_host::hub().cache.initializing().borrow());
                let built = download_host::blocking(move || storage_data::input(&db, kind, &*backend, hide_hidden, initializing)).await;
                match built {
                    Some(Ok((input, by_source))) => {
                        *sources.borrow_mut() = by_source;
                        screen.set_input(input);
                    }
                    Some(Err(message)) => tracing::warn!("the storage page could not load: {message}"),
                    None => tracing::warn!("the storage page task was dropped"),
                }
            });
        }
    };
    reload();
    let hub = download_host::hub();
    let (mut changes, mut initializing) = (hub.cache.subscribe(), hub.cache.initializing());
    let (on_change, on_init) = (reload.clone(), reload);
    let watch_changes = glib::spawn_future_local(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match changes.recv().await {
                Ok(()) | Err(RecvError::Lagged(_)) => on_change(),
                Err(RecvError::Closed) => return,
            }
        }
    });
    let watch_init = glib::spawn_future_local(async move {
        while initializing.changed().await.is_ok() {
            on_init();
        }
    });
    (screen, vec![watch_changes, watch_init])
}

/// The sidebar item `nav_id` was chosen. `already_visible` says it was the showing page: History then resumes the
/// newest row of the visible sub-tab, and Updates opens the Downloads page.
pub fn reselect(nav_id: &str, already_visible: bool) {
    match reselect_action(nav_id, already_visible) {
        Reselect::ResumeHistory => {
            let screen = HISTORY.with(|h| {
                h.borrow().as_ref().map(|(stack, anime, manga)| match kind_of(stack.visible_child_name().as_deref()) {
                    EntryKind::Anime => anime.clone(),
                    EntryKind::Manga => manga.clone(),
                })
            });
            if let Some(screen) = screen {
                screen.resume_last();
            }
        }
        Reselect::OpenDownloads => crate::ui::open("download_queue"),
        Reselect::Nothing => {}
    }
}
