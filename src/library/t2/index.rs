//! Which test answers each T2 id. The ids with a `t2_<area>_<nn>` test of their own are found by name in this
//! directory's sources. The rest are answered by a test that already lives in another crate, listed below with the
//! file and function; this module checks that each listed function still exists. Ids that no test answers are listed
//! with the reason, so the gap is visible and a new test has to remove the entry.

const BULK: &str = include_str!("bulk.rs");
const CATSCR: &str = include_str!("catscr.rs");
const CODEC: &str = include_str!("codec.rs");
const FILTER: &str = include_str!("filter.rs");
const SORT: &str = include_str!("sort.rs");
const SEARCH: &str = include_str!("search.rs");
const PIPELINE: &str = include_str!("pipeline.rs");
const SELECTION: &str = include_str!("selection.rs");
const CATEGORY: &str = include_str!("category.rs");
const ACTIONS: &str = include_str!("../actions.rs");
const MOUNT: &str = include_str!("../mount.rs");
const UI: &str = include_str!("ui.rs");

/// Every id of section T-2.
const IDS: &[&str] = &[
    "CODEC-01", "CODEC-02", "CODEC-03", "CODEC-04", "CODEC-05", "CODEC-06", "CODEC-07", "CODEC-08",
    "FILTER-01", "FILTER-02", "FILTER-03", "FILTER-04", "FILTER-05", "FILTER-06", "FILTER-07",
    "TRACK-01", "TRACK-02", "TRACK-03", "TRACK-04", "TRACK-05",
    "SORT-01", "SORT-02", "SORT-03", "SORT-04", "SORT-05", "SORT-06", "SORT-07", "SORT-08", "SORT-09", "SORT-10",
    "SEARCH-01", "SEARCH-02", "SEARCH-03", "SEARCH-04", "SEARCH-05", "SEARCH-06", "SEARCH-07", "SEARCH-08", "SEARCH-09",
    "PIPE-01", "PIPE-02", "PIPE-03", "PIPE-04", "PIPE-05", "PIPE-06", "PIPE-07", "PIPE-08", "PIPE-09", "PIPE-10",
    "PIPE-11", "PIPE-12", "PIPE-13",
    "SEL-01", "SEL-02", "SEL-03", "SEL-04", "SEL-05", "SEL-06", "SEL-07", "SEL-08", "SEL-09",
    "CAT-01", "CAT-02", "CAT-03", "CAT-04", "CAT-05", "CAT-06", "CAT-07", "CAT-08",
    "BULK-01", "BULK-02", "BULK-03", "BULK-04", "DEL-01", "DEL-02", "DEL-03", "DEL-04",
    "CONT-01", "CONT-02", "CONT-03", "UPD-01", "UPD-02", "CATSCR-01", "CATSCR-02", "CATSCR-03", "UI-01", "UI-02",
];

/// Ids answered by a test in another crate: (id, source text, test function).
const ELSEWHERE: &[(&str, &str, &str)] = &[
    ("CAT-05", include_str!("../../../crates/kuro-domain/src/category/tests.rs"), "move_before_renumbers_and_never_writes_the_system_row"),
    ("CAT-06", include_str!("../../../crates/kuro-domain/src/category/tests.rs"), "delete_renumbers_every_category_and_cleans_the_preference_sets"),
    ("CAT-07", include_str!("../../../crates/kuro-domain/src/category/tests.rs"), "create_numbers_orders_and_takes_the_global_sort_as_flags"),
    ("PIPE-11", include_str!("../../../crates/kuro-db/src/views_anime.rs"), "season_parent_counts_children_and_empty_seasons_as_fully_seen"),
];

/// Ids no test answers yet, with the reason. Empty: every id of section T-2 has a test.
const OPEN: &[(&str, &str)] = &[];

fn has_test(source: &str, id: &str) -> bool {
    let name = format!("fn t2_{}(", id.to_lowercase().replace('-', "_"));
    let prefix = format!("fn t2_{}_", id.to_lowercase().replace('-', "_"));
    source.contains(&name) || source.contains(&prefix)
}

fn has_any(id: &str) -> bool {
    [BULK, CATSCR, CODEC, FILTER, SORT, SEARCH, PIPELINE, SELECTION, CATEGORY, ACTIONS, MOUNT, UI].iter().any(|s| has_test(s, id))
}

#[test]
fn every_t2_id_has_a_test_an_outside_test_or_a_listed_reason() {
    let mut missing = Vec::new();
    for id in IDS {
        let outside = ELSEWHERE.iter().any(|(i, _, _)| i == id);
        let open = OPEN.iter().any(|(i, _)| i == id);
        if !(has_any(id) || outside || open) {
            missing.push(*id);
        }
    }
    assert!(missing.is_empty(), "no test or reason for {missing:?}");
}

#[test]
fn outside_tests_still_exist() {
    for (id, source, name) in ELSEWHERE {
        let found = source.contains(&format!("fn {name}("));
        assert!(found, "{id}: {name} is gone");
    }
}

#[test]
fn an_open_id_that_gained_a_test_leaves_the_open_list() {
    for (id, _) in OPEN {
        assert!(!has_any(id), "{id} now has a test: remove it from OPEN");
    }
}
