//! The anime extension state the UI reads (T-19a-023 to T-19a-026, T-19a-030, `p1-rust-design.md` section 7).
//! The state is plain data. The GTK side owns it on the main thread and feeds it the events a worker sends. The
//! startup scan reads the package folder on a worker thread, so the main thread never waits for the disk.

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use async_channel::Sender;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use kuro_prefs::PrefError;

use crate::anime::icons::Icon;
use crate::anime::install::{InstallTracker, Ticket, uninstall};
use crate::anime::loader::{LoadContext, load_package};
use crate::anime::model::{AnimeSourceRef, Available, InstallStep, Installed, LoadResult, SourceError, Untrusted};
use crate::anime::store::refresh::Aggregate;
use crate::anime::store::rules::failed_store_banner;
use crate::anime::trust::TrustStore;

/// What a worker sends to the main thread. The main thread applies events in arrival order with
/// [`ExtensionState::apply`].
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// The startup scan finished: one result per package file (T-19a-024).
    Loaded(Vec<LoadResult>),
    /// A refresh finished with at least one readable store (T-19a-052, T-19a-057). It replaces the listing.
    Refreshed(Aggregate),
    /// `<pkg>.kuroext` and `<pkg>.signer` are deleted (T-19a-036). The state drops the package and its install step.
    Removed(String),
    /// The files of `pkg` could not be deleted, so the state keeps the package.
    RemoveFailed { pkg: String, message: String },
    /// One step of an install. The ticket tells the tracker which install it belongs to, so a step of an older install
    /// never overwrites a newer one (T-19a-032).
    Step { ticket: Ticket, step: InstallStep },
}

/// Runs `scan` on the worker runtime's blocking pool and sends its results as [`Event::Loaded`]. It returns as soon as
/// the task is queued, so the caller never waits for the scan. A dropped receiver only loses the result.
pub fn spawn_scan<F>(runtime: &Handle, scan: F, events: Sender<Event>) -> JoinHandle<()>
where
    F: FnOnce() -> Vec<LoadResult> + Send + 'static,
{
    runtime.spawn_blocking(move || {
        let results = scan();
        if events.send_blocking(Event::Loaded(results)).is_err() {
            tracing::debug!("the extension state is gone, so the startup scan result is dropped");
        }
    })
}

/// Deletes the files of `pkg` on a worker and sends [`Event::Removed`], or [`Event::RemoveFailed`] (T-19a-036). The
/// main thread then applies the event, which clears the package and its step.
pub fn spawn_uninstall(runtime: &Handle, package_dir: PathBuf, pkg: String, events: Sender<Event>) -> JoinHandle<()> {
    runtime.spawn_blocking(move || {
        let event = match uninstall(&package_dir, &pkg) {
            Ok(()) => Event::Removed(pkg),
            Err(err) => Event::RemoveFailed { pkg, message: err.to_string() },
        };
        if events.send_blocking(event).is_err() {
            tracing::debug!("the extension state is gone, so the uninstall result is dropped");
        }
    })
}

/// The extension state. Only the GTK main thread owns and mutates it (T-19a-023): the `Rc` marker makes it `!Send`, so a
/// worker can only send [`Event`]s, never reach the state.
#[derive(Debug, Default)]
pub struct ExtensionState {
    installed: BTreeMap<String, Installed>,
    untrusted: BTreeMap<String, Untrusted>,
    available: Vec<Available>,
    initialized: bool,
    installs: InstallTracker,
    banners: Vec<String>,
    _main_thread_only: PhantomData<Rc<()>>,
}

/// What a source id resolves to: an installed package's source, or a listed source that is not installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceHandle<'a> {
    Installed { pkg: &'a str, source: &'a AnimeSourceRef },
    Stub(&'a AnimeSourceRef),
}

impl SourceHandle<'_> {
    /// The name to show. A blank name shows the id.
    pub fn display_name(&self) -> String {
        let source = match self {
            SourceHandle::Installed { source, .. } | SourceHandle::Stub(source) => source,
        };
        if source.name.trim().is_empty() { source.id.to_string() } else { source.name.clone() }
    }

    /// The installed package and source. A stub answers `NotInstalled`.
    pub fn installed(&self) -> Result<(&str, &AnimeSourceRef), SourceError> {
        match *self {
            SourceHandle::Installed { pkg, source } => Ok((pkg, source)),
            SourceHandle::Stub(_) => Err(SourceError::NotInstalled),
        }
    }
}

impl ExtensionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// False until the first startup scan has been applied. Browse shows a spinner until then.
    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn installed(&self) -> &BTreeMap<String, Installed> {
        &self.installed
    }

    pub fn untrusted(&self) -> &BTreeMap<String, Untrusted> {
        &self.untrusted
    }

    pub fn available(&self) -> &[Available] {
        &self.available
    }

    /// The install steps, owned by the main thread with the rest of the state.
    pub fn installs(&self) -> &InstallTracker {
        &self.installs
    }

    pub fn installs_mut(&mut self) -> &mut InstallTracker {
        &mut self.installs
    }

    /// Applies one worker event. Events must be applied in the order they arrive.
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Loaded(results) => self.apply_loaded(results),
            Event::Refreshed(aggregate) => self.apply_refresh(aggregate),
            Event::Removed(pkg) => self.remove_package(&pkg),
            Event::RemoveFailed { pkg, message } => {
                tracing::error!(pkg = %pkg, "could not delete the extension files: {message}");
            }
            Event::Step { ticket, step } => self.installs.set_step(&ticket, step),
        }
    }

    /// A completed refresh (T-19a-057): the listing is replaced, and each failed store adds one banner text, which
    /// [`take_banners`](Self::take_banners) hands to the UI.
    pub fn apply_refresh(&mut self, aggregate: Aggregate) {
        self.banners.extend(aggregate.failures.iter().map(|failure| {
            let name = aggregate
                .stores
                .iter()
                .find(|store| store.index_url == failure.index_url)
                .map_or(failure.index_url.as_str(), |store| store.name.as_str());
            failed_store_banner(name)
        }));
        self.apply_available(aggregate.available);
    }

    /// The failure banners queued by refreshes since the last call.
    pub fn take_banners(&mut self) -> Vec<String> {
        std::mem::take(&mut self.banners)
    }

    /// The startup scan result. Successes are installed, untrusted packages are kept apart, and errors were already
    /// logged by the loader, so they are dropped here.
    pub fn apply_loaded(&mut self, results: Vec<LoadResult>) {
        self.installed.clear();
        self.untrusted.clear();
        for result in results {
            match result {
                LoadResult::Success(installed) => {
                    self.installed.insert(installed.pkg.clone(), installed);
                }
                LoadResult::Untrusted(untrusted) => {
                    self.untrusted.insert(untrusted.pkg.clone(), untrusted);
                }
                LoadResult::Error(_) => {}
            }
        }
        self.initialized = true;
        self.reconcile();
    }

    /// A completed refresh replaces the listing. A cancelled refresh never gets here, so the old list stays.
    pub fn apply_available(&mut self, available: Vec<Available>) {
        self.available = available;
        self.reconcile();
    }

    /// Reconciliation of the installed packages with the listing (`p1-rust-design.md` section 7.4). The first listing
    /// for a package wins. An empty listing changes nothing, as in Aniyomi.
    pub fn reconcile(&mut self) {
        if self.available.is_empty() {
            return;
        }
        let first = first_listing(&self.available);
        for installed in self.installed.values_mut() {
            match first.get(installed.pkg.as_str()) {
                None => {
                    installed.is_obsolete = true;
                    installed.has_update = false;
                }
                Some(listed) => {
                    installed.is_obsolete = false;
                    installed.has_update = listed.version_code > installed.version_code
                        || listed.abi.is_some_and(|abi| abi > installed.abi);
                    installed.store = Some(listed.store.clone());
                }
            }
        }
    }

    /// Installed packages with an update that are not obsolete. Zero while the listing is empty.
    pub fn update_count(&self) -> usize {
        if self.available.is_empty() {
            return 0;
        }
        self.installed.values().filter(|installed| installed.has_update && !installed.is_obsolete).count()
    }

    /// The first listing for `pkg`, the one an install uses.
    pub fn find_listing(&self, pkg: &str) -> Option<&Available> {
        self.available.iter().find(|listed| listed.pkg == pkg)
    }

    /// The icon of an installed package. A package that is gone has none.
    pub fn icon_for(&self, pkg: &str) -> Option<&Path> {
        self.installed.get(pkg).and_then(|installed| installed.icon.as_deref())
    }

    /// The icon to draw for `pkg` (T-19a-046): the cached file, else the listed `iconUrl`, else the generic icon.
    pub fn icon(&self, pkg: &str) -> Icon {
        if let Some(path) = self.icon_for(pkg) {
            return Icon::File(path.to_path_buf());
        }
        match self.find_listing(pkg) {
            Some(listed) if !listed.icon_url.trim().is_empty() => Icon::Remote(listed.icon_url.clone()),
            _ => Icon::Generic,
        }
    }

    /// Source lookup by id, with no panic on a miss. An installed source wins, then a listed one, then `None`.
    pub fn source_for_id(&self, id: i64) -> Option<SourceHandle<'_>> {
        for installed in self.installed.values() {
            if let Some(source) = installed.sources.iter().find(|source| source.id == id) {
                return Some(SourceHandle::Installed { pkg: &installed.pkg, source });
            }
        }
        self.available
            .iter()
            .flat_map(|listed| listed.sources.iter())
            .find(|source| source.id == id)
            .map(SourceHandle::Stub)
    }
}

/// Why a trust action did not leave the package installed.
#[derive(Debug, thiserror::Error)]
pub enum TrustError {
    #[error("{0} is not in the untrusted list")]
    NotUntrusted(String),
    #[error("the trust entry could not be saved: {0}")]
    Save(#[from] PrefError),
    #[error("the package is still untrusted after the reload")]
    StillUntrusted,
    #[error("the package could not be loaded after trusting it: {0}")]
    Load(String),
}

impl ExtensionState {
    /// Drops a package from the installed and untrusted maps and clears its install step once `uninstall` has removed
    /// its files (T-19a-036).
    pub fn remove_package(&mut self, pkg: &str) {
        self.installed.remove(pkg);
        self.untrusted.remove(pkg);
        self.installs.forget(pkg);
    }

    /// Trusts an untrusted package at its current version (T-19a-051): writes the per-version entry, reloads the file,
    /// and moves the package to installed. Then reconciliation runs, so `has_update` reflects the listing. A reload that
    /// is still untrusted keeps the package out of the installed map.
    pub fn trust(
        &mut self,
        pkg: &str,
        trust: &TrustStore,
        path: &Path,
        ctx: &LoadContext<'_>,
    ) -> Result<(), TrustError> {
        let untrusted = self.untrusted.get(pkg).cloned().ok_or_else(|| TrustError::NotUntrusted(pkg.to_owned()))?;
        trust.trust_version(pkg, untrusted.version_code, &untrusted.signer)?;
        match load_package(path, ctx) {
            LoadResult::Success(installed) => {
                self.untrusted.remove(pkg);
                self.installed.insert(pkg.to_owned(), installed);
                self.reconcile();
                Ok(())
            }
            LoadResult::Untrusted(still) => {
                self.untrusted.insert(pkg.to_owned(), still);
                Err(TrustError::StillUntrusted)
            }
            LoadResult::Error(error) => {
                self.untrusted.remove(pkg);
                Err(TrustError::Load(error.to_string()))
            }
        }
    }
}

/// The first listing of each package name, in list order.
fn first_listing(list: &[Available]) -> BTreeMap<&str, &Available> {
    let mut first = BTreeMap::new();
    for listed in list {
        first.entry(listed.pkg.as_str()).or_insert(listed);
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time check that [`ExtensionState`] is not `Send` (T-19a-023). If it became `Send`, the probe below would be
    /// ambiguous between the two impls and this module would not compile.
    trait AmbiguousIfSend<A> {
        fn probe() {}
    }
    impl<T: ?Sized> AmbiguousIfSend<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}

    #[test]
    fn the_extension_state_is_not_send() {
        <ExtensionState as AmbiguousIfSend<_>>::probe();
    }
    use crate::anime::icons::IconCache;
    use crate::anime::loader::PackageDir;
    use crate::anime::model::StoreRow;
    use kuro_prefs::source::SourcePrefs;
    use std::thread;
    use crate::anime::abi::AbiVersion;
    use crate::anime::model::{Kind, SignerDigest, StoreRef};
    use kuro_prefs::store::PrefStore;

    fn source(id: i64, name: &str) -> AnimeSourceRef {
        AnimeSourceRef { id, name: name.into(), lang: "en".into(), base_url: String::new() }
    }

    fn installed(pkg: &str, version_code: i64, sources: Vec<AnimeSourceRef>) -> Installed {
        Installed {
            name: pkg.into(),
            pkg: pkg.into(),
            version_name: "1.0.0".into(),
            version_code,
            abi: AbiVersion { major: 1, minor: 0 },
            lang: "en".into(),
            nsfw: false,
            torrent: false,
            sources,
            icon: None,
            signer: SignerDigest("ab".repeat(32)),
            has_update: false,
            is_obsolete: false,
            store: None,
        }
    }

    fn listed(pkg: &str, version_code: i64, store: &str, sources: Vec<AnimeSourceRef>) -> Available {
        Available {
            name: pkg.into(),
            raw_name: pkg.into(),
            pkg: pkg.into(),
            version_name: "1.0.0".into(),
            version_code,
            abi: None,
            aniyomi_lib: None,
            lang: "en".into(),
            nsfw: false,
            torrent: false,
            sources,
            kind: Kind::Wasm,
            apk_url: String::new(),
            icon_url: String::new(),
            kuroext: None,
            store: StoreRef { index_url: store.into(), name: store.into() },
        }
    }

    fn state_with(installed_list: Vec<Installed>) -> ExtensionState {
        let mut state = ExtensionState::new();
        state.apply_loaded(installed_list.into_iter().map(LoadResult::Success).collect());
        state
    }

    #[test]
    fn startup_loads_successes_and_untrusted_and_drops_errors() {
        let mut state = ExtensionState::new();
        assert!(!state.initialized());
        let untrusted = Untrusted {
            name: "U".into(),
            pkg: "p.u".into(),
            version_name: "1.0.0".into(),
            version_code: 1,
            abi: AbiVersion { major: 1, minor: 0 },
            nsfw: false,
            signer: SignerDigest("cd".repeat(32)),
        };
        state.apply_loaded(vec![
            LoadResult::Success(installed("p.a", 1, Vec::new())),
            LoadResult::Untrusted(untrusted),
            LoadResult::Error(crate::anime::model::LoadError::NotSigned("p.bad".into())),
        ]);
        assert!(state.initialized());
        assert_eq!(state.installed().keys().collect::<Vec<_>>(), ["p.a"]);
        assert_eq!(state.untrusted().keys().collect::<Vec<_>>(), ["p.u"]);
    }

    #[test]
    fn trusting_an_untrusted_package_moves_it_to_installed_and_reconciles_its_update() {
        use crate::anime::testkit::{MANIFEST, PKG, fixture_key, signed_package, trust_with_entries};
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = root.path().join(format!("{PKG}.kuroext"));
        std::fs::write(&path, &fixture.bytes).unwrap();
        std::fs::write(path.with_extension("signer"), &fixture.signer_hex).unwrap();
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &[], nsfw_allowed: true };
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![load_package(&path, &ctx)]);
        let version_code = state.untrusted()[PKG].version_code;
        state.apply_available(vec![listed(PKG, version_code + 1, "S", Vec::new())]);
        assert!(state.untrusted().contains_key(PKG));
        state.trust(PKG, &trust, &path, &ctx).unwrap();
        assert!(state.untrusted().is_empty());
        assert!(state.installed()[PKG].has_update, "reconciliation runs after the trust reload");
        assert!(matches!(state.trust(PKG, &trust, &path, &ctx), Err(TrustError::NotUntrusted(_))));
        state.remove_package(PKG);
        assert!(state.installed().is_empty());
    }

    #[test]
    fn tst_19_093_a_trust_whose_reload_is_still_untrusted_registers_nothing() {
        use crate::anime::testkit::{MANIFEST, PKG, fixture_key, other_key, signed_package, trust_with_entries};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(format!("{PKG}.kuroext"));
        let first = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        std::fs::write(&path, &first.bytes).unwrap();
        std::fs::write(path.with_extension("signer"), &first.signer_hex).unwrap();
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &[], nsfw_allowed: true };
        let mut state = ExtensionState::new();
        state.apply_loaded(vec![load_package(&path, &ctx)]);
        // The file changes signer between the scan and the trust, so the reload is still untrusted.
        let other = signed_package(&other_key(), MANIFEST, b"\0asm");
        std::fs::write(&path, &other.bytes).unwrap();
        std::fs::write(path.with_extension("signer"), &other.signer_hex).unwrap();
        let (result, logged) = crate::anime::testkit::capture(|| state.trust(PKG, &trust, &path, &ctx));
        assert!(matches!(result, Err(TrustError::StillUntrusted)));
        // E-TRU-02: the reload goes through the loader, which logs the untrusted text once (WARN, with the package).
        let text = crate::anime::logs::untrusted(PKG);
        let lines: Vec<_> = logged.iter().filter(|event| event.message == text).collect();
        assert_eq!(lines.len(), 1, "{logged:?}");
        assert_eq!(lines[0].level, tracing::Level::WARN);
        assert!(state.installed().get(PKG).is_none(), "nothing is registered as installed");
        assert!(state.untrusted().contains_key(PKG), "the package stays untrusted");
    }

    #[test]
    fn a_newer_version_code_marks_an_update_and_counts_it() {
        let mut state = state_with(vec![installed("p.a", 5, Vec::new()), installed("p.b", 5, Vec::new())]);
        state.apply_available(vec![listed("p.a", 6, "s", Vec::new()), listed("p.b", 5, "s", Vec::new())]);
        assert!(state.installed()["p.a"].has_update);
        assert!(!state.installed()["p.b"].has_update);
        assert_eq!(state.update_count(), 1);
        assert_eq!(state.installed()["p.a"].store.as_ref().unwrap().index_url, "s");
    }

    #[test]
    fn a_newer_abi_tuple_also_marks_an_update() {
        let mut state = state_with(vec![installed("p.a", 5, Vec::new())]);
        let mut newer = listed("p.a", 5, "s", Vec::new());
        newer.abi = Some(AbiVersion { major: 1, minor: 2 });
        state.apply_available(vec![newer]);
        assert!(state.installed()["p.a"].has_update);
    }

    #[test]
    fn a_package_that_disappears_is_obsolete_and_reappearing_clears_it() {
        let mut state = state_with(vec![installed("p.a", 1, Vec::new())]);
        state.apply_available(vec![listed("p.other", 1, "s", Vec::new())]);
        assert!(state.installed()["p.a"].is_obsolete);
        state.apply_available(vec![listed("p.a", 1, "s", Vec::new())]);
        assert!(!state.installed()["p.a"].is_obsolete, "found again, so no longer obsolete");
    }

    #[test]
    fn an_obsolete_package_with_a_newer_version_does_not_count_as_an_update() {
        let mut state = state_with(vec![installed("p.a", 1, Vec::new()), installed("p.b", 1, Vec::new())]);
        state.apply_available(vec![listed("p.b", 1, "s", Vec::new())]);
        state.installed.get_mut("p.a").unwrap().has_update = true;
        assert_eq!(state.update_count(), 0);
    }

    #[test]
    fn an_empty_listing_keeps_installed_unchanged_and_counts_zero() {
        let mut state = state_with(vec![installed("p.a", 1, Vec::new())]);
        state.apply_available(vec![listed("p.a", 2, "s", Vec::new())]);
        assert_eq!(state.update_count(), 1);
        state.apply_available(Vec::new());
        assert_eq!(state.update_count(), 0);
        assert!(state.installed().contains_key("p.a"), "the installed list is unchanged");
        assert!(!state.installed()["p.a"].is_obsolete);
    }

    #[test]
    fn duplicate_names_reconcile_against_the_first_listing() {
        let mut state = state_with(vec![installed("p.dup", 1, Vec::new())]);
        state.apply_available(vec![listed("p.dup", 2, "first", Vec::new()), listed("p.dup", 3, "second", Vec::new())]);
        assert_eq!(state.installed()["p.dup"].store.as_ref().unwrap().index_url, "first");
        assert_eq!(state.find_listing("p.dup").unwrap().version_code, 2);
    }

    #[test]
    fn removing_the_store_that_supplied_a_package_makes_it_obsolete() {
        let mut state = state_with(vec![installed("p.a", 1, Vec::new())]);
        state.apply_available(vec![listed("p.a", 1, "store-a", Vec::new()), listed("p.b", 1, "store-b", Vec::new())]);
        assert!(!state.installed()["p.a"].is_obsolete);
        state.apply_available(vec![listed("p.b", 1, "store-b", Vec::new())]);
        assert!(state.installed()["p.a"].is_obsolete);
    }

    #[test]
    fn an_installed_source_resolves_to_the_installed_handle() {
        let state = state_with(vec![installed("p.a", 1, vec![source(42, "Alpha")])]);
        let handle = state.source_for_id(42).expect("resolves");
        let (pkg, found) = handle.installed().expect("installed");
        assert_eq!((pkg, found.name.as_str()), ("p.a", "Alpha"));
    }

    #[test]
    fn a_listed_source_resolves_to_a_stub_that_refuses_calls() {
        let mut state = ExtensionState::new();
        state.apply_available(vec![listed("p.b", 1, "s", vec![source(7, "Beta")])]);
        let handle = state.source_for_id(7).expect("stub");
        assert!(matches!(handle, SourceHandle::Stub(_)));
        assert_eq!(handle.installed(), Err(SourceError::NotInstalled));
    }

    #[test]
    fn a_missing_source_id_is_none_and_a_blank_stub_name_shows_the_id() {
        let mut state = ExtensionState::new();
        state.apply_available(vec![listed("p.b", 1, "s", vec![source(9, "   ")])]);
        assert_eq!(state.source_for_id(404), None);
        assert_eq!(state.source_for_id(9).unwrap().display_name(), "9");
    }

    #[test]
    fn an_emptied_listing_leaves_no_stub_behind() {
        let mut state = ExtensionState::new();
        state.apply_available(vec![listed("p.b", 1, "s", vec![source(7, "Beta")])]);
        state.apply_available(Vec::new());
        assert_eq!(state.source_for_id(7), None);
    }

    #[test]
    fn the_icon_of_a_package_that_is_gone_is_none() {
        let state = ExtensionState::new();
        assert_eq!(state.icon_for("p.gone"), None);
    }

    // ---- the startup scan on a worker (T-19a-024, TST-19-201) ----

    use std::time::{Duration, Instant};

    use crate::anime::loader::SIGNER_EXT;
    use crate::anime::testkit::{MANIFEST, PKG, fixture_key, signed_package, signer_hex, trust_with_entries};

    /// A source file without its `#[cfg(test)]` part. Test modules sit at the end of each file.
    fn production_part(text: &str) -> &str {
        text.split("#[cfg(test)]").next().unwrap_or(text)
    }

    fn rust_files(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("src is readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                rust_files(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }

    #[test]
    fn tst_19_201_no_production_code_blocks_the_calling_thread_on_a_future() {
        let mut files = Vec::new();
        rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        assert!(files.len() > 20, "the walk found the crate sources");
        for file in files {
            let text = std::fs::read_to_string(&file).expect("source is utf-8");
            let code = production_part(&text);
            for needle in ["block_on", "recv_blocking"] {
                assert!(!code.contains(needle), "{} calls {needle}", file.display());
            }
        }
    }

    #[test]
    fn tst_19_201_the_main_thread_does_not_wait_for_the_scan() {
        let (events, inbox) = async_channel::unbounded();
        let caller = thread::current().id();
        let (seen_tx, seen_rx) = std::sync::mpsc::channel();
        let started = Instant::now();
        let runtime = crate::anime::runtime::build_runtime().unwrap();
        let worker = spawn_scan(
            runtime.handle(),
            move || {
                seen_tx.send(thread::current().id()).unwrap();
                thread::sleep(Duration::from_millis(600));
                Vec::new()
            },
            events,
        );
        assert!(started.elapsed() < Duration::from_millis(300), "starting the scan did not wait for it");
        assert!(matches!(inbox.try_recv(), Err(async_channel::TryRecvError::Empty)), "the scan is still running");
        let state = ExtensionState::new();
        assert!(!state.initialized(), "the browse list shows a spinner until Loaded arrives");

        let event = inbox.recv_blocking().expect("the worker sends Loaded");
        assert!(started.elapsed() >= Duration::from_millis(600), "the event came after the scan");
        assert!(matches!(event, Event::Loaded(ref results) if results.is_empty()));
        assert_ne!(seen_rx.recv().unwrap(), caller, "the scan ran on another thread");
        runtime.block_on(worker).unwrap();
    }

    #[test]
    fn the_startup_scan_sends_loaded_and_applying_it_installs_and_marks_the_state_initialized() {
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let package = root.path().join(format!("{PKG}.kuroext"));
        std::fs::write(&package, &fixture.bytes).unwrap();
        std::fs::write(package.with_extension(SIGNER_EXT), signer_hex(&fixture_key())).unwrap();
        std::fs::write(root.path().join("eu.example.broken.kuroext"), b"not a zip").unwrap();
        let store = StoreRow {
            index_url: "https://example.org/index.json".into(),
            name: "Example".into(),
            badge_label: "EX".into(),
            signing_key: signer_hex(&fixture_key()),
            contact_website: "https://example.org".into(),
            contact_discord: None,
            is_legacy: false,
            extension_list_url: None,
        };

        let (events, inbox) = async_channel::unbounded();
        let stale = root.path().join("dl");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("x-1.kuroext.part"), b"half").unwrap();
        let runtime = crate::anime::runtime::build_runtime().unwrap();
        let service = crate::anime::service::AnimeExtensions::new(runtime.handle().clone(), events);
        let worker = service.scan(
            PackageDir::new(root.path()),
            stale.clone(),
            trust_with_entries(&[]),
            vec![store],
            &SourcePrefs::new(PrefStore::in_memory()),
            IconCache::new(root.path().join("ext-icons")),
        );
        let event = inbox.recv_blocking().expect("Loaded arrives");
        runtime.block_on(worker).unwrap();

        let mut state = ExtensionState::new();
        state.apply(event);
        assert!(state.initialized());
        assert_eq!(state.installed().keys().collect::<Vec<_>>(), [PKG], "the broken package is logged and dropped");
        assert!(state.untrusted().is_empty());
        assert!(!stale.join("x-1.kuroext.part").exists(), "the scan cleans stale parts first");
    }

    #[test]
    fn tst_19_038_a_package_that_cannot_run_stays_installed_and_a_stub_refuses_calls() {
        use crate::anime::loader::load_package;
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"this is not a wasm component");
        let package = root.path().join(format!("{PKG}.kuroext"));
        std::fs::write(&package, &fixture.bytes).unwrap();
        std::fs::write(package.with_extension(SIGNER_EXT), signer_hex(&fixture_key())).unwrap();
        let store = StoreRow {
            index_url: "https://example.org/index.json".into(),
            name: "Example".into(),
            badge_label: "EX".into(),
            signing_key: signer_hex(&fixture_key()),
            contact_website: "https://example.org".into(),
            contact_discord: None,
            is_legacy: false,
            extension_list_url: None,
        };
        let trust = trust_with_entries(&[]);
        let loaded = load_package(&package, &LoadContext { trust: &trust, stores: &[store], nsfw_allowed: true });
        assert!(matches!(loaded, LoadResult::Success(_)), "discovery runs no package code: {loaded:?}");

        let mut state = ExtensionState::new();
        state.apply_loaded(vec![loaded]);
        let handle = state.source_for_id(crate::anime::testkit::SOURCE_ID).expect("the source resolves");
        assert!(handle.installed().is_ok(), "the package stays Success; a source fails on first use, in the host");
        state.apply_available(vec![listed("p.other", 1, "s", vec![source(7, "Beta")])]);
        assert_eq!(state.source_for_id(7).unwrap().installed(), Err(SourceError::NotInstalled));
    }

    #[test]
    fn a_scan_whose_receiver_is_gone_ends_quietly() {
        let (events, inbox) = async_channel::unbounded();
        drop(inbox);
        let runtime = crate::anime::runtime::build_runtime().unwrap();
        let worker = spawn_scan(runtime.handle(), Vec::new, events);
        runtime.block_on(worker).expect("the worker does not panic");
    }

    // ---- the NSFW gate is read at each scan (T-19a-007, TST-19-035) ----

    fn scan_once(root: &Path, trust: TrustStore, stores: Vec<StoreRow>, prefs: &SourcePrefs) -> ExtensionState {
        let (events, inbox) = async_channel::unbounded();
        let runtime = crate::anime::runtime::build_runtime().unwrap();
        let service = crate::anime::service::AnimeExtensions::new(runtime.handle().clone(), events);
        let worker =
            service.scan(PackageDir::new(root), root.join("dl"), trust, stores, prefs, IconCache::new(root.join("ext-icons")));
        let event = inbox.recv_blocking().expect("Loaded arrives");
        runtime.block_on(worker).unwrap();
        let mut state = ExtensionState::new();
        state.apply(event);
        state
    }

    fn example_store() -> StoreRow {
        StoreRow {
            index_url: "https://example.org/index.json".into(),
            name: "Example".into(),
            badge_label: "EX".into(),
            signing_key: signer_hex(&fixture_key()),
            contact_website: "https://example.org".into(),
            contact_discord: None,
            is_legacy: false,
            extension_list_url: None,
        }
    }

    #[test]
    fn tst_19_035_the_nsfw_preference_changed_between_two_scans_changes_the_result() {
        let root = tempfile::tempdir().unwrap();
        let manifest = MANIFEST.replace("\"nsfw\": false", "\"nsfw\": true");
        let fixture = signed_package(&fixture_key(), &manifest, b"\0asm");
        let package = root.path().join(format!("{PKG}.kuroext"));
        std::fs::write(&package, &fixture.bytes).unwrap();
        std::fs::write(package.with_extension(SIGNER_EXT), signer_hex(&fixture_key())).unwrap();
        let prefs = SourcePrefs::new(PrefStore::in_memory());

        prefs.show_nsfw_source.set(true).unwrap();
        let allowed = scan_once(root.path(), trust_with_entries(&[]), vec![example_store()], &prefs);
        assert_eq!(allowed.installed().keys().collect::<Vec<_>>(), [PKG]);

        prefs.show_nsfw_source.set(false).unwrap();
        let blocked = scan_once(root.path(), trust_with_entries(&[]), vec![example_store()], &prefs);
        assert!(blocked.installed().is_empty(), "the second scan reads the new value");
    }

    #[test]
    fn a_scan_extracts_the_package_icon_and_the_state_returns_its_path() {
        use crate::anime::testkit::build_zip;
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        // Re-pack the signed manifest and component with an icon; the signer sidecar is what the loader checks.
        let package = root.path().join(format!("{PKG}.kuroext"));
        std::fs::write(
            &package,
            build_zip(&[("manifest.json", MANIFEST.as_bytes()), ("source.wasm", b"\0asm"), ("icon.png", b"png")]),
        )
        .unwrap();
        std::fs::write(package.with_extension(SIGNER_EXT), &fixture.signer_hex).unwrap();
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        let mut stores = vec![example_store()];
        stores[0].signing_key = fixture.signer_hex.clone();
        let state = scan_once(root.path(), trust_with_entries(&[]), stores, &prefs);
        let icon = state.icon_for(PKG).expect("the icon was cached");
        assert_eq!(std::fs::read(icon).unwrap(), b"png");
        assert!(matches!(state.icon(PKG), Icon::File(_)));
    }

    // ---- uninstall (T-19a-036) ----

    #[test]
    fn uninstall_deletes_the_files_then_the_event_clears_the_package_and_its_step() {
        use crate::anime::model::InstallStep;
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(format!("{PKG}.kuroext")), b"x").unwrap();
        std::fs::write(root.path().join(format!("{PKG}.signer")), b"x").unwrap();
        let mut state = state_with(vec![installed(PKG, 1, Vec::new())]);
        let crate::anime::install::Begin::Started(ticket) = state.installs_mut().begin(PKG) else {
            panic!("a fresh install starts");
        };
        state.installs_mut().set_step(&ticket, InstallStep::Installed);
        assert_eq!(state.installs().step(PKG), Some(InstallStep::Installed));

        let (events, inbox) = async_channel::unbounded();
        let runtime = crate::anime::runtime::build_runtime().unwrap();
        runtime.block_on(spawn_uninstall(runtime.handle(), root.path().to_path_buf(), PKG.to_owned(), events)).unwrap();
        let event = inbox.recv_blocking().unwrap();
        assert!(matches!(event, Event::Removed(ref pkg) if pkg == PKG));
        state.apply(event);
        assert!(state.installed().is_empty());
        assert_eq!(state.installs().step(PKG), None, "the step is cleared");
        assert!(!root.path().join(format!("{PKG}.kuroext")).exists());
        assert!(!root.path().join(format!("{PKG}.signer")).exists());
    }

    #[test]
    fn a_failed_uninstall_keeps_the_package_in_the_state() {
        let (events, inbox) = async_channel::unbounded();
        let root = tempfile::tempdir().unwrap();
        let runtime = crate::anime::runtime::build_runtime().unwrap();
        runtime.block_on(spawn_uninstall(runtime.handle(), root.path().to_path_buf(), "../bad".into(), events)).unwrap();
        let event = inbox.recv_blocking().unwrap();
        assert!(matches!(event, Event::RemoveFailed { .. }));
        let mut state = state_with(vec![installed("p.a", 1, Vec::new())]);
        state.apply(event);
        assert_eq!(state.installed().len(), 1);
    }

    // ---- a refresh updates the list and queues one banner per failed store (T-19a-057) ----

    #[test]
    fn a_refresh_replaces_the_list_and_queues_one_banner_per_failed_store() {
        use crate::anime::store::refresh::StoreFailure;
        let mut state = state_with(vec![installed("p.a", 1, Vec::new())]);
        state.apply(Event::Refreshed(Aggregate {
            stores: vec![example_store()],
            available: vec![listed("p.a", 2, "https://example.org/index.json", Vec::new())],
            failures: vec![
                StoreFailure { index_url: "https://bad.test/index.json".into(), message: "HTTP 500".into() },
                StoreFailure { index_url: "https://example.org/index.json".into(), message: "x".into() },
            ],
        }));
        assert_eq!(state.update_count(), 1, "the Browse list updated");
        assert_eq!(
            state.take_banners(),
            ["Could not update store \"https://bad.test/index.json\"", "Could not update store \"Example\""]
        );
        assert!(state.take_banners().is_empty(), "taken once");
    }
}
