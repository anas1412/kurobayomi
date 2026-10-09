//! Which new items are queued for download after a pass (behaviour 6.3, `FilterChaptersForDownload` and
//! `FilterEpisodesForDownload`). The queue itself is the host's job; this only decides.

use crate::library_update::source::SourceItem;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadPolicy {
    /// `download_new` / `download_new_episode`.
    pub enabled: bool,
    /// `download_new_unread_chapters_only` / `download_new_unread_episodes_only`.
    pub unread_only: bool,
    pub include_categories: Vec<i64>,
    pub exclude_categories: Vec<i64>,
}

impl DownloadPolicy {
    /// Whether the entry's categories allow new items to be downloaded (`shouldDownloadNewChapters`, without the
    /// favourite check, which the caller applies). An entry with no category counts as category `0`.
    pub fn categories_allow(&self, categories: &[i64]) -> bool {
        let both_empty = self.include_categories.is_empty() && self.exclude_categories.is_empty();
        if both_empty {
            return true;
        }
        let entry_categories: Vec<i64> = if categories.is_empty() { vec![0] } else { categories.to_vec() };
        if entry_categories.iter().any(|c| self.exclude_categories.contains(c)) {
            return false;
        }
        if self.include_categories.is_empty() {
            return true;
        }
        entry_categories.iter().any(|c| self.include_categories.contains(c))
    }

    /// Whether this policy looks at new items of an entry at all.
    pub fn wants(&self, favorite: bool, categories: &[i64]) -> bool {
        self.enabled && favorite && self.categories_allow(categories)
    }

    /// The new items to queue. `read_numbers` are the numbers of the read items; only consulted when
    /// `unread_only` is set, so the caller may pass an empty slice otherwise.
    pub fn select(&self, new_items: &[SourceItem], read_numbers: &[f64]) -> Vec<SourceItem> {
        if new_items.is_empty() {
            return Vec::new();
        }
        if !self.unread_only {
            return new_items.to_vec();
        }
        new_items
            .iter()
            .filter(|item| !read_numbers.iter().any(|n| *n == item.number))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(number: f64) -> SourceItem {
        SourceItem { url: format!("/{number}"), name: format!("{number}"), number, ..SourceItem::default() }
    }

    fn policy(include: &[i64], exclude: &[i64]) -> DownloadPolicy {
        DownloadPolicy { enabled: true, unread_only: false, include_categories: include.to_vec(), exclude_categories: exclude.to_vec() }
    }

    #[test]
    fn category_rules_match_the_kotlin_order() {
        assert!(policy(&[], &[]).categories_allow(&[4])); // both empty: allowed
        assert!(!policy(&[], &[4]).categories_allow(&[4])); // excluded
        assert!(policy(&[], &[4]).categories_allow(&[5])); // not excluded, include empty
        assert!(policy(&[5], &[]).categories_allow(&[5])); // included
        assert!(!policy(&[5], &[]).categories_allow(&[6]));
        assert!(!policy(&[5], &[]).categories_allow(&[])); // no category counts as 0, not 5
        assert!(policy(&[0], &[]).categories_allow(&[])); // ... so include 0 matches
    }

    #[test]
    fn disabled_or_unfavourite_entries_are_never_wanted() {
        let mut p = policy(&[], &[]);
        assert!(p.wants(true, &[1]));
        assert!(!p.wants(false, &[1]));
        p.enabled = false;
        assert!(!p.wants(true, &[1]));
    }

    #[test]
    fn unread_only_drops_items_whose_number_was_read() {
        let mut p = policy(&[], &[]);
        p.unread_only = true;
        let new = vec![item(1.0), item(2.0), item(3.0)];
        let kept = p.select(&new, &[2.0]);
        assert_eq!(kept.iter().map(|i| i.number).collect::<Vec<_>>(), vec![1.0, 3.0]);
    }

    #[test]
    fn without_unread_only_every_new_item_is_selected() {
        let p = policy(&[], &[]);
        let new = vec![item(1.0), item(2.0)];
        assert_eq!(p.select(&new, &[1.0]).len(), 2);
        assert!(p.select(&[], &[]).is_empty());
    }
}
