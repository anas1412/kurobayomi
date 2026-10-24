//! The one size limit of the app's caches (Settings › Data and storage, owner rule of 2026-10-23). Chapter pages, covers
//! and other images, side subtitle files and the last browse results share it; when their total is over it, the least
//! recently used files go first (recency is the file time, which every cache refreshes on a read). Library covers live in
//! the data folder and downloads in the storage folder, outside these folders, so neither is ever removed.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kuro_platform::paths::Paths;
use kuro_prefs::backup::StoragePrefs;
use kuro_prefs::ranges::CACHE_SIZE_MB;

/// A megabyte as the settings row writes it (`si_size`).
const MB: u64 = 1_000_000;
const DEFAULT_MB: u64 = 500;
/// How often the background trim runs.
const TRIM_EVERY: Duration = Duration::from_secs(10 * 60);
/// How often the background trim looks for a new limit in the settings.
const POLL: Duration = Duration::from_secs(20);

static LIMIT: AtomicU64 = AtomicU64::new(DEFAULT_MB * MB);

/// The limit in bytes. Each cache also keeps itself under it when it writes.
pub fn bytes() -> u64 {
    LIMIT.load(Ordering::Relaxed)
}

/// The stored setting in bytes; a value outside the choices goes to the nearest one.
pub fn mb_to_bytes(mb: i32) -> u64 {
    u64::try_from(CACHE_SIZE_MB.clamped(i64::from(mb))).unwrap_or(DEFAULT_MB) * MB
}

/// The folders the limit covers.
pub fn dirs(paths: &dyn Paths) -> Vec<PathBuf> {
    vec![paths.chapter_cache_dir(), paths.image_cache_dir(), paths.subtitle_cache_dir(), paths.browse_cache_dir()]
}

/// The cache folders of this desktop.
pub fn app_dirs() -> Vec<PathBuf> {
    kuro_platform::system_paths::SystemPaths::system().map(|paths| dirs(&paths)).unwrap_or_default()
}

/// A file that belongs to a write in progress or to a cache's own bookkeeping, which neither the trim nor a clear touches.
fn is_kept(name: &str) -> bool {
    name == "journal" || name.starts_with("journal.") || name.ends_with(".part") || name.contains(".tmp-")
}

struct File {
    path: PathBuf,
    len: u64,
    used: SystemTime,
}

fn files(dirs: &[PathBuf]) -> Vec<File> {
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || is_kept(&entry.file_name().to_string_lossy()) {
                continue;
            }
            out.push(File { path: entry.path(), len: meta.len(), used: meta.modified().unwrap_or(UNIX_EPOCH) });
        }
    }
    out
}

/// The bytes the caches in `dirs` hold.
pub fn used(dirs: &[PathBuf]) -> u64 {
    files(dirs).iter().map(|file| file.len).sum()
}

/// Removes the least recently used files of `dirs` until their total is within `limit`. Returns the bytes removed.
pub fn trim(dirs: &[PathBuf], limit: u64) -> u64 {
    let mut files = files(dirs);
    let mut total: u64 = files.iter().map(|file| file.len).sum();
    if total <= limit {
        return 0;
    }
    files.sort_by_key(|file| file.used);
    let mut removed = 0;
    for file in files {
        if total <= limit {
            break;
        }
        if std::fs::remove_file(&file.path).is_ok() {
            total -= file.len;
            removed += file.len;
        }
    }
    removed
}

/// Empties every cache in `dirs` and returns how many files were deleted. A folder that does not exist yet is empty.
pub fn clear(dirs: &[PathBuf]) -> io::Result<usize> {
    let mut deleted = 0;
    for file in files(dirs) {
        match std::fs::remove_file(&file.path) {
            Ok(()) => deleted += 1,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(deleted)
}

/// Reads the limit from the settings, then trims the caches on a background thread now, every ten minutes, and soon
/// after the setting changes. Call once at start.
pub fn start(prefs: &StoragePrefs, dirs: Vec<PathBuf>) {
    let setting = prefs.cache_size_mb.clone();
    LIMIT.store(mb_to_bytes(setting.get()), Ordering::Relaxed);
    let spawned = std::thread::Builder::new().name("cache-trim".into()).spawn(move || {
        let mut since_trim = TRIM_EVERY;
        loop {
            let limit = mb_to_bytes(setting.get());
            let changed = LIMIT.swap(limit, Ordering::Relaxed) != limit;
            if changed || since_trim >= TRIM_EVERY {
                let removed = trim(&dirs, limit);
                if removed > 0 {
                    tracing::info!(removed, limit, "cache over its size limit: least recently used files removed");
                }
                since_trim = Duration::ZERO;
            }
            std::thread::sleep(POLL);
            since_trim += POLL;
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "the cache trim does not run");
    }
}

/// The Reload action of a chapter or an episode: its cached files are forgotten, so the next open fetches them from
/// the site again (the chapter's page list and pages now, the episode's subtitle files at its next play).
pub fn forget_item(kind: kuro_platform::kind::Kind, item_id: i64) {
    if kind.is_anime() {
        return crate::player_view::sub_cache::fetch_again(item_id);
    }
    let item = kuro_data::item::ItemRepo::new(std::sync::Arc::clone(&crate::data::app().db)).by_id(item_id);
    let (Ok(Some(item)), Ok(paths)) = (item, kuro_platform::system_paths::SystemPaths::system()) else { return };
    match crate::reader::cache::DiskCache::open(paths.chapter_cache_dir(), bytes()) {
        Ok(cache) => {
            cache.forget_chapter(item.entry_id, &item.url);
        }
        Err(error) => tracing::warn!(%error, "the chapter cache could not be opened"),
    }
}

/// Marks a file as just used, so the trim keeps it longer. A failure only changes the order.
pub fn touch(path: &Path) {
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(dir: &Path, name: &str, len: usize, age_secs: u64) {
        let path = dir.join(name);
        std::fs::write(&path, vec![0u8; len]).unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(age_secs)).unwrap();
    }

    #[test]
    fn the_trim_removes_the_least_recently_used_files_across_every_cache_until_within_the_limit() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        put(a.path(), "old", 40, 300);
        put(b.path(), "older", 40, 400);
        put(a.path(), "new", 40, 10);
        put(b.path(), "journal", 40, 900);
        put(b.path(), "x.part", 40, 900);
        let dirs = vec![a.path().to_path_buf(), b.path().to_path_buf(), a.path().join("missing")];
        assert_eq!(used(&dirs), 120, "bookkeeping and writes in progress are not counted");
        assert_eq!(trim(&dirs, 50), 80);
        assert!(a.path().join("new").exists() && !a.path().join("old").exists() && !b.path().join("older").exists());
        assert!(b.path().join("journal").exists() && b.path().join("x.part").exists());
        assert_eq!(trim(&dirs, 50), 0, "within the limit: nothing more goes");
    }

    #[test]
    fn clearing_empties_every_cache_and_the_setting_maps_to_bytes() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        put(a.path(), "p", 1, 0);
        put(b.path(), "q", 1, 0);
        put(b.path(), "journal", 1, 0);
        assert_eq!(clear(&[a.path().to_path_buf(), b.path().to_path_buf(), a.path().join("gone")]).unwrap(), 2);
        assert!(b.path().join("journal").exists());
        assert_eq!(mb_to_bytes(500), 500 * MB);
        assert_eq!(mb_to_bytes(7), 100 * MB, "an unknown value goes to the nearest choice");
    }

    #[test]
    fn the_limit_covers_the_cache_folders_only() {
        let paths = kuro_platform::system_paths::SystemPaths::system().unwrap();
        let dirs = dirs(&paths);
        assert!(dirs.iter().all(|dir| dir.starts_with(paths.cache_dir())), "library covers and downloads are elsewhere");
        assert!(!dirs.contains(&paths.covers_dir()));
    }
}
