//! Planning for read / unread and bookmark writes. The store applies the plan in one update.
//! Upstream: `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt`,
//! `MangaScreenModel.kt:832-840` (bookmarkChapters). Plan: `p2-behavior.md` 6.9.9a and 6.10.

use crate::entry_core::chapter::ChapterRow;

/// One chapter update. `last_page_read` is `Some(0)` when marking unread, and `None` (unchanged) when marking read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadWrite {
    pub id: i64,
    pub read: bool,
    pub last_page_read: Option<i64>,
}

/// The read plan. `delete_downloads` lists the changed chapters whose downloads go after a mark-read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadPlan {
    pub writes: Vec<ReadWrite>,
    pub delete_downloads: Vec<i64>,
}

/// Plans a mark read (`read = true`) or mark unread (`read = false`) over `chapters`.
/// Mark read writes only rows that are not read yet. Mark unread writes rows that are read or have progress,
/// and resets their progress. Returns `None` when nothing changes (the upstream `Result.NoChapters`).
pub fn plan_mark_read(
    read: bool,
    chapters: &[ChapterRow],
    remove_after_read: bool,
) -> Option<ReadPlan> {
    let changed: Vec<&ChapterRow> = chapters
        .iter()
        .filter(|c| {
            if read {
                !c.read
            } else {
                c.read || c.last_page_read > 0
            }
        })
        .collect();
    if changed.is_empty() {
        return None;
    }
    let writes: Vec<ReadWrite> = changed
        .iter()
        .map(|c| ReadWrite {
            id: c.id,
            read,
            last_page_read: if read { None } else { Some(0) },
        })
        .collect();
    let delete_downloads = if read && remove_after_read {
        changed.iter().map(|c| c.id).collect()
    } else {
        Vec::new()
    };
    Some(ReadPlan {
        writes,
        delete_downloads,
    })
}

/// Bookmark writes: only the rows whose bookmark differs from `bookmark`. Returns `(id, bookmark)` pairs.
pub fn plan_bookmark(bookmark: bool, chapters: &[ChapterRow]) -> Vec<(i64, bool)> {
    chapters
        .iter()
        .filter(|c| c.bookmark != bookmark)
        .map(|c| (c.id, bookmark))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_downloads::DownloadState;

    fn row(id: i64, read: bool, last_page_read: i64) -> ChapterRow {
        ChapterRow {
            read,
            last_page_read,
            download: DownloadState::Downloaded,
            ..ChapterRow::new(id, "c", id as f64)
        }
    }

    #[test]
    fn mark_read_writes_only_unread_rows() {
        let rows = [row(1, true, 0), row(2, false, 5), row(3, false, 0)];
        let plan = plan_mark_read(true, &rows, false).expect("two rows change");
        let ids: Vec<i64> = plan.writes.iter().map(|w| w.id).collect();
        assert_eq!(ids, vec![2, 3]);
        assert!(
            plan.writes
                .iter()
                .all(|w| w.read && w.last_page_read.is_none())
        );
        assert!(plan.delete_downloads.is_empty());
    }

    #[test]
    fn mark_unread_resets_progress_of_read_or_partly_read_rows() {
        let rows = [
            row(1, false, 0),
            row(2, false, 9),
            row(3, true, 0),
            row(4, false, 0),
        ];
        let plan = plan_mark_read(false, &rows, true).expect("rows 2 and 3 change");
        assert_eq!(
            plan.writes.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert!(
            plan.writes
                .iter()
                .all(|w| !w.read && w.last_page_read == Some(0))
        );
        assert!(
            plan.delete_downloads.is_empty(),
            "removal happens only after mark read"
        );
    }

    #[test]
    fn nothing_to_change_is_none() {
        assert!(plan_mark_read(true, &[row(1, true, 0)], true).is_none());
        assert!(plan_mark_read(false, &[row(1, false, 0)], true).is_none());
        assert!(plan_mark_read(true, &[], true).is_none());
    }

    #[test]
    fn remove_after_read_deletes_only_changed_chapters() {
        let rows = [row(1, true, 0), row(2, false, 0)];
        let plan = plan_mark_read(true, &rows, true).expect("row 2 changes");
        assert_eq!(plan.delete_downloads, vec![2]);
    }

    #[test]
    fn bookmark_writes_only_differing_rows() {
        let mut a = row(1, false, 0);
        a.bookmark = true;
        let rows = [a, row(2, false, 0)];
        assert_eq!(plan_bookmark(true, &rows), vec![(2, true)]);
        assert_eq!(plan_bookmark(false, &rows), vec![(1, false)]);
    }
}
