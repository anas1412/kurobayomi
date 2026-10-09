//! Per-source selection: at most 5 sources, one chapter each, ERROR skipped (port of `Dl.launchDownloaderJob`).

use std::collections::HashSet;

use crate::model::DownloadState;

/// Sources that may download at the same time (`Dl.kt:216`, `take(5)`).
pub const MAX_ACTIVE_SOURCES: usize = 5;

/// Chooses the chapters to run. `items` is `(source id, chapter id, status)` in queue order. Each active item
/// (states 0 to 2) counts for its source only when it is the first active item of that source, so ERROR items
/// are skipped and the next chapter of the same source runs. Output is in first-occurrence order.
pub fn pick_active<I>(items: I, max_sources: usize) -> Vec<i64>
where
    I: IntoIterator<Item = (i64, i64, DownloadState)>,
{
    let mut seen_sources = HashSet::new();
    let mut picked = Vec::new();
    for (source_id, chapter_id, status) in items {
        if picked.len() >= max_sources {
            break;
        }
        if status.is_active() && seen_sources.insert(source_id) {
            picked.push(chapter_id);
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use DownloadState::*;

    #[test]
    fn seven_sources_two_chapters_each_gives_five_first_chapters() {
        let items: Vec<_> = (1..=7).flat_map(|s| [(s, s * 10, Queue), (s, s * 10 + 1, Queue)]).collect();
        assert_eq!(pick_active(items, MAX_ACTIVE_SOURCES), vec![10, 20, 30, 40, 50]);
    }

    #[test]
    fn an_error_chapter_is_skipped_for_its_source() {
        let items = vec![(1, 10, Error), (1, 11, Queue), (2, 20, Downloading), (2, 21, Queue)];
        assert_eq!(pick_active(items, MAX_ACTIVE_SOURCES), vec![11, 20]);
    }

    #[test]
    fn nothing_is_picked_when_every_item_is_finished() {
        let items = vec![(1, 10, Downloaded), (2, 20, Error)];
        assert!(pick_active(items, MAX_ACTIVE_SOURCES).is_empty());
    }

    #[test]
    fn order_follows_the_first_active_occurrence_per_source() {
        let items = vec![(2, 20, Queue), (1, 10, Queue), (2, 21, Queue), (3, 30, Downloading)];
        assert_eq!(pick_active(items, MAX_ACTIVE_SOURCES), vec![20, 10, 30]);
    }
}
