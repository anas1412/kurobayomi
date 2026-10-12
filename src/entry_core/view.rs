//! Chapter view pipeline: unread, bookmarked and downloaded filters, then the stored sort. Next-unread lookup.
//! Upstream: `MangaScreenModel.kt:1205-1220` (`applyFilters`), `ChapterGetNextUnread.kt`.

use kuro_downloads::DownloadState;
use kuro_prefs::tristate::TriState;

use crate::entry_core::chapter::{ChapterRow, sort_rows};
use crate::entry_core::flags::ChapterFlags;

/// The rows the list shows, in view order. Filters run in upstream order, then the sort.
/// `downloaded_only` forces the downloaded filter to `ENABLED_IS` without changing the stored bits.
pub fn view<'a>(
    rows: &'a [ChapterRow],
    flags: ChapterFlags,
    downloaded_only: bool,
    is_local: bool,
) -> Vec<&'a ChapterRow> {
    let unread = flags.unread();
    let bookmarked = flags.bookmarked();
    let downloaded = flags.downloaded(downloaded_only);
    let mut out: Vec<&ChapterRow> = rows
        .iter()
        .filter(|r| apply(unread, || !r.read))
        .filter(|r| apply(bookmarked, || r.bookmark))
        .filter(|r| apply(downloaded, || is_downloaded(r, is_local)))
        .collect();
    sort_rows(flags, &mut out);
    out
}

/// `isDownloaded || isLocal`. Only `DownloadState::Downloaded` counts: queued, running and failed rows do not.
fn is_downloaded(row: &ChapterRow, is_local: bool) -> bool {
    is_local || row.download == DownloadState::Downloaded
}

/// `applyFilter` from `TriState.kt`: `DISABLED` keeps, `ENABLED_IS` keeps when the predicate holds,
/// `ENABLED_NOT` keeps when it does not.
fn apply(filter: TriState, predicate: impl FnOnce() -> bool) -> bool {
    filter.apply_filter(predicate)
}

/// `(selected_total, selected_visible)`: every selected chapter, then the selected ones the view shows. The two are
/// counted separately, so a selection hidden by a filter still counts in the total (E-04).
pub fn selection_counts(selected: &[i64], view: &[&ChapterRow]) -> (usize, usize) {
    let picked: std::collections::HashSet<i64> = selected.iter().copied().collect();
    let visible = view.iter().filter(|r| picked.contains(&r.id)).count();
    (picked.len(), visible)
}

/// `getNextUnread`: the first unread row in view order, or the last one when the view is descending.
/// `view` must already have the view filters applied. Returns the chapter id.
pub fn next_unread(view: &[&ChapterRow], descending: bool) -> Option<i64> {
    if descending {
        view.iter().rev().find(|r| !r.read).map(|r| r.id)
    } else {
        view.iter().find(|r| !r.read).map(|r| r.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_db::flags::item as c;

    fn row(
        id: i64,
        number: f64,
        read: bool,
        bookmark: bool,
        download: DownloadState,
    ) -> ChapterRow {
        ChapterRow {
            read,
            bookmark,
            download,
            ..ChapterRow::new(id, &format!("ch {id}"), number)
        }
    }

    /// Eight rows covering every read / bookmark / downloaded combination.
    fn fixture() -> Vec<ChapterRow> {
        let states = [DownloadState::Downloaded, DownloadState::NotDownloaded];
        let mut out = Vec::new();
        let mut id = 0;
        for read in [false, true] {
            for bookmark in [false, true] {
                for download in states {
                    id += 1;
                    out.push(row(id, id as f64, read, bookmark, download));
                }
            }
        }
        out
    }

    fn word(unread: TriState, downloaded: TriState, bookmarked: TriState) -> ChapterFlags {
        let group = |state: TriState, is: i64, not: i64| match state {
            TriState::DISABLED => 0,
            TriState::ENABLED_IS => is,
            TriState::ENABLED_NOT => not,
        };
        ChapterFlags(
            group(unread, c::SHOW_UNDONE, c::SHOW_DONE)
                | group(downloaded, c::SHOW_DOWNLOADED, c::SHOW_NOT_DOWNLOADED)
                | group(bookmarked, c::SHOW_BOOKMARKED, c::SHOW_NOT_BOOKMARKED),
        )
    }

    #[test]
    fn selection_total_and_visible_are_counted_separately() {
        let rows = fixture();
        // Only the unread rows are in the view; the selection holds one visible and one hidden chapter.
        let flags = word(TriState::ENABLED_IS, TriState::DISABLED, TriState::DISABLED);
        let shown = view(&rows, flags, false, false);
        let visible_id = shown[0].id;
        let hidden_id = rows.iter().find(|r| r.read).map(|r| r.id).unwrap();
        assert_eq!(selection_counts(&[visible_id, hidden_id], &shown), (2, 1));
        assert_eq!(selection_counts(&[hidden_id], &shown), (1, 0), "a hidden selection still counts in the total");
        assert_eq!(selection_counts(&[], &shown), (0, 0));
    }

    #[test]
    fn ten_thousand_rows_process_under_twenty_milliseconds() {
        let rows: Vec<ChapterRow> = (0..10_000).map(|i| row(i, i as f64, i % 3 == 0, i % 5 == 0, DownloadState::NotDownloaded)).collect();
        let started = std::time::Instant::now();
        let shown = view(&rows, ChapterFlags(0), false, false);
        let elapsed = started.elapsed();
        assert_eq!(shown.len(), rows.len());
        // The 20 ms bound is for release builds only; debug builds are not timed.
        if !cfg!(debug_assertions) {
            assert!(elapsed < std::time::Duration::from_millis(20), "took {elapsed:?}");
        }
    }

    fn keep(state: TriState, value: bool) -> bool {
        match state {
            TriState::DISABLED => true,
            TriState::ENABLED_IS => value,
            TriState::ENABLED_NOT => !value,
        }
    }

    #[test]
    fn view_all_27_combinations_match_reference() {
        let rows = fixture();
        let all = [
            TriState::DISABLED,
            TriState::ENABLED_IS,
            TriState::ENABLED_NOT,
        ];
        for u in all {
            for d in all {
                for b in all {
                    let flags = word(u, d, b);
                    let got: Vec<i64> = view(&rows, flags, false, false)
                        .iter()
                        .map(|r| r.id)
                        .collect();
                    let mut expected: Vec<i64> = rows
                        .iter()
                        .filter(|r| keep(u, !r.read))
                        .filter(|r| keep(b, r.bookmark))
                        .filter(|r| keep(d, r.download == DownloadState::Downloaded))
                        .map(|r| r.id)
                        .collect();
                    expected.sort();
                    let mut got_sorted = got.clone();
                    got_sorted.sort();
                    assert_eq!(
                        got_sorted, expected,
                        "unread {u:?} downloaded {d:?} bookmarked {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn downloaded_only_forces_downloaded_filter() {
        let rows = fixture();
        let flags = ChapterFlags(0);
        let shown: Vec<i64> = view(&rows, flags, true, false)
            .iter()
            .map(|r| r.id)
            .collect();
        assert!(
            shown
                .iter()
                .all(|id| rows[(*id - 1) as usize].download == DownloadState::Downloaded)
        );
        assert!(flags.filter_active(true));
    }

    #[test]
    fn local_manga_rows_all_count_as_downloaded() {
        let rows = vec![row(1, 1.0, false, false, DownloadState::NotDownloaded)];
        let flags = ChapterFlags(c::SHOW_DOWNLOADED);
        assert_eq!(view(&rows, flags, false, true).len(), 1);
        assert_eq!(view(&rows, flags, false, false).len(), 0);
        assert_eq!(rows[0].display_download(true), DownloadState::Downloaded);
    }

    #[test]
    fn queued_and_failed_rows_are_not_downloaded() {
        let rows = vec![
            row(1, 1.0, false, false, DownloadState::Queue),
            row(2, 2.0, false, false, DownloadState::Downloading),
            row(3, 3.0, false, false, DownloadState::Error),
        ];
        let flags = ChapterFlags(c::SHOW_DOWNLOADED);
        assert!(view(&rows, flags, false, false).is_empty());
    }

    #[test]
    fn next_unread_desc_last() {
        // View [3u, 2r, 1u] in descending order: the last unread is chapter 1.
        let rows = [
            row(3, 3.0, false, false, DownloadState::NotDownloaded),
            row(2, 2.0, true, false, DownloadState::NotDownloaded),
            row(1, 1.0, false, false, DownloadState::NotDownloaded),
        ];
        let refs: Vec<&ChapterRow> = rows.iter().collect();
        assert_eq!(next_unread(&refs, true), Some(1));
        assert_eq!(next_unread(&refs, false), Some(3));
    }

    #[test]
    fn next_unread_none_when_all_read() {
        let rows = [row(1, 1.0, true, false, DownloadState::NotDownloaded)];
        let refs: Vec<&ChapterRow> = rows.iter().collect();
        assert_eq!(next_unread(&refs, true), None);
        assert_eq!(next_unread(&[], false), None);
    }

    #[test]
    fn next_unread_uses_the_view_it_is_given() {
        // The bookmarked filter hides chapter 1, so the next unread in that view is chapter 2.
        let rows = [
            row(1, 1.0, false, false, DownloadState::NotDownloaded),
            row(2, 2.0, false, true, DownloadState::NotDownloaded),
        ];
        let flags = ChapterFlags(c::SHOW_BOOKMARKED);
        let v = view(&rows, flags, false, false);
        assert_eq!(next_unread(&v, false), Some(2));
    }
}
