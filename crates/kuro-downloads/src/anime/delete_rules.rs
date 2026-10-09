//! Delete-time filters for anime episodes (T-24a-033; behaviour 3.2). Used by every explicit delete, by the
//! remove-after-watched enqueue, and again when the pending list is flushed (E-P07).

use crate::anime::model::EpisodeRef;

/// The settings and the anime's categories that the filter reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeleteRules {
    /// Categories whose watched episodes are kept (`remove_exclude_anime_categories`).
    pub exclude_categories: Vec<i64>,
    /// Categories of the anime. Empty means category 0.
    pub item_categories: Vec<i64>,
    /// Allow deleting bookmarked episodes (`pref_remove_bookmarked`).
    pub remove_bookmarked: bool,
}

/// The episodes a delete may remove. When the anime sits in an excluded category its watched episodes are kept. The
/// bookmarked ones are kept unless `remove_bookmarked` is set. The category rule keeps the Kotlin logic as written.
pub fn episodes_to_delete(episodes: Vec<EpisodeRef>, rules: &DeleteRules) -> Vec<EpisodeRef> {
    let categories: &[i64] = if rules.item_categories.is_empty() { &[0] } else { &rules.item_categories };
    let watched_kept = categories.iter().any(|c| rules.exclude_categories.contains(c));
    episodes
        .into_iter()
        .filter(|e| !(watched_kept && e.seen))
        .filter(|e| rules.remove_bookmarked || !e.bookmark)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: i64, seen: bool, bookmark: bool) -> EpisodeRef {
        EpisodeRef {
            id,
            anime_id: 1,
            url: String::new(),
            name: format!("Episode {id}"),
            scanlator: None,
            seen,
            bookmark,
            fillermark: false,
            source_order: id,
            upload_date: 0,
            episode_number: 0.0,
        }
    }

    fn ids(episodes: Vec<EpisodeRef>) -> Vec<i64> {
        episodes.iter().map(|e| e.id).collect()
    }

    #[test]
    fn bookmarks_are_kept_unless_allowed() {
        let all = vec![ep(1, true, false), ep(2, true, true)];
        let kept = DeleteRules::default();
        assert_eq!(ids(episodes_to_delete(all.clone(), &kept)), vec![1]);
        let allowed = DeleteRules { remove_bookmarked: true, ..Default::default() };
        assert_eq!(ids(episodes_to_delete(all, &allowed)), vec![1, 2]);
    }

    #[test]
    fn an_excluded_category_keeps_watched_episodes_only() {
        let rules = DeleteRules { exclude_categories: vec![5], item_categories: vec![5, 6], remove_bookmarked: true };
        assert_eq!(ids(episodes_to_delete(vec![ep(1, true, false), ep(2, false, false)], &rules)), vec![2]);
    }

    #[test]
    fn an_uncategorised_anime_counts_as_category_zero() {
        let rules = DeleteRules { exclude_categories: vec![0], item_categories: vec![], remove_bookmarked: true };
        assert!(episodes_to_delete(vec![ep(1, true, false)], &rules).is_empty());
    }

    #[test]
    fn an_anime_outside_the_excluded_categories_loses_its_watched_episodes_too() {
        let rules = DeleteRules { exclude_categories: vec![5], item_categories: vec![7], remove_bookmarked: true };
        assert_eq!(ids(episodes_to_delete(vec![ep(1, true, false)], &rules)), vec![1]);
    }
}
