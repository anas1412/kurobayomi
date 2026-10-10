//! The downloaded-only filter of the reader and the player (T-52a-010): the local source passes everything, every
//! other source keeps only the units found in the download index. The filter applies only while downloaded-only is on.

use kuro_downloads::index::DownloadIndex;
use kuro_local::is_local_source;

/// Applies the filter. `downloaded_only` off, or the local source, returns `units` unchanged.
pub fn keep_downloaded<T>(downloaded_only: bool, source_id: i64, units: Vec<T>, is_downloaded: impl Fn(&T) -> bool) -> Vec<T> {
    if !downloaded_only || is_local_source(source_id) {
        return units;
    }
    units.into_iter().filter(|unit| is_downloaded(unit)).collect()
}

/// The index lookup for one entry: `(unit name, scanlator)` to downloaded or not. Call it from the reader's
/// `is_downloaded` with the entry's source id and title.
pub fn lookup(index: DownloadIndex, source_id: i64, title: String) -> impl Fn(&str, Option<&str>) -> bool {
    move |name, scanlator| index.is_chapter_downloaded(source_id, &title, name, scanlator)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units() -> Vec<&'static str> {
        vec!["ch 1", "ch 2", "ch 3"]
    }

    #[test]
    fn the_local_source_returns_its_units_unchanged() {
        assert_eq!(keep_downloaded(true, 0, units(), |_| false), units());
    }

    #[test]
    fn another_source_keeps_only_the_units_the_lookup_finds() {
        assert_eq!(keep_downloaded(true, 7, units(), |u| *u != "ch 2"), vec!["ch 1", "ch 3"]);
    }

    #[test]
    fn with_downloaded_only_off_nothing_is_filtered() {
        assert_eq!(keep_downloaded(false, 7, units(), |_| false), units());
    }

    #[test]
    fn an_empty_index_finds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let find = lookup(DownloadIndex::open(dir.path(), dir.path()), 7, "Title".into());
        assert!(!find("ch 1", None));
        assert!(keep_downloaded(true, 7, units(), |u| find(u, None)).is_empty());
    }
}
