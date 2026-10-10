//! Related anime sync and display groups (T-39b-035). Upstream: `SyncRelatedAnimeWithSource.kt`. A source reports its
//! relation groups for an entry; the groups are stored with the time they were fetched. Pure rules: whether a sync runs,
//! which entries of a group are kept (distinct, not the entry itself, at most ten), which empty groups go, and the typed
//! result when the source is missing. The network fetch and the store write are not here.

use std::collections::HashSet;

/// A stored group is fetched again once it is this old (7 days).
pub const TTL_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// The most entries one group keeps.
pub const MAX_PER_GROUP: usize = 10;

/// Whether the groups are fetched again. A forced refresh always fetches; otherwise a fetch runs when none is stored or
/// the stored one is at least [`TTL_MS`] old (`T-REL-1`).
pub fn fetch_due(last_fetched_at: Option<i64>, now_ms: i64, force: bool) -> bool {
    if force {
        return true;
    }
    match last_fetched_at {
        None => true,
        Some(at) => now_ms - at >= TTL_MS,
    }
}

/// The outcome of one sync request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sync {
    /// The groups are fetched and stored.
    Fetch,
    /// A stored copy is still fresh: nothing to do.
    Fresh,
    /// The source does not support related anime: nothing to do.
    Unsupported,
    /// The source is not installed or not found (`T-REL-5`). Typed, so the caller can say so.
    MissingSource,
}

/// Decides one sync. `source` is `None` when the source is missing, otherwise whether it supports related anime.
pub fn decide(source: Option<bool>, last_fetched_at: Option<i64>, now_ms: i64, force: bool) -> Sync {
    match source {
        None => Sync::MissingSource,
        Some(false) => Sync::Unsupported,
        Some(true) if fetch_due(last_fetched_at, now_ms, force) => Sync::Fetch,
        Some(true) => Sync::Fresh,
    }
}

/// The urls one group keeps (`T-REL-2`, `T-REL-3`): the first ten distinct urls that are not the entry's own.
pub fn kept_urls<'a>(own_url: &str, urls: &[&'a str]) -> Vec<&'a str> {
    let mut seen = HashSet::new();
    urls.iter().copied().filter(|url| *url != own_url && seen.insert(*url)).take(MAX_PER_GROUP).collect()
}

/// Drops the groups with nothing left in them (`T-REL-4`). Each group is `(name, entries)`.
pub fn non_empty<T>(groups: Vec<(String, Vec<T>)>) -> Vec<(String, Vec<T>)> {
    groups.into_iter().filter(|(_, entries)| !entries.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_copy_is_fresh_for_seven_days_then_fetched_again() {
        let day = 24 * 60 * 60 * 1000;
        assert!(fetch_due(None, 0, false), "never fetched");
        assert!(!fetch_due(Some(0), 7 * day - 1, false), "still fresh");
        assert!(fetch_due(Some(0), 7 * day, false), "exactly the TTL is stale");
        assert!(fetch_due(Some(0), 1, true), "a forced refresh always fetches");
    }

    #[test]
    fn a_group_keeps_ten_distinct_entries_other_than_the_entry_itself() {
        let urls = ["a", "self", "b", "a", "c", "d", "e", "f", "g", "h", "i", "j", "k"];
        let kept = kept_urls("self", &urls);
        assert_eq!(kept, vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]);
        assert!(!kept.contains(&"self"));
    }

    #[test]
    fn an_empty_group_is_dropped_and_others_keep_their_order() {
        let groups = vec![("Sequels".to_owned(), vec![1]), ("Empty".to_owned(), Vec::<i64>::new()), ("Side".to_owned(), vec![2, 3])];
        let kept = non_empty(groups);
        assert_eq!(kept.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(), vec!["Sequels", "Side"]);
    }

    #[test]
    fn the_sync_outcome_follows_the_source_and_the_ttl() {
        assert_eq!(decide(None, None, 0, false), Sync::MissingSource, "T-REL-5");
        assert_eq!(decide(Some(false), None, 0, false), Sync::Unsupported);
        assert_eq!(decide(Some(true), Some(0), 1, false), Sync::Fresh);
        assert_eq!(decide(Some(true), Some(0), TTL_MS, false), Sync::Fetch);
        assert_eq!(decide(Some(true), Some(0), 1, true), Sync::Fetch, "manual refresh");
    }
}
