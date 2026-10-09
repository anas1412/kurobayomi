//! Include and exclude summary of category sets (task T-47a-005).
// Not reachable from `route.rs` yet: the include/exclude summary is for the update-category rows, which are still spec rows.
#![allow(dead_code)]
use std::collections::BTreeSet;

/// A library category as the settings need it: its stored id, display name and sort order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: i32,
    pub name: String,
    pub order: i64,
}

/// The sorted categories whose ids are in `ids`. Ids that match no category (deleted ones) are dropped.
fn resolve<'a>(ids: &BTreeSet<String>, categories: &'a [Category]) -> Vec<&'a Category> {
    let mut found: Vec<&Category> = categories
        .iter()
        .filter(|c| ids.contains(&c.id.to_string()))
        .collect();
    found.sort_by_key(|c| c.order);
    found
}

/// `"Include: <inc>\nExclude: <exc>"` for the include and exclude id sets (Kotlin `Commons.kt` `categoryLabel`).
pub fn label(include: &BTreeSet<String>, exclude: &BTreeSet<String>, categories: &[Category]) -> String {
    let included = resolve(include, categories);
    let excluded = resolve(exclude, categories);
    let total = categories.len();
    let all_excluded = excluded.len() == total;
    let names = |list: &[&Category]| list.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ");

    let include_text = if !included.is_empty() && included.len() < total {
        names(&included)
    } else if included.len() == total {
        "All".to_owned()
    } else if all_excluded {
        "None".to_owned()
    } else {
        "All".to_owned()
    };
    let exclude_text = if excluded.is_empty() {
        "None".to_owned()
    } else if all_excluded {
        "All".to_owned()
    } else {
        names(&excluded)
    };
    format!("Include: {include_text}\nExclude: {exclude_text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cats() -> Vec<Category> {
        vec![
            Category { id: 1, name: "Action".into(), order: 2 },
            Category { id: 2, name: "Drama".into(), order: 1 },
        ]
    }

    fn set(ids: &[i32]) -> BTreeSet<String> {
        ids.iter().map(|i| i.to_string()).collect()
    }

    #[test]
    fn partial_include_lists_names_in_category_order() {
        let text = label(&set(&[1, 2]), &set(&[]), &cats());
        assert_eq!(text, "Include: All\nExclude: None");
        let partial = label(&set(&[1]), &set(&[]), &cats());
        assert_eq!(partial, "Include: Action\nExclude: None");
    }

    #[test]
    fn included_all_is_all() {
        assert_eq!(label(&set(&[1, 2]), &set(&[]), &cats()), "Include: All\nExclude: None");
    }

    #[test]
    fn nothing_included_and_everything_excluded_is_none_and_all() {
        assert_eq!(label(&set(&[]), &set(&[1, 2]), &cats()), "Include: None\nExclude: All");
    }

    #[test]
    fn nothing_included_with_some_excluded_is_all() {
        assert_eq!(label(&set(&[]), &set(&[1]), &cats()), "Include: All\nExclude: Action");
    }

    #[test]
    fn partial_exclude_joins_names_in_order() {
        assert_eq!(label(&set(&[]), &set(&[2]), &cats()), "Include: All\nExclude: Drama");
    }

    #[test]
    fn zero_categories_give_all_for_include_and_none_for_exclude() {
        assert_eq!(label(&set(&[]), &set(&[]), &[]), "Include: All\nExclude: None");
    }

    #[test]
    fn unknown_ids_are_dropped_before_counting() {
        assert_eq!(label(&set(&[1, 99]), &set(&[]), &cats()), "Include: Action\nExclude: None");
        assert_eq!(label(&set(&[]), &set(&[1, 2, 99]), &cats()), "Include: None\nExclude: All");
    }

    /// The ten rows of `p1-edge-cases-and-tests.md` section 3 (categories A, B, C with orders 0, 1, 2).
    #[test]
    fn the_spec_combinations_for_three_categories() {
        let abc = vec![
            Category { id: 1, name: "A".into(), order: 0 },
            Category { id: 2, name: "B".into(), order: 1 },
            Category { id: 3, name: "C".into(), order: 2 },
        ];
        let case = |inc: &[i32], exc: &[i32], categories: &[Category]| label(&set(inc), &set(exc), categories);
        assert_eq!(case(&[], &[], &abc), "Include: All\nExclude: None");
        assert_eq!(case(&[1], &[], &abc), "Include: A\nExclude: None");
        assert_eq!(case(&[1, 2, 3], &[], &abc), "Include: All\nExclude: None");
        assert_eq!(case(&[], &[1, 2, 3], &abc), "Include: None\nExclude: All");
        assert_eq!(case(&[1], &[2], &abc), "Include: A\nExclude: B");
        assert_eq!(case(&[1, 2], &[1], &abc), "Include: A, B\nExclude: A");
        assert_eq!(case(&[], &[1], &abc), "Include: All\nExclude: A");
        assert_eq!(case(&[], &[], &[]), "Include: All\nExclude: None");
        assert_eq!(case(&[1, 2, 3], &[1, 2, 3], &abc), "Include: All\nExclude: All");
        assert_eq!(case(&[99, 1], &[], &abc), "Include: A\nExclude: None");
    }

    #[test]
    fn joined_names_follow_the_category_order_not_the_id_order() {
        let stored_reversed = vec![
            Category { id: 2, name: "B".into(), order: 1 },
            Category { id: 1, name: "A".into(), order: 0 },
            Category { id: 3, name: "C".into(), order: 2 },
        ];
        assert_eq!(label(&set(&[2, 1]), &set(&[]), &stored_reversed), "Include: A, B\nExclude: None");
    }
}
