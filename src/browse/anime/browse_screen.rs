//! The anime browse screen (T-41b-014, T-41b-024, T-41b-025, T-41b-026): one anime source's listing, built over the anime
//! directory and mounted on the `browse_source` screen when an anime source row, a global search row or an entry page
//! parked an `AnimeBrowseSource` route. The page itself is `anime_browse_source`; this builds its dependencies from the
//! app, as `browse_screen` does for manga. A source that is listed but has no runnable instance gets the missing-source
//! page, the way the manga screen treats a source its directory cannot load.

use std::rc::Rc;
use std::sync::Arc;

use kuro_domain::source::browse::BrowseQuery;
use kuro_source::filter_list::FilterList;
use tokio::sync::watch;

use super::browse_view::{AnimeBrowseDeps, anime_browse_source};
use super::directory::AnimeDirectory;
use super::prefs::AnimeBrowsePrefs;
use crate::browse::browse_screen::{NoLoader, register_page};
use crate::browse::directory::{SourceDirectory, source_or_stub};
use crate::browse::entries::FavoriteStore;
use crate::browse::filters::SourceFilters;
use crate::browse::listing_view::ListingDeps;
use crate::browse::pages::{FavoritesDeps, SourcePage};
use crate::browse::primitives::{EmptyAction, ToastSpec};
use crate::browse::sources::LOCAL_SOURCE_ID;
use crate::browse::view::file_cover_loader;

/// What an anime source page needs of the app.
pub struct AppServices {
    pub prefs: Rc<AnimeBrowsePrefs>,
    pub directory: Arc<AnimeDirectory>,
    pub store: Arc<dyn FavoriteStore>,
    pub open_entry: Rc<dyn Fn(i64)>,
    /// The duplicate dialog's Migrate existing entry.
    pub migrate: Rc<dyn Fn(i64, i64)>,
    pub edit_categories: Rc<dyn Fn()>,
    pub on_back: Rc<dyn Fn()>,
    /// Open in WebView: the url and the source's name.
    pub open_webview: Rc<dyn Fn(&str, &str)>,
    /// The Settings item of a configurable source.
    pub on_settings: Rc<dyn Fn(i64)>,
    pub toast: Rc<dyn Fn(ToastSpec)>,
    pub now_ms: Rc<dyn Fn() -> i64>,
}

/// The page of `source_id` opened on `query`.
pub fn source_page(env: &AppServices, source_id: i64, query: BrowseQuery) -> SourcePage {
    let listed = env.directory.sources().into_iter().find(|entry| entry.source.id == source_id && !entry.source.is_stub);
    let instance = env.directory.instance(source_id);
    let loader = instance.as_ref().and_then(|_| env.directory.loader(source_id));
    let stub = loader.is_none();
    let source = listed.as_ref().map_or_else(|| source_or_stub(Some(&*env.directory), source_id), |entry| entry.source.clone());
    let package = listed.as_ref().map(|entry| entry.package.clone()).filter(|package| !package.is_empty());
    let home_url = listed.as_ref().and_then(|entry| entry.home_url.clone());
    let defaults = if stub { FilterList::default() } else { env.directory.filters(source_id).unwrap_or_default() };
    let filters = Rc::new(SourceFilters::new(defaults));
    let (_never_sent, ready) = watch::channel(true);

    let open_web: Rc<dyn Fn()> = {
        let (open, name, url) = (Rc::clone(&env.open_webview), source.name.clone(), home_url.clone());
        Rc::new(move || {
            if let Some(url) = &url {
                open(url, &name);
            }
        })
    };
    let settings = {
        let open = Rc::clone(&env.on_settings);
        Rc::new(move || open(source_id))
    };
    let page = anime_browse_source(AnimeBrowseDeps {
        supports_latest: instance.as_ref().map_or(source.supports_latest, |instance| instance.supports_latest()),
        configurable: env.directory.preferences(source_id).is_some(),
        prefs: Rc::clone(&env.prefs),
        package,
        home_url,
        listing: ListingDeps {
            loader: loader.unwrap_or_else(|| Arc::new(NoLoader)),
            prefs: env.prefs.browse_prefs(),
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
                    open_web();
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
            anime: true,
        }),
        on_back: Rc::clone(&env.on_back),
        open_webview: Rc::clone(&env.open_webview),
        on_settings: settings,
        parent: None,
        initial_query: query,
    });
    // a genre or author search from an anime entry page runs on this page when it is the one below
    if let SourcePage::Browse(shown) = &page {
        register_page(shown, filters);
    }
    page
}

/// The app side of an anime source page, over the anime library.
fn production_env() -> AppServices {
    use kuro_data::anime::AnimeRepo;
    use kuro_data::category::{CategoryKind, CategoryRepo};
    use kuro_domain::kind::EntryKind;

    use crate::browse::anime::favorite_store::AnimeStore;
    use crate::entry_core::handoff::{self, MigrateRequest, SourcePrefsRequest, WebViewRequest};

    let app = crate::data::app();
    let db = app.db(EntryKind::Anime);
    let prefs = Rc::new(AnimeBrowsePrefs::new(app.base.store().clone()));
    let anime = AnimeRepo::new(Arc::clone(&db));
    let store = AnimeStore::new(
        anime.clone(),
        CategoryRepo::new(db, CategoryKind::Anime),
        prefs.library.default_anime_category.clone(),
        // the cover and background caches belong to the image layer, which keeps no per-entry files here yet
        Arc::new(|_| 0),
        Arc::new(|_| 0),
    );
    AppServices {
        prefs,
        directory: Arc::new(AnimeDirectory::current(anime.clone())),
        store: Arc::new(store),
        open_entry: Rc::new(crate::entry_core::anime::route::open),
        migrate: Rc::new(move |id, _browsed| {
            let title = anime.by_id(id).map(|row| row.title).unwrap_or_default();
            handoff::open_migrate(MigrateRequest { entry_id: id, anime: true, title });
        }),
        edit_categories: Rc::new(crate::browse::glue::open_anime_categories),
        on_back: Rc::new(crate::ui::pop),
        open_webview: Rc::new(|url, title| handoff::open_webview(WebViewRequest { url: url.to_owned(), title: title.to_owned() })),
        on_settings: Rc::new(|source_id| handoff::open_source_preferences(SourcePrefsRequest { source_id, anime: true })),
        toast: Rc::new(|spec| crate::toast::notify(spec.message)),
        now_ms: Rc::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))),
    }
}

/// The page of the parked anime route, tagged as the `browse_source` screen. `None` before the app data is open.
pub fn page(source_id: i64, query: BrowseQuery) -> Option<gtk::Box> {
    if !crate::data::is_installed() {
        return None;
    }
    let root = source_page(&production_env(), source_id, query).widget().clone();
    crate::ui::tag(&root, "browse_source");
    Some(root)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use gtk::glib;
    use kuro_prefs::store::PrefStore;
    use kuro_source::anime::AnimeSource;

    use super::*;
    use crate::browse::anime::directory::{AnimeInstances, sources_of};
    use crate::browse::anime::fixtures::{FakeSource, anime, lookup, page};
    use crate::browse::entries::fake::FakeStore;
    use crate::browse::route::BrowseRoute;
    use kuro_extensions::anime::abi::AbiVersion;
    use kuro_extensions::anime::model::{AnimeSourceRef, Buckets, Installed, SignerDigest};

    struct Held(Arc<FakeSource>);

    impl AnimeInstances for Held {
        fn source(&self, id: i64) -> Option<Arc<dyn AnimeSource>> {
            (id == self.0.id()).then(|| Arc::clone(&self.0) as Arc<dyn AnimeSource>)
        }
    }

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

    fn installed(pkg: &str, sources: Vec<AnimeSourceRef>) -> Installed {
        Installed {
            name: pkg.into(),
            pkg: pkg.into(),
            version_name: "1".into(),
            version_code: 1,
            abi: AbiVersion { major: 1, minor: 0 },
            lang: "en".into(),
            nsfw: false,
            torrent: false,
            sources,
            icon: None,
            signer: SignerDigest(String::new()),
            has_update: false,
            is_obsolete: false,
            store: None,
        }
    }

    struct Rig {
        env: AppServices,
        source: Arc<FakeSource>,
        log: Rc<RefCell<Vec<String>>>,
        _entered: tokio::runtime::EnterGuard<'static>,
    }

    /// Sources 42 (held, so runnable) and 43 (listed, no instance) of the package `eu.example.alpha`.
    fn rig(setup: impl Fn(&AnimeBrowsePrefs)) -> Rig {
        let rt: &'static tokio::runtime::Runtime =
            Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime")));
        let entered = rt.enter();
        let prefs = Rc::new(AnimeBrowsePrefs::new(PrefStore::in_memory()));
        setup(&prefs);
        let source = FakeSource::new(42, vec![Ok(page(vec![anime("/a", "A")], false)), Ok(page(vec![anime("/b", "B")], false))]);
        let listed = |id: i64| AnimeSourceRef { id, name: format!("Src{id}"), lang: "en".into(), base_url: "https://src.example".into() };
        let buckets = Buckets { installed: vec![installed("eu.example.alpha", vec![listed(42), listed(43)])], ..Buckets::default() };
        let (lookup, _) = lookup(&[]);
        let directory = AnimeDirectory::new(sources_of(&buckets), Some(Arc::new(Held(Arc::clone(&source)))), Arc::new(move |_| Arc::clone(&lookup)));
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let (opened, web, back, settings) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let env = AppServices {
            prefs,
            directory: Arc::new(directory),
            store: Arc::new(FakeStore::default()),
            open_entry: Rc::new(move |id| opened.borrow_mut().push(format!("entry {id}"))),
            migrate: Rc::new(|_, _| {}),
            edit_categories: Rc::new(|| {}),
            on_back: Rc::new(move || back.borrow_mut().push("back".into())),
            open_webview: Rc::new(move |url, name| web.borrow_mut().push(format!("webview {name} {url}"))),
            on_settings: Rc::new(move |id| settings.borrow_mut().push(format!("settings {id}"))),
            toast: Rc::new(|_| {}),
            now_ms: Rc::new(|| 1),
        };
        Rig { env, source, log, _entered: entered }
    }

    fn a_listed_source_with_an_instance_opens_on_the_route_query_and_stores_its_package_as_last_used() {
        let rig = rig(|_| {});
        let page = source_page(&rig.env, 42, BrowseQuery::Latest);
        let SourcePage::Browse(shown) = &page else { panic!("a browse page") };
        assert!(pump_until(|| rig.source.calls() == ["latest 1"]), "the route's query is the first load: {:?}", rig.source.calls());
        assert_eq!(rig.env.prefs.sources.last_used_anime_source.get(), 42, "T-41b-025: opening stores the source");
        assert!(shown.latest.is_some(), "the source states it supports latest");
        assert!(pump_until(|| shown.listing.loaded_count() == 1));
    }

    fn the_per_extension_incognito_setting_blocks_the_last_used_write_of_its_sources() {
        let rig = rig(|prefs| prefs.sources.incognito_anime_extensions.set(["eu.example.alpha".to_owned()].into()).expect("set"));
        let _page = source_page(&rig.env, 42, BrowseQuery::Popular);
        assert_eq!(rig.env.prefs.sources.last_used_anime_source.get(), -1, "the package came from the directory");
    }

    fn a_listed_source_with_no_instance_and_an_unknown_one_get_the_missing_source_page() {
        let rig = rig(|_| {});
        assert!(matches!(source_page(&rig.env, 43, BrowseQuery::Popular), SourcePage::Stub(_)), "listed, but nothing to ask");
        assert!(matches!(source_page(&rig.env, 99, BrowseQuery::Popular), SourcePage::Stub(_)));
        assert_eq!(rig.env.prefs.sources.last_used_anime_source.get(), 99, "a stub is stored too");
        assert!(rig.source.calls().is_empty(), "neither loads");
    }

    fn the_webview_overflow_goes_to_the_sources_site() {
        let rig = rig(|_| {});
        let page = source_page(&rig.env, 42, BrowseQuery::Popular);
        let SourcePage::Browse(shown) = &page else { panic!("a browse page") };
        assert!(pump_until(|| rig.source.calls() == ["popular 1"]));
        let popover = shown.header.overflow.popover().expect("overflow popover");
        fn buttons(widget: &gtk::Widget, out: &mut Vec<gtk::Button>) {
            use gtk::prelude::*;
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                out.push(button.clone());
            }
            let mut child = widget.first_child();
            while let Some(next) = child {
                buttons(&next, out);
                child = next.next_sibling();
            }
        }
        use gtk::prelude::*;
        let mut found = Vec::new();
        buttons(popover.child().expect("child").upcast_ref(), &mut found);
        let label = kuro_i18n::tr(kuro_i18n::MsgKey::action_open_in_web_view);
        found.iter().find(|b| b.label().as_deref() == Some(label.as_str())).expect("item").emit_clicked();
        assert_eq!(*rig.log.borrow(), ["webview Src42 https://src.example"]);
    }

    fn the_anime_route_shares_the_browse_source_screen_and_is_taken_for_it() {
        crate::browse::pending::park(BrowseRoute::AnimeBrowseSource { source_id: 42, query: BrowseQuery::Popular });
        assert_eq!(crate::browse::pending::take_for("browse_source"), Some(BrowseRoute::AnimeBrowseSource { source_id: 42, query: BrowseQuery::Popular }));
    }

    pub(crate) fn smoke() {
        a_listed_source_with_an_instance_opens_on_the_route_query_and_stores_its_package_as_last_used();
        the_per_extension_incognito_setting_blocks_the_last_used_write_of_its_sources();
        a_listed_source_with_no_instance_and_an_unknown_one_get_the_missing_source_page();
        the_webview_overflow_goes_to_the_sources_site();
        the_anime_route_shares_the_browse_source_screen_and_is_taken_for_it();
    }
}
