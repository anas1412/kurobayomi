//! Download index: scan, hourly renew, debounced snapshot and queries (port of MangaDownloadCache.kt).
//!
//! The index is the in-memory tree `source -> manga -> chapter names`. It is trusted for one hour after a scan or a
//! snapshot load. A failed source wait keeps the previous tree (Kotlin replaces it with an empty one, a defect).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::time::Instant;

use crate::layout::{CBZ_EXTENSION, TMP_DIR_SUFFIX, chapter_dir_name};
use crate::sanitize::build_valid_filename;

/// The index is trusted for this long (`Cache.kt` renewInterval).
pub const RENEW_INTERVAL_MS: i64 = 3_600_000;
/// Longest wait for the source registry before a renew gives up and keeps the old tree.
pub const SOURCE_WAIT: Duration = Duration::from_secs(30);
/// Snapshot write delay after the last change.
pub const SNAPSHOT_DEBOUNCE: Duration = Duration::from_secs(1);
/// Snapshot file in the cache directory. Version 1 of the format.
pub const SNAPSHOT_FILE: &str = "download-index-v1.bin";
const SNAPSHOT_MAGIC: &[u8] = b"KIDX\x01";

/// `source id -> source folder`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Tree {
    pub sources: BTreeMap<i64, SourceDir>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SourceDir {
    pub dir_name: String,
    /// Keyed by the manga folder name.
    pub manga: BTreeMap<String, MangaDir>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MangaDir {
    /// The source folder this manga lives in. Two folders can match one source when they differ only by case.
    pub source_dir: String,
    pub dir_name: String,
    /// Bare chapter names: the folder name, or the `.cbz` name without its suffix.
    pub chapters: BTreeSet<String>,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    root: PathBuf,
    built_at_ms: i64,
    tree: Tree,
}

#[derive(Default)]
struct State {
    tree: Tree,
    last_renew_ms: Option<i64>,
}

struct Shared {
    root: PathBuf,
    snapshot_path: PathBuf,
    state: RwLock<State>,
    scanning: AtomicBool,
    /// When the running scan began. `None` when no scan runs (T-23a-040).
    scan_started: Mutex<Option<Instant>>,
    generation: AtomicU64,
    /// When the last change was noted. The snapshot timer runs from here, so every change restarts it.
    last_change: Mutex<Instant>,
    persist_scheduled: AtomicBool,
}

/// Cheap to clone. All methods take `&self`.
#[derive(Clone)]
pub struct DownloadIndex {
    shared: Arc<Shared>,
}

impl DownloadIndex {
    /// Creates the index for `root`. A snapshot in `cache_dir` is loaded when it matches `root`. A corrupt one, or one
    /// written for another root, is deleted.
    pub fn open(root: impl Into<PathBuf>, cache_dir: &Path) -> Self {
        let root = root.into();
        let snapshot_path = cache_dir.join(SNAPSHOT_FILE);
        let state = match load_snapshot(&snapshot_path, &root) {
            Some(snap) => State { tree: snap.tree, last_renew_ms: Some(snap.built_at_ms) },
            None => State::default(),
        };
        Self {
            shared: Arc::new(Shared {
                root,
                snapshot_path,
                state: RwLock::new(state),
                scanning: AtomicBool::new(false),
                scan_started: Mutex::new(None),
                generation: AtomicU64::new(0),
                last_change: Mutex::new(Instant::now()),
                persist_scheduled: AtomicBool::new(false),
            }),
        }
    }

    pub fn root(&self) -> &Path {
        &self.shared.root
    }

    /// How long the running scan has been going, `None` when no scan runs (T-23a-040).
    pub fn scan_elapsed(&self) -> Option<Duration> {
        let started = *self.shared.scan_started.lock().unwrap_or_else(|e| e.into_inner());
        started.map(|t| t.elapsed())
    }

    /// Whether the "Checking downloads" indicator should show now (T-23a-040).
    pub fn checking_indicator_visible(&self) -> bool {
        checking_indicator_visible(self.scan_elapsed())
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.shared.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.shared.state.write().unwrap_or_else(|e| e.into_inner())
    }

    /// `true` when a folder or `.cbz` for the chapter is in the index.
    pub fn is_chapter_downloaded(&self, source_id: i64, manga_title: &str, name: &str, scanlator: Option<&str>) -> bool {
        let chapter = chapter_dir_name(name, scanlator);
        let manga = build_valid_filename(manga_title);
        self.read()
            .tree
            .sources
            .get(&source_id)
            .and_then(|s| s.manga.get(&manga))
            .is_some_and(|m| m.chapters.contains(&chapter))
    }

    /// The source folder that holds a manga's downloads, as found on disk. `None` when the index has no folder for it.
    /// A caller whose source is not installed names the folder through this.
    pub fn manga_source_dir(&self, source_id: i64, manga_title: &str) -> Option<String> {
        self.read().tree.sources.get(&source_id)?.manga.get(&build_valid_filename(manga_title)).map(|m| m.source_dir.clone())
    }

    pub fn chapter_count_total(&self) -> usize {
        self.read().tree.sources.values().flat_map(|s| s.manga.values()).map(|m| m.chapters.len()).sum()
    }

    pub fn chapter_count_manga(&self, source_id: i64, manga_title: &str) -> usize {
        self.read()
            .tree
            .sources
            .get(&source_id)
            .and_then(|s| s.manga.get(&build_valid_filename(manga_title)))
            .map_or(0, |m| m.chapters.len())
    }

    /// Recursive size of every indexed manga folder. Runs on the blocking pool.
    pub async fn size_total(&self) -> u64 {
        let dirs: Vec<PathBuf> = {
            let st = self.read();
            st.tree
                .sources
                .values()
                .flat_map(|s| s.manga.values().map(|m| self.shared.root.join(&m.source_dir).join(&m.dir_name)))
                .collect()
        };
        sum_sizes(dirs).await
    }

    /// Recursive size of one manga folder. Runs on the blocking pool.
    pub async fn size_manga(&self, source_id: i64, manga_title: &str) -> u64 {
        let dir = {
            let st = self.read();
            st.tree
                .sources
                .get(&source_id)
                .and_then(|s| s.manga.get(&build_valid_filename(manga_title)).map(|m| self.shared.root.join(&m.source_dir).join(&m.dir_name)))
        };
        match dir {
            Some(dir) => sum_sizes(vec![dir]).await,
            None => 0,
        }
    }

    /// Records a finished chapter without a rescan. `chapter_dir` may end in `.cbz`; the stored name never does.
    pub fn add_chapter(&self, source_id: i64, source_dir: &str, manga_title: &str, chapter_dir: &str) {
        let manga_key = build_valid_filename(manga_title);
        {
            let mut st = self.write();
            let source = st.tree.sources.entry(source_id).or_insert_with(|| SourceDir {
                dir_name: source_dir.to_owned(),
                manga: BTreeMap::new(),
            });
            let manga = source.manga.entry(manga_key.clone()).or_insert_with(|| MangaDir {
                source_dir: source_dir.to_owned(),
                dir_name: manga_key,
                chapters: BTreeSet::new(),
            });
            manga.chapters.insert(bare_name(chapter_dir).to_owned());
        }
        self.touch();
    }

    /// Forgets chapters by folder or `.cbz` name.
    pub fn remove_chapter(&self, source_id: i64, manga_title: &str, names: &[String]) {
        let key = build_valid_filename(manga_title);
        {
            let mut st = self.write();
            if let Some(manga) = st.tree.sources.get_mut(&source_id).and_then(|s| s.manga.get_mut(&key)) {
                for name in names {
                    manga.chapters.remove(bare_name(name));
                }
            }
        }
        self.touch();
    }

    pub fn remove_manga(&self, source_id: i64, manga_title: &str) {
        let key = build_valid_filename(manga_title);
        {
            let mut st = self.write();
            if let Some(source) = st.tree.sources.get_mut(&source_id) {
                source.manga.remove(&key);
            }
        }
        self.touch();
    }

    pub fn remove_source(&self, source_id: i64) {
        self.write().tree.sources.remove(&source_id);
        self.touch();
    }

    /// Points a source and its manga at a renamed source folder. Kotlin keeps the old name until the next rescan.
    pub fn rename_source_dir(&self, source_id: i64, new_dir: &str) {
        {
            let mut st = self.write();
            if let Some(source) = st.tree.sources.get_mut(&source_id) {
                source.dir_name = new_dir.to_owned();
                for manga in source.manga.values_mut() {
                    manga.source_dir = new_dir.to_owned();
                }
            }
        }
        self.touch();
    }

    /// When the tree was last scanned or loaded from a snapshot. `None` after [`Self::invalidate`] or before any scan.
    pub fn last_renew_ms(&self) -> Option<i64> {
        self.read().last_renew_ms
    }

    /// Forces a rescan on the next [`Self::renew_if_stale`] and deletes the snapshot file.
    pub fn invalidate(&self) {
        self.write().last_renew_ms = None;
        match fs::remove_file(&self.shared.snapshot_path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(target: "kuro_downloads", "could not delete index snapshot: {e}"),
        }
    }

    /// Rescans the root when the tree is older than an hour. Returns `true` when a scan replaced the tree.
    ///
    /// Waits up to 30 s for `ready` (the source registry). On timeout the previous tree is kept and the next call
    /// tries again. `installed` lists `(source id, display name)` and is called only after the wait.
    pub async fn renew_if_stale<F>(&self, now_ms: i64, mut ready: watch::Receiver<bool>, installed: F) -> bool
    where
        F: FnOnce() -> Vec<(i64, String)> + Send,
    {
        if self.read().last_renew_ms.is_some_and(|t| now_ms - t < RENEW_INTERVAL_MS) {
            return false;
        }
        if self.shared.scanning.swap(true, Ordering::SeqCst) {
            return false;
        }
        *self.shared.scan_started.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
        let _scan = ScanGuard(&self.shared);
        match tokio::time::timeout(SOURCE_WAIT, ready.wait_for(|v| *v)).await {
            Ok(Ok(_)) => {}
            _ => {
                tracing::warn!(target: "kuro_downloads", "source registry not ready; keeping the previous index");
                return false;
            }
        }
        let entries = installed();
        let root = self.shared.root.clone();
        let tree = match tokio::task::spawn_blocking(move || scan(&root, &entries)).await {
            Ok(tree) => tree,
            Err(e) => {
                tracing::warn!(target: "kuro_downloads", "index scan failed: {e}");
                return false;
            }
        };
        tracing::debug!(target: "kuro_downloads", sources = tree.sources.len(), "index scan finished");
        {
            let mut st = self.write();
            st.tree = tree;
            st.last_renew_ms = Some(now_ms);
        }
        self.touch();
        true
    }

    /// Rescans now, whatever the age of the tree: the sources changed (a repository load ended), so the folders of
    /// sources that were not known before are matched. Waits and returns as [`Self::renew_if_stale`].
    pub async fn rescan<F>(&self, ready: watch::Receiver<bool>, installed: F) -> bool
    where
        F: FnOnce() -> Vec<(i64, String)> + Send,
    {
        self.write().last_renew_ms = None;
        self.renew_if_stale(crate::downloader::unix_millis(), ready, installed).await
    }

    /// Notes a change: the snapshot is written one second after the last change.
    fn touch(&self) {
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        *self.shared.last_change.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        schedule_persist(self.shared.clone());
    }
}

struct ScanGuard<'a>(&'a Shared);

impl Drop for ScanGuard<'_> {
    fn drop(&mut self) {
        *self.0.scan_started.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.0.scanning.store(false, Ordering::SeqCst);
    }
}

/// A scan that runs longer than this shows the "Checking downloads" indicator (EC-80, T-23a-040).
pub const INDICATOR_DELAY: Duration = Duration::from_secs(1);

/// Whether the indicator shows for a scan that has run for `elapsed` (`None` when no scan runs).
pub fn checking_indicator_visible(elapsed: Option<Duration>) -> bool {
    elapsed.is_some_and(|e| e > INDICATOR_DELAY)
}

fn bare_name(name: &str) -> &str {
    name.strip_suffix(&format!(".{CBZ_EXTENSION}")).unwrap_or(name)
}

async fn sum_sizes(dirs: Vec<PathBuf>) -> u64 {
    tokio::task::spawn_blocking(move || dirs.iter().map(|d| kuro_core::storage::dir_size(d).unwrap_or(0)).sum())
        .await
        .unwrap_or(0)
}

/// Walks `root`. A folder counts as a source only when its name matches an installed source's sanitised display name,
/// ignoring case. Two folders that match one source are merged, the first by name winning a clash. Chapter folders
/// ending in `_tmp` are skipped. `.cbz` files count by their stem.
pub fn scan(root: &Path, installed: &[(i64, String)]) -> Tree {
    let mut tree = Tree::default();
    let mut source_paths = list_dirs(root);
    source_paths.sort();
    for source_path in source_paths {
        let folder = file_name(&source_path);
        let folder_lower = folder.to_lowercase();
        let Some((source_id, _)) = installed
            .iter()
            .find(|(_, display)| build_valid_filename(display).to_lowercase() == folder_lower)
        else {
            continue;
        };
        let source = tree
            .sources
            .entry(*source_id)
            .or_insert_with(|| SourceDir { dir_name: folder.clone(), manga: BTreeMap::new() });
        let mut manga_paths = list_dirs(&source_path);
        manga_paths.sort();
        for manga_path in manga_paths {
            let manga_name = file_name(&manga_path);
            if source.manga.contains_key(&manga_name) {
                continue;
            }
            let chapters = scan_chapters(&manga_path);
            source.manga.insert(
                manga_name.clone(),
                MangaDir { source_dir: folder.clone(), dir_name: manga_name, chapters },
            );
        }
    }
    tree
}

fn scan_chapters(manga_path: &Path) -> BTreeSet<String> {
    let mut chapters = BTreeSet::new();
    for entry in read_entries(manga_path) {
        let name = file_name(&entry);
        if entry.is_dir() {
            if !name.ends_with(TMP_DIR_SUFFIX) {
                chapters.insert(name);
            }
        } else if let Some(stem) = name.strip_suffix(&format!(".{CBZ_EXTENSION}")) {
            if !stem.is_empty() {
                chapters.insert(stem.to_owned());
            }
        }
    }
    chapters
}

fn read_entries(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default()
}

fn list_dirs(dir: &Path) -> Vec<PathBuf> {
    read_entries(dir).into_iter().filter(|p| p.is_dir()).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn load_snapshot(path: &Path, root: &Path) -> Option<Snapshot> {
    let bytes = fs::read(path).ok()?;
    let parsed = bytes
        .strip_prefix(SNAPSHOT_MAGIC)
        .and_then(|body| serde_json::from_slice::<Snapshot>(body).ok());
    match parsed {
        Some(snap) if snap.root == root => Some(snap),
        _ => {
            tracing::warn!(target: "kuro_downloads", "discarding unreadable index snapshot");
            let _ = fs::remove_file(path);
            None
        }
    }
}

fn write_snapshot(shared: &Shared) -> io::Result<()> {
    let (tree, built_at_ms) = {
        let st = shared.state.read().unwrap_or_else(|e| e.into_inner());
        (st.tree.clone(), st.last_renew_ms.unwrap_or(0))
    };
    let snap = Snapshot { root: shared.root.clone(), built_at_ms, tree };
    let mut bytes = SNAPSHOT_MAGIC.to_vec();
    bytes.extend(serde_json::to_vec(&snap).map_err(io::Error::other)?);
    if let Some(parent) = shared.snapshot_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = shared.snapshot_path.with_extension("bin.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, &shared.snapshot_path)
}

/// Waits until [`SNAPSHOT_DEBOUNCE`] has passed since the last change. A change during the wait moves the deadline, so
/// the caller returns one second after the last change, not one second after the first.
async fn wait_for_quiet(shared: &Shared) {
    loop {
        let deadline = *shared.last_change.lock().unwrap_or_else(|e| e.into_inner()) + SNAPSHOT_DEBOUNCE;
        if Instant::now() >= deadline {
            return;
        }
        tokio::time::sleep_until(deadline).await;
    }
}

/// Debounced writer: one task per burst of changes. It writes once no change has landed for [`SNAPSHOT_DEBOUNCE`],
/// and writes again if a change arrived while it was writing. Without a Tokio runtime the write waits for the next
/// change.
fn schedule_persist(shared: Arc<Shared>) {
    if shared.persist_scheduled.swap(true, Ordering::SeqCst) {
        return;
    }
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        shared.persist_scheduled.store(false, Ordering::SeqCst);
        return;
    };
    handle.spawn(async move {
        loop {
            wait_for_quiet(&shared).await;
            let seen = shared.generation.load(Ordering::SeqCst);
            // The snapshot is small, so it is written inline rather than on the blocking pool.
            if let Err(e) = write_snapshot(&shared) {
                tracing::warn!(target: "kuro_downloads", "index snapshot write failed: {e}");
            }
            if shared.generation.load(Ordering::SeqCst) != seen {
                continue;
            }
            shared.persist_scheduled.store(false, Ordering::SeqCst);
            if shared.generation.load(Ordering::SeqCst) == seen {
                break;
            }
            // A change landed after the check, while the flag was still set and nobody scheduled it: keep going
            // unless a newer change already took the flag and scheduled its own task.
            if shared.persist_scheduled.swap(true, Ordering::SeqCst) {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::jpeg_bytes;

    const ROOT_SOURCE: &str = "MangaDex (EN)";

    fn installed() -> Vec<(i64, String)> {
        vec![(1, ROOT_SOURCE.to_owned()), (2, "Other (XX)".to_owned())]
    }

    /// Builds the TST-37 fixture tree and returns its root.
    fn fixture(root: &Path) {
        let manga = root.join("MangaDex (EN)").join("Berserk");
        fs::create_dir_all(manga.join("Ch 1")).unwrap();
        fs::create_dir_all(manga.join("Ch 2_tmp")).unwrap();
        fs::write(manga.join("Ch 3.cbz"), b"zip").unwrap();
        fs::write(manga.join("Ch 4.zip"), b"zip").unwrap();
        fs::create_dir_all(root.join("unknown (xx)").join("Other")).unwrap();
        fs::create_dir_all(root.join("mangadex (en)").join("Vagabond").join("Ch 9")).unwrap();
        fs::create_dir_all(root.join("Other (XX)").join("Ignored")).unwrap();
    }

    #[test]
    fn scan_builds_source_manga_and_chapter_names() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let tree = scan(tmp.path(), &installed());
        let source = &tree.sources[&1];
        assert_eq!(source.dir_name, "MangaDex (EN)", "the first matching folder names the source");
        let berserk = &source.manga["Berserk"];
        assert_eq!(berserk.chapters.iter().cloned().collect::<Vec<_>>(), vec!["Ch 1", "Ch 3"]);
        assert!(source.manga.contains_key("Vagabond"), "a case-only folder match counts");
        assert!(!tree.sources.contains_key(&99));
        assert!(!tree.sources.values().any(|s| s.manga.contains_key("Other")), "unknown source folders are ignored");
    }

    // TST-41
    #[test]
    fn source_folder_matching_ignores_case() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("mangadex (en)").join("Vagabond")).unwrap();
        let tree = scan(tmp.path(), &installed());
        assert_eq!(tree.sources[&1].dir_name, "mangadex (en)");
    }

    #[test]
    fn queries_and_mutations_update_the_tree_without_a_rescan() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let cache = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(tmp.path(), cache.path());
        index.shared.state.write().unwrap().tree = scan(tmp.path(), &installed());
        assert!(index.is_chapter_downloaded(1, "Berserk", "Ch 1", None));
        assert!(index.is_chapter_downloaded(1, "Berserk", "Ch 3", None), "a .cbz counts");
        assert!(!index.is_chapter_downloaded(1, "Berserk", "Ch 4", None));
        assert_eq!(index.chapter_count_total(), 3);
        assert_eq!(index.chapter_count_manga(1, "Berserk"), 2);

        index.add_chapter(1, ROOT_SOURCE, "Berserk", "Ch 5.cbz");
        assert!(index.is_chapter_downloaded(1, "Berserk", "Ch 5", None));
        assert_eq!(index.shared.state.read().unwrap().tree.sources[&1].manga["Berserk"].chapters.iter().filter(|c| c.ends_with(".cbz")).count(), 0);

        index.remove_chapter(1, "Berserk", &["Ch 1".to_owned(), "Ch 3.cbz".to_owned()]);
        assert!(!index.is_chapter_downloaded(1, "Berserk", "Ch 1", None));
        index.remove_manga(1, "Berserk");
        assert_eq!(index.chapter_count_manga(1, "Berserk"), 0);
        index.remove_source(1);
        assert_eq!(index.chapter_count_total(), 0, "source 2 in the fixture has no chapters");
    }

    // TST-38
    #[tokio::test(start_paused = true)]
    async fn renew_runs_once_per_hour() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let cache = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(tmp.path(), cache.path());
        let (_tx, rx) = watch::channel(true);
        let start = 1_000_000;
        assert!(index.renew_if_stale(start, rx.clone(), installed).await);
        assert!(!index.renew_if_stale(start + RENEW_INTERVAL_MS - 1, rx.clone(), installed).await);
        assert!(index.renew_if_stale(start + RENEW_INTERVAL_MS, rx, installed).await);
    }

    #[tokio::test(start_paused = true)]
    async fn invalidate_forces_a_rescan() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(tmp.path(), cache.path());
        let (_tx, rx) = watch::channel(true);
        assert!(index.renew_if_stale(10, rx.clone(), installed).await);
        index.invalidate();
        assert!(index.renew_if_stale(11, rx, installed).await);
    }

    #[tokio::test(start_paused = true)]
    async fn a_rescan_recognises_the_folder_of_a_source_added_after_the_last_scan() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        // a repository source whose name needs the sanitiser: its folder is the sanitised display name
        let repo = (77, "Weeb: Central (EN)".to_owned());
        fs::create_dir_all(tmp.path().join(build_valid_filename(&repo.1)).join("Solo").join("Chapter 1")).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(tmp.path(), cache.path());
        let (_tx, rx) = watch::channel(true);
        assert!(index.renew_if_stale(crate::downloader::unix_millis(), rx.clone(), installed).await);
        assert!(!index.is_chapter_downloaded(77, "Solo", "Chapter 1", None), "not known before the repository loaded");

        let with_repo = move || -> Vec<(i64, String)> { installed().into_iter().chain([repo]).collect() };
        assert!(index.rescan(rx, with_repo).await, "a fresh tree is scanned again");
        assert!(index.is_chapter_downloaded(77, "Solo", "Chapter 1", None));
        assert!(index.is_chapter_downloaded(1, "Berserk", "Ch 1", None), "the other sources stay");
    }

    // TST-39
    #[tokio::test(start_paused = true)]
    async fn a_source_wait_timeout_keeps_the_previous_tree() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let cache = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(tmp.path(), cache.path());
        let (_tx, rx) = watch::channel(true);
        assert!(index.renew_if_stale(0, rx, installed).await);
        let before = index.read().tree.clone();
        assert_eq!(before.sources.len(), 2, "MangaDex (EN) and Other (XX)");

        // The registry never reports ready: the wait times out after 30 s and the old tree stays.
        index.invalidate();
        let (_never_tx, never_ready) = watch::channel(false);
        let renewed = index.renew_if_stale(5, never_ready, Vec::new).await;
        assert!(!renewed);
        assert_eq!(index.read().tree, before, "the old tree survives a failed wait");
    }

    // TST-40
    #[tokio::test(start_paused = true)]
    async fn corrupt_snapshot_is_deleted_and_the_tree_rebuilt() {
        let tmp = tempfile::tempdir().unwrap();
        fixture(tmp.path());
        let cache = tempfile::tempdir().unwrap();
        let snapshot = cache.path().join(SNAPSHOT_FILE);
        fs::write(&snapshot, include_bytes!("../testdata/index/corrupt.bin")).unwrap();
        let index = DownloadIndex::open(tmp.path(), cache.path());
        assert!(!snapshot.exists(), "corrupt snapshot removed on open");
        let (_tx, rx) = watch::channel(true);
        assert!(index.renew_if_stale(0, rx, installed).await);
        assert_eq!(index.chapter_count_total(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn snapshot_is_written_one_second_after_the_last_change() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(tmp.path(), cache.path());
        let snapshot = cache.path().join(SNAPSHOT_FILE);
        index.add_chapter(1, "S (EN)", "M", "Ch 1");
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(!snapshot.exists());
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(snapshot.exists());
        let reopened = DownloadIndex::open(tmp.path(), cache.path());
        assert!(reopened.is_chapter_downloaded(1, "M", "Ch 1", None));
    }

    #[tokio::test(start_paused = true)]
    async fn snapshot_timer_restarts_on_every_change() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(tmp.path(), cache.path());
        let snapshot = cache.path().join(SNAPSHOT_FILE);
        index.add_chapter(1, "S (EN)", "M", "Ch 1");
        tokio::time::sleep(Duration::from_millis(900)).await;
        index.add_chapter(1, "S (EN)", "M", "Ch 2");
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(!snapshot.exists(), "1.5 s after the first change but 0.6 s after the second: no write yet");
        tokio::time::sleep(Duration::from_millis(350)).await;
        assert!(!snapshot.exists(), "1.85 s: still less than a second after the second change");
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(snapshot.exists(), "1.95 s: one second after the second change");
        let reopened = DownloadIndex::open(tmp.path(), cache.path());
        assert!(reopened.is_chapter_downloaded(1, "M", "Ch 2", None));
    }

    #[tokio::test]
    async fn sizes_sum_the_indexed_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let manga = tmp.path().join("S (EN)").join("M");
        fs::create_dir_all(manga.join("Ch 1")).unwrap();
        fs::write(manga.join("Ch 1").join("001.jpg"), jpeg_bytes(100)).unwrap();
        fs::write(manga.join("Ch 2.cbz"), jpeg_bytes(50)).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(tmp.path(), cache.path());
        index.add_chapter(1, "S (EN)", "M", "Ch 1");
        index.add_chapter(1, "S (EN)", "M", "Ch 2.cbz");
        assert_eq!(index.size_manga(1, "M").await, 150);
        assert_eq!(index.size_total().await, 150);
        assert_eq!(index.size_manga(1, "Missing").await, 0);
    }
}

#[cfg(test)]
mod indicator_tests {
    use super::*;

    #[test]
    fn the_checking_indicator_waits_for_a_scan_longer_than_one_second() {
        assert!(!checking_indicator_visible(None), "no scan, no indicator");
        assert!(!checking_indicator_visible(Some(Duration::from_millis(900))), "EC-80: short scans show nothing");
        assert!(checking_indicator_visible(Some(Duration::from_millis(1100))));
    }

    #[test]
    fn a_finished_scan_clears_the_start_time() {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("downloads");
        std::fs::create_dir_all(&root).unwrap();
        let index = DownloadIndex::open(&root, base.path());
        assert!(index.scan_elapsed().is_none());
        assert!(!index.checking_indicator_visible());
    }
}

