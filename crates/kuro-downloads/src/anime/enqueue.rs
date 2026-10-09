//! Enqueue filtering and ordering, and the queue-size warning (T-24a-012, T-24a-013; behaviour 2.8 and 3.1).

use std::collections::{HashMap, HashSet};

use crate::anime::model::EpisodeRef;
use crate::downloader::{BULK_WARNING_HELP_URL, BULK_WARNING_TEXT, WARNING_TIMEOUT_SECS};

/// The warning fires when more than this many metered items are queued (`queuedDownloads > 20`).
pub const METERED_TOTAL_THRESHOLD: usize = 20;
/// ... or when one metered source has more than this many items (`maxDownloadsFromSource > 10`).
pub const METERED_PER_SOURCE_THRESHOLD: usize = 10;

/// The notice sent when a run starts over a large queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueWarning {
    pub text: &'static str,
    pub timeout_secs: u32,
    pub help_url: &'static str,
}

pub fn queue_warning() -> QueueWarning {
    QueueWarning { text: BULK_WARNING_TEXT, timeout_secs: WARNING_TIMEOUT_SECS, help_url: BULK_WARNING_HELP_URL }
}

/// Episodes to append, in queue order (behaviour 2.8 steps 4a to 4d):
/// fillermarked episodes are dropped unless `download_fillermarked`, episodes that are already downloaded are dropped,
/// episodes already queued (by id, or earlier in this batch) are dropped, and the rest are sorted by `source_order`
/// descending. The sort is stable, so equal orders keep their input order.
pub fn plan_enqueue(
    episodes: &[EpisodeRef],
    queued: &HashSet<i64>,
    mut is_downloaded: impl FnMut(&EpisodeRef) -> bool,
    download_fillermarked: bool,
) -> Vec<EpisodeRef> {
    let mut candidates: Vec<&EpisodeRef> = episodes.iter().filter(|e| download_fillermarked || !e.fillermark).collect();
    candidates.sort_by_key(|e| std::cmp::Reverse(e.source_order));
    let mut taken: HashSet<i64> = HashSet::new();
    let mut picked = Vec::new();
    for episode in candidates {
        if queued.contains(&episode.id) || taken.contains(&episode.id) || is_downloaded(episode) {
            continue;
        }
        taken.insert(episode.id);
        picked.push(episode.clone());
    }
    picked
}

/// `true` when the queue is large enough to warn (T-24a-013). `queue` is `(source id, unmetered)` for each queued item.
/// Unmetered sources are left out of both counts.
pub fn needs_queue_warning(queue: impl IntoIterator<Item = (i64, bool)>) -> bool {
    let mut total = 0usize;
    let mut per_source: HashMap<i64, usize> = HashMap::new();
    for (source_id, unmetered) in queue {
        if unmetered {
            continue;
        }
        total += 1;
        *per_source.entry(source_id).or_default() += 1;
    }
    total > METERED_TOTAL_THRESHOLD || per_source.values().any(|&n| n > METERED_PER_SOURCE_THRESHOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: i64, source_order: i64, fillermark: bool) -> EpisodeRef {
        EpisodeRef {
            id,
            anime_id: 1,
            url: String::new(),
            name: format!("Ep {id}"),
            scanlator: None,
            seen: false,
            bookmark: false,
            fillermark,
            source_order,
            upload_date: 0,
            episode_number: 0.0,
        }
    }

    #[test]
    fn episodes_are_queued_newest_first_by_source_order() {
        let eps = vec![ep(1, 1, false), ep(2, 3, false), ep(3, 2, false)];
        let picked = plan_enqueue(&eps, &HashSet::new(), |_| false, false);
        assert_eq!(picked.iter().map(|e| e.id).collect::<Vec<_>>(), vec![2, 3, 1]);
    }

    #[test]
    fn downloaded_and_already_queued_episodes_are_skipped() {
        let eps = vec![ep(1, 1, false), ep(2, 2, false), ep(3, 3, false)];
        let queued: HashSet<i64> = HashSet::from([3]);
        let picked = plan_enqueue(&eps, &queued, |e| e.id == 2, false);
        assert_eq!(picked.iter().map(|e| e.id).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn fillermarked_episodes_are_dropped_unless_the_setting_is_on() {
        let eps = vec![ep(1, 1, true), ep(2, 2, false)];
        assert_eq!(plan_enqueue(&eps, &HashSet::new(), |_| false, false).len(), 1);
        assert_eq!(plan_enqueue(&eps, &HashSet::new(), |_| false, true).len(), 2);
    }

    #[test]
    fn a_duplicate_within_one_batch_is_queued_once() {
        let eps = vec![ep(7, 1, false), ep(7, 1, false)];
        assert_eq!(plan_enqueue(&eps, &HashSet::new(), |_| false, false).len(), 1);
    }

    #[test]
    fn warns_above_twenty_metered_items_in_total() {
        let twenty: Vec<(i64, bool)> = (0..20).map(|i| (i, false)).collect();
        assert!(!needs_queue_warning(twenty.clone()), "exactly 20 is fine");
        let mut more = twenty;
        more.push((99, false));
        assert!(needs_queue_warning(more));
    }

    #[test]
    fn warns_above_ten_items_from_one_metered_source() {
        let ten: Vec<(i64, bool)> = (0..10).map(|_| (5, false)).collect();
        assert!(!needs_queue_warning(ten.clone()));
        let mut eleven = ten;
        eleven.push((5, false));
        assert!(needs_queue_warning(eleven));
    }

    #[test]
    fn unmetered_sources_do_not_count() {
        let big: Vec<(i64, bool)> = (0..40).map(|_| (9, true)).collect();
        assert!(!needs_queue_warning(big));
        let mixed: Vec<(i64, bool)> = (0..15).map(|i| (i, false)).chain((0..15).map(|_| (9, true))).collect();
        assert!(!needs_queue_warning(mixed), "15 metered items across sources is under both thresholds");
    }

    #[test]
    fn the_warning_carries_the_text_timeout_and_help_link() {
        let w = queue_warning();
        assert!(w.text.starts_with("Warning: large bulk downloads"));
        assert_eq!(w.timeout_secs, 30);
        assert!(w.help_url.starts_with("https://"));
    }
}
