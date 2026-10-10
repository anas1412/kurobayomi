//! The anime browse page (T-41b-010, T-41b-015, T-41b-016, T-41b-017, T-41b-023, T-41b-025). A thin wrapper over the
//! shared `pages::browse_source`: the chips, toolbar, display-mode menu, inline error with Retry and the grids are that
//! page's, fed the anime preference keys (`AnimeBrowsePrefs::browse_prefs`). What this adds is what is anime's: the
//! last-used write for any source with the per-extension incognito check, and the routing of the overflow items to the
//! WebView page, the settings page and an external link (`Link`).

use std::rc::Rc;

use crate::browse::anime::browse::{Link, open_external, help_link, webview_link};
use crate::browse::anime::prefs::AnimeBrowsePrefs;
use crate::browse::filters::SourceFilters;
use crate::browse::header::OverflowItem;
use crate::browse::listing_view::ListingDeps;
use crate::browse::pages::{BrowseSourceDeps, FavoritesDeps, SourcePage, browse_source};

/// What the anime browse page needs. The preferences, the incognito state, the overflow routing and the url opener of
/// the shared page are filled in here.
pub struct AnimeBrowseDeps {
    pub supports_latest: bool,
    pub configurable: bool,
    /// The anime preference scope; it supplies the columns, the display mode and the last-used write.
    pub prefs: Rc<AnimeBrowsePrefs>,
    /// The source's extension package, or `None` for the local source and for a stub (`GetAnimeIncognitoState`).
    pub package: Option<String>,
    /// The source's home page, for Open in WebView.
    pub home_url: Option<String>,
    /// `source_id`, name, language and stub flag are read from here; `prefs` is replaced by the anime browse prefs.
    pub listing: ListingDeps,
    pub filters: Rc<SourceFilters>,
    pub favorites: Option<FavoritesDeps>,
    pub on_back: Rc<dyn Fn()>,
    /// Open in WebView: the source's home url and its name.
    pub open_webview: Rc<dyn Fn(&str, &str)>,
    /// The Settings item of a configurable source.
    pub on_settings: Rc<dyn Fn()>,
    /// The window an external link launches from, when there is one.
    pub parent: Option<gtk::Window>,
}

/// What an overflow item does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverflowEffect {
    Open(Link),
    Settings,
    /// Nothing to open: the home url is missing, or there is no help page.
    Nothing,
}

/// The effect of an overflow item for a source with `home_url` and `source_name`.
pub fn overflow_effect(item: OverflowItem, home_url: Option<&str>, source_name: &str) -> OverflowEffect {
    let link = match item {
        OverflowItem::OpenInWebView => webview_link(home_url, source_name),
        OverflowItem::Help => help_link(),
        OverflowItem::Settings => return OverflowEffect::Settings,
    };
    link.map_or(OverflowEffect::Nothing, OverflowEffect::Open)
}

/// Opens a link: an external page through `gtk::UriLauncher`, the WebView page through `open_webview`.
pub fn open_link(link: &Link, open_webview: &dyn Fn(&str, &str), parent: Option<&gtk::Window>) {
    match link {
        Link::External(url) => open_external(url, parent),
        Link::WebView { url, source_name } => open_webview(url, source_name),
    }
}

/// Builds the anime browse page. Opening it stores the source as the last-used anime source once, for an installed or a
/// stub source, unless incognito applies (the global switch, or the source's extension in `incognito_anime_extensions`).
pub fn anime_browse_source(deps: AnimeBrowseDeps) -> SourcePage {
    let AnimeBrowseDeps { supports_latest, configurable, prefs, package, home_url, mut listing, filters, favorites, on_back, open_webview, on_settings, parent } = deps;
    if let Err(error) = prefs.record_last_used(listing.source_id, package.as_deref()) {
        tracing::warn!(%error, "could not store the last used anime source");
    }
    let hide_in_library = prefs.hide_in_library();
    let browse_prefs = prefs.browse_prefs();
    listing.prefs = Rc::clone(&browse_prefs);
    let source_name = listing.source_name.clone();
    let on_overflow: Rc<dyn Fn(OverflowItem)> = {
        let (open_webview, parent) = (Rc::clone(&open_webview), parent.clone());
        Rc::new(move |item| match overflow_effect(item, home_url.as_deref(), &source_name) {
            OverflowEffect::Open(link) => open_link(&link, &*open_webview, parent.as_ref()),
            OverflowEffect::Settings => on_settings(),
            OverflowEffect::Nothing => {}
        })
    };
    browse_source(BrowseSourceDeps {
        supports_latest,
        configurable,
        // the manga write inside `browse_source` is switched off: the anime write above replaced it
        incognito: true,
        prefs: browse_prefs,
        listing,
        filters,
        favorites,
        hide_in_library,
        initial_query: kuro_domain::source::browse::BrowseQuery::Popular,
        on_back,
        on_overflow,
        open_url: Rc::new(move |url| open_external(url, parent.as_ref())),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::Arc;

    use gtk::prelude::*;
    use kuro_domain::source::browse::BrowseQuery;
    use kuro_prefs::store::PrefStore;
    use kuro_source::filter::Filter;
    use kuro_source::filter_list::FilterList;
    use tokio::sync::watch;

    use crate::browse::anime::fixtures::{FakeSource, anime, lookup, page};
    use crate::browse::anime::loader::AnimeLoader;

    #[test]
    fn open_in_webview_carries_the_home_url_and_name_and_needs_a_home_url() {
        assert_eq!(
            overflow_effect(OverflowItem::OpenInWebView, Some("https://a.example/"), "Alpha"),
            OverflowEffect::Open(Link::WebView { url: "https://a.example/".into(), source_name: "Alpha".into() })
        );
        assert_eq!(overflow_effect(OverflowItem::OpenInWebView, None, "Alpha"), OverflowEffect::Nothing);
        assert_eq!(overflow_effect(OverflowItem::OpenInWebView, Some(""), "Alpha"), OverflowEffect::Nothing);
    }

    #[test]
    fn settings_goes_to_the_settings_page_and_help_follows_the_project_help_page() {
        assert_eq!(overflow_effect(OverflowItem::Settings, None, "Alpha"), OverflowEffect::Settings);
        let help = overflow_effect(OverflowItem::Help, None, "Local");
        assert_eq!(help, help_link().map_or(OverflowEffect::Nothing, OverflowEffect::Open));
    }

    #[test]
    fn a_webview_link_goes_to_the_webview_page_and_not_to_the_browser() {
        let calls = RefCell::new(Vec::new());
        open_link(&Link::WebView { url: "https://a.example/".into(), source_name: "Alpha".into() }, &|url, name| calls.borrow_mut().push(format!("{name} {url}")), None);
        assert_eq!(*calls.borrow(), ["Alpha https://a.example/"]);
    }

    struct Rig {
        page: SourcePage,
        prefs: Rc<AnimeBrowsePrefs>,
        log: Rc<RefCell<Vec<String>>>,
        source: Arc<FakeSource>,
        _runtime: tokio::runtime::EnterGuard<'static>,
    }

    fn rig(stub: bool, package: Option<&str>, setup: impl Fn(&AnimeBrowsePrefs)) -> Rig {
        let rt: &'static tokio::runtime::Runtime =
            Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime")));
        let entered = rt.enter();
        let prefs = Rc::new(AnimeBrowsePrefs::new(PrefStore::in_memory()));
        setup(&prefs);
        let source = FakeSource::new(42, vec![Ok(page(vec![anime("/a", "A")], false)), Ok(page(vec![anime("/b", "B")], false))]);
        let (lookup, _) = lookup(&[]);
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let (back, webview, settings, action) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let (tx, ready) = watch::channel(true);
        std::mem::forget(tx);
        let page = anime_browse_source(AnimeBrowseDeps {
            supports_latest: true,
            configurable: true,
            prefs: Rc::clone(&prefs),
            package: package.map(str::to_owned),
            home_url: Some("https://a.example/".into()),
            listing: ListingDeps {
                loader: Arc::new(AnimeLoader::new(source.clone(), lookup)),
                prefs: prefs.browse_prefs(),
                ready,
                stub,
                is_local: false,
                source_id: 42,
                source_name: "Alpha".into(),
                source_lang: "en".into(),
                load_cover: Rc::new(|_, _| {}),
                open_entry: Rc::new(|_| {}),
                on_long_press: Rc::new(|_| {}),
                favorite_urls: None,
                on_action: Rc::new(move |a| action.borrow_mut().push(format!("action {a:?}"))),
            },
            filters: Rc::new(SourceFilters::new(FilterList(vec![Filter::check_box("Completed")]))),
            favorites: None,
            on_back: Rc::new(move || back.borrow_mut().push("back".into())),
            open_webview: Rc::new(move |url, name| webview.borrow_mut().push(format!("webview {name} {url}"))),
            on_settings: Rc::new(move || settings.borrow_mut().push("settings".into())),
            parent: None,
        });
        Rig { page, prefs, log, source, _runtime: entered }
    }

    fn pump_until(mut done: impl FnMut() -> bool) -> bool {
        let context = gtk::glib::MainContext::default();
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

    fn opening_an_installed_source_stores_it_and_the_incognito_extension_blocks_it() {
        let installed = rig(false, Some("eu.example.alpha"), |_| {});
        assert_eq!(installed.prefs.sources.last_used_anime_source.get(), 42);
        let blocked = rig(false, Some("eu.example.alpha"), |p| {
            p.sources.incognito_anime_extensions.set(["eu.example.alpha".to_owned()].into()).expect("set");
        });
        assert_eq!(blocked.prefs.sources.last_used_anime_source.get(), -1, "the source's extension is incognito");
        let global = rig(false, None, |p| p.base.incognito_mode.set(true).expect("set"));
        assert_eq!(global.prefs.sources.last_used_anime_source.get(), -1, "the global switch");
        let SourcePage::Browse(page) = &installed.page else { panic!("browse page") };
        page.popular.set_active(true);
        page.latest.as_ref().expect("latest").set_active(true);
        assert_eq!(installed.prefs.sources.last_used_anime_source.get(), 42);
    }

    fn opening_a_stub_stores_it_too_and_shows_the_missing_source_page() {
        let stub = rig(true, None, |_| {});
        assert_eq!(stub.prefs.sources.last_used_anime_source.get(), 42, "a stub is stored as the last used source");
        assert!(matches!(stub.page, SourcePage::Stub(_)));
        assert!(stub.source.calls().is_empty(), "and never loads");
        let gated = rig(true, None, |p| p.base.incognito_mode.set(true).expect("set"));
        assert_eq!(gated.prefs.sources.last_used_anime_source.get(), -1);
    }

    fn the_page_lists_the_source_and_a_chip_press_switches_the_listing() {
        let rig = rig(false, None, |_| {});
        let SourcePage::Browse(page) = &rig.page else { panic!("browse page") };
        assert!(pump_until(|| rig.source.calls() == ["popular 1"]), "{:?}", rig.source.calls());
        page.latest.as_ref().expect("latest").set_active(true);
        assert!(pump_until(|| rig.source.calls() == ["popular 1", "latest 1"]), "{:?}", rig.source.calls());
        assert_eq!(page.listing.query(), BrowseQuery::Latest);
        assert!(page.filter.is_some(), "the source has filters, so the Filter chip is shown");
    }

    fn the_overflow_items_reach_the_webview_page_and_settings() {
        let rig = rig(false, None, |_| {});
        let SourcePage::Browse(page) = &rig.page else { panic!("browse page") };
        let popover = page.header.overflow.popover().expect("overflow popover");
        let mut found = Vec::new();
        fn go(w: &gtk::Widget, out: &mut Vec<gtk::Button>) {
            if let Some(button) = w.downcast_ref::<gtk::Button>() {
                out.push(button.clone());
            }
            let mut child = w.first_child();
            while let Some(c) = child {
                go(&c, out);
                child = c.next_sibling();
            }
        }
        go(popover.child().expect("child").upcast_ref(), &mut found);
        let by = |label: String| found.iter().find(|b| b.label().as_deref() == Some(label.as_str())).cloned().expect("item");
        by(kuro_i18n::tr(kuro_i18n::MsgKey::action_open_in_web_view)).emit_clicked();
        by(kuro_i18n::tr(kuro_i18n::MsgKey::label_settings)).emit_clicked();
        assert_eq!(*rig.log.borrow(), ["webview Alpha https://a.example/", "settings"]);
    }

    pub(crate) fn smoke() {
        opening_an_installed_source_stores_it_and_the_incognito_extension_blocks_it();
        opening_a_stub_stores_it_too_and_shows_the_missing_source_page();
        the_page_lists_the_source_and_a_chip_press_switches_the_listing();
        the_overflow_items_reach_the_webview_page_and_settings();
    }
}
