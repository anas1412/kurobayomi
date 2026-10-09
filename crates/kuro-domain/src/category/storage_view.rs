//! Which entries the Storage screen lists (T-08a-036). "All" lists every entry, except with `hidden_categories` on it
//! lists only entries from visible categories. An entry with no category is in the default one, which is visible. A
//! specific category lists its own entries whether it is hidden or not. Entries are distinct by id and sorted by size,
//! largest first.

use std::collections::HashSet;

/// One entry as the Storage screen sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageItem {
    pub entry_id: i64,
    pub size: u64,
    pub category_ids: Vec<i64>,
}

/// The selection on the Storage screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageView {
    All,
    Category(i64),
}

/// The entries `view` lists. `hidden` holds the ids of the hidden categories, and `hide_hidden` is the preference.
pub fn storage_items(
    items: &[StorageItem],
    view: StorageView,
    hidden: &HashSet<i64>,
    hide_hidden: bool,
) -> Vec<StorageItem> {
    let mut seen = HashSet::new();
    let mut out: Vec<StorageItem> = items
        .iter()
        .filter(|item| match view {
            StorageView::Category(id) => item.category_ids.contains(&id),
            StorageView::All => {
                !hide_hidden
                    || item.category_ids.is_empty()
                    || item.category_ids.iter().any(|c| !hidden.contains(c))
            }
        })
        .filter(|item| seen.insert(item.entry_id))
        .cloned()
        .collect();
    out.sort_by(|a, b| b.size.cmp(&a.size).then(a.entry_id.cmp(&b.entry_id)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(entry_id: i64, size: u64, categories: &[i64]) -> StorageItem {
        StorageItem { entry_id, size, category_ids: categories.to_vec() }
    }

    fn ids(items: &[StorageItem]) -> Vec<i64> {
        items.iter().map(|i| i.entry_id).collect()
    }

    fn sample() -> Vec<StorageItem> {
        vec![item(1, 10, &[1]), item(2, 50, &[2]), item(3, 30, &[1, 2]), item(3, 30, &[1, 2]), item(4, 5, &[])]
    }

    #[test]
    fn all_lists_every_entry_once_by_size_when_nothing_is_hidden() {
        let hidden = HashSet::new();
        let got = storage_items(&sample(), StorageView::All, &hidden, false);
        assert_eq!(ids(&got), vec![2, 3, 1, 4]);
    }

    #[test]
    fn all_skips_entries_that_only_belong_to_hidden_categories_when_the_preference_is_on() {
        let hidden: HashSet<i64> = [2].into_iter().collect();
        let got = storage_items(&sample(), StorageView::All, &hidden, true);
        assert_eq!(ids(&got), vec![3, 1, 4], "2 is only in hidden 2; 3 is also in visible 1");
    }

    #[test]
    fn a_specific_category_shows_its_own_entries_even_when_it_is_hidden() {
        let hidden: HashSet<i64> = [2].into_iter().collect();
        let got = storage_items(&sample(), StorageView::Category(2), &hidden, true);
        assert_eq!(ids(&got), vec![2, 3]);
    }
}
