//! The browse listing shared by manga and anime: folders under the root, filtered by query or by the latest window,
//! sorted, and mapped to the source's model (area:14 section 6, behavior 6.1 and 11.2).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use kuro_source::SortSelection;

use crate::error::LocalSourceError;
use crate::fs::{FsEntry, list_dir};
use crate::names::is_hidden;
use crate::sort::sort_entries;

/// `LATEST_THRESHOLD`: 7 days in milliseconds (604 800 000).
pub const LATEST_WINDOW_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Milliseconds since the epoch, now.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

/// Kotlin `getSearchManga` on the base folder. `latest` keeps folders modified within the window and ignores
/// `query`. Otherwise a blank `query` keeps everything and a non-blank one keeps names that contain it, ignoring
/// case. `sort` of `None` keeps the name order. A missing root gives no entries.
///
/// Runs on a blocking thread, so the directory reads do not hold up the async runtime.
pub async fn browse<T: Send + 'static>(
    root: Option<PathBuf>,
    query: String,
    latest: bool,
    sort: Option<SortSelection>,
    map: fn(&FsEntry) -> T,
) -> Result<Vec<T>, LocalSourceError> {
    let Some(root) = root else {
        return Ok(Vec::new());
    };
    crate::blocking(move || {
        let mut folders = folders_matching(&root, &query, latest)?;
        if let Some(sort) = sort {
            sort_entries(&mut folders, sort);
        }
        Ok(folders.iter().map(map).collect())
    })
    .await
}

fn folders_matching(root: &Path, query: &str, latest: bool) -> Result<Vec<FsEntry>, LocalSourceError> {
    let entries = match list_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let limit = now_ms() - LATEST_WINDOW_MS;
    let needle = query.to_lowercase();
    let keep_all = query.trim().is_empty();
    Ok(entries
        .into_iter()
        .filter(|e| e.is_dir && !is_hidden(&e.name))
        .filter(|e| {
            if latest {
                e.modified_ms >= limit
            } else {
                keep_all || e.name.to_lowercase().contains(&needle)
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sort::{LATEST_SORT, POPULAR_SORT};
    use std::fs;
    use std::io;
    use std::time::Duration;

    fn name_of(e: &FsEntry) -> String {
        e.name.clone()
    }

    fn make_folders(names: &[&str]) -> io::Result<tempfile::TempDir> {
        let dir = tempfile::tempdir()?;
        for name in names {
            fs::create_dir(dir.path().join(name))?;
        }
        Ok(dir)
    }

    #[tokio::test]
    async fn popular_lists_title_ascending_and_skips_dot_folders() -> io::Result<()> {
        let dir = make_folders(&["9", "10", "a", ".hidden"])?;
        fs::write(dir.path().join("loose.txt"), b"x")?;
        let got = browse(
            Some(dir.path().to_path_buf()),
            String::new(),
            false,
            Some(POPULAR_SORT),
            name_of,
        )
        .await;
        assert_eq!(got.map_err(io::Error::other)?, ["10", "9", "a"]);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn t_14_browse_entry_removed_mid_scan_skipped() -> io::Result<()> {
        // A child that vanishes between the listing and its metadata read has no metadata: a link whose target was
        // removed is the same state on disk.
        let dir = make_folders(&["Stays", "Target"])?;
        std::os::unix::fs::symlink(dir.path().join("Target"), dir.path().join("Gone"))?;
        fs::remove_dir(dir.path().join("Target"))?;
        let got = browse(Some(dir.path().to_path_buf()), String::new(), false, None, name_of).await;
        assert_eq!(got.map_err(io::Error::other)?, ["Stays"]);
        Ok(())
    }

    #[tokio::test]
    async fn a_blank_query_keeps_everything_and_a_query_filters_ignoring_case() -> io::Result<()> {
        let dir = make_folders(&["Naruto", "Bleach", "naruto x"])?;
        let root = Some(dir.path().to_path_buf());
        let all = browse(root.clone(), "   ".into(), false, None, name_of)
            .await
            .map_err(io::Error::other)?;
        assert_eq!(all.len(), 3);
        let some = browse(root, "NARUTO".into(), false, None, name_of)
            .await
            .map_err(io::Error::other)?;
        assert_eq!(some, ["Naruto", "naruto x"]);
        Ok(())
    }

    #[tokio::test]
    async fn search_without_a_sort_keeps_name_order() -> io::Result<()> {
        let dir = make_folders(&["b", "a", "c"])?;
        let got = browse(Some(dir.path().to_path_buf()), String::new(), false, None, name_of).await;
        assert_eq!(got.map_err(io::Error::other)?, ["a", "b", "c"]);
        Ok(())
    }

    #[tokio::test]
    async fn latest_keeps_recent_folders_newest_first_and_ignores_the_query() -> io::Result<()> {
        let dir = make_folders(&["old", "new", "newer"])?;
        let eight_days = SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
        let set_modified = kuro_platform::storage::set_modified;
        set_modified(&dir.path().join("old"), eight_days)?;
        let base = SystemTime::now() - Duration::from_secs(60);
        set_modified(&dir.path().join("new"), base)?;
        set_modified(&dir.path().join("newer"), SystemTime::now())?;
        let got = browse(
            Some(dir.path().to_path_buf()),
            "zzz".into(),
            true,
            Some(LATEST_SORT),
            name_of,
        )
        .await;
        assert_eq!(got.map_err(io::Error::other)?, ["newer", "new"]);
        Ok(())
    }

    #[tokio::test]
    async fn a_missing_root_is_an_empty_listing() {
        let got = browse(None, String::new(), false, Some(POPULAR_SORT), name_of).await;
        assert_eq!(got.expect("no error"), Vec::<String>::new());
        let base = tempfile::tempdir().expect("temp dir");
        let gone = browse(Some(base.path().join("gone")), String::new(), false, None, name_of).await;
        assert_eq!(gone.expect("no error"), Vec::<String>::new());
    }
}
