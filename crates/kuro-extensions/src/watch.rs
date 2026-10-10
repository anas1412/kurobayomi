//! Hot reload of the manga package folders (T-20a-040). CRATE-MAP C-21 keeps the `notify` crate out of P1, so there is
//! no inotify watch: [`Watcher::run`] rescans on a timer, which is the recorded deviation from the plan's inotify bullet
//! (a `notify` source can feed [`Debouncer`] later without changing anything else). A rescan fingerprints each package
//! folder and diffs the fingerprints against the previous scan. A [`Debouncer`] holds the changes until the folder has been quiet for
//! [`DEBOUNCE`], so one install (which writes the component, then the manifest) reloads once. A package folder's name is
//! its package name, which `install::install_package` enforces when it writes the folder.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kuro_core::hash::sha256_hex;

use crate::icon;
use crate::loader::{COMPONENT_FILE, LoadResult, MANIFEST_FILE, is_hidden, load_package};
use crate::registry::{ExtensionRegistry, Installed, Untrusted};

/// The quiet time a burst of changes must have before it is applied.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// One package folder that changed since the previous scan, by folder name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A folder appeared.
    Added(String),
    /// A folder's manifest or component changed.
    Replaced(String),
    /// A folder is gone.
    Removed(String),
}

impl Change {
    pub fn folder(&self) -> &str {
        match self {
            Self::Added(folder) | Self::Replaced(folder) | Self::Removed(folder) => folder,
        }
    }
}

/// The fingerprint of each package folder, by folder name: the digests of its manifest and component.
pub type Fingerprints = BTreeMap<String, String>;

/// Fingerprints the package folders under `extensions_dir`, the folders `loader::discover` lists. A folder with no
/// component fingerprints differently from one with a component.
pub fn fingerprints(extensions_dir: &Path) -> Fingerprints {
    let Ok(entries) = fs::read_dir(extensions_dir) else {
        return Fingerprints::new();
    };
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir() && !is_hidden(path) && path.join(MANIFEST_FILE).is_file())
        .map(|path| {
            let manifest = fs::read(path.join(MANIFEST_FILE)).unwrap_or_default();
            let component = match fs::read(path.join(COMPONENT_FILE)) {
                Ok(bytes) => sha256_hex(&bytes),
                Err(_) => "-".to_owned(),
            };
            let folder = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            (folder, format!("{}/{component}", sha256_hex(&manifest)))
        })
        .collect()
}

/// What changed between two scans. Folders are compared by name, in name order.
pub fn diff(old: &Fingerprints, new: &Fingerprints) -> Vec<Change> {
    let mut changes = Vec::new();
    for (folder, print) in new {
        match old.get(folder) {
            None => changes.push(Change::Added(folder.clone())),
            Some(before) if before != print => changes.push(Change::Replaced(folder.clone())),
            Some(_) => {}
        }
    }
    changes.extend(old.keys().filter(|folder| !new.contains_key(*folder)).map(|folder| Change::Removed(folder.clone())));
    changes
}

/// Holds changes until the folders have been quiet for [`DEBOUNCE`]. A later change to a folder replaces the earlier one,
/// except that a removal followed by an add reads as a replacement, and an add followed by a replacement stays an add,
/// since the UI never saw the first version.
#[derive(Debug, Default)]
pub struct Debouncer {
    pending: BTreeMap<String, Change>,
    last_event: Option<Instant>,
}

impl Debouncer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the changes seen at `now` and restarts the quiet period. An empty list changes nothing.
    pub fn push(&mut self, changes: Vec<Change>, now: Instant) {
        if changes.is_empty() {
            return;
        }
        for change in changes {
            let folder = change.folder().to_owned();
            let merged = match (self.pending.remove(&folder), change) {
                (None, change) => change,
                (Some(Change::Removed(_)), Change::Added(name) | Change::Replaced(name)) => Change::Replaced(name),
                (Some(Change::Replaced(name)), Change::Added(_)) => Change::Replaced(name),
                (Some(Change::Added(name)), Change::Replaced(_)) => Change::Added(name),
                (_, change) => change,
            };
            self.pending.insert(folder, merged);
        }
        self.last_event = Some(now);
    }

    /// The coalesced changes, once [`DEBOUNCE`] has passed since the last change. Taking them clears the debouncer.
    pub fn take_ready(&mut self, now: Instant) -> Option<Vec<Change>> {
        let last = self.last_event?;
        if now.saturating_duration_since(last) < DEBOUNCE {
            return None;
        }
        self.last_event = None;
        Some(std::mem::take(&mut self.pending).into_values().collect())
    }
}

/// Applies one coalesced change to `registry`, which has a single writer. An added or replaced folder is loaded alone and
/// its icon is copied. A removed folder leaves the installed and untrusted maps and loses its problem. Every other package
/// keeps its entry, including its obsolete flag.
pub fn apply(
    registry: &ExtensionRegistry,
    extensions_dir: &Path,
    change: &Change,
    signed: &BTreeMap<String, String>,
    icons: &Path,
) {
    let folder = change.folder();
    let dir = extensions_dir.join(folder);
    let snapshot = registry.snapshot();
    let mut installed: Vec<Installed> = snapshot.installed.values().filter(|ext| ext.pkg != folder).cloned().collect();
    let mut untrusted: Vec<Untrusted> = snapshot.untrusted.values().filter(|ext| ext.pkg != folder).cloned().collect();
    match change {
        Change::Removed(_) => registry.clear_problem(folder),
        Change::Added(_) | Change::Replaced(_) => match load_package(&dir, signed) {
            LoadResult::Verified { manifest, .. } => {
                icon::extract(icons, &dir, &manifest.pkg);
                registry.clear_problem(folder);
                installed.push(Installed {
                    pkg: manifest.pkg.clone(),
                    name: manifest.name.clone(),
                    version_code: manifest.version_code,
                    lib_version: manifest.api.parse().unwrap_or(0.0),
                    has_update: false,
                    is_obsolete: false,
                });
            }
            LoadResult::Untrusted { manifest, .. } => {
                icon::extract(icons, &dir, &manifest.pkg);
                registry.clear_problem(folder);
                untrusted.push(Untrusted { pkg: manifest.pkg.clone(), version_code: manifest.version_code });
            }
            LoadResult::Error { error, .. } => registry.set_problem(folder, &error),
        },
    }
    registry.set_installed(installed);
    registry.set_untrusted(untrusted);
}

/// How often [`Watcher::run`] rescans. The 300 ms debounce is checked at this resolution.
pub const POLL: Duration = Duration::from_millis(500);

/// The hot-reload loop: keeps the last fingerprints and a [`Debouncer`], and applies each settled change to the registry.
/// Start it after the first `loader::scan_into`, so the folders already loaded are not reported again.
pub struct Watcher {
    dir: PathBuf,
    signed: BTreeMap<String, String>,
    icons: PathBuf,
    seen: Fingerprints,
    debouncer: Debouncer,
}

impl Watcher {
    pub fn new(dir: PathBuf, signed: BTreeMap<String, String>, icons: PathBuf) -> Self {
        let seen = fingerprints(&dir);
        Self { dir, signed, icons, seen, debouncer: Debouncer::new() }
    }

    /// One rescan at `now`: records what changed since the last one, then applies the changes that have been quiet for
    /// [`DEBOUNCE`]. Returns the applied changes.
    pub fn tick(&mut self, registry: &ExtensionRegistry, now: Instant) -> Vec<Change> {
        let current = fingerprints(&self.dir);
        self.debouncer.push(diff(&self.seen, &current), now);
        self.seen = current;
        let ready = self.debouncer.take_ready(now).unwrap_or_default();
        for change in &ready {
            apply(registry, &self.dir, change, &self.signed, &self.icons);
        }
        ready
    }

    /// Rescans every `every` until the task is dropped. Each scan reads files, so it runs on a blocking thread.
    pub async fn run(mut self, registry: Arc<ExtensionRegistry>, every: Duration) {
        let mut timer = tokio::time::interval(every);
        loop {
            timer.tick().await;
            let registry = registry.clone();
            let moved = tokio::task::spawn_blocking(move || {
                self.tick(&registry, Instant::now());
                self
            })
            .await;
            match moved {
                Ok(watcher) => self = watcher,
                Err(_) => return,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PKG: &str = "app.kurobayomi.ext.en.example";

    /// Writes one package folder called `folder` with a valid manifest and `component`.
    fn write_package(root: &Path, folder: &str, component: &[u8]) -> std::path::PathBuf {
        let dir = root.join(folder);
        fs::create_dir_all(&dir).unwrap();
        let manifest = format!("pkg = \"{PKG}\"\nname = \"Example\"\nversion = \"1.0.0\"\nversion_code = 1\napi = \"1.0\"\n");
        fs::write(dir.join(MANIFEST_FILE), manifest).unwrap();
        fs::write(dir.join(COMPONENT_FILE), component).unwrap();
        dir
    }

    #[test]
    fn a_new_changed_or_missing_folder_is_added_replaced_or_removed_and_hidden_folders_are_ignored() {
        let root = tempfile::tempdir().unwrap();
        let empty = fingerprints(root.path());
        write_package(root.path(), PKG, b"v1");
        write_package(root.path(), ".tmp", b"download");
        let first = fingerprints(root.path());
        assert!(!first.contains_key(".tmp"), "the download folder is not a package");
        assert_eq!(diff(&empty, &first), [Change::Added(PKG.into())]);

        write_package(root.path(), PKG, b"v2");
        let second = fingerprints(root.path());
        assert_eq!(diff(&first, &second), [Change::Replaced(PKG.into())]);

        fs::remove_dir_all(root.path().join(PKG)).unwrap();
        assert_eq!(diff(&second, &fingerprints(root.path())), [Change::Removed(PKG.into())]);
        assert!(diff(&second, &second).is_empty(), "an unchanged folder is no change");
    }

    #[test]
    fn a_burst_waits_for_the_quiet_period_and_coalesces_per_folder() {
        let start = Instant::now();
        let mut debouncer = Debouncer::new();
        assert_eq!(debouncer.take_ready(start), None, "nothing is pending");
        debouncer.push(vec![Change::Removed(PKG.into())], start);
        debouncer.push(vec![Change::Added(PKG.into())], start + Duration::from_millis(100));
        assert_eq!(debouncer.take_ready(start + Duration::from_millis(300)), None, "the last change is 200 ms old");
        assert_eq!(
            debouncer.take_ready(start + Duration::from_millis(400)),
            Some(vec![Change::Replaced(PKG.into())]),
            "a removal then an add is one replacement"
        );
        assert_eq!(debouncer.take_ready(start + Duration::from_secs(5)), None, "taking clears the debouncer");
    }

    #[test]
    fn an_add_followed_by_a_replacement_stays_an_add() {
        let start = Instant::now();
        let mut debouncer = Debouncer::new();
        debouncer.push(vec![Change::Added(PKG.into())], start);
        debouncer.push(vec![Change::Replaced(PKG.into())], start);
        assert_eq!(debouncer.take_ready(start + DEBOUNCE), Some(vec![Change::Added(PKG.into())]));
    }

    #[test]
    fn an_added_package_is_listed_then_a_replaced_one_that_no_longer_matches_moves_to_untrusted() {
        let root = tempfile::tempdir().unwrap();
        let icons = root.path().join("icons");
        let registry = ExtensionRegistry::new();
        let signed = BTreeMap::from([(PKG.to_owned(), sha256_hex(b"v1"))]);
        write_package(root.path(), PKG, b"v1");
        apply(&registry, root.path(), &Change::Added(PKG.into()), &signed, &icons);
        assert!(registry.snapshot().installed.contains_key(PKG));

        write_package(root.path(), PKG, b"v2");
        apply(&registry, root.path(), &Change::Replaced(PKG.into()), &signed, &icons);
        let state = registry.snapshot();
        assert!(!state.installed.contains_key(PKG), "the digest no longer matches the repo");
        assert!(state.untrusted.contains_key(PKG));
    }

    #[test]
    fn a_replaced_folder_that_fails_to_load_keeps_its_problem_and_a_removed_one_leaves_the_state() {
        let root = tempfile::tempdir().unwrap();
        let icons = root.path().join("icons");
        let registry = ExtensionRegistry::new();
        let signed = BTreeMap::from([(PKG.to_owned(), sha256_hex(b"v1"))]);
        let dir = write_package(root.path(), PKG, b"v1");
        apply(&registry, root.path(), &Change::Added(PKG.into()), &signed, &icons);

        fs::remove_file(dir.join(COMPONENT_FILE)).unwrap();
        apply(&registry, root.path(), &Change::Replaced(PKG.into()), &signed, &icons);
        let state = registry.snapshot();
        assert!(state.problems.contains_key(PKG), "a missing component is a problem");
        assert!(!state.installed.contains_key(PKG));

        fs::remove_dir_all(&dir).unwrap();
        apply(&registry, root.path(), &Change::Removed(PKG.into()), &signed, &icons);
        assert!(registry.snapshot().installed.is_empty());
    }

    #[test]
    fn the_watcher_reloads_one_package_once_the_folder_has_been_quiet_for_the_debounce() {
        let root = tempfile::tempdir().unwrap();
        let icons = root.path().join("icons");
        let exts = root.path().join("exts");
        fs::create_dir_all(&exts).unwrap();
        let registry = ExtensionRegistry::new();
        let signed = BTreeMap::from([(PKG.to_owned(), sha256_hex(b"v1"))]);
        let mut watcher = Watcher::new(exts.clone(), signed, icons);
        let start = Instant::now();
        assert!(watcher.tick(&registry, start).is_empty(), "nothing changed");

        write_package(&exts, PKG, b"v1");
        assert!(watcher.tick(&registry, start).is_empty(), "the add waits for the quiet period");
        assert!(registry.snapshot().installed.is_empty());
        assert_eq!(watcher.tick(&registry, start + DEBOUNCE), [Change::Added(PKG.into())]);
        assert!(registry.snapshot().installed.contains_key(PKG));

        write_package(&exts, PKG, b"v2");
        let later = start + Duration::from_secs(1);
        assert!(watcher.tick(&registry, later).is_empty());
        assert_eq!(watcher.tick(&registry, later + DEBOUNCE), [Change::Replaced(PKG.into())]);
        assert!(registry.snapshot().untrusted.contains_key(PKG), "the new digest is not the signed one");

        fs::remove_dir_all(exts.join(PKG)).unwrap();
        let end = later + Duration::from_secs(1);
        watcher.tick(&registry, end);
        assert_eq!(watcher.tick(&registry, end + DEBOUNCE), [Change::Removed(PKG.into())]);
        assert!(registry.snapshot().untrusted.is_empty());
    }

    #[tokio::test]
    async fn run_applies_a_package_that_appears_while_it_polls() {
        let root = tempfile::tempdir().unwrap();
        let exts = root.path().join("exts");
        fs::create_dir_all(&exts).unwrap();
        let registry = Arc::new(ExtensionRegistry::new());
        let signed = BTreeMap::from([(PKG.to_owned(), sha256_hex(b"v1"))]);
        let watcher = Watcher::new(exts.clone(), signed, root.path().join("icons"));
        let task = tokio::spawn(watcher.run(registry.clone(), Duration::from_millis(50)));
        write_package(&exts, PKG, b"v1");
        let mut changes = registry.subscribe();
        tokio::time::timeout(Duration::from_secs(5), changes.wait_for(|state| state.installed.contains_key(PKG)))
            .await
            .expect("the package is picked up")
            .unwrap();
        task.abort();
    }
}
