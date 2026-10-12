//! Downloaded-episode index (T-24a-035, T-24a-036, T-24a-037; behaviour 8; edge cases E-D07, E-C03, E-D08).
//!
//! The index is an in-memory tree `source id -> anime folder -> episode names`. It is a snapshot, not the truth: the
//! user may delete folders at any time. Readers clone an `Arc` of the tree and iterate that, so a listing never waits
//! for a write. The tree is trusted for an hour after a scan or a snapshot load. Every change schedules a write of
//! `dl_anime_index_v3.bin` one second later, written to a temp file and renamed into place.
//!
//! Episode names are folder names, or file names without `.mp4` or `.mkv`. Folders ending in `_tmp` (unfinished
//! downloads) and folders with no installed source are left out.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kuro_source::local::is_local;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::anime::model::{AnimeRef, EpisodeRef};
use crate::anime::naming::{anime_dir_name, fat_safe, valid_episode_dir_names};
use crate::local::anime_episode_count;

/// The index is trusted this long after a scan (`renewInterval`, 1 hour).
pub const RENEW_INTERVAL_MS: i64 = 3_600_000;
/// Longest wait for the source registry before a renewal gives up and keeps the old tree (behaviour 8.5).
pub const SOURCE_WAIT: Duration = Duration::from_secs(30);
/// Delay between the last change and the snapshot write.
pub const WRITE_DELAY: Duration = Duration::from_secs(1);
/// `isInitializing` shows only after a scan has run this long (behaviour 8.1).
pub const INIT_DEBOUNCE: Duration = Duration::from_secs(1);
/// Snapshot file in the cache directory.
pub const SNAPSHOT_FILE: &str = "dl_anime_index_v3.bin";
const MAGIC: &[u8] = b"KAIDX\x01";
const UNFINISHED_SUFFIX: &str = "_tmp";

/// `source -> anime -> episodes`, as stored.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tree {
    pub sources: BTreeMap<i64, SourceDir>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDir {
    pub dir_name: String,
    pub animes: BTreeMap<String, AnimeDir>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimeDir {
    pub episodes: BTreeSet<String>,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    root: PathBuf,
    built_at_ms: i64,
    tree: Tree,
}

#[derive(Default)]
struct State {
    tree: Arc<Tree>,
    last_renew_ms: Option<i64>,
}

struct Shared {
    root: PathBuf,
    snapshot_path: PathBuf,
    state: RwLock<State>,
    generation: AtomicU64,
    changes: watch::Sender<u64>,
    scanning_since: Mutex<Option<Instant>>,
    scanning: AtomicBool,
}

/// Cheap to clone. All methods take `&self`.
#[derive(Clone)]
pub struct AnimeIndex {
    shared: Arc<Shared>,
    local_anime_root: Option<PathBuf>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl AnimeIndex {
    /// Creates the index for `root`. A snapshot in `cache_dir` is loaded when it was written for the same root. A file
    /// that fails to parse, or belongs to another root, is deleted.
    pub fn open(root: impl Into<PathBuf>, cache_dir: &Path) -> Self {
        let root = root.into();
        let snapshot_path = cache_dir.join(SNAPSHOT_FILE);
        let state = match load_snapshot(&snapshot_path, &root) {
            Some(snap) => State { tree: Arc::new(snap.tree), last_renew_ms: Some(snap.built_at_ms) },
            None => State::default(),
        };
        Self {
            shared: Arc::new(Shared {
                root,
                snapshot_path,
                state: RwLock::new(state),
                generation: AtomicU64::new(0),
                changes: watch::channel(0).0,
                scanning_since: Mutex::new(None),
                scanning: AtomicBool::new(false),
            }),
            local_anime_root: None,
        }
    }

    /// Sets the local anime folder (the folder that holds one sub-folder per local anime). The index does not scan it:
    /// [`episode_count_anime`](Self::episode_count_anime) reads it for the local source (T-24a-038).
    pub fn with_local_anime_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.local_anime_root = Some(root.into());
        self
    }

    pub fn root(&self) -> &Path {
        &self.shared.root
    }

    /// A receiver that counts the changes. It changes on every mutation and after each renewal.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.shared.changes.subscribe()
    }

    /// A snapshot of the tree. Later writes do not affect it (E-D07).
    pub fn tree(&self) -> Arc<Tree> {
        self.shared.state.read().unwrap_or_else(|e| e.into_inner()).tree.clone()
    }

    /// `isEpisodeDownloaded`: `true` when a folder or a `.mkv` or `.mp4` for the episode is in the index.
    pub fn is_episode_downloaded(&self, source_id: i64, anime_title: &str, episode: &EpisodeRef) -> bool {
        let tree = self.tree();
        let Some(anime) = tree.sources.get(&source_id).and_then(|s| s.animes.get(&anime_dir_name(anime_title))) else {
            return false;
        };
        valid_episode_dir_names(&episode.name, episode.scanlator.as_deref()).iter().any(|n| anime.episodes.contains(n))
    }

    /// `getTotalDownloadCount`: episodes in every anime folder.
    pub fn episode_count_total(&self) -> usize {
        self.tree().sources.values().flat_map(|s| s.animes.values()).map(|a| a.episodes.len()).sum()
    }

    /// `getDownloadCount(anime)`: episodes in one anime folder, 0 when it is not indexed. For the local source the count
    /// comes from the supported files in the anime folder under the local root, not from the index, and `anime_title`
    /// is the anime's url, which names that folder (`getFilesInAnimeDirectory(anime.url)`). T-24a-038.
    pub fn episode_count_anime(&self, source_id: i64, anime_title: &str) -> usize {
        if is_local(source_id) {
            let Some(root) = &self.local_anime_root else {
                return 0;
            };
            return anime_episode_count(&root.join(anime_title)).unwrap_or(0);
        }
        self.tree()
            .sources
            .get(&source_id)
            .and_then(|s| s.animes.get(&anime_dir_name(anime_title)))
            .map_or(0, |a| a.episodes.len())
    }

    /// `getTotalDownloadSize`: the recursive size of every indexed source folder. Walks the disk on a blocking thread.
    pub async fn size_total(&self) -> u64 {
        let dirs: Vec<PathBuf> = self
            .tree()
            .sources
            .values()
            .map(|s| self.shared.root.join(&s.dir_name))
            .collect();
        sum_sizes(dirs).await
    }

    /// `getDownloadSize(anime)`: the recursive size of one anime folder.
    pub async fn size_anime(&self, source_id: i64, anime_title: &str) -> u64 {
        let tree = self.tree();
        let Some(source) = tree.sources.get(&source_id) else {
            return 0;
        };
        let dir = self.shared.root.join(&source.dir_name).join(anime_dir_name(anime_title));
        if !source.animes.contains_key(&anime_dir_name(anime_title)) {
            return 0;
        }
        sum_sizes(vec![dir]).await
    }

    /// `addEpisode`: records a finished episode folder without a rescan.
    pub fn add_episode(&self, source_id: i64, source_dir: &str, anime_dir: &str, episode_dir: &str) {
        self.mutate(|tree| {
            let source = tree
                .sources
                .entry(source_id)
                .or_insert_with(|| SourceDir { dir_name: source_dir.to_owned(), animes: BTreeMap::new() });
            source
                .animes
                .entry(anime_dir.to_owned())
                .or_default()
                .episodes
                .insert(episode_dir.to_owned());
        });
    }

    /// `removeEpisodes`: removes both valid folder names of each episode.
    pub fn remove_episodes(&self, source_id: i64, anime: &AnimeRef, episodes: &[EpisodeRef]) {
        let anime_dir = anime_dir_name(&anime.title);
        self.mutate(|tree| {
            let Some(entry) = tree.sources.get_mut(&source_id).and_then(|s| s.animes.get_mut(&anime_dir)) else {
                return;
            };
            for episode in episodes {
                for name in valid_episode_dir_names(&episode.name, episode.scanlator.as_deref()) {
                    entry.episodes.remove(&name);
                }
            }
        });
    }

    /// `removeAnime`: drops the anime folder from the index.
    pub fn remove_anime(&self, source_id: i64, anime_title: &str) {
        let dir = anime_dir_name(anime_title);
        self.mutate(|tree| {
            if let Some(source) = tree.sources.get_mut(&source_id) {
                source.animes.remove(&dir);
            }
        });
    }

    /// `removeSource`: drops the source folder from the index.
    pub fn remove_source(&self, source_id: i64) {
        self.mutate(|tree| {
            tree.sources.remove(&source_id);
        });
    }

    /// Forgets the tree and the snapshot. The next [`AnimeIndex::renew_if_stale`] rescans.
    pub fn invalidate(&self) {
        {
            let mut st = self.shared.state.write().unwrap_or_else(|e| e.into_inner());
            st.last_renew_ms = None;
        }
        let _ = fs::remove_file(&self.shared.snapshot_path);
        self.notify();
    }

    /// `true` while a first scan has run longer than [`INIT_DEBOUNCE`] (`isInitializing`, debounced for the UI).
    pub fn is_initialising(&self, now: Instant) -> bool {
        let since = self.shared.scanning_since.lock().unwrap_or_else(|e| e.into_inner());
        since.is_some_and(|t| now.saturating_duration_since(t) >= INIT_DEBOUNCE)
    }

    /// Rescans when the tree is older than an hour. The source registry is awaited for up to [`SOURCE_WAIT`]; when it
    /// never becomes ready the old tree is kept (the Kotlin code would wipe it). `installed` is read after the wait, and
    /// gives `(source id, "Name (LANG)")` for each installed source. Returns `true` when a scan ran.
    pub async fn renew_if_stale(
        &self,
        now_ms: i64,
        mut ready: watch::Receiver<bool>,
        installed: impl FnOnce() -> Vec<(i64, String)>,
    ) -> bool {
        let fresh = {
            let st = self.shared.state.read().unwrap_or_else(|e| e.into_inner());
            st.last_renew_ms.is_some_and(|t| t + RENEW_INTERVAL_MS >= now_ms)
        };
        if fresh || self.shared.scanning.swap(true, Ordering::SeqCst) {
            return false;
        }
        let first_load = self.shared.state.read().unwrap_or_else(|e| e.into_inner()).last_renew_ms.is_none();
        if first_load {
            *self.shared.scanning_since.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
        }
        // The wait's result borrows the channel value; it is dropped here, before the scan, so the sender is never
        // blocked by a running scan and the future stays `Send`.
        let waited = tokio::time::timeout(SOURCE_WAIT, ready.wait_for(|v| *v)).await.is_ok();
        let ran = if waited {
            let sources = installed();
            let root = self.shared.root.clone();
            let tree = tokio::task::spawn_blocking(move || scan(&root, &sources)).await.unwrap_or_default();
            let mut st = self.shared.state.write().unwrap_or_else(|e| e.into_inner());
            st.tree = Arc::new(tree);
            st.last_renew_ms = Some(now_ms);
            true
        } else {
            let mut st = self.shared.state.write().unwrap_or_else(|e| e.into_inner());
            st.last_renew_ms = Some(now_ms);
            false
        };
        *self.shared.scanning_since.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.shared.scanning.store(false, Ordering::SeqCst);
        if ran {
            self.notify();
            self.schedule_write();
        }
        ran
    }

    /// Rescans now, whatever the age of the tree: the sources changed (a repository load ended), so the folders of
    /// sources that were not known before are matched. Waits and returns as [`AnimeIndex::renew_if_stale`].
    pub async fn rescan(&self, ready: watch::Receiver<bool>, installed: impl FnOnce() -> Vec<(i64, String)>) -> bool {
        self.shared.state.write().unwrap_or_else(|e| e.into_inner()).last_renew_ms = None;
        self.renew_if_stale(now_ms(), ready, installed).await
    }

    fn mutate(&self, change: impl FnOnce(&mut Tree)) {
        {
            let mut st = self.shared.state.write().unwrap_or_else(|e| e.into_inner());
            change(Arc::make_mut(&mut st.tree));
        }
        self.notify();
        self.schedule_write();
    }

    fn notify(&self) {
        self.shared.changes.send_modify(|n| *n = n.wrapping_add(1));
    }

    /// Writes the snapshot `WRITE_DELAY` after the last change. Each change restarts the timer. Without a runtime the
    /// write is skipped, and the next change or renewal writes it.
    fn schedule_write(&self) {
        let generation = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let index = self.clone();
        handle.spawn(async move {
            tokio::time::sleep(WRITE_DELAY).await;
            if index.shared.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let (tree, built) = {
                let st = index.shared.state.read().unwrap_or_else(|e| e.into_inner());
                (st.tree.clone(), st.last_renew_ms.unwrap_or_else(now_ms))
            };
            let path = index.shared.snapshot_path.clone();
            let root = index.shared.root.clone();
            let _ = tokio::task::spawn_blocking(move || write_snapshot(&path, &root, built, &tree)).await;
        });
    }
}

/// Scans the downloads root for installed sources and their anime and episode folders (behaviour 8.5). `installed`
/// gives `(source id, "Name (LANG)")`; a folder matches the sanitised name, ignoring case.
pub fn scan(root: &Path, installed: &[(i64, String)]) -> Tree {
    let mut tree = Tree::default();
    let by_name: BTreeMap<String, i64> = installed.iter().map(|(id, name)| (fat_safe(name).to_lowercase(), *id)).collect();
    for source_path in list_dirs(root) {
        let name = file_name(&source_path);
        let Some(&source_id) = by_name.get(&name.to_lowercase()) else {
            continue;
        };
        let mut animes = BTreeMap::new();
        for anime_path in list_dirs(&source_path) {
            animes.insert(file_name(&anime_path), AnimeDir { episodes: scan_episodes(&anime_path) });
        }
        tree.sources.insert(source_id, SourceDir { dir_name: name, animes });
    }
    tree
}

fn scan_episodes(anime_path: &Path) -> BTreeSet<String> {
    let Ok(entries) = fs::read_dir(anime_path) else {
        return BTreeSet::new();
    };
    let mut episodes = BTreeSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = file_name(&path);
        if name.trim().is_empty() || name.ends_with(UNFINISHED_SUFFIX) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            episodes.insert(name);
        } else if meta.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "mkv" || ext == "mp4" {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    episodes.insert(stem.to_owned());
                }
            }
        }
    }
    episodes
}

fn list_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.metadata().is_ok_and(|m| m.is_dir()) && !e.file_name().to_string_lossy().trim().is_empty())
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The recursive size of a folder: the sum of its file lengths. Symbolic links are not followed.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => dir_size(&entry.path()),
            Ok(meta) if meta.is_file() => meta.len(),
            _ => 0,
        })
        .sum()
}

async fn sum_sizes(dirs: Vec<PathBuf>) -> u64 {
    tokio::task::spawn_blocking(move || dirs.iter().map(|d| dir_size(d)).sum()).await.unwrap_or(0)
}

fn load_snapshot(path: &Path, root: &Path) -> Option<Snapshot> {
    let bytes = fs::read(path).ok()?;
    let parsed = bytes
        .strip_prefix(MAGIC)
        .and_then(|json| serde_json::from_slice::<Snapshot>(json).ok())
        .filter(|snap| snap.root == root);
    if parsed.is_none() {
        let _ = fs::remove_file(path);
    }
    parsed
}

fn write_snapshot(path: &Path, root: &Path, built_at_ms: i64, tree: &Tree) -> io::Result<()> {
    let snapshot = Snapshot { root: root.to_path_buf(), built_at_ms, tree: tree.clone() };
    let mut bytes = MAGIC.to_vec();
    bytes.extend(serde_json::to_vec(&snapshot).map_err(io::Error::other)?);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anime(id: i64, title: &str) -> AnimeRef {
        AnimeRef { id, source_id: 7, url: format!("/a/{id}"), title: title.into() }
    }

    fn ep(name: &str) -> EpisodeRef {
        EpisodeRef {
            id: 1,
            anime_id: 1,
            url: String::new(),
            name: name.into(),
            scanlator: None,
            seen: false,
            bookmark: false,
            fillermark: false,
            source_order: 1,
            upload_date: 0,
            episode_number: 0.0,
        }
    }

    #[test]
    fn the_local_source_counts_files_in_its_folder_not_the_index() {
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("local");
        fs::create_dir_all(local.join("Show")).unwrap();
        fs::write(local.join("Show").join("Ep 1.mkv"), b"v").unwrap();
        fs::write(local.join("Show").join("Ep 2.mp4"), b"v").unwrap();
        fs::write(local.join("Show").join("cover.jpg"), b"x").unwrap();
        let index = AnimeIndex::open(dir.path().join("downloads"), dir.path()).with_local_anime_root(&local);
        // An index entry for source 0 must not be used.
        index.add_episode(0, "Local (OTHER)", "Show", "Indexed 1");
        assert_eq!(index.episode_count_anime(0, "Show"), 2, "two supported files on disk");
        assert_eq!(index.episode_count_anime(0, "Missing"), 0);
        assert_eq!(index.episode_count_anime(7, "Show"), 0, "other sources still use the index");
        let no_root = AnimeIndex::open(dir.path().join("downloads"), dir.path());
        assert_eq!(no_root.episode_count_anime(0, "Show"), 0, "no local root, no count");
    }

    /// `<root>/Demo (EN)/Show/{Episode 1.mkv, Episode 2/, Episode 3_tmp/, notes.txt}` plus an uninstalled source.
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("downloads");
        let show = root.join("Demo (EN)").join("Show");
        fs::create_dir_all(show.join("Episode 2")).unwrap();
        fs::create_dir_all(show.join("Episode 3_tmp")).unwrap();
        fs::write(show.join("Episode 1.mkv"), b"12345").unwrap();
        fs::write(show.join("Episode 4.webm"), b"x").unwrap();
        fs::write(show.join("notes.txt"), b"hi").unwrap();
        fs::create_dir_all(root.join("Gone (EN)").join("Other")).unwrap();
        (dir, root)
    }

    fn installed() -> Vec<(i64, String)> {
        vec![(7, "Demo (EN)".to_owned())]
    }

    #[test]
    fn scan_records_mkv_mp4_and_folders_only() {
        let (_dir, root) = fixture();
        let tree = scan(&root, &installed());
        let anime = &tree.sources[&7].animes["Show"];
        let names: Vec<&str> = anime.episodes.iter().map(String::as_str).collect();
        assert_eq!(names, vec!["Episode 1", "Episode 2"]);
        assert!(!tree.sources.contains_key(&99), "uninstalled folders are ignored");
        assert_eq!(tree.sources.len(), 1);
    }

    #[test]
    fn queries_use_the_valid_episode_names() {
        let (_dir, root) = fixture();
        let index = AnimeIndex::open(&root, &root.join("cache"));
        *index.shared.state.write().unwrap() = State { tree: Arc::new(scan(&root, &installed())), last_renew_ms: Some(now_ms()) };
        assert!(index.is_episode_downloaded(7, "Show", &ep("Episode 1")));
        assert!(!index.is_episode_downloaded(7, "Show", &ep("Episode 9")));
        assert!(!index.is_episode_downloaded(7, "Other", &ep("Episode 1")));
        assert_eq!(index.episode_count_total(), 2);
        assert_eq!(index.episode_count_anime(7, "Show"), 2);
        assert_eq!(index.episode_count_anime(7, "Missing"), 0);
    }

    #[test]
    fn a_snapshot_taken_before_a_write_does_not_change() {
        let index = AnimeIndex::open("/nonexistent-root", Path::new("/nonexistent-cache"));
        index.add_episode(7, "Demo (EN)", "Show", "Episode 1");
        let before = index.tree();
        index.add_episode(7, "Demo (EN)", "Show", "Episode 2");
        assert_eq!(before.sources[&7].animes["Show"].episodes.len(), 1, "the earlier snapshot is untouched");
        assert_eq!(index.tree().sources[&7].animes["Show"].episodes.len(), 2);
    }

    #[test]
    fn removing_an_episode_drops_its_folder_and_the_anime_drops_with_it() {
        let index = AnimeIndex::open("/nonexistent-root", Path::new("/nonexistent-cache"));
        let scanned = EpisodeRef { scanlator: Some("grp".into()), ..ep("Episode 1") };
        index.add_episode(7, "Demo (EN)", "Show", "grp_Episode 1");
        index.add_episode(7, "Demo (EN)", "Show", "Episode 2");
        index.remove_episodes(7, &anime(1, "Show"), &[scanned]);
        assert_eq!(index.episode_count_total(), 1, "only the matching folder goes");
        index.remove_episodes(7, &anime(1, "Show"), &[ep("Episode 2")]);
        assert_eq!(index.episode_count_total(), 0);
        index.add_episode(7, "Demo (EN)", "Show", "Episode 5");
        index.remove_anime(7, "Show");
        assert_eq!(index.episode_count_total(), 0);
        index.add_episode(7, "Demo (EN)", "Show", "Episode 5");
        index.remove_source(7);
        assert!(index.tree().sources.is_empty());
    }

    #[test]
    fn dir_size_sums_file_lengths_and_zero_length_files_count_zero() {
        let (_dir, root) = fixture();
        let show = root.join("Demo (EN)").join("Show");
        fs::write(show.join("Episode 2").join("empty.bin"), b"").unwrap();
        fs::write(show.join("Episode 2").join("a.bin"), b"abcd").unwrap();
        assert_eq!(dir_size(&show), 5 + 1 + 2 + 4);
        assert_eq!(dir_size(&root.join("missing")), 0);
    }

    #[tokio::test]
    async fn sizes_come_from_the_disk_for_indexed_folders() {
        let (_dir, root) = fixture();
        let index = AnimeIndex::open(&root, &root.join("cache"));
        let (_tx, rx) = watch::channel(true);
        assert!(index.renew_if_stale(now_ms(), rx, installed).await);
        assert!(index.size_total().await >= 5 + 2);
        assert_eq!(index.size_anime(7, "Show").await, dir_size(&root.join("Demo (EN)").join("Show")));
        assert_eq!(index.size_anime(7, "Unknown").await, 0);
    }

    #[tokio::test]
    async fn renewal_is_skipped_within_an_hour_and_runs_when_stale() {
        let (_dir, root) = fixture();
        let index = AnimeIndex::open(&root, &root.join("cache"));
        let (_tx, rx) = watch::channel(true);
        let t0 = now_ms();
        assert!(index.renew_if_stale(t0, rx.clone(), installed).await);
        assert!(!index.renew_if_stale(t0 + RENEW_INTERVAL_MS - 1, rx.clone(), installed).await, "still fresh");
        assert!(index.renew_if_stale(t0 + RENEW_INTERVAL_MS + 1, rx, installed).await, "an hour later it rescans");
    }

    #[tokio::test]
    async fn a_rescan_recognises_the_folder_of_a_source_added_after_the_last_scan() {
        let (_dir, root) = fixture();
        // a repository source whose name needs the sanitiser: its folder is the sanitised display name
        let repo = (88, "Kick: Ass (EN)".to_owned());
        fs::create_dir_all(root.join(fat_safe(&repo.1)).join("Show").join("Episode 1")).unwrap();
        let index = AnimeIndex::open(&root, &root.join("cache"));
        let (_tx, rx) = watch::channel(true);
        assert!(index.renew_if_stale(now_ms(), rx.clone(), installed).await);
        assert!(!index.is_episode_downloaded(88, "Show", &ep("Episode 1")), "not known before the repository loaded");

        let with_repo = move || -> Vec<(i64, String)> { installed().into_iter().chain([repo]).collect() };
        assert!(index.rescan(rx, with_repo).await, "a fresh tree is scanned again");
        assert!(index.is_episode_downloaded(88, "Show", &ep("Episode 1")));
        assert!(index.is_episode_downloaded(7, "Show", &ep("Episode 1")), "the other sources stay");
    }

    #[tokio::test(start_paused = true)]
    async fn a_source_wait_that_never_ends_keeps_the_previous_tree() {
        let (_dir, root) = fixture();
        let index = AnimeIndex::open(&root, &root.join("cache"));
        let (_tx, rx) = watch::channel(false);
        index.add_episode(7, "Demo (EN)", "Show", "Keep me");
        let t0 = now_ms();
        assert!(!index.renew_if_stale(t0, rx, installed).await);
        assert_eq!(index.episode_count_total(), 1, "the old tree stays");
    }

    #[test]
    fn initialising_is_debounced_by_one_second() {
        let index = AnimeIndex::open("/nonexistent-root", Path::new("/nonexistent-cache"));
        let start = Instant::now();
        *index.shared.scanning_since.lock().unwrap() = Some(start);
        assert!(!index.is_initialising(start + Duration::from_millis(500)));
        assert!(index.is_initialising(start + Duration::from_millis(1000)));
        *index.shared.scanning_since.lock().unwrap() = None;
        assert!(!index.is_initialising(start + Duration::from_secs(9)));
    }

    #[tokio::test]
    async fn the_snapshot_is_written_after_the_delay_and_reloaded_for_the_same_root() {
        let (_dir, root) = fixture();
        let cache = root.parent().unwrap().join("cache");
        let index = AnimeIndex::open(&root, &cache);
        index.add_episode(7, "Demo (EN)", "Show", "Episode 1");
        assert!(!cache.join(SNAPSHOT_FILE).exists(), "nothing is written before the delay");
        tokio::time::sleep(WRITE_DELAY + Duration::from_millis(500)).await;
        assert!(cache.join(SNAPSHOT_FILE).exists());
        assert!(!cache.join("dl_anime_index_v3.tmp").exists(), "the temp file is renamed away");

        let reloaded = AnimeIndex::open(&root, &cache);
        assert_eq!(reloaded.episode_count_total(), 1);
        let other = AnimeIndex::open(root.parent().unwrap().join("elsewhere"), &cache);
        assert_eq!(other.episode_count_total(), 0, "a snapshot for another root is discarded");
        assert!(!cache.join(SNAPSHOT_FILE).exists());
    }

    #[test]
    fn a_corrupt_snapshot_is_deleted_on_open() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SNAPSHOT_FILE), b"not a snapshot").unwrap();
        let index = AnimeIndex::open(dir.path().join("root"), dir.path());
        assert_eq!(index.episode_count_total(), 0);
        assert!(!dir.path().join(SNAPSHOT_FILE).exists());
    }

    #[test]
    fn invalidate_forgets_the_snapshot_and_the_renewal_time() {
        let index = AnimeIndex::open("/nonexistent-root", Path::new("/nonexistent-cache"));
        index.shared.state.write().unwrap().last_renew_ms = Some(now_ms());
        index.invalidate();
        assert!(index.shared.state.read().unwrap().last_renew_ms.is_none());
    }

    #[test]
    fn the_tmp_fixture_tree_lists_only_the_finished_episode_folders() {
        let tree = crate::testing::dl_fixture("tree_tmp");
        let scanned = scan(tree.path(), &[(1, "Example (EN)".to_owned())]);
        let episodes: Vec<&str> =
            scanned.sources[&1].animes["Show"].episodes.iter().map(String::as_str).collect();
        assert!(episodes.is_empty(), "staging folders are not episodes: {episodes:?}");
    }

    #[test]
    fn the_new_fixture_tree_lists_its_episode_folder_and_source_names_match_ignoring_case() {
        let tree = crate::testing::dl_fixture("tree_new");
        let scanned = scan(tree.path(), &[(1, "Example (EN)".to_owned())]);
        assert!(scanned.sources[&1].animes["Show"].episodes.contains("Scan_Episode 1"));

        let cased = crate::testing::dl_fixture("tree_case");
        let scanned = scan(cased.path(), &[(1, "example (en)".to_owned())]);
        assert!(scanned.sources.contains_key(&1), "a source folder matches whatever its case");
        assert!(scanned.sources[&1].animes["Show"].episodes.contains("Episode 1"));
    }
}
