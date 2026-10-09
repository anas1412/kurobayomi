//! The migration source list: the sort settings and their toggles (`SetMigrateSorting.kt`; T-17a-023) and the list with
//! favourite counts (`GetMangaSourcesWithFavoriteCount.kt`; T-17a-024). The ordering is `sortFn` of that file, with its
//! stub rule and its reversal for DESCENDING.

use std::cmp::Ordering;

use kuro_prefs::source::SourcePrefs;
use tokio::sync::watch;

use crate::app::rx::{combine3, pref_stream};
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

/// `SetMigrateSorting.await(mode, direction)`: the mode is written, then the direction (two writes, in that order).
pub fn set_migration_sorting(prefs: &SourcePrefs, mode: Mode, direction: Direction) {
    prefs.migration_sorting_mode.set(mode);
    prefs.migration_sorting_direction.set(direction);
}

/// Flips the mode and keeps the direction. Returns the new mode.
pub fn toggle_mode(prefs: &SourcePrefs) -> Mode {
    let next = toggled_mode(prefs.migration_sorting_mode.get());
    prefs.migration_sorting_mode.set(next);
    next
}

/// Flips the direction and keeps the mode. Returns the new direction.
pub fn toggle_direction(prefs: &SourcePrefs) -> Direction {
    let next = toggled_direction(prefs.migration_sorting_direction.get());
    prefs.migration_sorting_direction.set(next);
    next
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

/// The sorted migration list as a stream. It is recomputed when the mode, the direction or the source list changes, and
/// published only when it differs from the last one.
pub fn subscribe_migration_sources(
    prefs: &SourcePrefs,
    sources: watch::Receiver<Vec<(Source, i64)>>,
) -> watch::Receiver<Vec<(Source, i64)>> {
    let mut inputs = combine3(
        pref_stream(&prefs.migration_sorting_direction),
        pref_stream(&prefs.migration_sorting_mode),
        sources,
    );
    let (tx, rx) = watch::channel(sorted_from(&inputs.borrow()));
    tokio::spawn(async move {
        while inputs.changed().await.is_ok() {
            let list = sorted_from(&inputs.borrow_and_update());
            if tx.is_closed() {
                break;
            }
            tx.send_if_modified(|current| {
                if *current == list {
                    false
                } else {
                    *current = list;
                    true
                }
            });
        }
    });
    rx
}

fn sorted_from((direction, mode, list): &(Direction, Mode, Vec<(Source, i64)>)) -> Vec<(Source, i64)> {
    sort_migration_sources(list.clone(), *mode, *direction)
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::app::rx::TEST_WAIT;
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
    fn the_mode_toggle_keeps_the_direction_and_the_direction_toggle_keeps_the_mode() {
        assert_eq!(toggled_mode(Mode::ALPHABETICAL), Mode::TOTAL);
        assert_eq!(toggled_mode(Mode::TOTAL), Mode::ALPHABETICAL);
        assert_eq!(toggled_direction(Direction::ASCENDING), Direction::DESCENDING);
        assert_eq!(toggled_direction(Direction::DESCENDING), Direction::ASCENDING);

        let prefs = SourcePrefs::new(PrefStore::in_memory());
        set_migration_sorting(&prefs, Mode::ALPHABETICAL, Direction::DESCENDING);
        assert_eq!(toggle_mode(&prefs), Mode::TOTAL);
        assert_eq!(prefs.migration_sorting_direction.get(), Direction::DESCENDING, "the mode toggle keeps the direction");
        assert_eq!(toggle_direction(&prefs), Direction::ASCENDING);
        assert_eq!(prefs.migration_sorting_mode.get(), Mode::TOTAL, "the direction toggle keeps the mode");
    }

    #[test]
    fn set_migration_sorting_writes_the_mode_then_the_direction() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        let changes = store.subscribe();
        set_migration_sorting(&prefs, Mode::TOTAL, Direction::DESCENDING);
        let written: Vec<String> = changes.try_iter().filter_map(|changed| changed.key).collect();
        assert_eq!(written, ["pref_migration_sorting", "pref_migration_direction"]);
        assert_eq!(prefs.migration_sorting_mode.get(), Mode::TOTAL);
        assert_eq!(prefs.migration_sorting_direction.get(), Direction::DESCENDING);
    }

    #[tokio::test]
    async fn the_stream_resorts_on_a_setting_change_and_publishes_only_real_changes() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        let (_tx, rx) = watch::channel(section3());
        let mut stream = subscribe_migration_sources(&prefs, rx);
        assert_eq!(ids(&stream.borrow_and_update()), [4, 2, 3, 5, 1], "default: alphabetical ascending");

        toggle_mode(&prefs);
        tokio::time::timeout(TEST_WAIT, stream.changed()).await.expect("waited").expect("open");
        assert_eq!(ids(&stream.borrow_and_update()), [4, 1, 3, 2, 5], "total ascending");

        // Writing the same direction again changes nothing in the list, so nothing is published.
        prefs.migration_sorting_direction.set(Direction::ASCENDING);
        let waited = tokio::time::timeout(std::time::Duration::from_millis(200), stream.changed()).await;
        assert!(waited.is_err(), "an equal list is not published");
    }
}
