//! The last first page of each source listing (Popular, Latest, a search, a global-search row), so a source opened
//! again or a page gone back to shows its results at once while the site is asked again (owner rule, 2026-10-23). The
//! answer replaces them in place. Results older than [`MAX_AGE`] are dropped. One small JSON file per listing in the
//! browse results cache, keyed by the source, the query and the filters.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use kuro_core::hash::md5_hex;
use kuro_domain::source::browse::BrowseQuery;
use kuro_platform::paths::Paths;
use kuro_source::filter_list::FilterList;

use super::listing::{ListItem, ListedPage};

/// Saved results are shown for about a day.
pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The cache folder of this desktop; tests never touch it.
fn app_dir() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    kuro_platform::system_paths::SystemPaths::system().ok().map(|paths| paths.browse_cache_dir())
}

fn file(dir: &Path, source_id: i64, query: &BrowseQuery, filters: &FilterList) -> PathBuf {
    dir.join(format!("{}.json", md5_hex(format!("{source_id}\n{query:?}\n{filters:?}").as_bytes())))
}

/// The saved results of a listing, or `None` when there are none or they are older than [`MAX_AGE`] (then removed).
pub fn read_in(dir: &Path, source_id: i64, query: &BrowseQuery, filters: &FilterList) -> Option<Vec<ListItem>> {
    let path = file(dir, source_id, query, filters);
    let modified = std::fs::metadata(&path).and_then(|meta| meta.modified()).ok()?;
    if SystemTime::now().duration_since(modified).unwrap_or_default() > MAX_AGE {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    serde_json::from_slice(&std::fs::read(&path).ok()?).ok()
}

/// Saves the first page a listing got. An empty page is not saved.
pub fn write_in(dir: &Path, source_id: i64, query: &BrowseQuery, filters: &FilterList, items: &[ListItem]) {
    if items.is_empty() {
        return;
    }
    let written = serde_json::to_vec(items)
        .map_err(std::io::Error::other)
        .and_then(|raw| std::fs::create_dir_all(dir).and_then(|()| crate::reader::cache::write_atomic(&file(dir, source_id, query, filters), &raw)));
    if let Err(error) = written {
        tracing::debug!(%error, "the browse results were not saved");
    }
}

/// [`read_in`] in the app's cache folder, as the first page of a listing (more pages are asked as usual).
pub fn read(source_id: i64, query: &BrowseQuery, filters: &FilterList) -> Option<ListedPage> {
    read_in(&app_dir()?, source_id, query, filters).map(|items| ListedPage { items, has_next: false })
}

/// [`write_in`] in the app's cache folder.
pub fn write(source_id: i64, query: &BrowseQuery, filters: &FilterList, items: &[ListItem]) {
    if let Some(dir) = app_dir() {
        write_in(&dir, source_id, query, filters, items);
    }
}

/// Forgets every saved result: they name database rows, and Clear database removes the rows outside the library.
pub fn clear() {
    if let Some(dir) = app_dir() {
        let _ = crate::cache_limit::clear(&[dir]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_round_trip_per_source_query_and_filters_and_expire_after_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let none = FilterList::default();
        let items = vec![ListItem { id: 3, source_id: 9, url: "/a".into(), title: "A".into(), favorite: true, cover: Some("https://c".into()) }];
        assert!(read_in(dir.path(), 9, &BrowseQuery::Popular, &none).is_none());
        write_in(dir.path(), 9, &BrowseQuery::Popular, &none, &items);
        assert_eq!(read_in(dir.path(), 9, &BrowseQuery::Popular, &none), Some(items.clone()));
        assert!(read_in(dir.path(), 9, &BrowseQuery::Latest, &none).is_none(), "another listing");
        assert!(read_in(dir.path(), 8, &BrowseQuery::Popular, &none).is_none(), "another source");
        write_in(dir.path(), 9, &BrowseQuery::Search("x".into()), &none, &[]);
        assert!(read_in(dir.path(), 9, &BrowseQuery::Search("x".into()), &none).is_none(), "an empty page is not kept");

        let path = file(dir.path(), 9, &BrowseQuery::Popular, &none);
        let old = SystemTime::now() - MAX_AGE - Duration::from_secs(60);
        std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_modified(old).unwrap();
        assert!(read_in(dir.path(), 9, &BrowseQuery::Popular, &none).is_none());
        assert!(!path.exists(), "an expired page is removed");
    }
}
