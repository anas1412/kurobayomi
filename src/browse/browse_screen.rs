//! The browse source screens (`browse_source`, `migrate_source_search`; T-40b-034, T-40b-038): one source's listing built
//! over the installed source directory, and the open page that a genre search from a manga page reaches. The page itself is
//! `pages::browse_source`; this builds its dependencies from the app, parks nothing and awaits nothing.
//!
//! A genre or source-line search of an entry page arrives through `entry_core::handoff`. When the page below the entry page
//! is a browse page, that page runs the search on its own listing (`search_open`) and the entry page and the placeholder
//! this builds are popped; with no browse page below, global search runs it (the Aniyomi fallback).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gtk::glib;
use gtk::prelude::*;
use kuro_domain::source::browse::BrowseQuery;
use kuro_i18n::{MsgKey, tr};
use kuro_source::filter_list::FilterList;
use tokio::sync::watch;

use super::directory::{SourceDirectory, source_or_stub};
use super::entries::FavoriteStore;
use super::filters::{SourceFilters, genre_search};
use super::header::OverflowItem;
use super::listing::{LoadFuture, PageLoader};
use super::listing_view::{ListingDeps, WeakListingView};
use super::pages::{BrowseSourceDeps, FavoritesDeps, SourcePage, browse_source};
use super::pending;
use super::prefs::BrowsePrefs;
use super::primitives::{EmptyAction, ToastSpec};
use super::route::{BrowseRoute, search_listing};
use super::sources::LOCAL_SOURCE_ID;
use super::view::file_cover_loader;

/// What a source page needs of the app.
pub struct BrowseEnv {
    pub prefs: Rc<BrowsePrefs>,
    pub directory: Arc<dyn SourceDirectory>,
    pub store: Arc<dyn FavoriteStore>,
    pub open_entry: Rc<dyn Fn(i64)>,
    /// The duplicate dialog's Migrate existing entry.
    pub migrate: Rc<dyn Fn(i64, i64)>,
    pub edit_categories: Rc<dyn Fn()>,
    pub on_back: Rc<dyn Fn()>,
    pub open_url: Rc<dyn Fn(&str)>,
    /// Open in WebView: the url and the source's name.
    pub open_webview: Rc<dyn Fn(String, String)>,
    pub toast: Rc<dyn Fn(ToastSpec)>,
    pub now_ms: Rc<dyn Fn() -> i64>,
}

/// The loader of a source that is not installed. The stub page never asks it.
pub(crate) struct NoLoader;

impl PageLoader for NoLoader {
    fn load(&self, _query: BrowseQuery, _filters: FilterList, _page: u32) -> LoadFuture {
        Box::pin(async { Err(tr(MsgKey::unknown_error)) })
    }
}

/// The page of `source_id` opened on `query`. A source the directory does not know gets the missing-source page. The
/// hide-in-library preference is read here, when the page is made (T-40b-034). `incognito` says that opening the source
/// stores no last-used id.
pub fn source_page(env: &BrowseEnv, source_id: i64, query: BrowseQuery, incognito: bool) -> SourcePage {
    let listed = env.directory.sources().into_iter().find(|entry| entry.source.id == source_id && !entry.source.is_stub);
    let loader = listed.as_ref().and_then(|_| env.directory.loader(source_id));
    let stub = loader.is_none();
    let source = listed.as_ref().map_or_else(|| source_or_stub(Some(&*env.directory), source_id), |entry| entry.source.clone());
    let home = listed.as_ref().and_then(|entry| entry.home_url.clone());
    let defaults = if stub { FilterList::default() } else { env.directory.filters(source_id).unwrap_or_default() };
    let filters = Rc::new(SourceFilters::new(defaults));
    let (_never_sent, ready) = watch::channel(true);

    let open_web: Rc<dyn Fn()> = {
        let (open, name) = (Rc::clone(&env.open_webview), source.name.clone());
        Rc::new(move || {
            if let Some(url) = &home {
                open(url.clone(), name.clone());
            }
        })
    };
    let (web_overflow, web_action) = (Rc::clone(&open_web), open_web);
    let page = browse_source(BrowseSourceDeps {
        supports_latest: source.supports_latest,
        configurable: false,
        incognito,
        prefs: Rc::clone(&env.prefs),
        listing: ListingDeps {
            loader: loader.unwrap_or_else(|| Arc::new(NoLoader)),
            prefs: Rc::clone(&env.prefs),
            ready,
            stub,
            is_local: source_id == LOCAL_SOURCE_ID,
            source_id,
            source_name: source.name.clone(),
            source_lang: source.lang.clone(),
            load_cover: file_cover_loader(),
            open_entry: {
                let open = Rc::clone(&env.open_entry);
                Rc::new(move |item| open(item.id))
            },
            on_long_press: Rc::new(|_| {}),
            favorite_urls: None,
            on_action: Rc::new(move |action| {
                if action == EmptyAction::OpenInWebView {
                    web_action();
                }
            }),
        },
        filters: Rc::clone(&filters),
        favorites: Some(FavoritesDeps {
            store: Arc::clone(&env.store),
            toast: Rc::clone(&env.toast),
            open_entry: Rc::clone(&env.open_entry),
            migrate: Rc::clone(&env.migrate),
            now_ms: Rc::clone(&env.now_ms),
            edit_categories: Rc::clone(&env.edit_categories),
            anime: false,
        }),
        hide_in_library: env.prefs.hide_in_library(),
        initial_query: query,
        on_back: Rc::clone(&env.on_back),
        on_overflow: Rc::new(move |item| {
            if item == OverflowItem::OpenInWebView {
                web_overflow();
            }
        }),
        open_url: Rc::clone(&env.open_url),
    });
    if let SourcePage::Browse(shown) = &page {
        register_page(shown, filters);
    }
    page
}

/// The browse page a genre search can reach: its listing, its filters, its search box and its Popular and Latest chips.
struct OpenBrowse {
    listing: WeakListingView,
    filters: Rc<SourceFilters>,
    search: glib::WeakRef<gtk::SearchEntry>,
    chips: [Option<glib::WeakRef<gtk::ToggleButton>>; 2],
}

thread_local! {
    /// The browse page opened last. It holds weak references only, so it never keeps a popped page alive.
    static OPEN: RefCell<Option<OpenBrowse>> = const { RefCell::new(None) };
}

fn register(page: OpenBrowse) {
    OPEN.with(|slot| *slot.borrow_mut() = Some(page));
}

/// Makes `shown` the page a genre search from an entry page reaches. The anime browse page registers itself the same way,
/// so an anime entry's genre or author search runs on the anime listing below it (T-41b-024).
pub fn register_page(shown: &super::pages::BrowseSourcePage, filters: Rc<SourceFilters>) {
    register(OpenBrowse {
        listing: shown.listing.downgrade(),
        filters,
        search: shown.search.downgrade(),
        chips: [Some(shown.popular.downgrade()), shown.latest.as_ref().map(|chip| chip.downgrade())],
    });
}

/// A genre search on the open browse page (`searchGenre` of the page the entry page was opened from, T-40b-038): a genre
/// that a group or select holds sets that filter and searches with no text, any other genre is the search text. The
/// listing restarts and the call returns at once; nothing is awaited. False when no browse page is open.
pub fn search_open(genre: &str) -> bool {
    OPEN.with(|slot| {
        let open = slot.borrow();
        let Some(open) = open.as_ref() else { return false };
        let Some(listing) = open.listing.upgrade() else { return false };
        let (text, list) = genre_search(&open.filters.fresh(), genre);
        open.filters.load(list.clone());
        let text = text.unwrap_or_default();
        if let Some(entry) = open.search.upgrade() {
            entry.set_text(&text);
        }
        for chip in open.chips.iter().flatten().filter_map(|chip| chip.upgrade()) {
            chip.set_active(false);
        }
        listing.restart_with(search_listing(&text), list);
        true
    })
}

/// Whether the page under the one on show is a browse page. `names` are the widget names of the stacked pages and `shown`
/// the index of the page on show (`ui::page_stack`).
pub fn previous_is_browse(names: &[String], shown: usize) -> bool {
    shown > 0 && names.get(shown - 1).is_some_and(|name| name == "screen:browse_source")
}

/// The widget name of the page on show.
pub fn visible_page() -> String {
    let (names, shown) = crate::ui::page_stack();
    names.get(shown).cloned().unwrap_or_default()
}

/// The app side of a source page, over the manga library.
fn production_env(directory: Arc<dyn SourceDirectory>) -> BrowseEnv {
    use kuro_data::category::{CategoryKind, CategoryRepo};
    use kuro_data::manga::MangaRepo;
    use kuro_domain::kind::EntryKind;

    let app = crate::data::app();
    let db = app.db(EntryKind::Manga);
    let prefs = Rc::new(BrowsePrefs::new(app.base.store().clone()));
    let manga = MangaRepo::new(Arc::clone(&db));
    let store = super::repo_store::RepoStore::new(
        manga.clone(),
        CategoryRepo::new(db, CategoryKind::Manga),
        prefs.default_manga_category.clone(),
        // the cover cache belongs to the image layer, which keeps no per-entry files here yet
        Arc::new(|_| 0),
    );
    BrowseEnv {
        prefs,
        directory,
        store: Arc::new(store),
        open_entry: Rc::new(crate::entry_core::route::open),
        // the migrate dialog opens at once with old = the duplicate and new = the browsed manga (T-40b-046)
        migrate: Rc::new(|old, new| (super::search_screen::manga_migrate_dialog(old))(new)),
        edit_categories: Rc::new(super::glue::open_manga_categories),
        on_back: Rc::new(crate::ui::pop),
        open_url: Rc::new(|url| super::anime::browse::open_external(url, None)),
        open_webview: Rc::new(|url, title| crate::entry_core::handoff::open_webview(crate::entry_core::handoff::WebViewRequest { url, title })),
        toast: Rc::new(|spec| crate::toast::notify(spec.message)),
        now_ms: Rc::new(|| {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        }),
    }
}

/// The page for `browse_source` or `migrate_source_search`, as `screens::build` asks for it. `None` leaves the screen to its
/// sample page: no app data, no source directory, or nothing was parked for this screen.
pub fn page(screen: &str) -> Option<gtk::Box> {
    if !crate::data::is_installed() {
        return None;
    }
    let route = pending::take_for(screen);
    // an anime source row, a global search row or an anime entry page parked the anime route for the same screen
    if let Some(BrowseRoute::AnimeBrowseSource { source_id, query }) = route {
        return super::anime::browse_screen::page(source_id, query);
    }
    if let Some(BrowseRoute::BrowseSource { source_id, query, .. }) = route {
        let directory = super::directory::current()?;
        let env = production_env(directory);
        let package = env.directory.sources().into_iter().find(|entry| entry.source.id == source_id).map(|entry| entry.package);
        let incognito = crate::data::app().base.incognito_mode.get()
            || package.is_some_and(|package| env.prefs.sources.incognito_manga_extensions.get().contains(&package));
        let root = source_page(&env, source_id, query, incognito).widget().clone();
        crate::ui::tag(&root, screen);
        return Some(root);
    }
    entry_search(screen)
}

/// The entry page on show that a parked genre or title search comes from, and the kind of its catalogue.
pub(super) fn search_caller() -> Option<super::route::BrowseKind> {
    match visible_page().as_str() {
        "screen:manga_entry_details" => Some(super::route::BrowseKind::Manga),
        "screen:anime_entry_details" => Some(super::route::BrowseKind::Anime),
        _ => None,
    }
}

/// A genre or source-line search of the manga or anime page on show.
fn entry_search(screen: &str) -> Option<gtk::Box> {
    let kind = search_caller()?;
    let request = crate::entry_core::handoff::take_search()?;
    let (names, shown) = crate::ui::page_stack();
    if previous_is_browse(&names, shown) && search_open(&request.query) {
        // the search ran on the page below: this placeholder and the manga page go
        glib::idle_add_local_once(|| {
            crate::ui::pop();
            crate::ui::pop();
        });
        let placeholder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        crate::ui::tag(&placeholder, screen);
        return Some(placeholder);
    }
    pending::park(super::global_search::deep_link_fallback_for(kind, &request.query));
    Some(crate::screens::build("global_search"))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::browse::directory::tests::{FakeDirectory, entry};
    use crate::browse::entries::fake::FakeStore;
    use crate::browse::listing::ListItem;

    fn pump_until(mut done: impl FnMut() -> bool) -> bool {
        let context = glib::MainContext::default();
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(3) {
            while context.iteration(false) {}
            if done() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        done()
    }

    struct Rig {
        env: BrowseEnv,
        failing: Arc<std::sync::atomic::AtomicBool>,
        asked: Arc<Mutex<Vec<BrowseQuery>>>,
        log: Rc<RefCell<Vec<String>>>,
        _entered: tokio::runtime::EnterGuard<'static>,
    }

    /// A directory with the source 1 (supports no latest) whose listings hold a favourite and a new entry.
    fn rig() -> Rig {
        let rt: &'static tokio::runtime::Runtime =
            Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime")));
        let entered = rt.enter();
        let asked: Arc<Mutex<Vec<BrowseQuery>>> = Arc::default();
        let log_asked = Arc::clone(&asked);
        let failing = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fails = Arc::clone(&failing);
        let mut alpha = entry(1, "Alpha", "en", "ext.a");
        alpha.home_url = Some("https://alpha.example".into());
        let directory = Arc::new(FakeDirectory {
            entries: vec![alpha],
            answers: Arc::new(move |_, query| {
                log_asked.lock().expect("lock").push(query.clone());
                if fails.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err("offline".into());
                }
                Ok(vec![
                    ListItem { id: 1, title: "mine".into(), url: "/mine".into(), favorite: true, ..ListItem::default() },
                    ListItem { id: 2, title: "new".into(), url: "/new".into(), ..ListItem::default() },
                ])
            }),
        });
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let (opened, web, back) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let env = BrowseEnv {
            prefs: Rc::new(BrowsePrefs::new(PrefStore::in_memory())),
            directory,
            store: Arc::new(FakeStore::default()),
            open_entry: Rc::new(move |id| opened.borrow_mut().push(format!("entry {id}"))),
            migrate: Rc::new(|_, _| {}),
            edit_categories: Rc::new(|| {}),
            on_back: Rc::new(move || back.borrow_mut().push("back".into())),
            open_url: Rc::new(|_| {}),
            open_webview: Rc::new(move |url, name| web.borrow_mut().push(format!("webview {url} {name}"))),
            toast: Rc::new(|_| {}),
            now_ms: Rc::new(|| 1),
        };
        Rig { env, failing, asked, log, _entered: entered }
    }

    fn browse(page: &SourcePage) -> &crate::browse::pages::BrowseSourcePage {
        match page {
            SourcePage::Browse(page) => page,
            SourcePage::Stub(_) => panic!("a browse page"),
        }
    }

    fn a_source_the_directory_does_not_know_gets_the_missing_source_page() {
        let rig = rig();
        let page = source_page(&rig.env, 99, BrowseQuery::Popular, false);
        assert!(matches!(page, SourcePage::Stub(_)), "an unknown id is a stub page");
        assert!(rig.asked.lock().expect("lock").is_empty(), "and never loads");
        assert_eq!(rig.env.prefs.sources.last_used_manga_source.get(), -1);
    }

    fn a_page_opened_on_a_search_starts_with_that_search_and_shows_the_text() {
        let rig = rig();
        let page = source_page(&rig.env, 1, BrowseQuery::Search("one piece".into()), false);
        let shown = browse(&page);
        assert!(pump_until(|| !rig.asked.lock().expect("lock").is_empty()));
        assert_eq!(rig.asked.lock().expect("lock")[0], BrowseQuery::Search("one piece".into()), "the first load is the search, not Popular");
        assert_eq!(shown.search.text().as_str(), "one piece");
        assert!(!shown.popular.is_active(), "no chip claims a search");
        assert_eq!(rig.env.prefs.sources.last_used_manga_source.get(), 1, "opening the source stores it");
    }

    fn the_hide_in_library_preference_is_read_when_the_page_is_made() {
        let rig = rig();
        rig.env.prefs.sources.hide_in_manga_library_items.set(true).expect("set");
        let hidden = source_page(&rig.env, 1, BrowseQuery::Popular, false);
        assert!(pump_until(|| browse(&hidden).listing.loaded_count() == 1), "the favourite is left out of the listing");
        rig.env.prefs.sources.hide_in_manga_library_items.set(false).expect("set");
        let shown = source_page(&rig.env, 1, BrowseQuery::Popular, false);
        assert!(pump_until(|| browse(&shown).listing.loaded_count() == 2), "with the flag off both are shown");
    }

    fn a_genre_search_reaches_the_open_page_and_restarts_its_listing_at_once() {
        let rig = rig();
        let page = source_page(&rig.env, 1, BrowseQuery::Popular, false);
        let shown = browse(&page);
        assert!(pump_until(|| !rig.asked.lock().expect("lock").is_empty()));
        assert!(shown.popular.is_active());
        assert!(search_open("Drama"), "the open page takes the search");
        assert_eq!(shown.listing.query(), BrowseQuery::Search("Drama".into()), "no group holds the genre: its name is the text");
        assert!(!shown.popular.is_active(), "Popular no longer applies");
        assert_eq!(shown.search.text().as_str(), "Drama");
        assert!(pump_until(|| rig.asked.lock().expect("lock").contains(&BrowseQuery::Search("Drama".into()))), "the listing loads the search");
        drop(page);
    }

    fn open_in_web_view_goes_to_the_source_site() {
        let rig = rig();
        rig.failing.store(true, std::sync::atomic::Ordering::SeqCst);
        let page = source_page(&rig.env, 1, BrowseQuery::Popular, false);
        let wanted = EmptyAction::OpenInWebView.label();
        let find = || {
            let mut buttons = Vec::new();
            fn walk(widget: &gtk::Widget, out: &mut Vec<gtk::Button>) {
                if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                    out.push(button.clone());
                }
                let mut child = widget.first_child();
                while let Some(next) = child {
                    walk(&next, out);
                    child = next.next_sibling();
                }
            }
            walk(browse(&page).listing.widget().upcast_ref(), &mut buttons);
            buttons.into_iter().find(|b| b.tooltip_text().as_deref() == Some(wanted.as_str()))
        };
        assert!(pump_until(|| find().is_some()), "a failed first page shows the error actions");
        find().expect("button").emit_clicked();
        assert_eq!(*rig.log.borrow(), ["webview https://alpha.example Alpha"], "the source's site opens in the web view");
    }

    pub(crate) fn smoke() {
        a_source_the_directory_does_not_know_gets_the_missing_source_page();
        a_page_opened_on_a_search_starts_with_that_search_and_shows_the_text();
        the_hide_in_library_preference_is_read_when_the_page_is_made();
        a_genre_search_reaches_the_open_page_and_restarts_its_listing_at_once();
        open_in_web_view_goes_to_the_source_site();
    }

    #[test]
    fn the_previous_page_is_a_browse_page_only_when_the_one_below_the_shown_page_is() {
        let stack = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
        let names = stack(&["root", "screen:browse_source", "screen:manga_entry_details"]);
        assert!(previous_is_browse(&names, 2));
        assert!(!previous_is_browse(&names, 1), "a browse page on show has the root below it");
        assert!(!previous_is_browse(&names, 0), "nothing is below the root");
        let other = stack(&["root", "screen:library", "screen:manga_entry_details"]);
        assert!(!previous_is_browse(&other, 2));
    }

    #[test]
    fn a_genre_search_with_no_open_page_reports_it() {
        OPEN.with(|slot| *slot.borrow_mut() = None);
        assert!(!search_open("Drama"), "the caller falls back to global search");
    }
}
