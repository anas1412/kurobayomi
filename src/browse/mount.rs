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
use kuro_domain::source::toggles::{flip_disabled, toggle_pin};
use kuro_extensions::InstallStep;
use kuro_extensions::anime::model::Buckets;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::pref::Pref;
use kuro_prefs::source::SourcePrefs;
use tokio::sync::watch;

use super::anime::sources::{ListPrefs, TabRoute};
use super::anime::sources_view::{AnimeSourcesNav, AnimeSourcesPage};
use super::ext_details::{DetailSource, ExtDetails};
use super::ext_details_page::{DetailsDeps, ExtDetailsPage};
use super::extensions::{Ext, ExtSource, ExtState};
use super::extensions_page::{ExtNav, ExtensionBackend, JobFuture, RefreshFuture, Reporter};
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

/// The details of an installed extension from the rows the page shows. `lib_version` and `repo_url` are not part of the
/// shared row, so the debug text says `n/a` and `null` for them rather than invent a value.
pub fn details_of(ext: &Ext) -> Option<ExtDetails> {
    let ExtState::Installed { has_update, obsolete } = ext.state else { return None };
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
        repo_url: None,
        icon_file: ext.icon_file.clone(),
        sources: ext
            .sources
            .iter()
            .map(|s| DetailSource {
                id: s.id,
                name: s.name.clone(),
                lang: s.lang.clone(),
                label_as_name: s.lang.is_empty(),
                configurable: false,
                base_url: s.base_url.clone(),
            })
            .collect(),
    })
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

/// Pushes a page of the tab with a title and a Back button.
fn push_page(title: &str, content: &impl IsA<gtk::Widget>) {
    crate::ui::push(&crate::ui::page(title, content, Vec::new(), true));
}

fn open_route(route: &BrowseRoute) {
    crate::ui::open(route.screen_id());
}

/// The language filter of the visible page: Show/hide sources for a sources page, the language switches for an
/// extensions page (B§4, B§8).
fn open_filter(page: BrowsePage, prefs: &Rc<SourcePrefs>, rows: &dyn Fn(BrowsePage) -> Vec<Ext>) {
    if page.is_extensions() {
        let languages = ExtensionLanguagesPage::new(prefs.enabled_languages.clone(), Rc::new(|text: String| crate::toast::notify(text)));
        languages.set_exts(&rows(page));
        push_page(&tr(MsgKey::label_extensions), languages.widget());
    } else {
        let hidden = match page.kind() {
            BrowseKind::Anime => prefs.disabled_anime_sources.clone(),
            BrowseKind::Manga => prefs.disabled_manga_sources.clone(),
        };
        let filter = SourcesFilterPage::new(prefs.enabled_languages.clone(), hidden);
        // no source registry yet: the page says that no installed source was found
        filter.set_sources(Vec::new());
        push_page(&tr(MsgKey::label_sources), filter.widget());
    }
}

/// The details page of an installed extension, over the preferences of its kind.
fn open_details(kind: BrowseKind, prefs: &Rc<SourcePrefs>, ext: Option<Ext>) {
    let (incognito, disabled) = match kind {
        BrowseKind::Anime => (prefs.incognito_anime_extensions.clone(), prefs.disabled_anime_sources.clone()),
        BrowseKind::Manga => (prefs.incognito_manga_extensions.clone(), prefs.disabled_manga_sources.clone()),
    };
    let deps = DetailsDeps {
        incognito,
        disabled,
        uninstall: Rc::new(|| {}),
        open_source_prefs: Rc::new(|_| {}),
        open_url: Rc::new(|url| {
            gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
                if let Err(error) = result {
                    tracing::warn!(%error, "the repository link could not be opened");
                }
            });
        }),
        // no HTTP client with a cookie jar is wired into the app yet, so there is nothing to clear
        clear_cookies: Rc::new(|_| 0),
        toast: Rc::new(|text: String| crate::toast::notify(text)),
    };
    let page = ExtDetailsPage::new(deps);
    page.set_details(ext.as_ref().and_then(details_of));
    push_page(&tr(MsgKey::label_extension_info), page.widget());
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
    let manga_sources = {
        let (pin, hide) = (prefs.pinned_manga_sources.clone(), prefs.disabled_manga_sources.clone());
        let nav = SourcesNav {
            open_listing: Rc::new(|_, _| crate::ui::open("browse_source")),
            toggle_pin: Rc::new(move |id| flip(&pin, id, toggle_pin)),
            toggle_disabled: Rc::new(move |id| flip(&hide, id, flip_disabled)),
        };
        let page = super::pages::sources(nav, ready.clone());
        page.update(SourcesSnapshot {
            sources: Vec::new(),
            enabled_languages: prefs.enabled_languages.get(),
            disabled: prefs.disabled_manga_sources.get(),
            pinned: prefs.pinned_manga_sources.get(),
            last_used: prefs.last_used_manga_source.get(),
            icon_files: BTreeMap::new(),
        });
        page
    };
    let anime_sources = {
        let (pin, hide) = (prefs.pinned_anime_sources.clone(), prefs.disabled_anime_sources.clone());
        let filter_prefs = Rc::clone(&prefs);
        let filter_rows = Rc::clone(&rows_of);
        let nav = AnimeSourcesNav {
            route: Rc::new(move |route| match route {
                TabRoute::GlobalSearch => crate::ui::open("global_search"),
                TabRoute::Visibility => open_filter(BrowsePage::AnimeSources, &filter_prefs, &*filter_rows),
                TabRoute::Browse { .. } => crate::ui::open("browse_source"),
            }),
            toggle_pin: Rc::new(move |id| flip(&pin, id, toggle_pin)),
            toggle_disabled: Rc::new(move |id| flip(&hide, id, flip_disabled)),
            retry: Rc::new(|| {}),
            toast: Rc::new(|spec| crate::toast::notify(spec.message)),
        };
        let page = AnimeSourcesPage::new(nav, ready.clone());
        page.update(Vec::new(), ListPrefs::read(&prefs), BTreeMap::new());
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
                open_details(kind, &details_prefs, ext);
            }),
            trust: Rc::new(move |pkg| {
                let mut trusted = trust_prefs.trusted_extensions.get();
                trusted.insert(pkg.to_owned());
                if let Err(error) = trust_prefs.trusted_extensions.set(trusted) {
                    tracing::warn!(%error, "a trusted extension could not be stored");
                }
            }),
            web_view: Rc::new(|_| crate::ui::open("webview")),
            open_repos: Rc::new(move || crate::ui::open(store_page)),
            repos_label,
            show_nsfw: Rc::new(move || nsfw_prefs.show_nsfw_source.get()),
        };
        let page = super::pages::extensions(Arc::new(HostBackend), nav);
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
        // migration lists wait for the library's per-source counts, which this lane does not read
        (BrowsePage::AnimeMigrate, empty_page(&tr(MsgKey::information_empty_library))),
        (BrowsePage::MangaMigrate, empty_page(&tr(MsgKey::information_empty_library))),
    ];
    let filter_prefs = Rc::clone(&prefs);
    let filter_rows = Rc::clone(&rows_of);
    let deps = ShellDeps {
        pages,
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
        on_leave: Rc::new(crate::ui::pop),
    };
    let shell = Rc::new(BrowseShell::new(deps));
    // the tab badge follows the update count of each extension page
    for (page, extensions) in [(BrowsePage::AnimeExtensions, &anime_extensions), (BrowsePage::MangaExtensions, &manga_extensions)] {
        let weak = Rc::downgrade(&shell);
        extensions.set_on_updates(Rc::new(move |count| {
            if let Some(shell) = weak.upgrade() {
                shell.set_badge(page, count);
            }
        }));
    }
    let built = Built { content: shell.root.clone().upcast(), end: shell.end.clone() };
    SHELL.with(|s| *s.borrow_mut() = Some(shell));
    Some(built)
}

/// A page that says there is nothing to list.
fn empty_page(text: &str) -> gtk::Widget {
    let label = gtk::Label::new(Some(text));
    label.set_valign(gtk::Align::Center);
    label.set_vexpand(true);
    label.upcast()
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

    #[test]
    fn only_an_installed_row_has_details_and_they_carry_no_invented_fields() {
        let mut ext = anime_exts(&Buckets { installed: vec![installed("eu.in", false)], ..Buckets::default() }).remove(0);
        let details = details_of(&ext).expect("installed");
        assert_eq!((details.lib_version, details.repo_url.as_deref(), details.shared), (None, None, false));
        assert_eq!(details.sources.len(), 1);
        assert_eq!(details.sources[0].base_url.as_deref(), Some("https://src.example"));
        ext.state = ExtState::Available;
        assert_eq!(details_of(&ext), None);
    }
}
