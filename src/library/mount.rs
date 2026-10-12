//! Mounts the library view in the window (T-37a-061): builds the view's dependencies from the app data, registers
//! the views so the shell can reach them, and exposes what the shell needs. The two kinds share one sidebar
//! destination (`library`) with an Anime and a Manga page; each kind still has its own sidebar index per nav style.
//!
//! Decision (T-37a-045, replaces B-14.11): with `MOVE_MANGA_TO_MORE` the manga library is a page under More, at index
//! 5. Aniyomi jumps from it to the anime tab; here Back returns to the tab it was opened from, and to More when it was
//! entered from the More list. The sidebar index below is the part this module owns; the Back target is the shell's
//! (`src/shell/tabs.rs`, `TabState::back`), which has to remember the origin tab when the manga page opens.
//!
//! - Re-selecting the Library sidebar item opens the library settings sheet ([`reselect`], B-1.2).
//! - Other screens can fill the search box ([`search_library`] and [`inject_search`], B-1.3).
//! - The sidebar hides while entries are selected ([`selecting`] and [`on_selection_change`], B-1.4).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::glib;
use gtk::prelude::*;
use kuro_data::anime::{AnimeRepo, AnimeUpdate};
use kuro_data::chapter::ChapterRepo;
use kuro_data::manga::{MangaRepo, MangaUpdate};
use kuro_db::handle::Db;
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};
use kuro_images::art::{ArtKind, ArtRef, Owner};
use kuro_images::invalidate::remove_covers;
use kuro_platform::linux::LinuxPaths;
use kuro_platform::paths::Paths;
use kuro_prefs::library::LibraryPrefs;

use crate::library::actions::delete_plan;
use crate::library::trackers::{self, Logins};
use crate::library::download_facts;
use crate::library::query::{Facts, SourceInfo};
use crate::library::view::{Deps, LibraryView, Nav};
use crate::browse::route::{BrowseKind, BrowseRoute};
use crate::screens::Built;

thread_local! {
    static VIEWS: RefCell<Vec<(EntryKind, LibraryView)>> = const { RefCell::new(Vec::new()) };
    /// Bit 0: the anime view is selecting. Bit 1: the manga view is.
    static SELECTING: Cell<u8> = const { Cell::new(0) };
    /// The kinds whose database watch is bound (same bits as `SELECTING`): one watch serves every view of a kind.
    static WATCHED: Cell<u8> = const { Cell::new(0) };
    static LISTENER: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    /// Searches injected before the view of their kind was built; each is applied when that view mounts.
    static PENDING_SEARCH: RefCell<Vec<(EntryKind, String)>> = const { RefCell::new(Vec::new()) };
    /// What the selection Download action does with its entries (the app's download manager registers here).
    static DELETE_DOWNLOADS_SINK: RefCell<Option<Rc<dyn Fn(EntryKind, Vec<i64>)>>> = const { RefCell::new(None) };
    /// What the refresh button does (the update runner in `crate::misc` registers here).
    static UPDATE_HOST: RefCell<Option<UpdateHost>> = const { RefCell::new(None) };
    static DOWNLOAD_SINK: RefCell<Option<Rc<dyn Fn(EntryKind, Vec<i64>, DownloadAmount)>>> = const { RefCell::new(None) };
}

pub use crate::library::actions::DownloadAmount;

/// What the library's refresh button asks of the app (T-26a-102).
#[derive(Clone)]
pub struct UpdateHost {
    /// Starts a pass of one category of a kind; `false` when a pass of that kind is already running.
    pub start: Rc<dyn Fn(EntryKind, Option<i64>) -> bool>,
    /// Whether a pass of that kind is running.
    pub running: Rc<dyn Fn(EntryKind) -> bool>,
}

/// Registers the update runner behind the refresh button. Replaces an earlier one.
pub fn on_update(host: UpdateHost) {
    UPDATE_HOST.with(|h| *h.borrow_mut() = Some(host));
}

/// Registers what "delete downloads" in the remove dialog does. Replaces an earlier one.
pub fn on_delete_downloads(sink: impl Fn(EntryKind, Vec<i64>) + 'static) {
    DELETE_DOWNLOADS_SINK.with(|s| *s.borrow_mut() = Some(Rc::new(sink)));
}

/// Registers what the Download action of a selection does. Replaces an earlier one.
pub fn on_download(sink: impl Fn(EntryKind, Vec<i64>, DownloadAmount) + 'static) {
    DOWNLOAD_SINK.with(|s| *s.borrow_mut() = Some(Rc::new(sink)));
}

const fn bit(kind: EntryKind) -> u8 {
    match kind {
        EntryKind::Anime => 1,
        EntryKind::Manga => 2,
    }
}

/// Whether any library view is in selection mode. The shell hides its sidebar while this holds.
pub fn selecting() -> bool {
    SELECTING.with(Cell::get) != 0
}

/// Registers the shell's reaction to selection mode starting or ending. Replaces an earlier one.
pub fn on_selection_change(listener: impl Fn() + 'static) {
    LISTENER.with(|l| *l.borrow_mut() = Some(Rc::new(listener)));
}

fn set_selecting(kind: EntryKind, on: bool) {
    SELECTING.with(|s| s.set(if on { s.get() | bit(kind) } else { s.get() & !bit(kind) }));
    let listener = LISTENER.with(|l| l.borrow().clone());
    if let Some(listener) = listener {
        listener();
    }
}

/// The view of `kind` that is on screen, or the first one when none is mapped yet.
fn visible(kind: Option<EntryKind>) -> Option<LibraryView> {
    VIEWS.with(|views| {
        let views = views.borrow();
        views
            .iter()
            .find(|(k, v)| kind.is_none_or(|want| want == *k) && v.widget().is_mapped())
            .or_else(|| views.iter().find(|(k, _)| kind.is_none_or(|want| want == *k)))
            .map(|(_, v)| v.clone())
    })
}

/// The Library sidebar item was selected while it was already showing: open the settings sheet of the visible kind.
pub fn reselect() {
    if let Some(view) = visible(None) {
        view.open_settings(crate::library::view::sheet::Tab::Filter);
    }
}

/// Reloads the library of `kind`, for changes made outside it such as a local folder scan.
pub fn refresh(kind: EntryKind) {
    if let Some(view) = visible(Some(kind)) {
        view.refresh();
    }
}

/// Puts `query` in the search box of one kind's library (B-1.3). A view that is not built yet gets it on mount.
pub fn inject_search(kind: EntryKind, query: &str) {
    match visible(Some(kind)) {
        Some(view) => view.set_query(query),
        None => PENDING_SEARCH.with(|p| p.borrow_mut().push((kind, query.to_owned()))),
    }
}

/// Opens the Library destination on `kind`'s page with `query` in its search box. This is the entry point for
/// screens that search the library (B-1.3).
pub fn search_library(kind: EntryKind, query: &str) {
    crate::ui::pop_to_root();
    crate::ui::open("library");
    inject_search(kind, query);
    show_kind(kind);
}

/// Switches the Library tab to `kind`'s page (T-36a-060: a deep link to an entry selects the matching library).
pub fn show_kind(kind: EntryKind) {
    if let Some(view) = visible(Some(kind)) {
        let page = if kind == EntryKind::Anime { "anime" } else { "manga" };
        if let Some(stack) = view.widget().ancestor(gtk::Stack::static_type()).and_downcast::<gtk::Stack>() {
            stack.set_visible_child_name(page);
        }
    }
}

/// The route of "search globally" from the library of `kind`: the global search page, with `query` in its box.
fn global_search_route(kind: EntryKind, query: &str) -> BrowseRoute {
    let kind = if kind == EntryKind::Anime { BrowseKind::Anime } else { BrowseKind::Manga };
    BrowseRoute::GlobalSearch { kind, source_id: None, query: query.to_owned() }
}

/// Opens the global search page with `query` in its search box (T-37a-022). The request is parked first, because the
/// page builds itself from it (`browse::search_screen::body`).
fn open_global_search(kind: EntryKind, query: &str) {
    crate::browse::pending::open(global_search_route(kind, query));
}

/// `(id, is_local)` for each entry. An entry that cannot be read counts as local, so its files are never deleted.
fn local_flags(db: &Arc<Db>, kind: EntryKind, ids: &[i64]) -> Vec<(i64, bool)> {
    ids.iter()
        .map(|id| {
            let source = match kind {
                EntryKind::Manga => MangaRepo::new(db.clone()).by_id(*id).map(|m| m.source),
                EntryKind::Anime => AnimeRepo::new(db.clone()).by_id(*id).map(|a| a.source),
            };
            (*id, source.map_or(true, |s| s == 0))
        })
        .collect()
}

/// Source name and language for the search and the language badge. A source with no stored row shows its id.
pub fn source_info(db: &Arc<Db>, kind: EntryKind, id: i64) -> SourceInfo {
    let found = db
        .read(|c| {
            Ok(match kind {
                EntryKind::Manga => kuro_db::statements_manga::sources::find_one(c, id)?.map(|s| (s.name, s.lang)),
                EntryKind::Anime => kuro_db::statements_anime::stub_source(c, id)?.map(|s| (s.name, s.lang)),
            })
        })
        .ok()
        .flatten();
    match found {
        Some((name, language)) => SourceInfo { name, language },
        None => SourceInfo { name: id.to_string(), language: String::new() },
    }
}

/// Takes the selected entries out of the library: favorite off, and the cached covers (an anime's background too)
/// deleted. A cover timestamp moves only when a file was deleted, as Kotlin's `removeCovers` does, and it is written
/// with the removal (E-27). Downloads stay (T2-DEL-01, T2-DEL-03). Local entries keep their files.
pub fn remove_from_library(paths: &dyn Paths, db: &Arc<Db>, kind: EntryKind, ids: &[i64]) -> usize {
    let now = kuro_data::time::now_millis();
    ids.iter()
        .filter(|id| {
            let done = match kind {
                EntryKind::Manga => remove_manga(paths, db, **id, now).map_err(|e| e.to_string()),
                EntryKind::Anime => remove_anime(paths, db, **id, now).map_err(|e| e.to_string()),
            };
            done.inspect_err(|e| tracing::error!(error = %e, entry = id, "removing from the library failed")).is_ok()
        })
        .count()
}

fn remove_manga(paths: &dyn Paths, db: &Arc<Db>, id: i64, now: i64) -> kuro_data::DataResult<()> {
    let repo = MangaRepo::new(db.clone());
    let manga = repo.by_id(id)?;
    let cover = ArtRef {
        owner: Owner::Manga,
        entry_id: id,
        source_id: manga.source,
        kind: ArtKind::Cover,
        url: manga.thumbnail_url.clone(),
        favorite: manga.favorite,
        last_modified: manga.cover_last_modified,
    };
    let removed = remove_covers(paths, &cover, now);
    repo.update(&MangaUpdate {
        favorite: Some(false),
        cover_last_modified: (removed != cover).then_some(now),
        ..MangaUpdate::new(id)
    })
}

fn remove_anime(paths: &dyn Paths, db: &Arc<Db>, id: i64, now: i64) -> kuro_data::DataResult<()> {
    let repo = AnimeRepo::new(db.clone());
    let anime = repo.by_id(id)?;
    let cover = ArtRef {
        owner: Owner::Anime,
        entry_id: id,
        source_id: anime.source,
        kind: ArtKind::Cover,
        url: anime.thumbnail_url.clone(),
        favorite: anime.favorite,
        last_modified: anime.cover_last_modified,
    };
    let background = ArtRef {
        kind: ArtKind::Background,
        url: anime.background_url.clone(),
        last_modified: anime.background_last_modified,
        ..cover.clone()
    };
    let cover_after = remove_covers(paths, &cover, now);
    let background_after = remove_covers(paths, &background, now);
    repo.update(&AnimeUpdate {
        favorite: Some(false),
        cover_last_modified: (cover_after != cover).then_some(now),
        background_last_modified: (background_after != background).then_some(now),
        ..AnimeUpdate::new(id)
    })
}

fn nav(kind: EntryKind, db: Arc<Db>) -> Nav {
    let anime = kind == EntryKind::Anime;
    // every entry opens by its id: the page for its kind, and the reader or the player for a chapter or an episode
    let open = Rc::new(move |id| crate::screens::open_entry(anime, id));
    // one launcher hook for every opener: the external player when the preference says so and a launcher is installed
    let resume = Rc::new(move |r: crate::library::view::Resume| {
        crate::updates::open_route::open(anime, r.entry, r.item, crate::updates::open_route::route(anime, r.external, false))
    });
    let delete = {
        let db = db.clone();
        Rc::new(move |ids: Vec<i64>| {
            let db = db.clone();
            let flags = local_flags(&db, kind, &ids);
            let offered = !flags.iter().any(|(_, local)| *local);
            crate::library::delete_dialog::show(kind, offered, move |choice| {
                let plan = delete_plan(&flags, choice);
                let Ok(paths) = LinuxPaths::system() else {
                    tracing::error!("no data paths; the selected entries stay in the library");
                    return;
                };
                let done = remove_from_library(&paths, &db, kind, &plan.unfavorite);
                if let Some(text) = crate::library::actions::partial_failure_toast(done, plan.unfavorite.len()) {
                    crate::library::toast::notify(text);
                }
                if !plan.delete_downloads.is_empty() {
                    let sink = DELETE_DOWNLOADS_SINK.with(|s| s.borrow().clone());
                    match sink {
                        Some(sink) => sink(kind, plan.delete_downloads),
                        None => tracing::debug!(entries = ?plan.delete_downloads, "no download manager is registered for the library"),
                    }
                }
                if let Some(view) = visible(Some(kind)) {
                    view.refresh();
                }
            });
        })
    };
    Nav {
        open,
        resume,
        // the selection bar's Download menu picks the amount; the registered manager queues the items
        download: Rc::new(move |ids, amount| {
            let sink = DOWNLOAD_SINK.with(|s| s.borrow().clone());
            match sink {
                Some(sink) => sink(kind, ids, amount),
                None => tracing::debug!(?ids, "no download manager is registered for the library"),
            }
        }),
        delete,
        global_search: Rc::new(move |query| open_global_search(kind, query)),
        edit_categories: Rc::new(|| crate::ui::open("categories")),
        selecting: Rc::new(move |on| set_selecting(kind, on)),
        update: Rc::new(move |category| match UPDATE_HOST.with(|h| h.borrow().clone()) {
            Some(host) => (host.start)(kind, category),
            None => {
                tracing::debug!(?category, "no update runner is registered for the library");
                false
            }
        }),
        update_running: Rc::new(move || UPDATE_HOST.with(|h| h.borrow().clone()).is_some_and(|host| (host.running)(kind))),
    }
}

/// The app's secret store, opened on the worker thread of [`Logins`]. Without data paths nobody is logged in.
fn open_secrets() -> Arc<dyn kuro_platform::secrets::SecretStore> {
    match LinuxPaths::system() {
        Ok(paths) => Arc::new(kuro_platform::linux::secrets::open(&paths)),
        Err(_) => Arc::new(kuro_prefs::memory::MemorySecrets::default()),
    }
}

/// Whether a chapter is on disk, for the continue button's downloaded filter. A local manga has every chapter; the
/// app keeps no anime download cache yet, so an episode is never downloaded.
fn item_downloaded(kind: EntryKind, db: Arc<Db>, index: Option<kuro_downloads::index::DownloadIndex>) -> Rc<dyn Fn(i64) -> bool> {
    Rc::new(move |id| {
        if kind == EntryKind::Anime {
            return false;
        }
        let Some(chapter) = ChapterRepo::new(db.clone()).by_id(id).ok().flatten() else { return false };
        let Ok(manga) = MangaRepo::new(db.clone()).by_id(chapter.manga_id) else { return false };
        crate::reader::launch::downloaded_lookup(index.clone(), &manga)(&chapter)
    })
}

fn deps(kind: EntryKind) -> Deps {
    let app = crate::data::app();
    let db = app.db(kind);
    let store = app.base.store().clone();
    let prefs = Rc::new(LibraryPrefs::new(store.clone()));
    let categories = Rc::new(kuro_domain::category::service::CategoryService::new(
        db.clone(),
        kind,
        Arc::new(LibraryPrefs::new(store)),
    ));
    let source_db = db.clone();
    let downloaded_only = app.base.downloaded_only.clone();
    // the per-row download count comes from the download index; the tracker logins from the secret store
    let index = crate::shell::host::Shell::get().map(|shell| shell.downloads().clone());
    let item_downloaded = item_downloaded(kind, db.clone(), index.clone());
    let logins = Rc::new(Logins::new(kind, open_secrets, move || {
        glib::MainContext::default().invoke(move || {
            if let Some(view) = visible(Some(kind)) {
                view.refresh();
            }
        });
    }));
    let (names_logins, ids_logins) = (logins.clone(), logins);
    Deps {
        kind,
        db,
        prefs,
        categories,
        facts: Facts {
            downloads: download_facts::lookup(kind, index),
            source: Arc::new(move |id| source_info(&source_db, kind, id)),
        },
        downloaded_only: Rc::new(move || downloaded_only.get()),
        logged_in: Rc::new(move || ids_logins.get()),
        tracker_names: Rc::new(move || trackers::names(kind, &names_logins.get())),
        always_external: Rc::new(|| crate::misc::diet_defaults::PLAYER_ALWAYS_EXTERNAL),
        item_downloaded,
    }
}

/// Marks `kind` as watched in `slot`; `false` when it already was.
fn claim_watch(slot: &Cell<u8>, kind: EntryKind) -> bool {
    let fresh = slot.get() & bit(kind) == 0;
    slot.set(slot.get() | bit(kind));
    fresh
}

/// Reloads the library of `kind` whenever the database changes under it: a chapter read, an entry added from Browse, a
/// library update, a category renamed (T-08a-032). The first publication of each watch is the state the view loads
/// itself, so it is skipped; an error publication is logged and the next change tries again. Bound once per kind:
/// `refresh` reloads the visible view, so a second watch (another build of the page) would only reload it twice.
fn watch_database(kind: EntryKind) {
    use crate::watch::bind_watch;
    if !WATCHED.with(|w| claim_watch(w, kind)) {
        return;
    }
    let app = crate::data::app();
    let rows = on_change("library", move || refresh(kind));
    let categories = on_change("categories", move || refresh(kind));
    match kind {
        EntryKind::Anime => bind_watch(AnimeRepo::new(app.db(kind)).subscribe_library(), move |p| rows(p.and_then(|s| (*s).as_ref().map(drop).map_err(String::clone)))),
        EntryKind::Manga => bind_watch(MangaRepo::new(app.db(kind)).subscribe_library(), move |p| rows(p.and_then(|s| (*s).as_ref().map(drop).map_err(String::clone)))),
    }
    bind_watch(app.categories(kind).repo().watch_all(), move |p| categories(p.map(drop)));
}

/// A watch callback: the first publication is ignored (the view loaded that state itself), an error is logged, and
/// every later change runs `reload`.
fn on_change(label: &'static str, reload: impl Fn() + 'static) -> impl Fn(Result<(), String>) {
    let first = Cell::new(true);
    move |published| match published {
        Err(error) => tracing::warn!(%error, watch = label, "the library watch published an error"),
        Ok(()) if first.replace(false) => {}
        Ok(()) => reload(),
    }
}

/// The library page of one kind: the view, and for the page header its search entry, its refresh button and the one
/// menu (docs/design/main-window.md). `None` when the worker thread cannot start, so the caller keeps its fallback.
pub fn build(anime: bool) -> Option<Built> {
    let kind = if anime { EntryKind::Anime } else { EntryKind::Manga };
    let app = crate::data::app();
    // subscribed before the view's first load, so a write between the two is not missed
    watch_database(kind);
    let view = match LibraryView::new(deps(kind), nav(kind, app.db(kind))) {
        Ok(view) => view,
        Err(e) => {
            tracing::error!(error = %e, "the library view could not start");
            return None;
        }
    };
    VIEWS.with(|views| views.borrow_mut().push((kind, view.clone())));
    let pending: Vec<String> = PENDING_SEARCH.with(|p| {
        let mut p = p.borrow_mut();
        let mine = p.iter().filter(|(k, _)| *k == kind).map(|(_, q)| q.clone()).collect();
        p.retain(|(k, _)| *k != kind);
        mine
    });
    if let Some(query) = pending.last() {
        view.set_query(query);
    }
    let menu = header_menu(kind, &view);
    // the menu icon takes the theme's selected colour while a filter is on, the forced downloaded-only view included
    // (T-37a-056)
    let tinted = menu.clone();
    view.on_filters_active(move |active| {
        if active {
            tinted.add_css_class(crate::theme::style::class::ACTIVE);
        } else {
            tinted.remove_css_class(crate::theme::style::class::ACTIVE);
        }
    });
    let (search, refresh) = view.controls();
    Some(Built { content: view.widget(), end: vec![search.upcast(), refresh.upcast(), menu.upcast()] })
}

/// The labels of the header menu, section by section: the sheet tabs, the library actions, the local folders.
pub fn menu_sections(kind: EntryKind) -> [Vec<(String, &'static str)>; 3] {
    let what = if kind == EntryKind::Anime { "anime" } else { "manga" };
    [
        vec![
            (format!("{}…", tr(MsgKey::action_filter)), "filter"),
            (format!("{}…", tr(MsgKey::action_sort)), "sort"),
            (format!("{}…", tr(MsgKey::action_display)), "display"),
        ],
        vec![(tr(MsgKey::action_update_library), "update-library"), ("Open random entry".to_owned(), "random")],
        vec![(format!("Add local {what} folder…"), "add-local"), (format!("Rescan local {what} folders"), "rescan-local")],
    ]
}

/// The one header menu: a `GtkPopoverMenu` over the actions of a `lib` group on the button.
fn header_menu(kind: EntryKind, view: &LibraryView) -> gtk::MenuButton {
    let model = gtk::gio::Menu::new();
    for section in menu_sections(kind) {
        let part = gtk::gio::Menu::new();
        for (label, action) in section {
            part.append(Some(&label), Some(&format!("lib.{action}")));
        }
        model.append_section(None, &part);
    }
    let button = crate::ui::quiet_menu(gtk::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&model).tooltip_text("Library menu").build());
    button.update_property(&[gtk::accessible::Property::Label("Library menu")]);
    button.insert_action_group("lib", Some(&menu_actions(kind, view)));
    button
}

/// The `lib` actions behind the header menu's items.
fn menu_actions(kind: EntryKind, view: &LibraryView) -> gtk::gio::SimpleActionGroup {
    use crate::library::view::sheet::Tab;
    let group = gtk::gio::SimpleActionGroup::new();
    let add = |name: &str, run: Box<dyn Fn()>| {
        let action = gtk::gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| run());
        group.add_action(&action);
    };
    for (name, tab) in [("filter", Tab::Filter), ("sort", Tab::Sort), ("display", Tab::Display)] {
        let v = view.clone();
        add(name, Box::new(move || v.open_settings(tab)));
    }
    let v = view.clone();
    add("update-library", Box::new(move || v.update_library()));
    let v = view.clone();
    add("random", Box::new(move || v.open_random()));
    let (folder, rescan) = local_folder_actions(kind);
    add("add-local", folder);
    add("rescan-local", rescan);
    group
}

/// Add-folder and rescan, from the local modules of `kind`.
fn local_folder_actions(kind: EntryKind) -> (Box<dyn Fn()>, Box<dyn Fn()>) {
    let [add, rescan]: [(&'static str, fn()); 2] = match kind {
        EntryKind::Manga => [("manga add", crate::local_flow::ui::pick_folder), ("manga rescan", crate::local_flow::ui::rescan_all)],
        EntryKind::Anime => [("anime add", crate::local_flow_anime::choose_folder), ("anime rescan", crate::local_flow_anime::rescan_in_background)],
    };
    (Box::new(move || run_local(add)), Box::new(move || run_local(rescan)))
}

#[cfg(test)]
thread_local! {
    static LOCAL_CALLS: RefCell<Option<Vec<&'static str>>> = const { RefCell::new(None) };
}

fn run_local((name, run): (&'static str, fn())) {
    #[cfg(test)]
    if LOCAL_CALLS.with(|c| c.borrow_mut().as_mut().map(|calls| calls.push(name)).is_some()) {
        return;
    }
    let _ = name;
    run();
}

#[cfg(test)]
mod tests {
    use kuro_data::manga::{Manga, MangaUpdate};
    use kuro_testkit::TempPaths;

    use super::*;

    #[test]
    fn the_database_watch_binds_once_per_kind() {
        let slot = Cell::new(0);
        assert!(claim_watch(&slot, EntryKind::Manga), "the first build binds it");
        assert!(!claim_watch(&slot, EntryKind::Manga), "a second build of the page does not");
        assert!(claim_watch(&slot, EntryKind::Anime), "the other kind has its own");
    }

    /// Needs a display and runs in a child process with fresh app data (see `screens::alone`).
    #[test]
    fn a_favourite_added_to_the_database_shows_in_the_mounted_library() {
        crate::screens::alone::run(module_path!(), "a_favourite_added_to_the_database_shows_in_the_mounted_library", |ctx| {
            let (_stack, _window) = crate::testing::nav();
            let built = build(false).expect("the library mounts");
            build(false).expect("a second build of the page");
            let view = visible(Some(EntryKind::Manga)).expect("the manga view is registered");
            assert!(crate::screens::alone::pump(ctx, || view.cover_count() == 0), "an empty library loads");
            let repo = MangaRepo::new(crate::data::app().db(EntryKind::Manga));
            let id = repo.insert(&Manga { source: 7, url: "/b".into(), title: "Berserk".into(), ..Manga::create() }).expect("insert");
            repo.update(&MangaUpdate { favorite: Some(true), ..MangaUpdate::new(id) }).expect("favourite");
            assert!(crate::screens::alone::pump(ctx, || view.cover_count() == 1), "the favourite shows without a refresh");
            drop(built);
        });
    }

    fn manga_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("open manga db");
        (dir, Arc::new(Db::new(&path, writer).expect("manga db handle")))
    }

    #[test]
    fn selection_mode_of_either_kind_is_reported_to_the_shell() {
        let calls = Rc::new(Cell::new(0));
        let seen = calls.clone();
        on_selection_change(move || seen.set(seen.get() + 1));
        assert!(!selecting());
        set_selecting(EntryKind::Anime, true);
        set_selecting(EntryKind::Manga, true);
        assert!(selecting());
        set_selecting(EntryKind::Anime, false);
        assert!(selecting(), "the manga view is still selecting");
        set_selecting(EntryKind::Manga, false);
        assert!(!selecting());
        assert_eq!(calls.get(), 4);
    }

    #[test]
    fn t2_del_01_removal_clears_the_favorite_and_leaves_the_rest() {
        let (_dir, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&Manga { favorite: true, source: 7, url: "u".into(), title: "Berserk".into(), ..Manga::create() }).expect("insert");
        assert_eq!(repo.by_id(id).expect("stored").title, "Berserk");
        let paths = TempPaths::new().expect("temp paths");
        assert_eq!(remove_from_library(&paths, &db, EntryKind::Manga, &[id]), 1);
        assert!(!repo.by_id(id).expect("still stored").favorite);
    }

    #[test]
    fn t2_del_03_removal_deletes_the_cover_and_stamps_its_timestamp() {
        let (_dir, db) = manga_db();
        let paths = TempPaths::new().expect("temp paths");
        let repo = MangaRepo::new(db.clone());
        let id = repo
            .insert(&Manga { favorite: true, source: 7, url: "u".into(), title: "Berserk".into(), thumbnail_url: Some("https://img.test/c.jpg".into()), ..Manga::create() })
            .expect("insert");
        let art = ArtRef {
            owner: Owner::Manga,
            entry_id: id,
            source_id: 7,
            kind: ArtKind::Cover,
            url: Some("https://img.test/c.jpg".into()),
            favorite: true,
            last_modified: 0,
        };
        let file = kuro_images::cover_paths::cover_file_for(&paths, &art).expect("a cover path");
        std::fs::create_dir_all(file.parent().expect("a directory")).expect("cover dir");
        std::fs::write(&file, b"png").expect("cover file");
        remove_from_library(&paths, &db, EntryKind::Manga, &[id]);
        assert!(!file.exists(), "the cached cover is deleted");
        assert!(repo.by_id(id).expect("stored").cover_last_modified > 0);
    }

    #[test]
    fn a_removal_with_no_cached_cover_leaves_the_cover_timestamp_alone() {
        // Kotlin's removeCovers moves the timestamp only when it deleted a file.
        let (_dir, db) = manga_db();
        let paths = TempPaths::new().expect("temp paths");
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&Manga { favorite: true, source: 7, url: "u".into(), title: "Berserk".into(), ..Manga::create() }).expect("insert");
        remove_from_library(&paths, &db, EntryKind::Manga, &[id]);
        assert_eq!(repo.by_id(id).expect("stored").cover_last_modified, 0);
    }

    #[test]
    fn t_08a_032_a_library_watch_skips_its_first_publication_and_reloads_on_each_change_after_it() {
        let reloads = Rc::new(Cell::new(0));
        let seen = reloads.clone();
        let watch = on_change("library", move || seen.set(seen.get() + 1));
        watch(Ok(()));
        assert_eq!(reloads.get(), 0, "the state the view already loaded");
        watch(Ok(()));
        watch(Err("database is locked".into()));
        watch(Ok(()));
        assert_eq!(reloads.get(), 2, "each change reloads; an error only logs and the watcher goes on");
    }

    #[test]
    fn t_37a_061_a_search_for_an_unbuilt_view_waits_for_its_mount() {
        PENDING_SEARCH.with(|p| p.borrow_mut().clear());
        inject_search(EntryKind::Manga, "one piece");
        let waiting = PENDING_SEARCH.with(|p| p.borrow().clone());
        assert_eq!(waiting, vec![(EntryKind::Manga, "one piece".to_owned())]);
        PENDING_SEARCH.with(|p| p.borrow_mut().clear());
    }

    #[test]
    fn the_header_menu_holds_the_sheet_tabs_the_library_actions_and_the_local_folders_once_each() {
        for kind in [EntryKind::Anime, EntryKind::Manga] {
            let sections = menu_sections(kind);
            let actions: Vec<&str> = sections.iter().flatten().map(|(_, a)| *a).collect();
            assert_eq!(actions, ["filter", "sort", "display", "update-library", "random", "add-local", "rescan-local"]);
            let labels: Vec<&str> = sections.iter().flatten().map(|(l, _)| l.as_str()).collect();
            assert!(!labels.iter().any(|l| l.contains("Update category")), "the header refresh button is the only category update");
        }
        assert_eq!(menu_sections(EntryKind::Anime)[2][0].0, "Add local anime folder…");
        assert_eq!(menu_sections(EntryKind::Manga)[2][1].0, "Rescan local manga folders");
    }

    #[test]
    fn an_unknown_source_shows_its_id_and_no_language() {
        let (_dir, db) = manga_db();
        assert_eq!(source_info(&db, EntryKind::Manga, 42), SourceInfo { name: "42".into(), language: String::new() });
    }

    #[test]
    fn the_real_header_menu_registers_every_action_and_reaches_the_local_flow() {
        crate::testing::with_display(|| {
            let (_dir, view) = crate::library::view::test_view(crate::library::view::test_nav());
            for (kind, word) in [(EntryKind::Manga, "manga"), (EntryKind::Anime, "anime")] {
                let group = menu_actions(kind, &view);
                let mut names: Vec<String> = group.list_actions().iter().map(|n| n.to_string()).collect();
                names.sort();
                assert_eq!(names, ["add-local", "display", "filter", "random", "rescan-local", "sort", "update-library"]);
                for section in menu_sections(kind) {
                    for (_, action) in section {
                        assert!(group.has_action(action), "the menu item lib.{action} has an action");
                    }
                }
                LOCAL_CALLS.with(|c| *c.borrow_mut() = Some(Vec::new()));
                // through the real button, so the group must be inserted under `lib`
                let button = header_menu(kind, &view);
                button.activate_action("lib.add-local", None).expect("lib.add-local");
                button.activate_action("lib.rescan-local", None).expect("lib.rescan-local");
                let calls = LOCAL_CALLS.with(|c| c.borrow_mut().take()).unwrap_or_default();
                assert_eq!(calls, [format!("{word} add"), format!("{word} rescan")]);
            }
        });
    }
}
