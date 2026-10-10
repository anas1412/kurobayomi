//! Mounts the library view in the window (T-37a-061): builds the view's dependencies from the app data, registers
//! the views so the shell can reach them, and exposes what the shell needs. The two kinds share one sidebar
//! destination (`library`) with an Anime and a Manga page; each kind still has its own sidebar index per nav style
//! ([`sidebar_index`]).
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
use kuro_data::manga::{MangaRepo, MangaUpdate};
use kuro_db::handle::Db;
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};
use kuro_images::art::{ArtKind, ArtRef, Owner};
use kuro_images::invalidate::remove_covers;
use kuro_local::LOCAL_SOURCE_ID;
use kuro_platform::linux::LinuxPaths;
use kuro_platform::paths::Paths;
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::ui::NavStyle;

use crate::library::actions::delete_plan;
use crate::library::trackers::{self, Logins};
use crate::library::download_facts;
use crate::library::query::{Facts, SourceInfo};
use crate::library::view::{Deps, LibraryView, Nav};
use crate::screens::Built;

thread_local! {
    static VIEWS: RefCell<Vec<(EntryKind, LibraryView)>> = const { RefCell::new(Vec::new()) };
    /// Bit 0: the anime view is selecting. Bit 1: the manga view is.
    static SELECTING: Cell<u8> = const { Cell::new(0) };
    static LISTENER: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    /// Searches injected before the view of their kind was built; each is applied when that view mounts.
    static PENDING_SEARCH: RefCell<Vec<(EntryKind, String)>> = const { RefCell::new(Vec::new()) };
    /// What the selection Download action does with its entries (the app's download manager registers here).
    static DELETE_DOWNLOADS_SINK: RefCell<Option<Rc<dyn Fn(EntryKind, Vec<i64>)>>> = const { RefCell::new(None) };
    /// What the refresh button does (the update runner in `crate::misc` registers here).
    static UPDATE_HOST: RefCell<Option<UpdateHost>> = const { RefCell::new(None) };
    static DOWNLOAD_SINK: RefCell<Option<Rc<dyn Fn(EntryKind, Vec<i64>, DownloadAmount)>>> = const { RefCell::new(None) };
}

/// The screen id of the global search page (`spec/entries.json`).
pub const GLOBAL_SEARCH_SCREEN: &str = "global_search";

pub use crate::library::actions::DownloadAmount;

/// The sidebar index of a kind's library for a nav style: anime is 0, manga is 1, or 5 when the style moves the
/// manga library into More (`MangaLibraryTab.kt`, B-1.1).
pub fn sidebar_index(kind: EntryKind, style: NavStyle) -> u8 {
    match kind {
        EntryKind::Anime => 0,
        EntryKind::Manga if style == NavStyle::MOVE_MANGA_TO_MORE => 5,
        EntryKind::Manga => 1,
    }
}

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
        view.open_settings();
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
    if let Some(view) = visible(Some(kind)) {
        let page = if kind == EntryKind::Anime { "anime" } else { "manga" };
        if let Some(stack) = view.widget().ancestor(gtk::Stack::static_type()).and_downcast::<gtk::Stack>() {
            stack.set_visible_child_name(page);
        }
    }
}

/// Opens the global search page with `query` in its search box (T-37a-022).
fn open_global_search(query: &str) {
    crate::ui::open(GLOBAL_SEARCH_SCREEN);
    if query.is_empty() {
        return;
    }
    let name = format!("screen:{GLOBAL_SEARCH_SCREEN}");
    let window = crate::ui::window();
    // the page pushed last is the newest widget carrying the screen name
    if let Some(entry) = find_in_named(window.upcast_ref(), &name).and_then(|page| first_search_entry(&page)) {
        entry.set_text(query);
    }
}

fn find_in_named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    let mut found = (root.widget_name() == name).then(|| root.clone());
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(inner) = find_in_named(&c, name) {
            found = Some(inner);
        }
        child = c.next_sibling();
    }
    found
}

fn first_search_entry(root: &gtk::Widget) -> Option<gtk::SearchEntry> {
    if let Some(entry) = root.downcast_ref::<gtk::SearchEntry>() {
        return Some(entry.clone());
    }
    let mut child = root.first_child();
    while let Some(c) = child {
        if let Some(entry) = first_search_entry(&c) {
            return Some(entry);
        }
        child = c.next_sibling();
    }
    None
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

/// The title of an entry, for the pages that still open by title.
pub fn entry_title(db: &Arc<Db>, kind: EntryKind, id: i64) -> Option<String> {
    match kind {
        EntryKind::Manga => MangaRepo::new(db.clone()).by_id(id).ok().map(|m| m.title),
        EntryKind::Anime => AnimeRepo::new(db.clone()).by_id(id).ok().map(|a| a.title),
    }
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

fn spec_id(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Anime => "anime_library",
        EntryKind::Manga => "manga_library",
    }
}

/// Whether the entry is a local one (source 0). Local entries open by id, so two folders with one title stay apart.
fn is_local_entry(db: &Arc<Db>, kind: EntryKind, id: i64) -> bool {
    let source = match kind {
        EntryKind::Manga => MangaRepo::new(db.clone()).by_id(id).map(|m| m.source),
        EntryKind::Anime => AnimeRepo::new(db.clone()).by_id(id).map(|a| a.source),
    };
    source.is_ok_and(|s| s == LOCAL_SOURCE_ID)
}

fn nav(kind: EntryKind, db: Arc<Db>) -> Nav {
    let anime = kind == EntryKind::Anime;
    let open = {
        let db = db.clone();
        Rc::new(move |id| {
            if is_local_entry(&db, kind, id) {
                match kind {
                    EntryKind::Manga => crate::local_flow::open::entry(id),
                    EntryKind::Anime => crate::entry_core::anime::route::open(id),
                }
            } else if let Some(title) = entry_title(&db, kind, id) {
                crate::screens::open_entry(&title, anime);
            }
        })
    };
    let resume = {
        let db = db.clone();
        Rc::new(move |r: crate::library::view::Resume| {
            if is_local_entry(&db, kind, r.entry) {
                // the item is a chapter or episode id, so the local reader or player finds it without its name
                match kind {
                    EntryKind::Manga => crate::ui::push(&crate::local_flow::reader::open_local_chapter(r.entry, r.item)),
                    EntryKind::Anime => {
                        crate::local_flow_anime::open_local_episode(r.item);
                    }
                }
            } else if let Some(title) = entry_title(&db, kind, r.entry) {
                crate::screens::open_media(anime, &title, &r.item.to_string());
            }
        })
    };
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
        global_search: Rc::new(open_global_search),
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
        always_external: Rc::new(|| false),
        item_downloaded: Rc::new(|_| false),
    }
}

/// The library page of one kind: the view and the header actions (settings sheet, overflow menu). `None` when the
/// worker thread cannot start, so the caller keeps its fallback.
pub fn build(anime: bool) -> Option<Built> {
    let kind = if anime { EntryKind::Anime } else { EntryKind::Manga };
    let app = crate::data::app();
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
    let filter = crate::ui::icon_button("view-filter-symbolic", &tr(MsgKey::action_filter));
    let v = view.clone();
    filter.connect_clicked(move |_| v.open_settings());
    // tinted while a filter is on, the forced downloaded-only view included (T-37a-056)
    let tinted = filter.clone();
    view.on_filters_active(move |active| {
        if active {
            tinted.add_css_class("accent");
        } else {
            tinted.remove_css_class("accent");
        }
    });
    let mut entries = crate::ui::screen(spec_id(kind)).menu;
    let update_label = tr(MsgKey::action_update_library);
    if !entries.contains(&update_label) {
        entries.insert(0, update_label.clone());
    }
    let v = view.clone();
    let menu = crate::ui::menu_button("view-more-symbolic", &entries, move |label| {
        if label == "Open random entry" {
            v.open_random();
        } else if label == update_label {
            v.update_library();
        }
    });
    Some(Built { content: view.widget(), end: vec![filter.upcast(), menu.upcast()] })
}

#[cfg(test)]
mod tests {
    use kuro_data::manga::Manga;
    use kuro_testkit::TempPaths;

    use super::*;

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
        assert_eq!(entry_title(&db, EntryKind::Manga, id).as_deref(), Some("Berserk"));
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
    fn t_37a_061_anime_is_index_zero_and_manga_one_or_five_by_nav_style() {
        for style in NavStyle::ALL.iter().copied() {
            assert_eq!(sidebar_index(EntryKind::Anime, style), 0, "{style:?}");
            let manga = if style == NavStyle::MOVE_MANGA_TO_MORE { 5 } else { 1 };
            assert_eq!(sidebar_index(EntryKind::Manga, style), manga, "{style:?}");
        }
    }

    #[test]
    fn t_37a_022_the_global_search_screen_is_a_spec_screen() {
        crate::ui::with_spec(|s| assert!(s.screen(GLOBAL_SEARCH_SCREEN).is_some()));
        assert_ne!(GLOBAL_SEARCH_SCREEN, "browse", "the Browse sidebar destination is not the search screen");
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
    fn an_unknown_source_shows_its_id_and_no_language() {
        let (_dir, db) = manga_db();
        assert_eq!(source_info(&db, EntryKind::Manga, 42), SourceInfo { name: "42".into(), language: String::new() });
    }
}
