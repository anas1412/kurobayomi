//! Collection helpers (Kotlin `CollectionUtils.kt`, T-52a-004).
//!
//! `fastFilterNot`, `fastPartition` and `fastCountNot` are not ported: Rust has `Iterator::filter`,
//! `Iterator::partition` and `Iterator::filter(..).count()`, so no wrapper is exported. The tests below hold the
//! std behaviour the callers rely on (order kept in both halves).

use std::collections::HashSet;
use std::hash::Hash;

/// Adds `value` when `should_add` is true, removes it otherwise. Removing an absent value does nothing.
pub fn add_or_remove<T: Eq + Hash>(set: &mut HashSet<T>, value: T, should_add: bool) {
    if should_add {
        set.insert(value);
    } else {
        set.remove(&value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_an_absent_value_is_a_no_op() {
        // TC-04-a
        let mut set: HashSet<i32> = [1, 2].into();
        add_or_remove(&mut set, 9, false);
        assert_eq!(set, HashSet::from([1, 2]));
    }

    #[test]
    fn true_adds_and_false_removes() {
        let mut set = HashSet::new();
        add_or_remove(&mut set, "a", true);
        add_or_remove(&mut set, "a", true);
        assert_eq!(set.len(), 1);
        add_or_remove(&mut set, "a", false);
        assert!(set.is_empty());
    }

    #[test]
    fn partition_keeps_input_order_in_both_halves() {
        let (even, odd): (Vec<i32>, Vec<i32>) = [5, 2, 3, 8, 1, 4].into_iter().partition(|n| n % 2 == 0);
        assert_eq!(even, [2, 8, 4]);
        assert_eq!(odd, [5, 3, 1]);
    }

    #[test]
    fn filter_not_and_count_not_are_filter_with_a_negated_predicate() {
        let items = [1, 2, 3, 4, 5];
        let kept: Vec<i32> = items.iter().copied().filter(|n| n % 2 != 0).collect();
        assert_eq!(kept, [1, 3, 5]);
        assert_eq!(items.iter().filter(|n| **n % 2 != 0).count(), 3);
    }
}
