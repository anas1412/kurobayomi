//! The extension state the UI reads (`MangaExtensionManager.kt`, T-20a-002): installed, available and untrusted
//! packages, each keyed by package name. A change is published on a tokio watch channel, so the GTK thread reads a snapshot and
//! never waits for a writer.

use std::collections::BTreeMap;

use tokio::sync::watch;

/// An installed package and its status flags.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub pkg: String,
    pub name: String,
    pub version_code: i64,
    pub lib_version: f64,
    /// A repo lists a newer version code or a newer lib version (`updateExists`).
    pub has_update: bool,
    /// No repo lists the package any more. Set once and never cleared, as in Kotlin.
    pub is_obsolete: bool,
}

/// A package a repo lists.
#[derive(Debug, Clone, PartialEq)]
pub struct Available {
    pub pkg: String,
    pub version_code: i64,
    pub lib_version: f64,
}

/// A package that loaded but is not trusted (`LoadResult::Untrusted`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Untrusted {
    pub pkg: String,
    pub version_code: i64,
}

/// The whole state at one moment. Readers take a copy and work on it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub installed: BTreeMap<String, Installed>,
    pub available: BTreeMap<String, Available>,
    pub untrusted: BTreeMap<String, Untrusted>,
}

/// The writer side of the extension state. Each setter publishes the new snapshot.
pub struct ExtensionRegistry {
    tx: watch::Sender<Snapshot>,
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self { tx: watch::channel(Snapshot::default()).0 }
    }
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A receiver that wakes on every change.
    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.tx.subscribe()
    }

    /// A copy of the current state.
    pub fn snapshot(&self) -> Snapshot {
        self.tx.borrow().clone()
    }

    /// Replaces the installed packages, then recomputes their statuses against the available list.
    pub fn set_installed(&self, installed: Vec<Installed>) {
        self.tx.send_modify(|state| {
            state.installed = installed.into_iter().map(|ext| (ext.pkg.clone(), ext)).collect();
            update_statuses(&mut state.installed, &state.available);
        });
    }

    /// Replaces the available packages and recomputes the installed statuses (`updatedInstalledExtensionsStatuses`).
    pub fn set_available(&self, available: Vec<Available>) {
        self.tx.send_modify(|state| {
            state.available = available.into_iter().map(|ext| (ext.pkg.clone(), ext)).collect();
            update_statuses(&mut state.installed, &state.available);
        });
    }

    /// Replaces the untrusted packages.
    pub fn set_untrusted(&self, untrusted: Vec<Untrusted>) {
        self.tx.send_modify(|state| {
            state.untrusted = untrusted.into_iter().map(|ext| (ext.pkg.clone(), ext)).collect();
        });
    }

    /// The badge count: installed packages with an update. Zero while no repo lists anything, as in Kotlin.
    pub fn pending_updates_count(&self) -> usize {
        let state = self.tx.borrow();
        if state.available.is_empty() {
            return 0;
        }
        state.installed.values().filter(|ext| ext.has_update).count()
    }
}

/// The status rules of `updatedInstalledExtensionsStatuses`. With no available packages nothing changes. An installed
/// package that no repo lists becomes obsolete. A listed one gets `has_update` when its version code or lib version is
/// newer than the installed one.
fn update_statuses(installed: &mut BTreeMap<String, Installed>, available: &BTreeMap<String, Available>) {
    if available.is_empty() {
        return;
    }
    for (pkg, ext) in installed.iter_mut() {
        match available.get(pkg) {
            None => ext.is_obsolete = true,
            Some(listed) => {
                ext.has_update = listed.version_code > ext.version_code || listed.lib_version > ext.lib_version;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(pkg: &str, version_code: i64, lib_version: f64) -> Installed {
        Installed {
            pkg: pkg.into(),
            name: pkg.into(),
            version_code,
            lib_version,
            has_update: false,
            is_obsolete: false,
        }
    }

    fn available(pkg: &str, version_code: i64, lib_version: f64) -> Available {
        Available { pkg: pkg.into(), version_code, lib_version }
    }

    #[test]
    fn a_newer_version_code_or_a_newer_lib_version_marks_an_update() {
        let registry = ExtensionRegistry::new();
        registry.set_installed(vec![installed("a", 5, 1.0), installed("b", 5, 1.0), installed("c", 5, 1.0)]);
        registry.set_available(vec![available("a", 6, 1.0), available("b", 5, 1.5), available("c", 5, 1.0)]);
        let state = registry.snapshot();
        assert!(state.installed["a"].has_update, "a newer version code");
        assert!(state.installed["b"].has_update, "a newer lib version");
        assert!(!state.installed["c"].has_update, "the same version is not an update");
        assert_eq!(registry.pending_updates_count(), 2);
    }

    #[test]
    fn an_installed_package_no_repo_lists_becomes_obsolete_and_stays_so() {
        let registry = ExtensionRegistry::new();
        registry.set_installed(vec![installed("gone", 1, 1.0)]);
        registry.set_available(vec![available("other", 1, 1.0)]);
        assert!(registry.snapshot().installed["gone"].is_obsolete);
        registry.set_available(vec![available("gone", 1, 1.0)]);
        assert!(registry.snapshot().installed["gone"].is_obsolete, "Kotlin never clears the flag here");
    }

    #[test]
    fn with_no_available_packages_nothing_changes_and_the_count_is_zero() {
        let registry = ExtensionRegistry::new();
        registry.set_installed(vec![installed("a", 1, 1.0)]);
        registry.set_available(Vec::new());
        let state = registry.snapshot();
        assert!(!state.installed["a"].is_obsolete && !state.installed["a"].has_update);
        assert_eq!(registry.pending_updates_count(), 0);
    }

    #[test]
    fn the_untrusted_map_is_keyed_by_package_name_and_replaced_as_a_whole() {
        let registry = ExtensionRegistry::new();
        registry.set_untrusted(vec![Untrusted { pkg: "x".into(), version_code: 2 }]);
        registry.set_untrusted(vec![Untrusted { pkg: "y".into(), version_code: 3 }]);
        let state = registry.snapshot();
        assert_eq!(state.untrusted.keys().collect::<Vec<_>>(), ["y"]);
        assert_eq!(state.untrusted["y"].version_code, 3);
    }

    #[tokio::test]
    async fn a_change_wakes_the_subscriber_with_the_new_snapshot() {
        let registry = ExtensionRegistry::new();
        let mut rx = registry.subscribe();
        registry.set_installed(vec![installed("a", 1, 1.0)]);
        assert!(rx.has_changed().expect("sender alive"));
        assert!(rx.borrow_and_update().installed.contains_key("a"));
    }
}
