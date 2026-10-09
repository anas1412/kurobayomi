//! Library update candidates (T-02a-011): the entry rules of `MangaLibraryUpdateJob` and the category
//! include and exclude sets. Pure functions; the scheduler (area 26) feeds them the database rows.

use std::collections::BTreeSet;

use crate::library::{
    ENTRY_HAS_UNVIEWED, ENTRY_NON_COMPLETED, ENTRY_NON_VIEWED, ENTRY_OUTSIDE_RELEASE_PERIOD,
    MARK_DUPLICATE_CHAPTER_READ_EXISTING, MARK_DUPLICATE_CHAPTER_READ_NEW,
    MARK_DUPLICATE_EPISODE_SEEN_EXISTING, MARK_DUPLICATE_EPISODE_SEEN_NEW,
};

/// The fields of one library entry that the update rules read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateEntry {
    pub id: i64,
    pub title: String,
    /// Category ids the entry belongs to.
    pub categories: Vec<i64>,
    /// `UpdateStrategy` is `ALWAYS_UPDATE`.
    pub always_update: bool,
    /// The status is COMPLETED.
    pub completed: bool,
    /// Chapters (or episodes) not yet read (or seen).
    pub unread: i64,
    pub total: i64,
    /// At least one chapter has been read.
    pub started: bool,
    /// The next expected release, epoch milliseconds.
    pub next_update: i64,
}

/// Why an entry was left out of the update. Only logged; the message keys are the Kotlin string names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    NotAlwaysUpdate,
    Completed,
    NotCaughtUp,
    NotStarted,
    NotInReleasePeriod,
}

impl SkipReason {
    /// `skipped_reason_*` message key.
    pub fn message_key(self) -> &'static str {
        match self {
            SkipReason::NotAlwaysUpdate => "skipped_reason_not_always_update",
            SkipReason::Completed => "skipped_reason_completed",
            SkipReason::NotCaughtUp => "skipped_reason_not_caught_up",
            SkipReason::NotStarted => "skipped_reason_not_started",
            SkipReason::NotInReleasePeriod => "skipped_reason_not_in_release_period",
        }
    }
}

/// The first rule (in order) that skips `entry`, under the item restrictions.
pub fn skip_reason(entry: &UpdateEntry, restrictions: &BTreeSet<String>, fetch_window_upper_bound: i64) -> Option<SkipReason> {
    let has = |r: &str| restrictions.contains(r);
    if !entry.always_update {
        Some(SkipReason::NotAlwaysUpdate)
    } else if has(ENTRY_NON_COMPLETED) && entry.completed {
        Some(SkipReason::Completed)
    } else if has(ENTRY_HAS_UNVIEWED) && entry.unread != 0 {
        Some(SkipReason::NotCaughtUp)
    } else if has(ENTRY_NON_VIEWED) && entry.total > 0 && !entry.started {
        Some(SkipReason::NotStarted)
    } else if has(ENTRY_OUTSIDE_RELEASE_PERIOD) && entry.next_update > fetch_window_upper_bound {
        Some(SkipReason::NotInReleasePeriod)
    } else {
        None
    }
}

/// Applies the five skip rules in order. Returns the kept entries sorted by title, and the skipped ids with
/// their reasons (for the log). Duplicate entry ids are dropped after the first.
pub fn filter_update_candidates(
    entries: Vec<UpdateEntry>,
    restrictions: &BTreeSet<String>,
    fetch_window_upper_bound: i64,
) -> (Vec<UpdateEntry>, Vec<(i64, SkipReason)>) {
    let mut seen = BTreeSet::new();
    let mut kept = Vec::new();
    let mut skipped = Vec::new();
    for entry in entries {
        if !seen.insert(entry.id) {
            continue;
        }
        match skip_reason(&entry, restrictions, fetch_window_upper_bound) {
            Some(reason) => skipped.push((entry.id, reason)),
            None => kept.push(entry),
        }
    }
    kept.sort_by(|a, b| a.title.cmp(&b.title));
    (kept, skipped)
}

/// The category filter: `(included or include set empty) and not in any exclude category`.
/// Category ids are compared as their decimal strings, which is how the preference sets store them.
pub fn passes_category_sets(entry_categories: &[i64], include: &BTreeSet<String>, exclude: &BTreeSet<String>) -> bool {
    let contains = |set: &BTreeSet<String>| entry_categories.iter().any(|c| set.contains(&c.to_string()));
    (include.is_empty() || contains(include)) && !contains(exclude)
}

/// The duplicate rules are subsets of their allowed values (`new`, `existing` for chapters; `new_episode`,
/// `existing_episode` for episodes).
pub fn is_chapter_duplicate_rule_set(set: &BTreeSet<String>) -> bool {
    is_subset(set, &[MARK_DUPLICATE_CHAPTER_READ_NEW, MARK_DUPLICATE_CHAPTER_READ_EXISTING])
}

/// See [`is_chapter_duplicate_rule_set`].
pub fn is_episode_duplicate_rule_set(set: &BTreeSet<String>) -> bool {
    is_subset(set, &[MARK_DUPLICATE_EPISODE_SEEN_NEW, MARK_DUPLICATE_EPISODE_SEEN_EXISTING])
}

fn is_subset(set: &BTreeSet<String>, allowed: &[&str]) -> bool {
    set.iter().all(|v| allowed.contains(&v.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_four() -> BTreeSet<String> {
        ["manga_fully_read", "manga_ongoing", "manga_started", "manga_outside_release_period"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }

    fn entry(id: i64, title: &str) -> UpdateEntry {
        UpdateEntry {
            id,
            title: title.to_owned(),
            categories: vec![],
            always_update: true,
            completed: false,
            unread: 0,
            total: 3,
            started: true,
            next_update: 0,
        }
    }

    #[test]
    fn each_rule_skips_with_its_own_reason_in_order() {
        let r = all_four();
        let mut e = entry(1, "a");
        e.always_update = false;
        e.completed = true;
        assert_eq!(skip_reason(&e, &r, 0), Some(SkipReason::NotAlwaysUpdate), "first rule wins");

        let mut e = entry(1, "a");
        e.completed = true;
        assert_eq!(skip_reason(&e, &r, 0), Some(SkipReason::Completed));

        let mut e = entry(1, "a");
        e.unread = 2;
        assert_eq!(skip_reason(&e, &r, 0), Some(SkipReason::NotCaughtUp));

        let mut e = entry(1, "a");
        e.started = false;
        assert_eq!(skip_reason(&e, &r, 0), Some(SkipReason::NotStarted));
        e.total = 0;
        assert_eq!(skip_reason(&e, &r, 0), None, "no chapters means not-started does not apply");

        let mut e = entry(1, "a");
        e.next_update = 101;
        assert_eq!(skip_reason(&e, &r, 100), Some(SkipReason::NotInReleasePeriod));
        assert_eq!(skip_reason(&e, &r, 101), None);
    }

    #[test]
    fn a_rule_applies_only_when_its_restriction_is_present() {
        let mut e = entry(1, "a");
        e.completed = true;
        e.unread = 5;
        assert_eq!(skip_reason(&e, &BTreeSet::new(), 0), None);
    }

    #[test]
    fn message_keys_are_the_kotlin_names() {
        assert_eq!(SkipReason::Completed.message_key(), "skipped_reason_completed");
        assert_eq!(SkipReason::NotInReleasePeriod.message_key(), "skipped_reason_not_in_release_period");
    }

    #[test]
    fn kept_entries_are_sorted_by_title_and_duplicates_are_dropped() {
        let mut done = entry(9, "Zed");
        done.completed = true;
        let input = vec![entry(2, "Mango"), done, entry(1, "Apple"), entry(1, "Apple again")];
        let (kept, skipped) = filter_update_candidates(input, &all_four(), 0);
        let titles: Vec<&str> = kept.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Apple", "Mango"]);
        assert_eq!(skipped, [(9, SkipReason::Completed)]);
    }

    #[test]
    fn category_sets_are_include_minus_exclude() {
        let set = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<String>>();
        assert!(passes_category_sets(&[1], &set(&[]), &set(&[])), "empty include means all");
        assert!(passes_category_sets(&[1], &set(&["1"]), &set(&[])));
        assert!(!passes_category_sets(&[2], &set(&["1"]), &set(&[])));
        assert!(!passes_category_sets(&[1, 3], &set(&[]), &set(&["3"])));
        assert!(!passes_category_sets(&[1], &set(&["1"]), &set(&["1"])), "exclude wins over include");
        assert!(!passes_category_sets(&[], &set(&["1"]), &set(&[])), "uncategorised entry is outside a non-empty include set");
    }

    #[test]
    fn duplicate_rules_are_subsets_of_the_allowed_values() {
        let set = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<String>>();
        assert!(is_chapter_duplicate_rule_set(&set(&["new", "existing"])));
        assert!(is_chapter_duplicate_rule_set(&set(&[])));
        assert!(!is_chapter_duplicate_rule_set(&set(&["new_episode"])));
        assert!(is_episode_duplicate_rule_set(&set(&["existing_episode"])));
        assert!(!is_episode_duplicate_rule_set(&set(&["existing"])));
    }
}
