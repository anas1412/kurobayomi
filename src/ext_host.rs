//! The extension host of the app: the state of the installed and listed extensions and the jobs that fill it. It starts
//! once the databases are open (T-18a-011, T-18a-023, T-18a-054, T-19a-064, T-19a-066):
//!
//! - the manga packages are scanned into an `ExtensionRegistry`, which caches their icons and keeps each folder's last
//!   load problem;
//! - the anime packages are scanned by `AnimeExtensions`, whose events feed one `ExtensionState` on the GTK thread. The
//!   update count is derived from that state and never stored;
//! - after every change the first-run language set is saved (once, while `source_languages` is unsaved) and the Browse
//!   lists are rebuilt from the state.
//!
//! The pieces that are plain functions over the state are tested below; the start-up wiring needs a running app.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use gtk::glib;
use kuro_core::crash_dump::ExtensionStatus;
use kuro_extensions::anime::browse::buckets;
use kuro_extensions::anime::icons::IconCache;
use kuro_extensions::anime::languages::enable_first_run_languages;
use kuro_extensions::anime::loader::PackageDir;
use kuro_extensions::anime::manager::ExtensionState;
use kuro_extensions::anime::model::Buckets;
use kuro_extensions::anime::sources::icon_for_source;
use kuro_extensions::anime::service::{AnimeExtensions, pump};
use kuro_extensions::anime::trust::TrustStore;
use kuro_extensions::registry::{ExtensionRegistry, Snapshot};
use kuro_domain::source::model::Source;
use kuro_downloads::layout::Layout;
use kuro_platform::paths::Paths;
use kuro_prefs::source::{SourcePrefs, system_locale};
use kuro_prefs::store::PrefStore;


/// Whether listings that cannot run here are shown: never (the settings diet cut the `show_not_runnable` switch).
const SHOW_NOT_RUNNABLE: bool = false;

/// The Browse lists of the anime extensions for the current preferences. The language set and `show_nsfw_source`
/// are read on every call, so a change takes effect on the next rebuild (T-19a-066). The not-runnable rows stay
/// hidden (the settings diet cut `show_not_runnable`).
pub fn anime_buckets(state: &ExtensionState, prefs: &SourcePrefs) -> Buckets {
    buckets(state, &prefs.enabled_languages.get(), prefs.show_nsfw_source.get(), SHOW_NOT_RUNNABLE)
}

/// What the Browse pages read: the newest anime lists and sources.
#[derive(Default)]
struct Latest {
    anime: Buckets,
    anime_sources: Vec<Source>,
    anime_icons: BTreeMap<i64, String>,
}

thread_local! {
    static LATEST: RefCell<Latest> = RefCell::new(Latest::default());
}

/// The anime Browse lists as the last state change left them, built with the current preferences.
#[allow(dead_code)] // read by the anime Browse page, which no screen mounts yet
pub fn anime_lists() -> Buckets {
    LATEST.with(|l| l.borrow().anime.clone())
}

/// The sources of the installed anime packages as the Sources list shows them, and the package icon file of each source
/// that has one (T-17a-025). A source without an icon file is left out of the map, and its row draws the generic icon.
pub fn anime_source_rows(state: &ExtensionState) -> (Vec<Source>, BTreeMap<i64, String>) {
    let refs: Vec<_> = state.installed().values().flat_map(|ext| &ext.sources).collect();
    let icons = refs.iter().filter_map(|s| Some((s.id, icon_for_source(state, Some(s))?.to_string_lossy().into_owned()))).collect();
    (refs.iter().map(|s| Source::new(s.id, &s.name, &s.lang)).collect(), icons)
}

/// The anime sources and their icon files as the last state change left them, then the recipe sources.
pub fn anime_sources() -> (Vec<Source>, BTreeMap<i64, String>) {
    LATEST.with(|l| {
        let latest = l.borrow();
        // the recipe sources (docs/source-recipes.md) have no package and no icon file
        let sources = latest.anime_sources.iter().cloned().chain(crate::recipe_sources::anime_list()).collect();
        (sources, latest.anime_icons.clone())
    })
}

/// What a state change publishes: the first-run languages (a no-op once `source_languages` is saved), then the Browse
/// lists and sources built with the current preferences.
fn publish(state: &ExtensionState, prefs: &SourcePrefs) {
    if let Err(e) = enable_first_run_languages(prefs, state.available(), &system_locale()) {
        tracing::warn!("could not save the first-run languages: {e}");
    }
    let lists = anime_buckets(state, prefs);
    let (sources, icons) = anime_source_rows(state);
    LATEST.with(|l| {
        let mut latest = l.borrow_mut();
        latest.anime = lists;
        latest.anime_sources = sources;
        latest.anime_icons = icons;
    });
}

/// The download folder names of every installed source (T-17a-048), as (anime, manga): `Name (LANG)` for the anime
/// packages, the extension names for the manga packages, and the same `Name (LANG)` for every loaded repository source of
/// either kind, so none of their folders is reported as an orphan. All come from the one folder naming of the download layout.
fn download_folder_names(state: &ExtensionState, manga: &Snapshot) -> (Vec<String>, Vec<String>) {
    use kuro_platform::kind::Kind;
    let loaded = |kind: Kind| -> Vec<String> {
        crate::browse::directory::current(kind).sources().iter().map(|row| Layout::source_dir_name(&row.source.name, &row.source.lang)).collect()
    };
    let anime = state.installed().values().flat_map(|ext| &ext.sources).map(|s| Layout::source_dir_name(&s.name, &s.lang));
    let manga = manga.installed.values().map(|ext| Layout::source_dir_name(&ext.name, ""));
    (anime.chain(loaded(Kind::Anime)).collect(), manga.chain(loaded(Kind::Manga)).collect())
}

/// Logs the download folders that match no installed source, once the first anime scan is in. Nothing is deleted.
fn log_orphans(root: std::path::PathBuf, names: Vec<String>, handle: &tokio::runtime::Handle) {
    handle.spawn_blocking(move || {
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        if let Err(e) = kuro_source::orphans::log_orphaned_download_folders(&root, &names) {
            tracing::warn!("the download folders could not be checked for orphans: {e}");
        }
    });
}

/// The running host.
struct Host {
    registry: Arc<ExtensionRegistry>,
    state: Rc<RefCell<ExtensionState>>,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

/// The manga registry, once [`start`] has run.
pub fn registry() -> Option<Arc<ExtensionRegistry>> {
    HOST.with(|h| h.borrow().as_ref().map(|host| host.registry.clone()))
}

/// The installed packages as the crash log lists them: the manga ones, then the anime ones. A manga listing carries only
/// version codes, so those stand in for the version names.
pub fn crash_log_extensions() -> (Vec<ExtensionStatus>, Vec<ExtensionStatus>) {
    let manga = registry().map(|r| manga_statuses(&r.snapshot())).unwrap_or_default();
    let anime = HOST.with(|h| h.borrow().as_ref().map(|host| anime_statuses(&host.state.borrow()))).unwrap_or_default();
    (manga, anime)
}

fn manga_statuses(snapshot: &Snapshot) -> Vec<ExtensionStatus> {
    snapshot
        .installed
        .values()
        .map(|ext| ExtensionStatus {
            name: ext.name.clone(),
            installed_name: ext.version_code.to_string(),
            installed_code: ext.version_code,
            available: snapshot.available.get(&ext.pkg).map(|a| (a.version_code.to_string(), a.version_code)),
            obsolete: ext.is_obsolete,
        })
        .collect()
}

fn anime_statuses(state: &ExtensionState) -> Vec<ExtensionStatus> {
    state
        .installed()
        .values()
        .map(|ext| ExtensionStatus {
            name: ext.name.clone(),
            installed_name: ext.version_name.clone(),
            installed_code: ext.version_code,
            available: state.available().iter().find(|a| a.pkg == ext.pkg).map(|a| (a.version_name.clone(), a.version_code)),
            obsolete: ext.is_obsolete,
        })
        .collect()
}

/// The number of anime packages with an update, derived from the state and never stored.
pub fn anime_update_count() -> usize {
    HOST.with(|h| h.borrow().as_ref().map_or(0, |host| host.state.borrow().update_count()))
}

/// Starts the host: scans both package folders and pumps the anime events into the state. Call once from the GTK main
/// thread inside the app's runtime.
pub fn start(store: &PrefStore) {
    if HOST.with(|h| h.borrow().is_some()) {
        return;
    }
    let paths = match kuro_platform::linux::LinuxPaths::system() {
        Ok(paths) => paths,
        Err(e) => {
            tracing::warn!("the extension host does not start: {e}");
            // No sources will load: open the gate empty so no screen waits on it forever.
            crate::source_gate::mark_loaded();
            return;
        }
    };
    let prefs = SourcePrefs::new(store.clone());
    let handle = tokio::runtime::Handle::current();

    // declarative sources, then the source gate the screens wait on (startup::load_sources)
    let shared: Arc<dyn kuro_platform::paths::Paths + Send + Sync> = Arc::new(paths.clone());
    crate::startup::load_sources(store, &shared);

    // manga packages: one scan, icons into the cache
    let registry = Arc::new(ExtensionRegistry::new());
    {
        let (registry, dir, icons) = (registry.clone(), paths.extensions_dir(), kuro_extensions::icon::icon_cache_dir(&paths));
        handle.spawn_blocking(move || {
            kuro_extensions::loader::scan_into(&registry, &dir, &BTreeMap::new(), &icons);
        });
    }

    // anime packages: the service reports through the channel, the state lives here
    let (events, inbox) = async_channel::unbounded();
    let service = Rc::new(AnimeExtensions::new(handle, events));
    let state = Rc::new(RefCell::new(ExtensionState::new()));
    service.scan(
        PackageDir::from_paths(&paths),
        PackageDir::download_dir(&paths),
        TrustStore::new(prefs.trusted_extensions.clone()),
        Vec::new(),
        &prefs,
        IconCache::from_paths(&paths),
    );
    let apply_state = state.clone();
    let apply_prefs = SourcePrefs::new(store.clone());
    let storage = crate::settings_ui::storage::manager(store);
    let roots = storage.map(|s| (s.downloads_path(kuro_platform::kind::Kind::Anime), s.downloads_path(kuro_platform::kind::Kind::Manga)));
    let (apply_registry, apply_handle) = (registry.clone(), tokio::runtime::Handle::current());
    let mut orphans_logged = false;
    let mut known_sources: Vec<i64> = Vec::new();
    glib::spawn_future_local(pump(inbox, move |event| {
        let mut state = apply_state.borrow_mut();
        state.apply(event);
        publish(&state, &apply_prefs);
        // An anime package was installed or removed: the index rescans and the waiting downloads are asked about their
        // source, as after a repository load.
        let sources: Vec<i64> = state.installed().values().flat_map(|ext| &ext.sources).map(|s| s.id).collect();
        if orphans_logged && sources != known_sources {
            if let Some(downloads) = crate::shell::host::Shell::get().and_then(|shell| shell.download_service()) {
                downloads.reindex(Some(kuro_platform::kind::Kind::Anime));
                downloads.sources_changed();
            }
        }
        known_sources = sources;
        if state.initialized() && !orphans_logged {
            orphans_logged = true;
            if let Some((anime_root, manga_root)) = &roots {
                let (anime, manga) = download_folder_names(&state, &apply_registry.snapshot());
                log_orphans(anime_root.clone(), anime, &apply_handle);
                log_orphans(manga_root.clone(), manga, &apply_handle);
            }
        }
    }));

    HOST.with(|h| *h.borrow_mut() = Some(Host { registry, state }));
}

#[cfg(test)]
mod tests {
    use kuro_extensions::anime::abi::AbiVersion;
    use kuro_extensions::anime::model::{AnimeSourceRef, Available, Kind, StoreRef};
    use kuro_extensions::registry::Installed;

    use super::*;

    #[test]
    fn manga_statuses_pair_each_installed_package_with_its_listing() {
        let installed = |pkg: &str, code: i64, obsolete: bool| Installed { pkg: pkg.into(), name: pkg.to_uppercase(), version_code: code, lib_version: 1.0, has_update: false, is_obsolete: obsolete };
        let mut snapshot = Snapshot::default();
        snapshot.installed.insert("a".into(), installed("a", 1, false));
        snapshot.installed.insert("b".into(), installed("b", 5, true));
        snapshot.available.insert("a".into(), kuro_extensions::registry::Available { pkg: "a".into(), version_code: 3, lib_version: 1.0 });
        let got = manga_statuses(&snapshot);
        assert_eq!(got[0].available, Some(("3".to_owned(), 3)));
        assert!(!got[0].obsolete);
        assert_eq!(got[1].available, None);
        assert!(got[1].obsolete);
    }

    fn listed(pkg: &str, kind: Kind, lang: &str) -> Available {
        Available {
            name: pkg.into(),
            raw_name: pkg.into(),
            pkg: pkg.into(),
            version_name: "1".into(),
            version_code: 1,
            abi: None::<AbiVersion>,
            aniyomi_lib: None,
            lang: lang.into(),
            nsfw: false,
            torrent: false,
            sources: vec![AnimeSourceRef { id: 1, name: pkg.into(), lang: lang.into(), base_url: String::new() }],
            kind,
            apk_url: String::new(),
            icon_url: String::new(),
            kuroext: None,
            store: StoreRef { index_url: "https://s/i.json".into(), name: "S".into() },
        }
    }

    #[test]
    fn a_source_row_carries_its_package_icon_when_the_package_has_one() {
        let source = |id: i64| AnimeSourceRef { id, name: format!("S{id}"), lang: "en".into(), base_url: String::new() };
        let package = |pkg: &str, icon: Option<&str>, id: i64| {
            kuro_extensions::anime::model::LoadResult::Success(kuro_extensions::anime::model::Installed {
                name: pkg.into(),
                pkg: pkg.into(),
                version_name: "1".into(),
                version_code: 1,
                abi: AbiVersion { major: 1, minor: 0 },
                lang: "en".into(),
                nsfw: false,
                torrent: false,
                sources: vec![source(id)],
                icon: icon.map(Into::into),
                signer: kuro_extensions::anime::model::SignerDigest("ab".repeat(32)),
                has_update: false,
                is_obsolete: false,
                store: None,
            })
        };
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![package("eu.a", Some("/icons/a.png"), 1), package("eu.b", None, 2)]);
        let (rows, icons) = anime_source_rows(&state);
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(icons, BTreeMap::from([(1, "/icons/a.png".to_owned())]), "no entry for the package without an icon: the generic icon");
    }

    #[test]
    fn download_folder_names_cover_the_anime_sources_and_the_manga_packages() {
        let source = |name: &str| AnimeSourceRef { id: 1, name: name.into(), lang: "en".into(), base_url: String::new() };
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![kuro_extensions::anime::model::LoadResult::Success(kuro_extensions::anime::model::Installed {
            name: "T".into(),
            pkg: "eu.t".into(),
            version_name: "1".into(),
            version_code: 1,
            abi: AbiVersion { major: 1, minor: 0 },
            lang: "en".into(),
            nsfw: false,
            torrent: false,
            sources: vec![source("Tube/One")],
            icon: None,
            signer: kuro_extensions::anime::model::SignerDigest("ab".repeat(32)),
            has_update: false,
            is_obsolete: false,
            store: None,
        })]);
        let registry = ExtensionRegistry::new();
        registry.set_installed(vec![Installed { pkg: "eu.m".into(), name: "Manga".into(), version_code: 1, lib_version: 1.0, has_update: false, is_obsolete: false }]);
        let names = download_folder_names(&state, &registry.snapshot());
        assert_eq!(names, (vec!["Tube_One (EN)".to_owned()], vec!["Manga".to_owned()]));
    }

    fn state() -> ExtensionState {
        let mut state = ExtensionState::new();
        state.apply_available(vec![listed("eu.ok", Kind::Wasm, "en"), listed("eu.apk", Kind::ApkOnly, "en"), listed("eu.unsigned", Kind::Unsigned, "en")]);
        state
    }

    /// T-19a-066: the Browse lists hide the not-runnable rows (`anime_buckets`); `buckets` shows them when asked.
    #[test]
    fn the_browse_lists_hide_the_not_runnable_rows_and_buckets_shows_them_when_the_flag_is_on() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        prefs.enabled_languages.set(["en".to_owned()].into()).unwrap();
        let state = state();
        let hidden = anime_buckets(&state, &prefs);
        assert_eq!(hidden.available.len(), 1);
        assert_eq!(hidden.not_runnable_hidden, 2);
        let shown = buckets(&state, &["en".to_owned()].into(), true, true);
        assert_eq!(shown.available.len(), 3);
        assert_eq!(shown.not_runnable_hidden, 0);
    }

    /// T-18a-023: a state change saves the device language set once, and the user's later choice stays.
    #[test]
    fn a_listing_saves_the_first_run_languages_once_and_publishes_the_page_data() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store);
        assert!(!prefs.enabled_languages.is_set());
        publish(&state(), &prefs);
        assert!(prefs.enabled_languages.is_set(), "the first listing saves the languages");
        prefs.enabled_languages.set(["ja".to_owned()].into()).unwrap();
        publish(&state(), &prefs);
        assert_eq!(prefs.enabled_languages.get(), ["ja".to_owned()].into(), "a later choice stays");
        assert_eq!(anime_lists().not_runnable_hidden, 2);
    }
}
