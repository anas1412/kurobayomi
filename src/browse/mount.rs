//! Mounts the Browse tab (T-41a-011, T-40a-034): the six pages behind the shell, the header actions, the update badge on
//! the extension tabs, and the pages the tab opens (the language filters, the extension details, global search and the
//! extension stores). The screen registry calls [`browse`]; it returns `None` before the app data is open, so the sample
//! page of `entry` stays until then.
//!
//! The app has no source registry yet, so the Sources pages list nothing and say so; the extension pages show what the
//! extension host last published (`ext_host`). Both fill in without a change here when those arrive.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use kuro_domain::source::model::Source;
use kuro_domain::source::toggles::{flip_disabled, toggle_pin};
use kuro_extensions::InstallStep;
use kuro_extensions::anime::model::Buckets;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::pref::Pref;
use kuro_prefs::source::SourcePrefs;
use tokio::sync::watch;

use super::anime::sources::{ListPrefs, TabRoute};
use super::anime::sources_view::{AnimeSourcesNav, AnimeSourcesPage};
use super::directory::SourceDirectory;
use super::ext_details::{DetailSource, ExtDetails, label_as_name};
use super::ext_details_page::{DetailsDeps, ExtDetailsPage};
use super::extensions::{Ext, ExtSource, ExtState};
use super::extensions_page::{ExtNav, ExtensionBackend, JobFuture, RefreshFuture, Reporter};
use super::glue;
use super::lang_pages::{ExtensionLanguagesPage, SourcesFilterPage};
use super::route::{BrowseKind, BrowsePage, BrowseRoute};
use super::shell::{BrowseShell, ShellDeps};
use super::sources_page::{SourcesNav, SourcesSnapshot};
use crate::screens::Built;

thread_local! {
    /// The mounted tab, for the sidebar's reselect and for the deep links to the extension tabs.
    static SHELL: RefCell<Option<Rc<BrowseShell>>> = const { RefCell::new(None) };
}

/// Re-selecting the Browse tab in the sidebar opens global search for the visible kind.
pub fn reselect() {
    if let Some(shell) = SHELL.with(|s| s.borrow().clone()) {
        shell.reselect();
    }
}

/// `show_extension()` and `show_anime_extension()`: the extension tab of a kind.
pub fn show_extension(kind: BrowseKind) {
    if let Some(shell) = SHELL.with(|s| s.borrow().clone()) {
        match kind {
            BrowseKind::Manga => shell.show_extension(),
            BrowseKind::Anime => shell.show_anime_extension(),
        }
    }
}

/// The extension rows the anime host published, as the shared list shows them (T-19a-066): the updates and installed
/// packages, the untrusted ones, and the listed packages split per source. A listed package carries its icon url, which
/// the row fetches through the image cache.
pub fn anime_exts(buckets: &Buckets) -> Vec<Ext> {
    let source = |s: &kuro_extensions::anime::model::AnimeSourceRef| ExtSource {
        id: s.id,
        name: s.name.clone(),
        lang: s.lang.clone(),
        base_url: Some(s.base_url.clone()).filter(|u| !u.is_empty()),
    };
    let installed = buckets.updates.iter().chain(&buckets.installed).map(|e| Ext {
        name: e.name.clone(),
        pkg: e.pkg.clone(),
        version: e.version_name.clone(),
        lang: Some(e.lang.clone()),
        nsfw: e.nsfw,
        sources: e.sources.iter().map(source).collect(),
        state: ExtState::Installed { has_update: e.has_update, obsolete: e.is_obsolete },
        icon_file: e.icon.as_ref().map(|p| p.to_string_lossy().into_owned()),
    });
    let untrusted = buckets.untrusted.iter().map(|e| Ext {
        name: e.name.clone(),
        pkg: e.pkg.clone(),
        version: e.version_name.clone(),
        lang: None,
        nsfw: e.nsfw,
        sources: Vec::new(),
        state: ExtState::Untrusted,
        icon_file: None,
    });
    // a listed package is one row of its own per source, so the page's own split sees it whole
    let mut seen = BTreeSet::new();
    let available = buckets.available.iter().filter(|row| seen.insert(row.ext.pkg.clone())).map(|row| Ext {
        name: row.ext.name.clone(),
        pkg: row.ext.pkg.clone(),
        version: row.ext.version_name.clone(),
        lang: Some(row.ext.lang.clone()),
        nsfw: row.ext.nsfw,
        sources: row.ext.sources.iter().map(source).collect(),
        state: ExtState::Available,
        icon_file: Some(row.ext.icon_url.clone()).filter(|u| !u.is_empty()),
    });
    installed.chain(untrusted).chain(available).collect()
}

/// What the details page shows about an installed package that the shared row does not carry: the store it came from, for
/// the repository link and the debug text, and which of its sources have a settings screen of their own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DetailFacts {
    pub repo_url: Option<String>,
    pub configurable: BTreeSet<i64>,
}

/// The details of an installed extension from the rows the page shows. `lib_version` is not part of the shared row, so the
/// debug text says `n/a` for it rather than invent a value. A source is labelled by its name when its extension has
/// several sources that do not all share one name (`multiSource && !multiLangSingleSource`, T-41a-019).
pub fn details_of(ext: &Ext, facts: &DetailFacts) -> Option<ExtDetails> {
    let ExtState::Installed { has_update, obsolete } = ext.state else { return None };
    let names: Vec<&str> = ext.sources.iter().map(|s| s.name.as_str()).collect();
    let label_as_name = label_as_name(&names);
    Some(ExtDetails {
        name: ext.name.clone(),
        pkg: ext.pkg.clone(),
        lang: ext.lang.clone().unwrap_or_default(),
        version_name: ext.version.clone(),
        lib_version: None,
        version_code: ext.version.parse().unwrap_or(0),
        nsfw: ext.nsfw,
        has_update,
        obsolete,
        shared: false,
        repo_url: facts.repo_url.clone(),
        icon_file: ext.icon_file.clone(),
        sources: ext
            .sources
            .iter()
            .map(|s| DetailSource {
                id: s.id,
                name: s.name.clone(),
                lang: s.lang.clone(),
                label_as_name,
                configurable: facts.configurable.contains(&s.id),
                base_url: s.base_url.clone(),
            })
            .collect(),
    })
}

/// The index url of the store an installed anime package came from (`extension.store.indexUrl`), `None` for a package that
/// has no store or is not installed. A manga package keeps no store, so its details have no repository link.
pub fn anime_repo_url(buckets: &Buckets, pkg: &str) -> Option<String> {
    buckets.updates.iter().chain(&buckets.installed).find(|e| e.pkg == pkg).and_then(|e| e.store.as_ref()).map(|store| store.index_url.clone())
}

/// The ids among `ids` whose source declares settings rows of its own.
pub fn configurable_ids(directory: Option<&dyn SourceDirectory>, ids: impl IntoIterator<Item = i64>) -> BTreeSet<i64> {
    ids.into_iter().filter(|id| directory.is_some_and(|directory| directory.preferences(*id).is_some())).collect()
}

/// The directory of a kind, as the pages read it: the manga directory the app installed, or the anime sources the
/// extension host published.
fn directory_of(kind: BrowseKind) -> Option<Arc<dyn SourceDirectory>> {
    use kuro_data::anime::AnimeRepo;
    use kuro_domain::kind::EntryKind;

    match kind {
        BrowseKind::Manga => super::directory::current(),
        BrowseKind::Anime => Some(Arc::new(super::anime::directory::AnimeDirectory::current(AnimeRepo::new(crate::data::app().db(EntryKind::Anime))))),
    }
}

/// The installed sources of a kind, as the Sources filter and the Sources page list them: `anime` is what the host
/// published for the anime kind.
pub fn installed_sources(kind: BrowseKind, directory: Option<&dyn SourceDirectory>, anime: Vec<Source>) -> Vec<Source> {
    match kind {
        BrowseKind::Anime => anime,
        BrowseKind::Manga => directory.map(|directory| directory.sources().into_iter().map(|entry| entry.source).collect()).unwrap_or_default(),
    }
}

/// The facts of one installed row.
fn facts_of(kind: BrowseKind, ext: &Ext) -> DetailFacts {
    let repo_url = match kind {
        BrowseKind::Anime => anime_repo_url(&crate::ext_host::anime_lists(), &ext.pkg),
        BrowseKind::Manga => None,
    };
    let directory = directory_of(kind);
    DetailFacts { repo_url, configurable: configurable_ids(directory.as_deref(), ext.sources.iter().map(|s| s.id)) }
}

/// Whether the package is no longer an installed row: the details page of it then has nothing to show and pops (E18, R7).
pub fn is_gone(rows: &[Ext], pkg: &str) -> bool {
    !rows.iter().any(|ext| ext.pkg == pkg && matches!(ext.state, ExtState::Installed { .. }))
}

/// What the app can do with an extension from this tab today: nothing downloads here yet, so an install ends in its
/// Error step with Retry, an uninstall is left to the host, and a refresh only re-reads what the host published.
struct HostBackend;

impl ExtensionBackend for HostBackend {
    fn install(&self, _pkg: String, report: Reporter) -> JobFuture {
        Box::pin(async move { report(InstallStep::Error) })
    }
    fn uninstall(&self, _pkg: String) -> JobFuture {
        Box::pin(async {})
    }
    fn refresh(&self) -> RefreshFuture {
        Box::pin(async { Ok(()) })
    }
}

/// A preference set with one id flipped in or out, written back (the pin and disable toggles).
fn flip(pref: &Pref<BTreeSet<String>>, id: i64, toggle: fn(&mut BTreeSet<String>, i64) -> bool) {
    let mut set = pref.get();
    toggle(&mut set, id);
    if let Err(error) = pref.set(set) {
        tracing::warn!(key = pref.key(), %error, "a source toggle could not be stored");
    }
}

/// The tab badge at start-up (T-40a-046): the extension manager's rows when it has published any, and then the count
/// they give is stored as `ext_updates_count`; without them the stored count stands. A stored count below zero reads
/// as none. Returns the badge number and the value to write, if any.
pub fn startup_badge(stored: i32, published: Option<&[Ext]>) -> (usize, Option<i32>) {
    match published {
        Some(rows) => {
            let count = i32::try_from(super::extensions::updates_count(rows)).unwrap_or(i32::MAX);
            (usize::try_from(count).unwrap_or(0), (count != stored).then_some(count))
        }
        None => (usize::try_from(stored).unwrap_or(0), None),
    }
}

/// Pushes a page of the tab with a title and a Back button.
/// What the manga Sources page shows of its preferences (T-40b-015) and the sources the directory lists.
fn manga_sources_snapshot(prefs: &SourcePrefs) -> SourcesSnapshot {
    SourcesSnapshot {
        sources: installed_sources(BrowseKind::Manga, super::directory::current().as_deref(), Vec::new()),
        enabled_languages: prefs.enabled_languages.get(),
        disabled: prefs.disabled_manga_sources.get(),
        pinned: prefs.pinned_manga_sources.get(),
        last_used: prefs.last_used_manga_source.get(),
        icon_files: BTreeMap::new(),
    }
}

/// One preference's change check for [`glue::poll_pref_changes`]: true once when the value changed since the last tick.
fn watch_check<T: kuro_prefs::values::PrefType + 'static>(pref: &Pref<T>) -> Box<dyn Fn() -> bool> {
    let watch = pref.watch();
    Box::new(move || watch.try_changed().is_some())
}

/// `has_installed(pkg)` (T-40a-047, D3): whether the local extension registry holds the package as installed. It replaces
/// the package-manager check of Aniyomi; with no registry yet, nothing is installed.
pub fn has_installed(pkg: &str) -> bool {
    crate::ext_host::registry().is_some_and(|registry| registry.snapshot().installed.contains_key(pkg))
}

fn push_page(title: &str, content: &impl IsA<gtk::Widget>) {
    crate::ui::push(&crate::ui::page(title, content, Vec::new(), true));
}

/// Opens the screen of a route with its request parked for the screen that builds (`pending`).
fn open_route(route: &BrowseRoute) {
    super::pending::open(route.clone());
}

/// A Migrate tab of one kind: the library's favourite counts per source, over the installed sources (T-40b-070,
/// T-41a-036). `read` is the kind's count read, which runs off the GTK thread.
fn migrate_page(
    kind: BrowseKind,
    prefs: &Rc<SourcePrefs>,
    read: impl Fn() -> Result<Vec<(i64, i64)>, String> + Clone + Send + 'static,
    directory: Rc<dyn Fn() -> Option<Arc<dyn SourceDirectory>>>,
) -> gtk::Widget {
    use super::migrate_list::{MigrateDeps, migrate_tab};

    migrate_tab(MigrateDeps {
        kind,
        prefs: Rc::clone(prefs),
        counts: Rc::new(move |deliver| {
            let read = read.clone();
            gtk::glib::spawn_future_local(async move {
                match gtk::gio::spawn_blocking(read).await {
                    Ok(Ok(counts)) => deliver(Ok(counts)),
                    Ok(Err(error)) => deliver(Err(error)),
                    Err(_) => deliver(Err("the favourite count read was cancelled".to_owned())),
                }
            });
        }),
        toast: Rc::new(|text: String| crate::toast::notify(text)),
        directory,
        open: Rc::new(move |source_id| super::pending::open(BrowseRoute::MigrateEntries { kind, source_id })),
        copy_id: Rc::new(|id| {
            if let Some(display) = gtk::gdk::Display::default() {
                display.clipboard().set_text(&id.to_string());
                crate::toast::notify(tr(MsgKey::copied_to_clipboard_plain));
            }
        }),
        open_url: Rc::new(|url| {
            gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
                if let Err(error) = result {
                    tracing::warn!(%error, "the help link could not be opened");
                }
            });
        }),
    })
    .upcast()
}

/// The manga Migrate tab.
fn manga_migrate(prefs: &Rc<SourcePrefs>) -> gtk::Widget {
    use kuro_data::manga::MangaRepo;
    use kuro_domain::kind::EntryKind;

    let repo = MangaRepo::new(crate::data::app().db(EntryKind::Manga));
    migrate_page(
        BrowseKind::Manga,
        prefs,
        move || repo.favorite_counts_by_source().map_err(|error| error.to_string()),
        Rc::new(super::directory::current),
    )
}

/// The anime Migrate tab: the anime library's favourite counts over the anime sources the host published.
fn anime_migrate(prefs: &Rc<SourcePrefs>) -> gtk::Widget {
    use kuro_data::anime::AnimeRepo;
    use kuro_domain::kind::EntryKind;

    let repo = AnimeRepo::new(crate::data::app().db(EntryKind::Anime));
    let for_sources = repo.clone();
    migrate_page(
        BrowseKind::Anime,
        prefs,
        move || repo.favorite_counts_by_source().map_err(|error| error.to_string()),
        Rc::new(move || Some(Arc::new(super::anime::directory::AnimeDirectory::current(for_sources.clone())) as Arc<dyn SourceDirectory>)),
    )
}

/// The language rows a language page can offer: the extension host's rows, or the failure when there is no host to ask.
/// Without a started extension host the available languages cannot be fetched (T-41a-018).
pub fn language_rows(host_running: bool, rows: Vec<Ext>) -> Result<Vec<Ext>, String> {
    if host_running { Ok(rows) } else { Err("the extension host is not running".to_owned()) }
}

/// The language filter of the visible page: Show/hide sources for a sources page, the language switches for an
/// extensions page (B§4, B§8).
fn open_filter(page: BrowsePage, prefs: &Rc<SourcePrefs>, rows: &dyn Fn(BrowsePage) -> Vec<Ext>) {
    if page.is_extensions() {
        let languages = ExtensionLanguagesPage::new(prefs.enabled_languages.clone());
        match language_rows(crate::ext_host::registry().is_some(), rows(page)) {
            Ok(rows) => languages.set_exts(&rows),
            Err(error) => {
                tracing::error!(%error, "the extension languages could not be fetched");
                languages.fetch_failed();
            }
        }
        push_page(&tr(MsgKey::label_extensions), languages.widget());
    } else {
        let kind = page.kind();
        let hidden = match kind {
            BrowseKind::Anime => prefs.disabled_anime_sources.clone(),
            BrowseKind::Manga => prefs.disabled_manga_sources.clone(),
        };
        let filter = SourcesFilterPage::new(prefs.enabled_languages.clone(), hidden.clone());
        let sources_now = move || installed_sources(kind, super::directory::current().as_deref(), crate::ext_host::anime_sources().0);
        filter.set_sources(sources_now());
        // an error shows the `internal_error` toast and pops the page (E-SF-05)
        filter.set_on_error(Rc::new(|| {
            crate::toast::notify(tr(MsgKey::internal_error));
            crate::ui::pop();
        }));
        // the switches and checkboxes of an open page follow the languages and hidden sources (T-40b-015)
        let checks: Vec<Box<dyn Fn() -> bool>> = vec![watch_check(&prefs.enabled_languages), watch_check(&hidden)];
        let alive = {
            let widget = filter.widget().downgrade();
            move || widget.upgrade().is_some()
        };
        let shown = filter.clone();
        glue::poll_pref_changes(checks, alive, move || shown.set_sources(sources_now()));
        push_page(&tr(MsgKey::label_sources), filter.widget());
    }
}

/// The details page of an installed extension, over the preferences of its kind. `rows` reads the kind's extension rows
/// again: the page pops once the package is no longer installed (E18, R7).
fn open_details(kind: BrowseKind, prefs: &Rc<SourcePrefs>, ext: Option<Ext>, rows: Rc<dyn Fn() -> Vec<Ext>>) {
    let (incognito, disabled) = match kind {
        BrowseKind::Anime => (prefs.incognito_anime_extensions.clone(), prefs.disabled_anime_sources.clone()),
        BrowseKind::Manga => (prefs.incognito_manga_extensions.clone(), prefs.disabled_manga_sources.clone()),
    };
    let pkg = ext.as_ref().map(|e| e.pkg.clone());
    let remove_cookies = super::cookies::remover();
    let deps = DetailsDeps {
        incognito,
        disabled,
        // the host's uninstall; the page leaves when the package drops out of the installed rows
        uninstall: {
            let pkg = pkg.clone();
            Rc::new(move || {
                if let Some(pkg) = &pkg {
                    gtk::glib::spawn_future_local(HostBackend.uninstall(pkg.clone()));
                }
            })
        },
        open_source_prefs: Rc::new(move |source_id| {
            crate::entry_core::handoff::open_source_preferences(crate::entry_core::handoff::SourcePrefsRequest { source_id, anime: kind == BrowseKind::Anime });
        }),
        open_url: Rc::new(|url| {
            gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
                if let Err(error) = result {
                    tracing::warn!(%error, "the repository link could not be opened");
                }
            });
        }),
        clear_cookies: Rc::new(remove_cookies),
        toast: Rc::new(|text: String| crate::toast::notify(text)),
    };
    let page = ExtDetailsPage::new(deps);
    page.set_details(ext.as_ref().and_then(|ext| details_of(ext, &facts_of(kind, ext))));
    if let Some(pkg) = pkg {
        pop_when_gone(page.widget(), pkg, rows);
    }
    crate::ui::push(&crate::ui::page(&tr(MsgKey::label_extension_info), page.widget(), vec![page.actions()], true));
}

/// Pops the details page once its package is gone from the installed rows, checked each second while the page is the one on
/// show: a page under another one waits until it is shown again, so a pop never takes the wrong page.
fn pop_when_gone(page: &gtk::Box, pkg: String, rows: Rc<dyn Fn() -> Vec<Ext>>) {
    let weak = page.downgrade();
    gtk::glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        let Some(page) = weak.upgrade() else { return gtk::glib::ControlFlow::Break };
        if page.is_mapped() && is_gone(&rows(), &pkg) {
            crate::ui::pop();
            return gtk::glib::ControlFlow::Break;
        }
        gtk::glib::ControlFlow::Continue
    });
}

/// The manga Sources page over `snapshot`, which reads the directory and the prefs. It reads again each time the page is
/// shown, as the anime page does: the tab is built before the built-in and recipe sources start, so the first read
/// lists none of them. A change to the languages, hidden or pinned sources or the last used source also reaches the
/// open page (T-40b-015).
fn manga_sources_page(prefs: &Rc<SourcePrefs>, ready: watch::Receiver<bool>, snapshot: Rc<dyn Fn(&SourcePrefs) -> SourcesSnapshot>) -> super::sources_page::SourcesPage {
    let (pin, hide) = (prefs.pinned_manga_sources.clone(), prefs.disabled_manga_sources.clone());
    let nav = SourcesNav {
        open_listing: Rc::new(|source_id, query| super::pending::open(BrowseRoute::BrowseSource { source_id, query, migrating: false })),
        toggle_pin: Rc::new(move |id| flip(&pin, id, toggle_pin)),
        toggle_disabled: Rc::new(move |id| flip(&hide, id, flip_disabled)),
    };
    let page = super::pages::sources(nav, ready);
    let show = {
        let (page, prefs) = (page.clone(), Rc::clone(prefs));
        move || page.update(snapshot(&prefs))
    };
    show();
    let shown = show.clone();
    page.widget().connect_map(move |_| shown());
    let checks: Vec<Box<dyn Fn() -> bool>> = vec![
        watch_check(&prefs.enabled_languages),
        watch_check(&prefs.disabled_manga_sources),
        watch_check(&prefs.pinned_manga_sources),
        watch_check(&prefs.last_used_manga_source),
    ];
    let alive = {
        let widget = page.widget().downgrade();
        move || widget.upgrade().is_some()
    };
    glue::poll_pref_changes(checks, alive, show);
    page
}

/// Builds the tab. `None` before the app data is open.
pub fn browse() -> Option<Built> {
    if !crate::data::is_installed() {
        return None;
    }
    let store = crate::data::app().base.store().clone();
    let prefs = Rc::new(SourcePrefs::new(store));
    let (_ready_tx, ready) = watch::channel(true);

    let manga_rows = || crate::ext_host::manga_rows();
    let anime_rows = || anime_exts(&crate::ext_host::anime_lists());
    let rows_of = Rc::new(move |page: BrowsePage| match page.kind() {
        BrowseKind::Manga => manga_rows(),
        BrowseKind::Anime => anime_rows(),
    });

    // Sources: the toggles write the sets, the lists wait for a source registry
    let manga_sources = manga_sources_page(&prefs, ready.clone(), Rc::new(manga_sources_snapshot));
    let anime_sources = {
        let (pin, hide) = (prefs.pinned_anime_sources.clone(), prefs.disabled_anime_sources.clone());
        let nav = AnimeSourcesNav {
            route: Rc::new(move |route| match route {
                TabRoute::Browse { source_id, query } => open_route(&BrowseRoute::AnimeBrowseSource { source_id, query }),
            }),
            toggle_pin: Rc::new(move |id| flip(&pin, id, toggle_pin)),
            toggle_disabled: Rc::new(move |id| flip(&hide, id, flip_disabled)),
            retry: Rc::new(|| {}),
            toast: Rc::new(|spec| crate::toast::notify(spec.message)),
        };
        let page = AnimeSourcesPage::new(nav, ready.clone());
        // the host's installed sources, each with its package icon; read again when the tab is shown and when a
        // language, hidden, pinned or last-used source changes
        let show = {
            let (page, prefs) = (page.clone(), Rc::clone(&prefs));
            move || {
                let (sources, icons) = crate::ext_host::anime_sources();
                page.update(sources, ListPrefs::read(&prefs), icons);
            }
        };
        show();
        let shown = show.clone();
        page.widget().connect_map(move |_| shown());
        let checks: Vec<Box<dyn Fn() -> bool>> = vec![
            watch_check(&prefs.enabled_languages),
            watch_check(&prefs.disabled_anime_sources),
            watch_check(&prefs.pinned_anime_sources),
            watch_check(&prefs.last_used_anime_source),
        ];
        let alive = {
            let widget = page.widget().downgrade();
            move || widget.upgrade().is_some()
        };
        glue::poll_pref_changes(checks, alive, show);
        page
    };

    // Extensions: the rows are the host's, read again each time the tab is shown
    let extension_page = |kind: BrowseKind| {
        let (repos_label, store_page) = match kind {
            BrowseKind::Anime => (MsgKey::extensionStores, "settings_anime_extension_stores"),
            BrowseKind::Manga => (MsgKey::label_extension_repos, "settings_manga_extension_repos"),
        };
        let (details_prefs, details_rows) = (Rc::clone(&prefs), Rc::clone(&rows_of));
        let (nsfw_prefs, trust_prefs) = (Rc::clone(&prefs), Rc::clone(&prefs));
        let page_kind = BrowsePage::extensions_of(kind);
        let nav = ExtNav {
            open_details: Rc::new(move |pkg| {
                let ext = details_rows(page_kind).into_iter().find(|e| e.pkg == pkg);
                let again = Rc::clone(&details_rows);
                open_details(kind, &details_prefs, ext, Rc::new(move || again(page_kind)));
            }),
            trust: Rc::new(move |pkg| {
                let mut trusted = trust_prefs.trusted_extensions.get();
                trusted.insert(pkg.to_owned());
                if let Err(error) = trust_prefs.trusted_extensions.set(trusted) {
                    tracing::warn!(%error, "a trusted extension could not be stored");
                }
            }),
            web_view: {
                let rows = Rc::clone(&rows_of);
                Rc::new(move |pkg| {
                    let target = rows(page_kind).into_iter().find(|e| e.pkg == pkg).and_then(|ext| super::extensions::webview_target(&ext));
                    if let Some((url, title)) = target {
                        crate::entry_core::handoff::open_webview(crate::entry_core::handoff::WebViewRequest { url, title });
                    }
                })
            },
            open_repos: Rc::new(move || crate::ui::open(store_page)),
            repos_label,
            show_nsfw: Rc::new(move || nsfw_prefs.show_nsfw_source.get()),
        };
        let page = super::pages::extensions(Arc::new(HostBackend), nav);
        // the install state of a package comes from the local registry (`has_installed`, T-40a-047)
        page.set_dir_exists(Rc::new(has_installed));
        let (reload, languages) = (Rc::clone(&rows_of), Rc::clone(&prefs));
        let shown = page.clone();
        page.widget().connect_map(move |_| {
            shown.set_enabled_languages(Some(languages.enabled_languages.get()));
            shown.set_exts(reload(page_kind));
        });
        page
    };
    let (anime_extensions, manga_extensions) = (extension_page(BrowseKind::Anime), extension_page(BrowseKind::Manga));

    let pages: Vec<(BrowsePage, gtk::Widget)> = vec![
        (BrowsePage::AnimeSources, anime_sources.widget().clone().upcast()),
        (BrowsePage::MangaSources, manga_sources.widget().clone().upcast()),
        (BrowsePage::AnimeExtensions, anime_extensions.widget().clone().upcast()),
        (BrowsePage::MangaExtensions, manga_extensions.widget().clone().upcast()),
        (BrowsePage::AnimeMigrate, anime_migrate(&prefs)),
        (BrowsePage::MangaMigrate, manga_migrate(&prefs)),
    ];
    let filter_prefs = Rc::clone(&prefs);
    let filter_rows = Rc::clone(&rows_of);
    let page_actions: Vec<(BrowsePage, gtk::Widget)> = vec![
        (BrowsePage::AnimeExtensions, anime_extensions.actions().clone().upcast()),
        (BrowsePage::MangaExtensions, manga_extensions.actions().clone().upcast()),
    ];
    let deps = ShellDeps {
        pages,
        page_actions,
        on_route: Rc::new(|route| open_route(&route)),
        on_query: {
            let (anime, manga) = (anime_extensions.clone(), manga_extensions.clone());
            Rc::new(move |page, text| match page {
                BrowsePage::AnimeExtensions => anime.set_query(&text),
                BrowsePage::MangaExtensions => manga.set_query(&text),
                _ => {}
            })
        },
        on_filter: Rc::new(move |page| open_filter(page, &filter_prefs, &*filter_rows)),
        on_repos: Rc::new(|kind| {
            crate::ui::open(match kind {
                BrowseKind::Anime => "settings_anime_extension_stores",
                BrowseKind::Manga => "settings_manga_extension_repos",
            })
        }),
        on_refresh: {
            let (anime, manga) = (anime_extensions.clone(), manga_extensions.clone());
            Rc::new(move |page| match page {
                BrowsePage::AnimeExtensions => anime.refresh(),
                BrowsePage::MangaExtensions => manga.refresh(),
                _ => {}
            })
        },
        on_update_all: {
            let (anime, manga) = (anime_extensions.clone(), manga_extensions.clone());
            Rc::new(move |page| match page {
                BrowsePage::AnimeExtensions => anime.update_all(),
                BrowsePage::MangaExtensions => manga.update_all(),
                _ => {}
            })
        },
        on_leave: Rc::new(crate::ui::pop),
    };
    let shell = Rc::new(BrowseShell::new(deps));
    // the tab badge follows the update count of each extension page
    for (page, extensions) in [(BrowsePage::AnimeExtensions, &anime_extensions), (BrowsePage::MangaExtensions, &manga_extensions)] {
        let weak = Rc::downgrade(&shell);
        // the manga count is stored as `ext_updates_count` (T-40a-046); a count of 0 hides the badge
        let stored = prefs.manga_extension_updates_count.clone();
        extensions.set_on_updates(Rc::new(move |count| {
            if page == BrowsePage::MangaExtensions {
                let _ = stored.set(i32::try_from(count).unwrap_or(i32::MAX));
            }
            if let Some(shell) = weak.upgrade() {
                shell.set_badge(page, count);
            }
        }));
    }
    // the manga badge starts from the manager's rows (or the stored count) and follows the stored count afterwards
    let stored = prefs.manga_extension_updates_count.clone();
    let published = crate::ext_host::registry().map(|_| manga_rows());
    let (badge, write) = startup_badge(stored.get(), published.as_deref());
    if let Some(count) = write {
        if let Err(error) = stored.set(count) {
            tracing::warn!(%error, "the extension update count could not be stored");
        }
    }
    shell.set_badge(BrowsePage::MangaExtensions, badge);
    {
        let (weak, alive) = (Rc::downgrade(&shell), Rc::downgrade(&shell));
        let (checks, count): (Vec<Box<dyn Fn() -> bool>>, _) = (vec![watch_check(&stored)], stored.clone());
        glue::poll_pref_changes(checks, move || alive.upgrade().is_some(), move || {
            if let Some(shell) = weak.upgrade() {
                shell.set_badge(BrowsePage::MangaExtensions, usize::try_from(count.get()).unwrap_or(0));
            }
        });
    }
    let built = Built { content: shell.root.clone().upcast(), end: shell.end.clone() };
    SHELL.with(|s| *s.borrow_mut() = Some(shell));
    Some(built)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use kuro_extensions::anime::abi::AbiVersion;
    use kuro_extensions::anime::model::{AnimeSourceRef, Available, AvailableRow, Installed, Kind, StoreRef, Untrusted};

    use super::*;

    fn source() -> AnimeSourceRef {
        AnimeSourceRef { id: 7, name: "Src".into(), lang: "en".into(), base_url: "https://src.example".into() }
    }

    fn installed(pkg: &str, has_update: bool) -> Installed {
        Installed {
            name: pkg.into(),
            pkg: pkg.into(),
            version_name: "2".into(),
            version_code: 2,
            abi: AbiVersion { major: 1, minor: 0 },
            lang: "en".into(),
            nsfw: false,
            torrent: false,
            sources: vec![source()],
            icon: Some(PathBuf::from("/icons/a.png")),
            signer: kuro_extensions::anime::model::SignerDigest(String::new()),
            has_update,
            is_obsolete: false,
            store: None,
        }
    }

    fn listed(pkg: &str) -> Available {
        Available {
            name: pkg.into(),
            raw_name: pkg.into(),
            pkg: pkg.into(),
            version_name: "1".into(),
            version_code: 1,
            abi: None,
            aniyomi_lib: None,
            lang: "en".into(),
            nsfw: true,
            torrent: false,
            sources: vec![source()],
            kind: Kind::Wasm,
            apk_url: String::new(),
            icon_url: "https://store.example/icon.png".into(),
            kuroext: None,
            store: StoreRef { index_url: "https://s/i.json".into(), name: "S".into() },
        }
    }

    #[test]
    fn the_host_buckets_become_rows_with_their_icons_and_states() {
        let buckets = Buckets {
            updates: vec![installed("eu.up", true)],
            installed: vec![installed("eu.in", false)],
            available: vec![
                AvailableRow { ext: listed("eu.av"), source: Some(source()), row_key: "eu.av-7".into() },
                AvailableRow { ext: listed("eu.av"), source: Some(source()), row_key: "eu.av-8".into() },
            ],
            untrusted: vec![Untrusted {
                name: "U".into(),
                pkg: "eu.un".into(),
                version_name: "3".into(),
                version_code: 3,
                abi: AbiVersion { major: 1, minor: 0 },
                nsfw: true,
                signer: kuro_extensions::anime::model::SignerDigest(String::new()),
            }],
            not_runnable_hidden: 0,
        };
        let rows = anime_exts(&buckets);
        let by_pkg = |pkg: &str| rows.iter().find(|e| e.pkg == pkg).unwrap_or_else(|| panic!("{pkg}"));
        assert_eq!(rows.len(), 4, "a listed package is one row; the page splits it per source");
        assert_eq!(by_pkg("eu.up").state, ExtState::Installed { has_update: true, obsolete: false });
        assert_eq!(by_pkg("eu.in").icon_file.as_deref(), Some("/icons/a.png"), "an installed icon is a file");
        assert_eq!(by_pkg("eu.av").icon_file.as_deref(), Some("https://store.example/icon.png"), "a listed icon is a url");
        assert_eq!(by_pkg("eu.av").state, ExtState::Available);
        assert_eq!(by_pkg("eu.un").state, ExtState::Untrusted);
        assert_eq!(by_pkg("eu.un").lang, None, "an untrusted extension has no language");
        assert_eq!(by_pkg("eu.in").sources[0].base_url.as_deref(), Some("https://src.example"));
        assert_eq!(super::super::extensions::updates_count(&rows), 1, "the tab badge counts the one with an update");
    }

    fn ext_row(pkg: &str, has_update: bool) -> Ext {
        Ext {
            name: pkg.into(),
            pkg: pkg.into(),
            version: "1".into(),
            lang: Some("en".into()),
            nsfw: false,
            sources: Vec::new(),
            state: ExtState::Installed { has_update, obsolete: false },
            icon_file: None,
        }
    }

    #[test]
    fn the_badge_starts_from_the_managers_rows_or_else_the_stored_count() {
        let rows = [ext_row("a", true), ext_row("b", true), ext_row("c", false)];
        assert_eq!(startup_badge(0, Some(&rows)), (2, Some(2)), "the manager's count is shown and stored");
        assert_eq!(startup_badge(2, Some(&rows)), (2, None), "an unchanged count is not written again");
        assert_eq!(startup_badge(5, None), (5, None), "no manager yet: the stored count stands");
        assert_eq!(startup_badge(-3, None), (0, None), "a stored count below zero reads as none");
        assert_eq!(startup_badge(4, Some(&[])), (0, Some(0)), "the manager says none: the badge goes away");
    }

    #[test]
    fn only_an_installed_row_has_details_and_they_carry_no_invented_fields() {
        let mut ext = anime_exts(&Buckets { installed: vec![installed("eu.in", false)], ..Buckets::default() }).remove(0);
        let details = details_of(&ext, &DetailFacts::default()).expect("installed");
        assert_eq!((details.lib_version, details.repo_url.as_deref(), details.shared), (None, None, false));
        assert_eq!(details.sources.len(), 1);
        assert_eq!(details.sources[0].base_url.as_deref(), Some("https://src.example"));
        ext.state = ExtState::Available;
        assert_eq!(details_of(&ext, &DetailFacts::default()), None);
    }

    fn row_with(sources: &[(i64, &str, &str)]) -> Ext {
        Ext {
            sources: sources.iter().map(|(id, name, lang)| ExtSource { id: *id, name: (*name).into(), lang: (*lang).into(), base_url: None }).collect(),
            ..ext_row("eu.multi", false)
        }
    }

    #[test]
    fn a_source_is_labelled_by_name_only_with_several_sources_that_do_not_all_share_one_name() {
        let label = |ext: &Ext| details_of(ext, &DetailFacts::default()).expect("installed").sources.iter().map(|s| s.label_as_name).collect::<Vec<_>>();
        assert_eq!(label(&row_with(&[(1, "Only", "en")])), [false], "one source: the language is its label");
        assert_eq!(label(&row_with(&[(1, "Same", "en"), (2, "Same", "fr")])), [false, false], "one name in several languages: multi-lang single source");
        assert_eq!(label(&row_with(&[(1, "Alpha", "en"), (2, "Beta", "en")])), [true, true], "T-41a-019: different sources are labelled by name");
        assert_eq!(label(&row_with(&[])), Vec::<bool>::new());
    }

    #[test]
    fn the_facts_set_the_repository_and_the_settings_button_of_each_source() {
        let ext = row_with(&[(1, "Alpha", "en"), (2, "Beta", "en")]);
        let facts = DetailFacts { repo_url: Some("https://raw.githubusercontent.com/u/r/main/index.min.json".into()), configurable: BTreeSet::from([2]) };
        let details = details_of(&ext, &facts).expect("installed");
        assert_eq!(details.repo_url.as_deref(), Some("https://raw.githubusercontent.com/u/r/main/index.min.json"));
        assert_eq!(details.sources.iter().map(|s| s.configurable).collect::<Vec<_>>(), [false, true], "only the source with settings rows has the button");
    }

    #[test]
    fn an_anime_package_has_the_index_url_of_its_store_and_other_packages_have_none() {
        let mut with_store = installed("eu.in", false);
        with_store.store = Some(kuro_extensions::anime::model::StoreRef { index_url: "https://s/i.json".into(), name: "S".into() });
        let buckets = Buckets { updates: vec![installed("eu.up", true)], installed: vec![with_store], ..Buckets::default() };
        assert_eq!(anime_repo_url(&buckets, "eu.in").as_deref(), Some("https://s/i.json"));
        assert_eq!(anime_repo_url(&buckets, "eu.up"), None, "an installed package with no store");
        assert_eq!(anime_repo_url(&buckets, "eu.missing"), None);
    }

    struct Declaring(i64);

    impl SourceDirectory for Declaring {
        fn sources(&self) -> Vec<super::super::directory::DirectorySource> {
            vec![super::super::directory::DirectorySource { source: Source::new(self.0, "Alpha", "en"), package: String::new(), home_url: None }]
        }
        fn loader(&self, _id: i64) -> Option<Arc<dyn super::super::listing::PageLoader>> {
            None
        }
        fn filters(&self, _id: i64) -> Option<kuro_source::filter_list::FilterList> {
            None
        }
        fn preferences(&self, id: i64) -> Option<Vec<kuro_source::SourcePref>> {
            (id == self.0).then(|| vec![kuro_source::SourcePref::Switch { key: "k".into(), title: "T".into(), summary: None, default: false }])
        }
    }

    #[test]
    fn a_source_is_configurable_when_the_directory_declares_rows_for_it() {
        assert_eq!(configurable_ids(Some(&Declaring(2)), [1, 2, 3]), BTreeSet::from([2]));
        assert!(configurable_ids(None, [1, 2]).is_empty(), "no directory: no settings buttons");
        assert_eq!(installed_sources(BrowseKind::Manga, Some(&Declaring(2)), vec![Source::new(9, "A", "en")]).iter().map(|s| s.id).collect::<Vec<_>>(), [2], "manga reads the directory");
        assert_eq!(installed_sources(BrowseKind::Anime, Some(&Declaring(2)), vec![Source::new(9, "A", "en")]).iter().map(|s| s.id).collect::<Vec<_>>(), [9], "anime reads the host's sources");
        assert!(installed_sources(BrowseKind::Manga, None, Vec::new()).is_empty());
    }

    #[test]
    fn a_package_that_drops_out_of_the_installed_rows_is_gone() {
        let rows = [ext_row("a", false), Ext { state: ExtState::Available, ..ext_row("b", false) }];
        assert!(!is_gone(&rows, "a"), "E18: still installed");
        assert!(is_gone(&rows, "b"), "listed but not installed counts as gone");
        assert!(is_gone(&rows, "c"));
        assert!(is_gone(&[], "a"), "the list emptied: the page pops");
    }

    #[test]
    fn the_languages_fail_to_fetch_only_when_there_is_no_extension_host() {
        assert_eq!(language_rows(true, vec![ext_row("a", false)]).map(|rows| rows.len()), Ok(1));
        assert!(language_rows(true, Vec::new()).is_ok_and(|rows| rows.is_empty()), "an empty list is the empty screen, not a failure");
        assert!(language_rows(false, vec![ext_row("a", false)]).is_err(), "T-41a-018: no host, no languages: the banner");
    }

    /// The page is built before the sources start (main builds the tabs first): a source that arrives afterwards is
    /// listed once the page is shown, with no pref written in between.
    #[test]
    fn the_manga_sources_page_lists_sources_that_arrive_after_it_was_built() {
        crate::testing::with_display(|| {
            let prefs = Rc::new(SourcePrefs::new(kuro_prefs::store::PrefStore::in_memory()));
            let arrived: Rc<RefCell<Vec<Source>>> = Rc::default();
            let read = {
                let arrived = Rc::clone(&arrived);
                Rc::new(move |_: &SourcePrefs| SourcesSnapshot {
                    sources: arrived.borrow().clone(),
                    enabled_languages: BTreeSet::from(["en".to_owned()]),
                    last_used: -1,
                    ..SourcesSnapshot::default()
                })
            };
            let (_ready_tx, ready) = watch::channel(true);
            let page = manga_sources_page(&prefs, ready, read);
            arrived.borrow_mut().push(Source::new(2131019126180322627, "Weeb Central", "en"));
            let window = gtk::Window::builder().default_width(600).default_height(600).child(page.widget()).build();
            window.present();
            let lists_it = || {
                let mut stack = vec![page.widget().clone().upcast::<gtk::Widget>()];
                while let Some(widget) = stack.pop() {
                    if widget.downcast_ref::<gtk::Label>().is_some_and(|l| l.label() == "Weeb Central") {
                        return true;
                    }
                    let mut child = widget.first_child();
                    while let Some(c) = child {
                        child = c.next_sibling();
                        stack.push(c);
                    }
                }
                false
            };
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while std::time::Instant::now() < deadline && !lists_it() {
                gtk::glib::MainContext::default().iteration(false);
            }
            assert!(lists_it(), "the source that arrived after the build is listed once the page is shown");
            window.close();
        });
    }

    /// The empty Sources page of a kind has one action, Extensions, and it shows that kind's extensions.
    #[test]
    fn the_empty_sources_action_opens_the_extensions_of_its_kind() {
        crate::testing::with_display(|| {
            let pages = BrowsePage::ALL.iter().map(|p| (*p, gtk::Label::new(Some(&p.title())).upcast::<gtk::Widget>())).collect();
            let deps = ShellDeps {
                pages,
                page_actions: Vec::new(),
                on_route: Rc::new(|_| {}),
                on_query: Rc::new(|_, _| {}),
                on_filter: Rc::new(|_| {}),
                on_repos: Rc::new(|_| {}),
                on_refresh: Rc::new(|_| {}),
                on_update_all: Rc::new(|_| {}),
                on_leave: Rc::new(|| {}),
            };
            let shell = Rc::new(BrowseShell::new(deps));
            SHELL.with(|s| *s.borrow_mut() = Some(Rc::clone(&shell)));
            let shows = |page: BrowsePage| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while std::time::Instant::now() < deadline && shell.visible() != page {
                    gtk::glib::MainContext::default().iteration(false);
                }
                shell.visible() == page
            };
            for (kind, page) in [(BrowseKind::Manga, BrowsePage::MangaExtensions), (BrowseKind::Anime, BrowsePage::AnimeExtensions)] {
                let empty = super::super::sources_page::sources_empty(kind);
                let action = empty.last_child().and_downcast::<gtk::Button>().expect("the one action");
                assert_eq!(action.label().as_deref(), Some(tr(MsgKey::label_extensions).as_str()));
                action.emit_clicked();
                assert!(shows(page), "{kind:?}: its extensions show");
            }
            SHELL.with(|s| s.borrow_mut().take());
        });
    }
}
