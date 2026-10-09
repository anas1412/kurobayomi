//! Incognito state for sources (`GetMangaIncognitoState`, `GetAnimeIncognitoState`, `ToggleMangaIncognito`,
//! `ToggleAnimeIncognito`; T-10a-022). A source is incognito when the global flag is on, or when the extension
//! package that owns it is in the incognito set of its kind. An unknown source, or one whose extension is not
//! installed, is not incognito (EC-41).

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use kuro_prefs::pref::Pref;
use kuro_prefs::store::{Changed, PrefStore};
use tokio::sync::watch;

/// How often a watch looks at its package stream while no preference changes. The preference store is a std
/// channel, so a watch cannot wait on both at once; a package change is seen within this time.
const PACKAGE_POLL: Duration = Duration::from_millis(100);

/// Maps a source id to the extension package that owns it. Area 20 implements this. `None` means the source is
/// unknown or its extension is not installed.
pub trait SourcePackages: Send + Sync {
    fn package_of(&self, source_id: i64) -> Option<String>;

    /// The package of `source_id` as a stream (`ExtensionCatalog::watch_extension_package`). A new value is
    /// published whenever the installed extensions change.
    fn watch_extension_package(&self, source_id: i64) -> watch::Receiver<Option<String>>;
}

/// `is_incognito(global, pkg, set)` (T-17a-021): the global flag makes everything incognito; otherwise a source is
/// incognito when its extension package is in `set`. A source with no package (a stub, or the local source) is incognito
/// only through the global flag.
pub fn is_incognito(global: bool, pkg_name: Option<&str>, set: &BTreeSet<String>) -> bool {
    global || pkg_name.is_some_and(|pkg| set.contains(pkg))
}

/// Adds `pkg_name` to the set when `enable` is true, removes it otherwise. The name is stored as given, so a package
/// that is uninstalled keeps its entry (behaviour 5.2).
pub fn set_incognito_pkg(set: &mut BTreeSet<String>, pkg_name: &str, enable: bool) {
    if enable {
        set.insert(pkg_name.to_owned());
    } else {
        set.remove(pkg_name);
    }
}

/// The incognito state of one kind of source (manga or anime): the global flag, that kind's package set, and
/// the package lookup.
#[derive(Clone)]
pub struct IncognitoState {
    store: PrefStore,
    global: Pref<bool>,
    extensions: Pref<BTreeSet<String>>,
    packages: Arc<dyn SourcePackages>,
}

impl IncognitoState {
    pub fn new(
        store: PrefStore,
        global: Pref<bool>,
        extensions: Pref<BTreeSet<String>>,
        packages: Arc<dyn SourcePackages>,
    ) -> Self {
        Self {
            store,
            global,
            extensions,
            packages,
        }
    }

    /// `await(sourceId)`. `None` asks only about the global flag.
    pub fn is_incognito(&self, source_id: Option<i64>) -> bool {
        let pkg = source_id.and_then(|id| self.packages.package_of(id));
        is_incognito(self.global.get(), pkg.as_deref(), &self.extensions.get())
    }

    /// `ToggleMangaIncognito.await(extensionPackage, enable)`: adds or removes the package name, in one write.
    pub fn set_extension(&self, pkg_name: &str, enable: bool) {
        let mut set = self.extensions.get();
        set_incognito_pkg(&mut set, pkg_name, enable);
        self.extensions.set(set);
    }

    /// `subscribe(sourceId)`: a change stream that yields a value only when it differs from the last one.
    pub fn watch(&self, source_id: Option<i64>) -> IncognitoWatch {
        IncognitoWatch {
            last: self.is_incognito(source_id),
            changes: self.store.subscribe(),
            package: source_id.map(|id| self.packages.watch_extension_package(id)),
            state: self.clone(),
            source_id,
        }
    }
}

/// A subscription to one source's incognito value. It wakes on a preference change and on a package change, so a
/// source that gains or loses its extension is reported without any preference change.
pub struct IncognitoWatch {
    state: IncognitoState,
    source_id: Option<i64>,
    changes: Receiver<Changed>,
    package: Option<watch::Receiver<Option<String>>>,
    last: bool,
}

impl IncognitoWatch {
    /// The value now.
    pub fn current(&self) -> bool {
        self.state.is_incognito(self.source_id)
    }

    /// Blocks until the value differs from the last one returned, then returns it. `None` once the store is gone.
    pub fn next_change(&mut self) -> Option<bool> {
        loop {
            let relevant = match self.changes.recv_timeout(PACKAGE_POLL) {
                Ok(event) => event.key.as_deref().is_none_or(|key| {
                    key == self.state.global.key() || key == self.state.extensions.key()
                }),
                Err(RecvTimeoutError::Timeout) => false,
                Err(RecvTimeoutError::Disconnected) => return None,
            };
            let package_moved = self.package_moved();
            if !relevant && !package_moved {
                continue;
            }
            let value = self.current();
            if value != self.last {
                self.last = value;
                return Some(value);
            }
        }
    }

    /// Whether the package stream published since it was last read. A closed stream counts once, then is dropped.
    fn package_moved(&mut self) -> bool {
        let Some(rx) = self.package.as_mut() else {
            return false;
        };
        match rx.has_changed() {
            Ok(true) => {
                rx.borrow_and_update();
                true
            }
            Ok(false) => false,
            Err(_) => {
                self.package = None;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use kuro_prefs::source::SourcePrefs;
    use kuro_prefs::base::BasePrefs;
    use std::sync::Mutex;
    use std::sync::mpsc;
    use std::thread;

    /// Package lookups that tests can change. Every source shares one change stream, which is enough here.
    struct Packages {
        map: Mutex<HashMap<i64, String>>,
        signal: watch::Sender<Option<String>>,
    }

    impl Packages {
        fn new(packages: &[(i64, &str)]) -> Self {
            Self {
                map: Mutex::new(packages.iter().map(|(id, pkg)| (*id, pkg.to_string())).collect()),
                signal: watch::channel(None).0,
            }
        }

        /// Installs (`Some`) or removes (`None`) the package of `id`, and publishes the change.
        fn set(&self, id: i64, package: Option<&str>) {
            let mut map = self.map.lock().unwrap();
            match package {
                Some(pkg) => map.insert(id, pkg.to_owned()),
                None => map.remove(&id),
            };
            self.signal.send_replace(package.map(str::to_owned));
        }
    }

    impl SourcePackages for Packages {
        fn package_of(&self, source_id: i64) -> Option<String> {
            self.map.lock().unwrap().get(&source_id).cloned()
        }

        fn watch_extension_package(&self, _source_id: i64) -> watch::Receiver<Option<String>> {
            self.signal.subscribe()
        }
    }

    fn state(packages: &[(i64, &str)]) -> (IncognitoState, PrefStore) {
        state_over(Arc::new(Packages::new(packages)))
    }

    fn state_over(packages: Arc<Packages>) -> (IncognitoState, PrefStore) {
        let store = PrefStore::in_memory();
        let base = BasePrefs::new(store.clone());
        let sources = SourcePrefs::new(store.clone());
        let state = IncognitoState::new(
            store.clone(),
            base.incognito_mode.clone(),
            sources.incognito_manga_extensions.clone(),
            packages,
        );
        (state, store)
    }

    /// Runs `next_change` on another thread, so a change that never comes fails the test instead of hanging it.
    fn next_change_within(mut watch: IncognitoWatch) -> (IncognitoWatch, Option<bool>) {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let value = watch.next_change();
            let _ = tx.send((watch, value));
        });
        rx.recv_timeout(Duration::from_secs(5)).expect("a change within 5 s")
    }

    #[test]
    fn a_package_change_emits_without_any_preference_change() {
        let packages = Arc::new(Packages::new(&[]));
        let (state, _store) = state_over(packages.clone());
        state.set_extension("pkg.a", true);
        let watch = state.watch(Some(1));
        assert!(!watch.current(), "source 1 has no installed package yet");
        packages.set(1, Some("pkg.a"));
        let (watch, installed) = next_change_within(watch);
        assert_eq!(installed, Some(true));
        packages.set(1, None);
        let (_watch, removed) = next_change_within(watch);
        assert_eq!(removed, Some(false));
    }

    #[test]
    fn is_incognito_truth_table() {
        let set: BTreeSet<String> = ["eu.example.ext".to_owned()].into_iter().collect();
        let empty = BTreeSet::new();
        assert!(is_incognito(true, None, &empty), "global true gives true, even with no package");
        assert!(is_incognito(true, Some("other"), &empty));
        assert!(!is_incognito(false, None, &set), "null package gives false unless global");
        assert!(is_incognito(false, Some("eu.example.ext"), &set), "package in the set gives true");
        assert!(!is_incognito(false, Some("eu.other"), &set), "package not in the set gives false");
        assert!(!is_incognito(false, Some("eu.example.ext"), &empty));
    }

    #[test]
    fn set_incognito_pkg_adds_and_removes_the_package_name() {
        let mut set = BTreeSet::new();
        set_incognito_pkg(&mut set, "eu.example.ext", true);
        assert!(set.contains("eu.example.ext"));
        set_incognito_pkg(&mut set, "eu.example.ext", false);
        assert!(set.is_empty());
        set_incognito_pkg(&mut set, "eu.gone", false);
        assert!(set.is_empty(), "removing an absent name changes nothing");
    }

    #[test]
    fn the_global_flag_makes_every_source_incognito() {
        let (state, _store) = state(&[(1, "pkg.a")]);
        state.global.set(true);
        assert!(state.is_incognito(Some(1)));
        assert!(state.is_incognito(Some(99)));
        assert!(state.is_incognito(None));
    }

    #[test]
    fn no_source_id_without_the_flag_is_not_incognito() {
        let (state, _store) = state(&[(1, "pkg.a")]);
        state.extensions.set(BTreeSet::from(["pkg.a".to_owned()]));
        assert!(!state.is_incognito(None));
    }

    #[test]
    fn an_unknown_source_is_not_incognito_even_when_some_package_is_in_the_set() {
        let (state, _store) = state(&[(1, "pkg.a")]);
        state.extensions.set(BTreeSet::from(["pkg.a".to_owned(), "pkg.gone".to_owned()]));
        assert!(!state.is_incognito(Some(7)));
    }

    #[test]
    fn a_known_source_is_incognito_when_its_package_is_in_the_set() {
        let (state, _store) = state(&[(1, "pkg.a"), (2, "pkg.b")]);
        state.set_extension("pkg.a", true);
        assert!(state.is_incognito(Some(1)));
        assert!(!state.is_incognito(Some(2)));
    }

    #[test]
    fn toggling_adds_and_removes_the_package_name_in_the_store() {
        let (state, store) = state(&[(1, "pkg.a")]);
        state.set_extension("pkg.a", true);
        let sources = SourcePrefs::new(store.clone());
        assert!(sources.incognito_manga_extensions.get().contains("pkg.a"));
        state.set_extension("pkg.a", false);
        assert!(sources.incognito_manga_extensions.get().is_empty());
        assert!(!state.is_incognito(Some(1)));
    }

    #[test]
    fn watch_yields_the_new_value_after_a_relevant_change_only() {
        let (state, store) = state(&[(1, "pkg.a")]);
        let mut watch = state.watch(Some(1));
        assert!(!watch.current());
        let base = BasePrefs::new(store.clone());
        base.downloaded_only.set(true);
        base.incognito_mode.set(true);
        assert_eq!(watch.next_change(), Some(true));
    }
}
