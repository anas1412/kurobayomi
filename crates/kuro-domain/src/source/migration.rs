//! The migration source list: the sort modes and their toggles (`SetMigrateSorting.kt`; T-17a-023) and the list with
//! favourite counts (`GetMangaSourcesWithFavoriteCount.kt`; T-17a-024). The ordering is `sortFn` of that file, with its
//! stub rule and its reversal for DESCENDING.

use std::cmp::Ordering;

use crate::app::source::LOCAL_SOURCE_ID;
use crate::items::sorter::collate;
use crate::source::model::Source;

/// `SetMigrateSorting.Mode`: `ALPHABETICAL` or `TOTAL`. Stored by name (T-17a-015).
pub use kuro_prefs::source::MigrationSortingMode as Mode;
/// `SetMigrateSorting.Direction`: `ASCENDING` or `DESCENDING`.
pub use kuro_prefs::source::MigrationDirection as Direction;

/// The other mode.
pub fn toggled_mode(mode: Mode) -> Mode {
    match mode {
        Mode::ALPHABETICAL => Mode::TOTAL,
        Mode::TOTAL => Mode::ALPHABETICAL,
    }
}

/// The other direction.
pub fn toggled_direction(direction: Direction) -> Direction {
    match direction {
        Direction::ASCENDING => Direction::DESCENDING,
        Direction::DESCENDING => Direction::ASCENDING,
    }
}

/// `sortFn` before the direction: a stub comes before a live source; otherwise the name (collated, lower case) for
/// ALPHABETICAL and the favourite count for TOTAL.
fn compare(a: &(Source, i64), b: &(Source, i64), mode: Mode) -> Ordering {
    match (a.0.is_stub, b.0.is_stub) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => match mode {
            Mode::ALPHABETICAL => collate(&a.0.name.to_lowercase(), &b.0.name.to_lowercase()),
            Mode::TOTAL => a.1.cmp(&b.1),
        },
    }
}

/// The migration list in display order. The local source is removed first. The sort is stable, and DESCENDING reverses
/// the whole comparator (`Collections.reverseOrder`), so ties keep their input order in both directions and stubs go last.
pub fn sort_migration_sources(list: Vec<(Source, i64)>, mode: Mode, direction: Direction) -> Vec<(Source, i64)> {
    let mut kept: Vec<(Source, i64)> = list.into_iter().filter(|(source, _)| source.id != LOCAL_SOURCE_ID).collect();
    kept.sort_by(|a, b| match direction {
        Direction::ASCENDING => compare(a, b, mode),
        Direction::DESCENDING => compare(a, b, mode).reverse(),
    });
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::fakes::migration_rows;

    fn ids(list: &[(Source, i64)]) -> Vec<i64> {
        list.iter().map(|(source, _)| source.id).collect()
    }

    /// Section 3 rows: a=1 (live, 3), b=2 (live, 10), c=3 (live, 3), d=4 (stub, 7), e=5 (live, 10).
    fn section3() -> Vec<(Source, i64)> {
        migration_rows()
    }

    #[test]
    fn e_m1_alphabetical_ascending_puts_the_stub_first() {
        let list = sort_migration_sources(section3(), Mode::ALPHABETICAL, Direction::ASCENDING);
        assert_eq!(ids(&list), [4, 2, 3, 5, 1]);
    }

    #[test]
    fn e_m2_alphabetical_descending_puts_the_stub_last() {
        let list = sort_migration_sources(section3(), Mode::ALPHABETICAL, Direction::DESCENDING);
        assert_eq!(ids(&list), [1, 5, 3, 2, 4]);
    }

    #[test]
    fn e_m3_total_ascending_keeps_input_order_for_equal_counts() {
        let list = sort_migration_sources(section3(), Mode::TOTAL, Direction::ASCENDING);
        assert_eq!(ids(&list), [4, 1, 3, 2, 5]);
    }

    #[test]
    fn e_m4_total_descending_is_compared_as_a_vector_not_a_set() {
        let list = sort_migration_sources(section3(), Mode::TOTAL, Direction::DESCENDING);
        assert_eq!(ids(&list), [2, 5, 1, 3, 4]);
    }

    #[test]
    fn e_m5_the_local_source_is_removed_before_sorting() {
        let mut input = section3();
        input.push((Source::new(0, "Local", ""), 99));
        let list = sort_migration_sources(input, Mode::ALPHABETICAL, Direction::ASCENDING);
        assert!(list.iter().all(|(source, _)| source.id != 0));
        assert_eq!(list.len(), 5);
    }

    #[test]
    fn e_m6_names_equal_under_the_collator_keep_their_input_order() {
        let input = vec![(Source::new(7, "same", "en"), 1), (Source::new(8, "SAME", "en"), 1)];
        let list = sort_migration_sources(input, Mode::ALPHABETICAL, Direction::ASCENDING);
        assert_eq!(ids(&list), [7, 8]);
    }

    #[test]
    fn each_toggle_gives_the_other_value() {
        assert_eq!(toggled_mode(Mode::ALPHABETICAL), Mode::TOTAL);
        assert_eq!(toggled_mode(Mode::TOTAL), Mode::ALPHABETICAL);
        assert_eq!(toggled_direction(Direction::ASCENDING), Direction::DESCENDING);
        assert_eq!(toggled_direction(Direction::DESCENDING), Direction::ASCENDING);
    }
}
