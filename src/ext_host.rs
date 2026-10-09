//! The extension host of the app: the state of the installed and listed extensions and the jobs that fill it. It starts
//! once the databases are open (T-18a-011, T-18a-023, T-18a-054, T-19a-064, T-19a-066):
//!
//! - the manga packages are scanned into an `ExtensionRegistry`, which caches their icons and keeps each folder's last
//!   load problem for the details screen;
//! - the anime packages are scanned by `AnimeExtensions`, whose events feed one `ExtensionState` on the GTK thread. The
//!   update count is derived from that state and never stored;
//! - after every change the first-run language set is saved (once, while `source_languages` is unsaved), and the badges
//!   of the listings that cannot run here and the per-store counts are published to the extension pages.
//!
//! The pieces that are plain functions over the state are tested below; the start-up wiring needs a running app.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use gtk::glib;
use kuro_data::extension_store::ExtensionStoreRepo;
use kuro_extensions::anime::browse::{RowBadges, buckets, not_runnable_by_store};
use kuro_extensions::anime::icons::IconCache;
use kuro_extensions::anime::languages::enable_first_run_languages;
use kuro_extensions::anime::loader::PackageDir;
use kuro_extensions::anime::manager::ExtensionState;
use kuro_extensions::anime::model::{Buckets, StoreRow};
use kuro_extensions::anime::service::{AnimeExtensions, pump};
use kuro_extensions::anime::trust::TrustStore;
use kuro_extensions::anime::update::LAST_CHECK_KEY;
use kuro_extensions::registry::{ExtensionRegistry, Snapshot};
use kuro_platform::paths::Paths;
use kuro_prefs::source::{SourcePrefs, system_locale};
use kuro_prefs::store::PrefStore;
use tokio_util::sync::CancellationToken;

use crate::browse::extensions::{Ext, ExtState};
use crate::browse::ext_badges;
use crate::browse::icons::cached_icon_file;

/// The Browse lists of the anime extensions for the current preferences: the language set, `show_nsfw_source` and
/// `show_not_runnable` are read on every call, so a change takes effect on the next rebuild (T-19a-066).
pub fn anime_buckets(state: &ExtensionState, prefs: &SourcePrefs) -> Buckets {
    buckets(state, &prefs.enabled_languages.get(), prefs.show_nsfw_source.get(), prefs.show_not_runnable.get())
}

/// The badge of every row of `buckets` that cannot run here (shown only while `show_not_runnable` is on), by package name.
pub fn badges_of(buckets: &Buckets) -> BTreeMap<String, RowBadges> {
    let mut badges = BTreeMap::new();
    for row in &buckets.available {
        let badge = RowBadges::of(row.ext.kind);
        if !badge.install_enabled() {
            badges.entry(row.ext.pkg.clone()).or_insert(badge);
        }
    }
    badges
}

/// What the extension pages read: the newest models of both kinds.
#[derive(Default)]
struct Latest {
    manga: Vec<Ext>,
    problems: Vec<(String, String)>,
    anime: Buckets,
}

thread_local! {
    static LATEST: RefCell<Latest> = RefCell::new(Latest::default());
}

/// The rows of the manga extension list, with their cached icons. Read by the Browse extension page when it mounts.
#[allow(dead_code)] // read by the Browse extension pages, which no screen mounts yet
pub fn manga_rows() -> Vec<Ext> {
    LATEST.with(|l| l.borrow().manga.clone())
}

/// The manga load problems as `(folder, message)`, for the list under the extensions.
#[allow(dead_code)] // read by the Browse extension pages, which no screen mounts yet
pub fn problems() -> Vec<(String, String)> {
    LATEST.with(|l| l.borrow().problems.clone())
}

/// The anime Browse lists as the last state change left them, built with the current preferences.
#[allow(dead_code)] // read by the anime Browse page, which no screen mounts yet
pub fn anime_lists() -> Buckets {
    LATEST.with(|l| l.borrow().anime.clone())
}

/// What a state change publishes: the first-run languages (a no-op once `source_languages` is saved), the Browse lists
/// built with the current preferences, and from them the badges and the per-store counts the pages read.
fn publish(state: &ExtensionState, prefs: &SourcePrefs) {
    if let Err(e) = enable_first_run_languages(prefs, state.available(), &system_locale()) {
        tracing::warn!("could not save the first-run languages: {e}");
    }
    let lists = anime_buckets(state, prefs);
    ext_badges::publish(badges_of(&lists));
    ext_badges::publish_store_counts(not_runnable_by_store(state, prefs.show_not_runnable.get()));
    LATEST.with(|l| l.borrow_mut().anime = lists);
}

/// The rows of the shared extension list for the manga packages in `snapshot`. Each row's icon is the cached `icon.png`
/// of its package when there is one, and none otherwise, which the list draws as the generic glyph.
pub fn manga_exts(snapshot: &Snapshot, icons: &Path) -> Vec<Ext> {
    let icon = |pkg: &str| cached_icon_file(icons, pkg);
    let installed = snapshot.installed.values().map(|ext| Ext {
        name: ext.name.clone(),
        pkg: ext.pkg.clone(),
        version: ext.version_code.to_string(),
        lang: Some(String::new()),
        nsfw: false,
        sources: Vec::new(),
        state: ExtState::Installed { has_update: ext.has_update, obsolete: ext.is_obsolete },
        icon_file: icon(&ext.pkg),
    });
    let untrusted = snapshot.untrusted.values().map(|ext| Ext {
        name: ext.pkg.clone(),
        pkg: ext.pkg.clone(),
        version: ext.version_code.to_string(),
        lang: None,
        nsfw: false,
        sources: Vec::new(),
        state: ExtState::Untrusted,
        icon_file: icon(&ext.pkg),
    });
    installed.chain(untrusted).collect()
}

/// The load problems of the manga packages as `(folder, message)` lines, in folder order.
pub fn problem_lines(snapshot: &Snapshot) -> Vec<(String, String)> {
    snapshot.problems.iter().map(|(file, message)| (file.clone(), message.clone())).collect()
}

/// The running host.
struct Host {
    registry: Arc<ExtensionRegistry>,
    state: Rc<RefCell<ExtensionState>>,
}

/// The manga models and problems after a scan of `snapshot`.
fn manga_models(snapshot: &Snapshot, icons: &Path) -> (Vec<Ext>, Vec<(String, String)>) {
    (manga_exts(snapshot, icons), problem_lines(snapshot))
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

/// The manga registry, once [`start`] has run.
pub fn registry() -> Option<Arc<ExtensionRegistry>> {
    HOST.with(|h| h.borrow().as_ref().map(|host| host.registry.clone()))
}

/// The number of anime packages with an update, derived from the state and never stored.
pub fn anime_update_count() -> usize {
    HOST.with(|h| h.borrow().as_ref().map_or(0, |host| host.state.borrow().update_count()))
}

fn store_rows(repo: &ExtensionStoreRepo) -> Vec<StoreRow> {
    repo.all()
        .unwrap_or_default()
        .into_iter()
        .map(|s| StoreRow {
            index_url: s.index_url,
            name: s.name,
            badge_label: s.badge_label,
            signing_key: s.signing_key,
            contact_website: s.contact_website,
            contact_discord: s.contact_discord,
            is_legacy: s.is_legacy,
            extension_list_url: s.extension_list_url,
        })
        .collect()
}

/// Starts the host: scans both package folders, pumps the anime events into the state, schedules the daily store check
/// and reloads the listing when the stores change. Call once from the GTK main thread inside the app's runtime.
pub fn start(store: &PrefStore, anime_db: Arc<kuro_db::handle::Db>) {
    if HOST.with(|h| h.borrow().is_some()) {
        return;
    }
    let paths = match kuro_platform::linux::LinuxPaths::system() {
        Ok(paths) => paths,
        Err(e) => {
            tracing::warn!("the extension host does not start: {e}");
            return;
        }
    };
    let prefs = SourcePrefs::new(store.clone());
    let handle = tokio::runtime::Handle::current();

    // manga packages: one scan, icons into the cache, problems kept by folder
    let registry = Arc::new(ExtensionRegistry::new());
    {
        let (registry, dir, icons) = (registry.clone(), paths.data_dir().join("extensions"), kuro_extensions::icon::icon_cache_dir(&paths));
        let (tx, rx) = async_channel::bounded(1);
        handle.spawn_blocking(move || {
            kuro_extensions::loader::scan_into(&registry, &dir, &BTreeMap::new(), &icons);
            let _ = tx.send_blocking(manga_models(&registry.snapshot(), &icons));
        });
        glib::spawn_future_local(async move {
            if let Ok((manga, problems)) = rx.recv().await {
                LATEST.with(|l| {
                    let mut latest = l.borrow_mut();
                    latest.manga = manga;
                    latest.problems = problems;
                });
            }
        });
    }

    // anime packages: the service reports through the channel, the state lives here
    let (events, inbox) = async_channel::unbounded();
    let service = Rc::new(AnimeExtensions::new(handle, events));
    let state = Rc::new(RefCell::new(ExtensionState::new()));
    let repo = ExtensionStoreRepo::new(anime_db);
    service.scan(
        PackageDir::from_paths(&paths),
        PackageDir::download_dir(&paths),
        TrustStore::new(prefs.trusted_extensions.clone()),
        store_rows(&repo),
        &prefs,
        IconCache::from_paths(&paths),
    );
    let apply_state = state.clone();
    let apply_prefs = SourcePrefs::new(store.clone());
    glib::spawn_future_local(pump(inbox, move |event| {
        let mut state = apply_state.borrow_mut();
        state.apply(event);
        publish(&state, &apply_prefs);
    }));

    // the daily check, and a refresh whenever the stores change
    match reqwest::Client::builder().user_agent(concat!("Kurobayomi/", env!("CARGO_PKG_VERSION"))).timeout(std::time::Duration::from_secs(30)).build() {
        Ok(client) => {
            let urls_repo = repo.clone();
            let urls: Arc<dyn Fn() -> Vec<String> + Send + Sync> = Arc::new(move || urls_repo.all().unwrap_or_default().into_iter().map(|s| s.index_url).collect());
            let cancel = CancellationToken::new();
            service.schedule_checks(client.clone(), urls.clone(), store.pref(LAST_CHECK_KEY, 0_i64), cancel);
            let refresh = service.clone();
            crate::pref_model::stores_page::on_extensions_reload(move || {
                refresh.refresh(client.clone(), urls(), CancellationToken::new());
            });
        }
        Err(e) => tracing::warn!("the extension store check does not run: {e}"),
    }
    HOST.with(|h| *h.borrow_mut() = Some(Host { registry, state }));
}

#[cfg(test)]
mod tests {
    use kuro_extensions::anime::abi::AbiVersion;
    use kuro_extensions::anime::model::{AnimeSourceRef, Available, Kind, StoreRef};
    use kuro_extensions::registry::{Installed, Untrusted};

    use super::*;

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

    fn state() -> ExtensionState {
        let mut state = ExtensionState::new();
        state.apply_available(vec![listed("eu.ok", Kind::Wasm, "en"), listed("eu.apk", Kind::ApkOnly, "en"), listed("eu.unsigned", Kind::Unsigned, "en")]);
        state
    }

    /// T-19a-066: the preference decides whether the not-runnable rows show, on each rebuild.
    #[test]
    fn show_not_runnable_hides_the_rows_by_default_and_shows_them_when_set() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        prefs.enabled_languages.set(["en".to_owned()].into()).unwrap();
        let state = state();
        let hidden = anime_buckets(&state, &prefs);
        assert_eq!(hidden.available.len(), 1);
        assert_eq!(hidden.not_runnable_hidden, 2);
        prefs.show_not_runnable.set(true).unwrap();
        let shown = anime_buckets(&state, &prefs);
        assert_eq!(shown.available.len(), 3);
        assert_eq!(shown.not_runnable_hidden, 0);
    }

    #[test]
    fn only_the_listings_that_cannot_run_carry_a_badge() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
    prefs.enabled_languages.set(["en".to_owned()].into()).unwrap();
    prefs.show_not_runnable.set(true).unwrap();
    let badges = badges_of(&anime_buckets(&state(), &prefs));
        assert_eq!(badges.len(), 2);
        assert_eq!(badges["eu.apk"], RowBadges { not_runnable: true, unsigned: false });
        assert_eq!(badges["eu.unsigned"], RowBadges { not_runnable: false, unsigned: true });
        assert!(!badges.contains_key("eu.ok"));
        // hidden rows carry no badge: nothing is shown for them
        prefs.show_not_runnable.set(false).unwrap();
        assert!(badges_of(&anime_buckets(&state(), &prefs)).is_empty());
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
        assert!(ext_badges::store_note_for("https://s/i.json").is_some_and(|n| n.starts_with("2 entries")));
        assert_eq!(anime_lists().not_runnable_hidden, 2);
        assert_eq!(ext_badges::of("eu.apk"), None, "hidden rows carry no badge");
        prefs.enabled_languages.set(["en".to_owned()].into()).unwrap();
        prefs.show_not_runnable.set(true).unwrap();
        publish(&state(), &prefs);
        assert_eq!(ext_badges::store_note_for("https://s/i.json"), None, "nothing is hidden when the flag is on");
        assert_eq!(ext_badges::of("eu.apk").map(|b| b.not_runnable), Some(true), "a shown row carries its badge");
        ext_badges::publish(BTreeMap::new());
        ext_badges::publish_store_counts(BTreeMap::new());
    }

    /// T-18a-011 and T-18a-054: the rows take the cached icon, a vanished package has none, and the problems list by folder.
    #[test]
    fn manga_rows_use_the_cached_icon_and_the_problems_list_by_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("eu.ok.png"), b"png").unwrap();
        let mut snapshot = Snapshot::default();
        snapshot.installed.insert(
            "eu.ok".into(),
            Installed { pkg: "eu.ok".into(), name: "Ok".into(), version_code: 3, lib_version: 1.0, has_update: true, is_obsolete: false },
        );
        snapshot.installed.insert(
            "eu.gone".into(),
            Installed { pkg: "eu.gone".into(), name: "Gone".into(), version_code: 1, lib_version: 1.0, has_update: false, is_obsolete: false },
        );
        snapshot.untrusted.insert("eu.u".into(), Untrusted { pkg: "eu.u".into(), version_code: 2 });
        snapshot.problems.insert("broken".into(), "component.wasm is missing".into());
        let rows = manga_exts(&snapshot, dir.path());
        let icon_of = |pkg: &str| rows.iter().find(|r| r.pkg == pkg).and_then(|r| r.icon_file.clone());
        assert_eq!(icon_of("eu.ok").map(std::path::PathBuf::from), Some(dir.path().join("eu.ok.png")));
        assert_eq!(icon_of("eu.gone"), None, "no cached icon: the generic glyph");
        assert!(matches!(rows.iter().find(|r| r.pkg == "eu.u").map(|r| &r.state), Some(ExtState::Untrusted)));
        assert_eq!(problem_lines(&snapshot), [("broken".to_owned(), "component.wasm is missing".to_owned())]);
    }
}
