//! The chapter row the details page works on, and the chapter comparator (`getChapterSort`).
//! Upstream: `ChapterSorter.kt:7-30` (table in `plans/38-ui-entry-details-manga/p2-behavior.md` 6.4).
//! The comparator and collation are the shared `kuro_domain::items::sorter` ones; this file only adapts the row.

use std::cmp::Ordering;

use kuro_domain::items::sorter::{SortItem, compare_items};
use kuro_downloads::DownloadState;

use crate::entry_core::flags::ChapterFlags;

/// One chapter as the details page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ChapterRow {
    pub id: i64,
    pub name: String,
    pub number: f64,
    pub source_order: i64,
    pub date_upload: i64,
    pub read: bool,
    pub bookmark: bool,
    pub last_page_read: i64,
    pub scanlator: Option<String>,
    pub download: DownloadState,
    pub download_progress: u8,
}

impl ChapterRow {
    /// A row with no read, bookmark or download state. Used by tests and by loaders before state is known.
    pub fn new(id: i64, name: &str, number: f64) -> Self {
        Self {
            id,
            name: name.to_owned(),
            number,
            source_order: id,
            date_upload: 0,
            read: false,
            bookmark: false,
            last_page_read: 0,
            scanlator: None,
            download: DownloadState::NotDownloaded,
            download_progress: 0,
        }
    }

    /// The download state shown for this row. Local manga rows are always downloaded and never queued.
    pub fn display_download(&self, is_local: bool) -> DownloadState {
        if is_local {
            DownloadState::Downloaded
        } else {
            self.download
        }
    }
}

impl SortItem for ChapterRow {
    fn source_order(&self) -> i64 {
        self.source_order
    }
    fn number(&self) -> f64 {
        self.number
    }
    fn date_upload(&self) -> i64 {
        self.date_upload
    }
    fn name(&self) -> &str {
        &self.name
    }
}

/// `getChapterSort(manga)`: the comparator for the sort key and direction stored in `flags`.
pub fn compare(flags: ChapterFlags, a: &ChapterRow, b: &ChapterRow) -> Ordering {
    compare_items(flags.sort_key(), flags.sort_descending(), a, b)
}

/// Sorts `rows` in place with the stored sort. The sort is stable, so equal keys keep their input order.
pub fn sort_rows(flags: ChapterFlags, rows: &mut [&ChapterRow]) {
    rows.sort_by(|a, b| compare(flags, a, b));
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_domain::items::sorter::collate;

    fn row(id: i64, name: &str, number: f64, source_order: i64, date: i64) -> ChapterRow {
        ChapterRow {
            source_order,
            date_upload: date,
            ..ChapterRow::new(id, name, number)
        }
    }

    fn ids(rows: &[ChapterRow], flags: u32) -> Vec<i64> {
        let mut refs: Vec<&ChapterRow> = rows.iter().collect();
        sort_rows(ChapterFlags(flags as i64), &mut refs);
        refs.iter().map(|r| r.id).collect()
    }

    #[test]
    fn source_desc_is_newest_first() {
        // Source order 0 is the newest chapter. Default flags (source, descending) list it first.
        let rows = [
            row(2, "c", 3.0, 2, 0),
            row(0, "a", 1.0, 0, 0),
            row(1, "b", 2.0, 1, 0),
        ];
        assert_eq!(ids(&rows, 0), vec![0, 1, 2]);
    }

    #[test]
    fn source_asc_is_oldest_first() {
        let rows = [
            row(2, "c", 3.0, 2, 0),
            row(0, "a", 1.0, 0, 0),
            row(1, "b", 2.0, 1, 0),
        ];
        assert_eq!(ids(&rows, 0x1), vec![2, 1, 0]);
    }

    #[test]
    fn number_desc_and_asc() {
        let rows = [
            row(1, "a", 1.0, 0, 0),
            row(2, "b", 3.0, 0, 0),
            row(3, "c", 2.0, 0, 0),
        ];
        assert_eq!(ids(&rows, 0x100), vec![2, 3, 1]);
        assert_eq!(ids(&rows, 0x101), vec![1, 3, 2]);
    }

    #[test]
    fn number_unrecognised_first_asc_and_last_desc() {
        let rows = [
            row(1, "a", 1.0, 0, 0),
            row(2, "x", -1.0, 0, 0),
            row(3, "c", 2.0, 0, 0),
        ];
        assert_eq!(ids(&rows, 0x101), vec![2, 1, 3], "-1.0 first ascending");
        assert_eq!(ids(&rows, 0x100), vec![3, 1, 2], "-1.0 last descending");
    }

    #[test]
    fn nan_numbers_do_not_panic_and_sort_last_ascending() {
        let rows = [row(1, "a", f64::NAN, 0, 0), row(2, "b", 1.0, 0, 0)];
        assert_eq!(ids(&rows, 0x101), vec![2, 1]);
    }

    #[test]
    fn upload_date_desc() {
        let rows = [
            row(1, "a", 1.0, 0, 10),
            row(2, "b", 2.0, 0, 30),
            row(3, "c", 3.0, 0, 20),
        ];
        assert_eq!(ids(&rows, 0x200), vec![2, 3, 1]);
        assert_eq!(ids(&rows, 0x201), vec![1, 3, 2]);
    }

    #[test]
    fn alphabet_primary_case_and_accents_compare_equal() {
        assert_eq!(collate("apple", "APPLE"), Ordering::Equal);
        assert_eq!(collate("resume", "résumé"), Ordering::Equal);
        let rows = [
            row(1, "banana", 1.0, 0, 0),
            row(2, "Apple", 2.0, 0, 0),
            row(3, "cherry", 3.0, 0, 0),
        ];
        assert_eq!(ids(&rows, 0x300), vec![3, 1, 2]);
        assert_eq!(ids(&rows, 0x301), vec![2, 1, 3]);
    }

    #[test]
    fn alphabet_handles_japanese_arabic_cyrillic_and_accents() {
        assert_eq!(collate("第一話", "第一話"), Ordering::Equal);
        assert_ne!(collate("あ", "か"), Ordering::Equal);
        assert_ne!(collate("ب", "ت"), Ordering::Equal);
        assert_eq!(collate("Глава", "глава"), Ordering::Equal);
        assert_eq!(collate("École", "ecole"), Ordering::Equal);
    }

    #[test]
    fn sort_is_stable_for_equal_keys() {
        let rows = [
            row(1, "same", 5.0, 0, 0),
            row(2, "same", 5.0, 0, 0),
            row(3, "same", 5.0, 0, 0),
        ];
        assert_eq!(ids(&rows, 0x100), vec![1, 2, 3]);
        assert_eq!(ids(&rows, 0x101), vec![1, 2, 3]);
    }
}
