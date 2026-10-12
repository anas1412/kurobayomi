//! The shared delete filter (T-25a-012) and the source lookup the pending-delete flush uses. The settings diet cut the
//! category exclusions of the delete rules.

use std::sync::Arc;

use crate::anime::model::EpisodeRef;
use crate::model::ChapterRef;
use crate::source::ChapterSource;

/// Looks up an installed manga or novel source by id (`sourceManager.get`). `None` means the source is not installed.
pub trait ChapterSourceResolver: Send + Sync {
    fn chapter_source(&self, source_id: i64) -> Option<Arc<dyn ChapterSource>>;
}

/// What the delete filter reads from one downloaded item: a chapter (`read`, `bookmark`) or an episode (`seen`,
/// `bookmark`).
pub trait DeleteCandidate {
    fn watched(&self) -> bool;
    fn bookmarked(&self) -> bool;
}

impl DeleteCandidate for ChapterRef {
    fn watched(&self) -> bool {
        self.read
    }
    fn bookmarked(&self) -> bool {
        self.bookmark
    }
}

impl DeleteCandidate for EpisodeRef {
    fn watched(&self) -> bool {
        self.seen
    }
    fn bookmarked(&self) -> bool {
        self.bookmark
    }
}

/// The pure delete filter of p1-behavior 1.4, the same for both kinds (T-25a-012). An empty `categories` counts as
/// `[0]`. When one of the categories is in `exclude`, watched items are kept. Bookmarked items are kept unless
/// `remove_bookmarked` is set. Empty input gives empty output (E-01).
pub fn select_for_delete<T: DeleteCandidate>(
    items: Vec<T>,
    exclude: &[i64],
    categories: &[i64],
    remove_bookmarked: bool,
) -> Vec<T> {
    let categories: &[i64] = if categories.is_empty() { &[0] } else { categories };
    let protect_watched = categories.iter().any(|c| exclude.contains(c));
    items
        .into_iter()
        .filter(|item| !(protect_watched && item.watched()))
        .filter(|item| remove_bookmarked || !item.bookmarked())
        .collect()
}

#[cfg(test)]
mod select_tests {
    use super::*;
    use crate::testing::chapter;

    fn read(id: i64) -> ChapterRef {
        let mut c = chapter(id, 1, "c");
        c.read = true;
        c
    }

    fn bookmarked(id: i64) -> ChapterRef {
        let mut c = chapter(id, 1, "c");
        c.bookmark = true;
        c
    }

    fn ids(items: Vec<ChapterRef>) -> Vec<i64> {
        items.iter().map(|c| c.id).collect()
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(select_for_delete(Vec::<ChapterRef>::new(), &[1], &[1], true).is_empty(), "E-01");
    }

    #[test]
    fn no_categories_counts_as_category_zero_for_the_exclusion() {
        let out = select_for_delete(vec![read(1), chapter(2, 1, "c")], &[0], &[], true);
        assert_eq!(ids(out), vec![2], "T-E04: the watched item is protected through category 0");
    }

    #[test]
    fn a_watched_item_in_an_excluded_category_is_not_selected() {
        let out = select_for_delete(vec![read(1), chapter(2, 1, "c")], &[5], &[5, 6], true);
        assert_eq!(ids(out), vec![2], "T-E18 and T-E18b");
    }

    #[test]
    fn bookmarks_are_kept_unless_removal_of_bookmarks_is_allowed() {
        assert!(select_for_delete(vec![bookmarked(3)], &[], &[], false).is_empty(), "T-E19: kept");
        assert_eq!(ids(select_for_delete(vec![bookmarked(3)], &[], &[], true)), vec![3], "T-E19: deleted");
    }
}
