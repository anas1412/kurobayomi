//! One download index for both kinds: scan, hourly renew, debounced snapshot and queries (merges `index.rs`, the manga
//! `MangaDownloadCache` port, and `anime/cache.rs`, the anime one).
//!
//! The index is an in-memory tree per kind, `source id -> entry folder -> unit names`. It is a cache, not the truth: the
//! user may delete folders at any time. Readers clone an `Arc` of the tree, so a listing never waits for a write. A tree
//! is trusted for one hour after a scan or a snapshot load. A failed source wait keeps the previous tree (Kotlin
//! replaces it with an empty one, a defect). Every change schedules one write of [`SNAPSHOT_FILE`], which holds both
//! trees and both roots, one second after the last change; an old manga or anime snapshot file is simply never read.
//!
//! Unit names are folder names, or file names without the kind's extension (see `naming::is_unit_entry`). Staging
//! folders (`_tmp`) and folders with no installed source are left out. The local source is not indexed: the caller
//! counts it from disk (`local.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kuro_platform::kind::Kind;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::time::Instant;

use crate::naming::{UnitName, bare_unit_name, entry_dir_name, fat_safe, is_unit_entry, unit_dir_stems};
use crate::watch::{RootWatch, watch_roots};

/// The index is trusted for this long (`Cache.kt` renewInterval).
pub const RENEW_INTERVAL_MS: i64 = 3_600_000;
/// Longest wait for the source registry before a renew gives up and keeps the old tree.
pub const SOURCE_WAIT: Duration = Duration::from_secs(30);
/// Snapshot write delay after the last change.
pub const SNAPSHOT_DEBOUNCE: Duration = Duration::from_secs(1);
/// Snapshot file in the cache directory. Version 2 of the format: both kinds in one file.
pub const SNAPSHOT_FILE: &str = "download-index-v2.bin";
const SNAPSHOT_MAGIC: &[u8] = b"KIDX\x02";
/// A scan that runs longer than this shows the "Checking downloads" indicator (EC-80, T-23a-040).
pub const INDICATOR_DELAY: Duration = Duration::from_secs(1);

/// `source id -> source folder`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Tree {
    pub sources: BTreeMap<i64, SourceDir>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SourceDir {
    pub dir_name: String,
    /// Keyed by the entry folder name.
    pub entries: BTreeMap<String, EntryDir>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EntryDir {
    /// The source folder this entry lives in. Two folders can match one source when they differ only by case.
    pub source_dir: String,
    pub dir_name: String,
    /// Bare unit names: the folder name, or the file name without its extension.
    pub units: BTreeSet<String>,
}

/// The downloads root of each kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexRoots {
    pub manga: PathBuf,
    pub anime: PathBuf,
}

impl IndexRoots {
    pub fn of(&self, kind: Kind) -> &Path {
        match kind {
            Kind::Manga => &self.manga,
            Kind::Anime => &self.anime,
        }
    }
}

/// The position of a kind in the per-kind arrays.
fn slot(kind: Kind) -> usize {
    match kind {
        Kind::Manga => 0,
        Kind::Anime => 1,
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[derive(Serialize, Deserialize)]
struct KindSnapshot {
    root: PathBuf,
    /// When the tree was scanned. `None`: stale, the next renew rescans.
    built_at_ms: Option<i64>,
    tree: Tree,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    manga: KindSnapshot,
    anime: KindSnapshot,
}

#[derive(Default)]
struct State {
    trees: [Arc<Tree>; 2],
    last_renew_ms: [Option<i64>; 2],
}

struct Shared {
    roots: IndexRoots,
    snapshot_path: PathBuf,
    state: RwLock<State>,
    scanning: [AtomicBool; 2],
    /// When the running scan of each kind began. `None` when no scan runs (T-23a-040).
    scan_started: Mutex<[Option<Instant>; 2]>,
    generation: AtomicU64,
    changes: watch::Sender<u64>,
}

/// Cheap to clone. All methods take `&self`.
#[derive(Clone)]
pub struct DownloadIndex {
    shared: Arc<Shared>,
}

impl DownloadIndex {
    /// Creates the index for `roots`. The snapshot in `cache_dir` is loaded for each kind whose root matches. A corrupt
    /// file, or one written for other roots, is deleted.
    pub fn open(roots: IndexRoots, cache_dir: &Path) -> Self {
        let snapshot_path = cache_dir.join(SNAPSHOT_FILE);
        let state = load_snapshot(&snapshot_path, &roots);
        Self {
            shared: Arc::new(Shared {
                roots,
                snapshot_path,
                state: RwLock::new(state),
                scanning: [AtomicBool::new(false), AtomicBool::new(false)],
                scan_started: Mutex::new([None, None]),
                generation: AtomicU64::new(0),
                changes: watch::channel(0).0,
            }),
        }
    }

    pub fn root(&self, kind: Kind) -> &Path {
        self.shared.roots.of(kind)
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.shared.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.shared.state.write().unwrap_or_else(|e| e.into_inner())
    }

    /// A receiver that counts the changes. It changes on every mutation and after each scan.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.shared.changes.subscribe()
    }

    /// A snapshot of one kind's tree. Later writes do not affect it (E-D07).
    pub fn tree(&self, kind: Kind) -> Arc<Tree> {
        self.read().trees[slot(kind)].clone()
    }

    /// When the tree was last scanned or loaded from a snapshot. `None` after [`Self::invalidate`] or before any scan.
    pub fn last_renew_ms(&self, kind: Kind) -> Option<i64> {
        self.read().last_renew_ms[slot(kind)]
    }

    /// How long the longest running scan has been going, `None` when no scan runs (T-23a-040).
    pub fn scan_elapsed(&self) -> Option<Duration> {
        let started = *self.shared.scan_started.lock().unwrap_or_else(|e| e.into_inner());
        started.iter().flatten().map(|t| t.elapsed()).max()
    }

    /// Whether the "Checking downloads" indicator should show now (T-23a-040).
    pub fn checking_indicator_visible(&self) -> bool {
        checking_indicator_visible(self.scan_elapsed())
    }

    /// `true` when a folder or file for the unit is in the index, under its current or its legacy name.
    pub fn is_downloaded(
        &self,
        kind: Kind,
        source_id: i64,
        entry_title: &str,
        unit_name: &str,
        scanlator: Option<&str>,
    ) -> bool {
        let tree = self.tree(kind);
        let Some(entry) = tree.sources.get(&source_id).and_then(|s| s.entries.get(&entry_dir_name(entry_title))) else {
            return false;
        };
        unit_dir_stems(kind, UnitName::new(unit_name, scanlator)).iter().any(|n| entry.units.contains(n))
    }

    /// Units in every entry folder of the kind.
    pub fn unit_count_total(&self, kind: Kind) -> usize {
        self.tree(kind).sources.values().flat_map(|s| s.entries.values()).map(|e| e.units.len()).sum()
    }

    /// Units in one entry folder, 0 when it is not indexed.
    pub fn unit_count_entry(&self, kind: Kind, source_id: i64, entry_title: &str) -> usize {
        self.tree(kind)
            .sources
            .get(&source_id)
            .and_then(|s| s.entries.get(&entry_dir_name(entry_title)))
            .map_or(0, |e| e.units.len())
    }

    /// Recursive size of every indexed entry folder of the kind. Runs on the blocking pool.
    pub async fn size_total(&self, kind: Kind) -> u64 {
        let root = self.root(kind).to_path_buf();
        let dirs: Vec<PathBuf> = self
            .tree(kind)
            .sources
            .values()
            .flat_map(|s| s.entries.values())
            .map(|e| root.join(&e.source_dir).join(&e.dir_name))
            .collect();
        sum_sizes(dirs).await
    }

    /// Recursive size of one entry folder. Runs on the blocking pool.
    pub async fn size_entry(&self, kind: Kind, source_id: i64, entry_title: &str) -> u64 {
        let tree = self.tree(kind);
        let dir = tree
            .sources
            .get(&source_id)
            .and_then(|s| s.entries.get(&entry_dir_name(entry_title)))
            .map(|e| self.root(kind).join(&e.source_dir).join(&e.dir_name));
        match dir {
            Some(dir) => sum_sizes(vec![dir]).await,
            None => 0,
        }
    }

    /// The source folder that holds an entry's downloads, as found on disk. `None` when the index has no folder for it.
    /// A caller whose source is not installed names the folder through this.
    pub fn entry_source_dir(&self, kind: Kind, source_id: i64, entry_title: &str) -> Option<String> {
        let tree = self.tree(kind);
        let entry = tree.sources.get(&source_id)?.entries.get(&entry_dir_name(entry_title))?;
        Some(entry.source_dir.clone())
    }

    /// Records a finished unit without a rescan. `unit_dir` may end in the kind's file extension; the stored name never
    /// does.
    pub fn add_unit(&self, kind: Kind, source_id: i64, source_dir: &str, entry_title: &str, unit_dir: &str) {
        let key = entry_dir_name(entry_title);
        let name = bare_unit_name(kind, unit_dir).to_owned();
        self.mutate(kind, |tree| add_to(tree, source_id, source_dir, &key, name));
    }

    /// Forgets units by folder or file name. Pass both names of an anime episode (current and legacy) to forget it
    /// whichever one is on disk.
    pub fn remove_units(&self, kind: Kind, source_id: i64, entry_title: &str, unit_dirs: &[String]) {
        let key = entry_dir_name(entry_title);
        self.mutate(kind, |tree| {
            if let Some(entry) = tree.sources.get_mut(&source_id).and_then(|s| s.entries.get_mut(&key)) {
                for name in unit_dirs {
                    entry.units.remove(bare_unit_name(kind, name));
                }
            }
        });
    }

    /// A unit folder or file was renamed on disk: the old name goes, the new one comes, in one change.
    pub fn rename_unit(
        &self,
        kind: Kind,
        source_id: i64,
        source_dir: &str,
        entry_title: &str,
        old_dir: &str,
        new_dir: &str,
    ) {
        let key = entry_dir_name(entry_title);
        let old = bare_unit_name(kind, old_dir).to_owned();
        let new = bare_unit_name(kind, new_dir).to_owned();
        self.mutate(kind, |tree| {
            if let Some(entry) = tree.sources.get_mut(&source_id).and_then(|s| s.entries.get_mut(&key)) {
                entry.units.remove(&old);
            }
            add_to(tree, source_id, source_dir, &key, new);
        });
    }

    pub fn remove_entry(&self, kind: Kind, source_id: i64, entry_title: &str) {
        let key = entry_dir_name(entry_title);
        self.mutate(kind, |tree| {
            if let Some(source) = tree.sources.get_mut(&source_id) {
                source.entries.remove(&key);
            }
        });
    }

    pub fn remove_source(&self, kind: Kind, source_id: i64) {
        self.mutate(kind, |tree| {
            tree.sources.remove(&source_id);
        });
    }

    /// Points a source and its entries at a renamed source folder. Kotlin keeps the old name until the next rescan.
    pub fn rename_source_dir(&self, kind: Kind, source_id: i64, new_dir: &str) {
        self.mutate(kind, |tree| {
            if let Some(source) = tree.sources.get_mut(&source_id) {
                source.dir_name = new_dir.to_owned();
                for entry in source.entries.values_mut() {
                    entry.source_dir = new_dir.to_owned();
                }
            }
        });
    }

    /// Forces a rescan on the next [`Self::renew_if_stale`] of `kind`, or of both kinds for `None` (Settings > Reindex
    /// downloads, the root watcher). The snapshot is rewritten now, so a restart rescans too.
    pub fn invalidate(&self, kind: Option<Kind>) {
        {
            let mut st = self.write();
            for k in Kind::ALL {
                if kind.is_none() || kind == Some(k) {
                    st.last_renew_ms[slot(k)] = None;
                }
            }
        }
        self.notify();
        if let Err(e) = write_snapshot(&self.shared) {
            tracing::warn!(target: "kuro_downloads", "could not write the index snapshot: {e}");
        }
    }

    /// Rescans `kind`'s root when its tree is older than an hour. Returns `true` when a scan replaced the tree.
    ///
    /// Waits up to 30 s for `ready` (the source registry). On timeout the previous tree is kept and the next call
    /// tries again. `installed` lists `(source id, "Name (LANG)")` and is called only after the wait.
    pub async fn renew_if_stale<F>(&self, kind: Kind, now_ms: i64, mut ready: watch::Receiver<bool>, installed: F) -> bool
    where
        F: FnOnce() -> Vec<(i64, String)> + Send,
    {
        let i = slot(kind);
        if self.read().last_renew_ms[i].is_some_and(|t| now_ms - t < RENEW_INTERVAL_MS) {
            return false;
        }
        if self.shared.scanning[i].swap(true, Ordering::SeqCst) {
            return false;
        }
        self.shared.scan_started.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(Instant::now());
        let _scan = ScanGuard { shared: &self.shared, slot: i };
        let waited = matches!(tokio::time::timeout(SOURCE_WAIT, ready.wait_for(|v| *v)).await, Ok(Ok(_)));
        if !waited {
            tracing::warn!(target: "kuro_downloads", "source registry not ready; keeping the previous index");
            return false;
        }
        let entries = installed();
        let root = self.root(kind).to_path_buf();
        let tree = match tokio::task::spawn_blocking(move || scan(kind, &root, &entries)).await {
            Ok(tree) => tree,
            Err(e) => {
                tracing::warn!(target: "kuro_downloads", "index scan failed: {e}");
                return false;
            }
        };
        tracing::debug!(target: "kuro_downloads", sources = tree.sources.len(), "index scan finished");
        {
            let mut st = self.write();
            st.trees[i] = Arc::new(tree);
            st.last_renew_ms[i] = Some(now_ms);
        }
        self.touch();
        true
    }

    /// Rescans now, whatever the age of the tree: the sources changed (a repository load ended), so the folders of
    /// sources that were not known before are matched. Waits and returns as [`Self::renew_if_stale`].
    pub async fn rescan<F>(&self, kind: Kind, ready: watch::Receiver<bool>, installed: F) -> bool
    where
        F: FnOnce() -> Vec<(i64, String)> + Send,
    {
        self.write().last_renew_ms[slot(kind)] = None;
        self.renew_if_stale(kind, now_ms(), ready, installed).await
    }

    /// Watches both roots and invalidates the kind whose root changed. `None` when no root can be watched, then the
    /// hourly renew is the fallback. The caller (the Shell) holds the guard.
    pub fn watch(&self) -> Option<RootWatch> {
        watch_roots(self)
    }

    fn mutate(&self, kind: Kind, change: impl FnOnce(&mut Tree)) {
        {
            let mut st = self.write();
            change(Arc::make_mut(&mut st.trees[slot(kind)]));
        }
        self.touch();
    }

    fn notify(&self) {
        self.shared.changes.send_modify(|n| *n = n.wrapping_add(1));
    }

    /// Notes a change: wakes the `changes` receivers and writes the snapshot one second after the last change. Each
    /// change restarts the timer. Without a Tokio runtime the write waits for the next change.
    fn touch(&self) {
        self.notify();
        let generation = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let shared = self.shared.clone();
        handle.spawn(async move {
            tokio::time::sleep(SNAPSHOT_DEBOUNCE).await;
            if shared.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            // The snapshot is small, so it is written inline rather than on the blocking pool.
            if let Err(e) = write_snapshot(&shared) {
                tracing::warn!(target: "kuro_downloads", "index snapshot write failed: {e}");
            }
        });
    }
}

fn add_to(tree: &mut Tree, source_id: i64, source_dir: &str, entry_key: &str, unit: String) {
    let source = tree
        .sources
        .entry(source_id)
        .or_insert_with(|| SourceDir { dir_name: source_dir.to_owned(), entries: BTreeMap::new() });
    let entry = source.entries.entry(entry_key.to_owned()).or_insert_with(|| EntryDir {
        source_dir: source_dir.to_owned(),
        dir_name: entry_key.to_owned(),
        units: BTreeSet::new(),
    });
    entry.units.insert(unit);
}

/// Ends a scan: the indicator goes and the next scan of the kind may start.
struct ScanGuard<'a> {
    shared: &'a Shared,
    slot: usize,
}

impl Drop for ScanGuard<'_> {
    fn drop(&mut self) {
        self.shared.scan_started.lock().unwrap_or_else(|e| e.into_inner())[self.slot] = None;
        self.shared.scanning[self.slot].store(false, Ordering::SeqCst);
    }
}

/// Whether the indicator shows for a scan that has run for `elapsed` (`None` when no scan runs).
pub fn checking_indicator_visible(elapsed: Option<Duration>) -> bool {
    elapsed.is_some_and(|e| e > INDICATOR_DELAY)
}

async fn sum_sizes(dirs: Vec<PathBuf>) -> u64 {
    tokio::task::spawn_blocking(move || dirs.iter().map(|d| kuro_core::storage::dir_size(d).unwrap_or(0)).sum())
        .await
        .unwrap_or(0)
}

/// Walks `root`. A folder counts as a source only when its name matches an installed source's sanitised display name,
/// ignoring case. Two folders that match one source are merged, the first by name winning a clash. Unit folders ending
/// in `_tmp` are skipped; unit files count by their stem (see `naming::is_unit_entry`).
pub fn scan(kind: Kind, root: &Path, installed: &[(i64, String)]) -> Tree {
    let mut tree = Tree::default();
    let mut source_paths = list_dirs(root);
    source_paths.sort();
    for source_path in source_paths {
        let folder = file_name(&source_path);
        let folder_lower = folder.to_lowercase();
        let Some((source_id, _)) = installed.iter().find(|(_, display)| fat_safe(display).to_lowercase() == folder_lower)
        else {
            continue;
        };
        let source = tree
            .sources
            .entry(*source_id)
            .or_insert_with(|| SourceDir { dir_name: folder.clone(), entries: BTreeMap::new() });
        let mut entry_paths = list_dirs(&source_path);
        entry_paths.sort();
        for entry_path in entry_paths {
            let entry_name = file_name(&entry_path);
            if source.entries.contains_key(&entry_name) {
                continue;
            }
            let units = scan_units(kind, &entry_path);
            source
                .entries
                .insert(entry_name.clone(), EntryDir { source_dir: folder.clone(), dir_name: entry_name, units });
        }
    }
    tree
}

fn scan_units(kind: Kind, entry_path: &Path) -> BTreeSet<String> {
    read_entries(entry_path)
        .into_iter()
        .filter_map(|path| is_unit_entry(kind, &file_name(&path), path.is_dir()))
        .collect()
}

fn read_entries(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default()
}

fn list_dirs(dir: &Path) -> Vec<PathBuf> {
    read_entries(dir).into_iter().filter(|p| p.is_dir() && !file_name(p).trim().is_empty()).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Loads the kinds whose root matches `roots`. A file that does not parse, or matches no root, is deleted.
fn load_snapshot(path: &Path, roots: &IndexRoots) -> State {
    let Ok(bytes) = fs::read(path) else {
        return State::default();
    };
    let parsed =
        bytes.strip_prefix(SNAPSHOT_MAGIC).and_then(|body| serde_json::from_slice::<Snapshot>(body).ok());
    let Some(snapshot) = parsed else {
        tracing::warn!(target: "kuro_downloads", "discarding unreadable index snapshot");
        let _ = fs::remove_file(path);
        return State::default();
    };
    let mut state = State::default();
    let mut used = false;
    for (kind, part) in [(Kind::Manga, snapshot.manga), (Kind::Anime, snapshot.anime)] {
        if part.root == roots.of(kind) {
            state.trees[slot(kind)] = Arc::new(part.tree);
            state.last_renew_ms[slot(kind)] = part.built_at_ms;
            used = true;
        }
    }
    if !used {
        let _ = fs::remove_file(path);
    }
    state
}

fn write_snapshot(shared: &Shared) -> io::Result<()> {
    let snapshot = {
        let st = shared.state.read().unwrap_or_else(|e| e.into_inner());
        let part = |kind: Kind| KindSnapshot {
            root: shared.roots.of(kind).to_path_buf(),
            built_at_ms: st.last_renew_ms[slot(kind)],
            tree: (*st.trees[slot(kind)]).clone(),
        };
        Snapshot { manga: part(Kind::Manga), anime: part(Kind::Anime) }
    };
    let mut bytes = SNAPSHOT_MAGIC.to_vec();
    bytes.extend(serde_json::to_vec(&snapshot).map_err(io::Error::other)?);
    if let Some(parent) = shared.snapshot_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = shared.snapshot_path.with_extension("bin.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, &shared.snapshot_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::jpeg_bytes;

    const SOURCE: &str = "S (EN)";

    fn installed() -> Vec<(i64, String)> {
        vec![(1, SOURCE.to_owned()), (2, "Other (XX)".to_owned())]
    }

    fn roots(base: &Path) -> IndexRoots {
        IndexRoots { manga: base.join("manga"), anime: base.join("anime") }
    }

    /// The extension of a single-file unit of the kind: `cbz` or `mkv`.
    fn ext(kind: Kind) -> &'static str {
        crate::naming::unit_extensions(kind)[0]
    }

    /// Builds a tree for the kind under `root`: unit folder `U 1`, staging `U 2_tmp`, file `U 3.<ext>`, junk files, an
    /// uninstalled source, a case-only twin of the source folder holding another entry, and an installed source
    /// without units.
    fn fixture(kind: Kind, root: &Path) {
        let entry = root.join(SOURCE).join("Berserk");
        fs::create_dir_all(entry.join("U 1")).unwrap();
        fs::create_dir_all(entry.join("U 2_tmp")).unwrap();
        fs::write(entry.join(format!("U 3.{}", ext(kind))), b"x").unwrap();
        fs::write(entry.join("U 4.zip"), b"x").unwrap();
        fs::write(entry.join("notes.txt"), b"x").unwrap();
        fs::create_dir_all(root.join("unknown (xx)").join("Other")).unwrap();
        fs::create_dir_all(root.join("s (en)").join("Vagabond").join("U 9")).unwrap();
        fs::create_dir_all(root.join("Other (XX)").join("Ignored")).unwrap();
    }

    /// An index for `kind` with the fixture scanned in, in a temp dir that lives as long as the returned guard.
    fn scanned(kind: Kind) -> (tempfile::TempDir, DownloadIndex) {
        let base = tempfile::tempdir().unwrap();
        let roots = roots(base.path());
        fs::create_dir_all(roots.of(kind)).unwrap();
        fixture(kind, roots.of(kind));
        let index = DownloadIndex::open(roots.clone(), base.path());
        index.write().trees[slot(kind)] = Arc::new(scan(kind, roots.of(kind), &installed()));
        (base, index)
    }

    #[test]
    fn scan_builds_source_entry_and_unit_names_for_both_kinds() {
        for kind in Kind::ALL {
            let base = tempfile::tempdir().unwrap();
            fixture(kind, base.path());
            let tree = scan(kind, base.path(), &installed());
            let source = &tree.sources[&1];
            assert_eq!(source.dir_name, "S (EN)", "the first matching folder names the source");
            let berserk = &source.entries["Berserk"];
            assert_eq!(berserk.units.iter().cloned().collect::<Vec<_>>(), vec!["U 1", "U 3"], "{kind}");
            assert_eq!(berserk.source_dir, "S (EN)");
            assert!(source.entries.contains_key("Vagabond"), "a case-only folder match counts");
            assert_eq!(source.entries["Vagabond"].source_dir, "s (en)");
            assert!(!tree.sources.contains_key(&99));
            assert!(!tree.sources.values().any(|s| s.entries.contains_key("Other")), "unknown source folders are ignored");
        }
    }

    #[test]
    fn a_file_of_the_other_kind_is_not_a_unit() {
        let base = tempfile::tempdir().unwrap();
        let entry = base.path().join(SOURCE).join("M");
        fs::create_dir_all(&entry).unwrap();
        fs::write(entry.join("A.cbz"), b"x").unwrap();
        fs::write(entry.join("B.mkv"), b"x").unwrap();
        fs::write(entry.join("C.mp4"), b"x").unwrap();
        let units = |kind| scan(kind, base.path(), &installed()).sources[&1].entries["M"].units.iter().cloned().collect::<Vec<_>>();
        assert_eq!(units(Kind::Manga), vec!["A"]);
        assert_eq!(units(Kind::Anime), vec!["B", "C"]);
    }

    // TST-41
    #[test]
    fn source_folder_matching_ignores_case() {
        for kind in Kind::ALL {
            let base = tempfile::tempdir().unwrap();
            fs::create_dir_all(base.path().join("s (en)").join("Vagabond")).unwrap();
            let tree = scan(kind, base.path(), &installed());
            assert_eq!(tree.sources[&1].dir_name, "s (en)");
        }
    }

    #[test]
    fn queries_and_mutations_update_the_tree_without_a_rescan() {
        for kind in Kind::ALL {
            let (_base, index) = scanned(kind);
            assert!(index.is_downloaded(kind, 1, "Berserk", "U 1", None));
            assert!(index.is_downloaded(kind, 1, "Berserk", "U 3", None), "a unit file counts");
            assert!(!index.is_downloaded(kind, 1, "Berserk", "U 4", None));
            assert!(!index.is_downloaded(kind, 1, "Nope", "U 1", None));
            assert_eq!(index.unit_count_total(kind), 3);
            assert_eq!(index.unit_count_entry(kind, 1, "Berserk"), 2);
            assert_eq!(index.entry_source_dir(kind, 1, "Vagabond"), Some("s (en)".to_owned()));
            assert_eq!(index.entry_source_dir(kind, 1, "Nope"), None);

            let file = format!("U 5.{}", ext(kind));
            index.add_unit(kind, 1, SOURCE, "Berserk", &file);
            assert!(index.is_downloaded(kind, 1, "Berserk", "U 5", None));
            assert!(
                !index.tree(kind).sources[&1].entries["Berserk"].units.iter().any(|u| u.contains('.')),
                "the stored name never has an extension"
            );

            index.remove_units(kind, 1, "Berserk", &["U 1".to_owned(), format!("U 3.{}", ext(kind))]);
            assert!(!index.is_downloaded(kind, 1, "Berserk", "U 1", None));
            assert!(!index.is_downloaded(kind, 1, "Berserk", "U 3", None));

            index.remove_entry(kind, 1, "Berserk");
            assert_eq!(index.unit_count_entry(kind, 1, "Berserk"), 0);
            index.remove_source(kind, 1);
            assert_eq!(index.unit_count_total(kind), 0, "source 2 in the fixture has no units");
        }
    }

    #[test]
    fn the_two_kinds_do_not_share_units() {
        let index = DownloadIndex::open(roots(Path::new("/nonexistent-root")), Path::new("/nonexistent-cache"));
        index.add_unit(Kind::Manga, 1, SOURCE, "M", "Ch 1");
        assert!(index.is_downloaded(Kind::Manga, 1, "M", "Ch 1", None));
        assert!(!index.is_downloaded(Kind::Anime, 1, "M", "Ch 1", None));
        assert_eq!(index.unit_count_total(Kind::Anime), 0);
        index.remove_source(Kind::Anime, 1);
        assert_eq!(index.unit_count_total(Kind::Manga), 1, "removing from one kind leaves the other");
    }

    #[test]
    fn an_anime_episode_is_found_and_removed_under_either_name() {
        let index = DownloadIndex::open(roots(Path::new("/nonexistent-root")), Path::new("/nonexistent-cache"));
        index.add_unit(Kind::Anime, 7, "Demo (EN)", "Show", "_Episode 1");
        assert!(index.is_downloaded(Kind::Anime, 7, "Show", "Episode 1", Some(" ")), "the legacy name counts");
        assert!(!index.is_downloaded(Kind::Anime, 7, "Show", "Episode 1", None));
        index.add_unit(Kind::Anime, 7, "Demo (EN)", "Show", "grp_Episode 2");
        let names: Vec<String> = ["Episode 2", "grp_Episode 2"].map(str::to_owned).to_vec();
        index.remove_units(Kind::Anime, 7, "Show", &names);
        assert_eq!(index.unit_count_entry(Kind::Anime, 7, "Show"), 1, "only the matching folder goes");
    }

    // Test 4.2 item 10: a rename is visible without a rescan.
    #[test]
    fn a_renamed_unit_is_downloaded_under_the_new_name_and_not_the_old() {
        for kind in Kind::ALL {
            let index = DownloadIndex::open(roots(Path::new("/nonexistent-root")), Path::new("/nonexistent-cache"));
            let old = format!("Old.{}", ext(kind));
            let new = format!("New.{}", ext(kind));
            index.add_unit(kind, 1, SOURCE, "M", &old);
            index.rename_unit(kind, 1, SOURCE, "M", &old, &new);
            assert!(index.is_downloaded(kind, 1, "M", "New", None));
            assert!(!index.is_downloaded(kind, 1, "M", "Old", None));
            assert_eq!(index.unit_count_entry(kind, 1, "M"), 1);
        }
    }

    #[test]
    fn a_source_rename_points_the_entries_at_the_new_folder() {
        for kind in Kind::ALL {
            let index = DownloadIndex::open(roots(Path::new("/nonexistent-root")), Path::new("/nonexistent-cache"));
            index.add_unit(kind, 1, "Old (EN)", "M", "U 1");
            index.rename_source_dir(kind, 1, "New (EN)");
            assert_eq!(index.entry_source_dir(kind, 1, "M"), Some("New (EN)".to_owned()));
            assert_eq!(index.tree(kind).sources[&1].dir_name, "New (EN)");
        }
    }

    #[test]
    fn a_tree_taken_before_a_write_does_not_change() {
        let index = DownloadIndex::open(roots(Path::new("/nonexistent-root")), Path::new("/nonexistent-cache"));
        index.add_unit(Kind::Anime, 7, "Demo (EN)", "Show", "Episode 1");
        let before = index.tree(Kind::Anime);
        index.add_unit(Kind::Anime, 7, "Demo (EN)", "Show", "Episode 2");
        assert_eq!(before.sources[&7].entries["Show"].units.len(), 1, "the earlier tree is untouched");
        assert_eq!(index.tree(Kind::Anime).sources[&7].entries["Show"].units.len(), 2);
    }

    #[test]
    fn every_change_bumps_the_changes_counter() {
        let index = DownloadIndex::open(roots(Path::new("/nonexistent-root")), Path::new("/nonexistent-cache"));
        let changes = index.changes();
        let before = *changes.borrow();
        index.add_unit(Kind::Manga, 1, SOURCE, "M", "Ch 1");
        assert_ne!(*changes.borrow(), before);
    }

    // TST-38
    #[tokio::test(start_paused = true)]
    async fn renew_runs_once_per_hour_for_each_kind() {
        for kind in Kind::ALL {
            let base = tempfile::tempdir().unwrap();
            let roots = roots(base.path());
            fs::create_dir_all(roots.of(kind)).unwrap();
            fixture(kind, roots.of(kind));
            let index = DownloadIndex::open(roots, base.path());
            let (_tx, rx) = watch::channel(true);
            let start = 1_000_000;
            assert!(index.renew_if_stale(kind, start, rx.clone(), installed).await);
            assert!(!index.renew_if_stale(kind, start + RENEW_INTERVAL_MS - 1, rx.clone(), installed).await);
            assert!(index.renew_if_stale(kind, start + RENEW_INTERVAL_MS, rx, installed).await);
            assert_eq!(index.unit_count_total(kind), 3);
        }
    }

    // Test 4.2 item 11: invalidate(None) marks both stale, invalidate(Some) only one.
    #[tokio::test(start_paused = true)]
    async fn invalidate_forces_a_rescan_of_the_named_kinds_only() {
        let base = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(roots(base.path()), base.path());
        let (_tx, rx) = watch::channel(true);
        for kind in Kind::ALL {
            assert!(index.renew_if_stale(kind, 10, rx.clone(), installed).await);
        }
        index.invalidate(Some(Kind::Manga));
        assert_eq!(index.last_renew_ms(Kind::Manga), None);
        assert_eq!(index.last_renew_ms(Kind::Anime), Some(10), "the other kind stays trusted");
        assert!(index.renew_if_stale(Kind::Manga, 11, rx.clone(), installed).await);
        assert!(!index.renew_if_stale(Kind::Anime, 11, rx.clone(), installed).await);

        index.invalidate(None);
        assert_eq!((index.last_renew_ms(Kind::Manga), index.last_renew_ms(Kind::Anime)), (None, None));
        for kind in Kind::ALL {
            assert!(index.renew_if_stale(kind, 12, rx.clone(), installed).await);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_rescan_recognises_the_folder_of_a_source_added_after_the_last_scan() {
        for kind in Kind::ALL {
            let base = tempfile::tempdir().unwrap();
            let roots = roots(base.path());
            fs::create_dir_all(roots.of(kind)).unwrap();
            fixture(kind, roots.of(kind));
            // a repository source whose name needs the sanitiser: its folder is the sanitised display name
            let repo = (77, "Weeb: Central (EN)".to_owned());
            fs::create_dir_all(roots.of(kind).join(fat_safe(&repo.1)).join("Solo").join("Chapter 1")).unwrap();
            let index = DownloadIndex::open(roots, base.path());
            let (_tx, rx) = watch::channel(true);
            assert!(index.renew_if_stale(kind, now_ms(), rx.clone(), installed).await);
            assert!(!index.is_downloaded(kind, 77, "Solo", "Chapter 1", None), "not known before the repository loaded");

            let with_repo = move || -> Vec<(i64, String)> { installed().into_iter().chain([repo]).collect() };
            assert!(index.rescan(kind, rx, with_repo).await, "a fresh tree is scanned again");
            assert!(index.is_downloaded(kind, 77, "Solo", "Chapter 1", None));
            assert!(index.is_downloaded(kind, 1, "Berserk", "U 1", None), "the other sources stay");
        }
    }

    // TST-39
    #[tokio::test(start_paused = true)]
    async fn a_source_wait_timeout_keeps_the_previous_tree() {
        for kind in Kind::ALL {
            let base = tempfile::tempdir().unwrap();
            let roots = roots(base.path());
            fs::create_dir_all(roots.of(kind)).unwrap();
            fixture(kind, roots.of(kind));
            let index = DownloadIndex::open(roots, base.path());
            let (_tx, rx) = watch::channel(true);
            assert!(index.renew_if_stale(kind, 0, rx, installed).await);
            let before = index.tree(kind);
            assert_eq!(before.sources.len(), 2, "S (EN) and Other (XX)");

            // The registry never reports ready: the wait times out after 30 s and the old tree stays.
            index.invalidate(Some(kind));
            let (_never_tx, never_ready) = watch::channel(false);
            assert!(!index.renew_if_stale(kind, 5, never_ready, Vec::new).await);
            assert_eq!(index.tree(kind), before, "the old tree survives a failed wait");
            assert_eq!(index.last_renew_ms(kind), None, "and the next call tries again");
        }
    }

    // TST-40
    #[tokio::test(start_paused = true)]
    async fn a_corrupt_snapshot_is_deleted_and_the_trees_rebuilt() {
        let base = tempfile::tempdir().unwrap();
        let roots = roots(base.path());
        fs::create_dir_all(roots.of(Kind::Manga)).unwrap();
        fixture(Kind::Manga, roots.of(Kind::Manga));
        let snapshot = base.path().join(SNAPSHOT_FILE);
        fs::write(&snapshot, b"KIDX\x02{not json").unwrap();
        let index = DownloadIndex::open(roots, base.path());
        assert!(!snapshot.exists(), "corrupt snapshot removed on open");
        let (_tx, rx) = watch::channel(true);
        assert!(index.renew_if_stale(Kind::Manga, 0, rx, installed).await);
        assert_eq!(index.unit_count_total(Kind::Manga), 3);
    }

    #[test]
    fn a_snapshot_of_the_old_format_is_not_read() {
        let base = tempfile::tempdir().unwrap();
        let snapshot = base.path().join(SNAPSHOT_FILE);
        fs::write(&snapshot, b"KIDX\x01{}").unwrap();
        let index = DownloadIndex::open(roots(base.path()), base.path());
        assert_eq!(index.unit_count_total(Kind::Manga), 0);
        assert!(!snapshot.exists());
    }

    // Test 4.2 item 11: both trees round trip through one snapshot file.
    #[tokio::test(start_paused = true)]
    async fn both_trees_round_trip_through_one_snapshot_file() {
        let base = tempfile::tempdir().unwrap();
        let roots = roots(base.path());
        let index = DownloadIndex::open(roots.clone(), base.path());
        let snapshot = base.path().join(SNAPSHOT_FILE);
        index.add_unit(Kind::Manga, 1, SOURCE, "M", "Ch 1");
        index.add_unit(Kind::Anime, 2, "A (EN)", "Show", "Ep 1");
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(!snapshot.exists(), "nothing is written before the delay");
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(snapshot.exists());
        assert!(!base.path().join("download-index-v2.bin.tmp").exists(), "the temp file is renamed away");

        let reopened = DownloadIndex::open(roots, base.path());
        assert!(reopened.is_downloaded(Kind::Manga, 1, "M", "Ch 1", None));
        assert!(reopened.is_downloaded(Kind::Anime, 2, "Show", "Ep 1", None));
        assert!(!reopened.is_downloaded(Kind::Manga, 2, "Show", "Ep 1", None), "each kind got its own tree back");
        assert_eq!(reopened.tree(Kind::Anime).sources[&2].entries["Show"].source_dir, "A (EN)");
    }

    #[tokio::test(start_paused = true)]
    async fn a_snapshot_for_another_root_is_discarded_for_that_kind_only() {
        let base = tempfile::tempdir().unwrap();
        let roots = roots(base.path());
        let index = DownloadIndex::open(roots.clone(), base.path());
        index.add_unit(Kind::Manga, 1, SOURCE, "M", "Ch 1");
        index.add_unit(Kind::Anime, 2, "A (EN)", "Show", "Ep 1");
        tokio::time::sleep(Duration::from_millis(1100)).await;

        let moved = IndexRoots { manga: roots.manga.clone(), anime: base.path().join("elsewhere") };
        let reopened = DownloadIndex::open(moved, base.path());
        assert_eq!(reopened.unit_count_total(Kind::Manga), 1, "the manga root is the same");
        assert_eq!(reopened.unit_count_total(Kind::Anime), 0, "the anime root moved");

        let neither = IndexRoots { manga: base.path().join("m2"), anime: base.path().join("a2") };
        let none = DownloadIndex::open(neither, base.path());
        assert_eq!(none.unit_count_total(Kind::Manga) + none.unit_count_total(Kind::Anime), 0);
        assert!(!base.path().join(SNAPSHOT_FILE).exists(), "a snapshot that matches no root is deleted");
    }

    #[tokio::test(start_paused = true)]
    async fn the_snapshot_timer_restarts_on_every_change() {
        let base = tempfile::tempdir().unwrap();
        let roots = roots(base.path());
        let index = DownloadIndex::open(roots.clone(), base.path());
        let snapshot = base.path().join(SNAPSHOT_FILE);
        index.add_unit(Kind::Manga, 1, SOURCE, "M", "Ch 1");
        tokio::time::sleep(Duration::from_millis(900)).await;
        index.add_unit(Kind::Manga, 1, SOURCE, "M", "Ch 2");
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(!snapshot.exists(), "1.5 s after the first change but 0.6 s after the second: no write yet");
        tokio::time::sleep(Duration::from_millis(350)).await;
        assert!(!snapshot.exists(), "1.85 s: still less than a second after the second change");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(snapshot.exists(), "1.95 s: one second after the second change");
        let reopened = DownloadIndex::open(roots, base.path());
        assert!(reopened.is_downloaded(Kind::Manga, 1, "M", "Ch 2", None));
    }

    #[tokio::test]
    async fn sizes_sum_the_indexed_folders_for_each_kind() {
        for kind in Kind::ALL {
            let base = tempfile::tempdir().unwrap();
            let roots = roots(base.path());
            let entry = roots.of(kind).join(SOURCE).join("M");
            fs::create_dir_all(entry.join("U 1")).unwrap();
            fs::write(entry.join("U 1").join("001.jpg"), jpeg_bytes(100)).unwrap();
            fs::write(entry.join(format!("U 2.{}", ext(kind))), jpeg_bytes(50)).unwrap();
            let index = DownloadIndex::open(roots, base.path());
            index.add_unit(kind, 1, SOURCE, "M", "U 1");
            index.add_unit(kind, 1, SOURCE, "M", &format!("U 2.{}", ext(kind)));
            assert_eq!(index.size_entry(kind, 1, "M").await, 150);
            assert_eq!(index.size_total(kind).await, 150);
            assert_eq!(index.size_entry(kind, 1, "Missing").await, 0);
            let other = if kind == Kind::Manga { Kind::Anime } else { Kind::Manga };
            assert_eq!(index.size_total(other).await, 0, "the other kind has nothing");
        }
    }

    #[test]
    fn the_checking_indicator_waits_for_a_scan_longer_than_one_second() {
        assert!(!checking_indicator_visible(None), "no scan, no indicator");
        assert!(!checking_indicator_visible(Some(Duration::from_millis(900))), "EC-80: short scans show nothing");
        assert!(checking_indicator_visible(Some(Duration::from_millis(1100))));
    }

    #[test]
    fn a_finished_scan_clears_the_start_time() {
        let base = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(roots(base.path()), base.path());
        assert!(index.scan_elapsed().is_none());
        assert!(!index.checking_indicator_visible());
    }

    #[test]
    fn the_tmp_fixture_tree_lists_only_the_finished_episode_folders() {
        let tree = crate::testing::dl_fixture("tree_tmp");
        let scanned = scan(Kind::Anime, tree.path(), &[(1, "Example (EN)".to_owned())]);
        let episodes: Vec<&str> =
            scanned.sources[&1].entries["Show"].units.iter().map(String::as_str).collect();
        assert!(episodes.is_empty(), "staging folders are not episodes: {episodes:?}");
    }

    #[test]
    fn the_new_fixture_tree_lists_its_episode_folder_and_source_names_match_ignoring_case() {
        let tree = crate::testing::dl_fixture("tree_new");
        let scanned = scan(Kind::Anime, tree.path(), &[(1, "Example (EN)".to_owned())]);
        assert!(scanned.sources[&1].entries["Show"].units.contains("Scan_Episode 1"));

        let cased = crate::testing::dl_fixture("tree_case");
        let scanned = scan(Kind::Anime, cased.path(), &[(1, "example (en)".to_owned())]);
        assert!(scanned.sources.contains_key(&1), "a source folder matches whatever its case");
        assert!(scanned.sources[&1].entries["Show"].units.contains("Episode 1"));
    }
}
