//! Collection helpers (port of `CollectionUtils.kt` and `ChapterRemoveDuplicates.kt`).
//!
//! Not ported: `addOrRemove`, `fastFilterNot`, `fastPartition`, `fastCountNot`.
//! Call sites use std instead: `HashSet::insert` / `HashSet::remove`, `Iterator::filter`,
//! `Iterator::partition`, and `filter(..).count()`.

use std::collections::HashMap;

/// One slot of a list with separators inserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry<T, S> {
    Separator(S),
    Item(T),
}

/// Inserts the optional separator for every adjacent pair, calling `separator_for` in
/// order `(None, first)` through `(last, None)`. Empty input returns an empty list and
/// makes no calls.
pub fn insert_separators<T, S>(
    items: Vec<T>,
    separator_for: impl FnMut(Option<&T>, Option<&T>) -> Option<S>,
) -> Vec<Entry<T, S>> {
    layout(items, separator_for, false)
}

/// Same output as [`insert_separators`], but calls `separator_for` in the exact reverse
/// order: `(last, None)` through `(None, first)`.
pub fn insert_separators_reversed<T, S>(
    items: Vec<T>,
    separator_for: impl FnMut(Option<&T>, Option<&T>) -> Option<S>,
) -> Vec<Entry<T, S>> {
    layout(items, separator_for, true)
}

fn layout<T, S>(
    items: Vec<T>,
    mut separator_for: impl FnMut(Option<&T>, Option<&T>) -> Option<S>,
    reversed: bool,
) -> Vec<Entry<T, S>> {
    if items.is_empty() {
        return Vec::new();
    }
    let n = items.len();
    // Slot i holds the separator between items[i - 1] and items[i]; slot n is the tail.
    let mut order: Vec<usize> = (0..=n).collect();
    if reversed {
        order.reverse();
    }
    let mut slots: Vec<Option<S>> = (0..=n).map(|_| None).collect();
    for i in order {
        let before = i.checked_sub(1).map(|p| &items[p]);
        slots[i] = separator_for(before, items.get(i));
    }
    let mut out = Vec::with_capacity(2 * n + 1);
    for (i, item) in items.into_iter().enumerate() {
        if let Some(sep) = slots[i].take() {
            out.push(Entry::Separator(sep));
        }
        out.push(Entry::Item(item));
    }
    if let Some(sep) = slots[n].take() {
        out.push(Entry::Separator(sep));
    }
    out
}

/// A chapter as far as duplicate removal cares.
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    pub id: i64,
    pub number: f64,
    pub scanlator: Option<String>,
}

/// Keeps one chapter per chapter number. Within a number the chapter with the current id
/// wins, then the first with the current chapter's scanlator (`None` matches `None`), then
/// the first of the group. Numbers compare by value (`-0.0` is `0.0`). Output follows the
/// first appearance of each number.
pub fn remove_duplicates<'a>(chapters: &'a [Chapter], current: &Chapter) -> Vec<&'a Chapter> {
    let mut groups: Vec<Vec<&'a Chapter>> = Vec::new();
    let mut group_of: HashMap<u64, usize> = HashMap::new();
    for chapter in chapters {
        match group_of.get(&number_key(chapter.number)) {
            Some(&g) => groups[g].push(chapter),
            None => {
                group_of.insert(number_key(chapter.number), groups.len());
                groups.push(vec![chapter]);
            }
        }
    }
    groups
        .into_iter()
        .map(|group| {
            group
                .iter()
                .copied()
                .find(|c| c.id == current.id)
                .or_else(|| {
                    group
                        .iter()
                        .copied()
                        .find(|c| c.scanlator == current.scanlator)
                })
                .unwrap_or(group[0])
        })
        .collect()
}

/// Bit key for a chapter number with `-0.0` folded into `0.0`.
fn number_key(number: f64) -> u64 {
    if number == 0.0 {
        0.0f64.to_bits()
    } else {
        number.to_bits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> String {
        text.to_owned()
    }

    fn flatten(entries: Vec<Entry<String, String>>) -> Vec<String> {
        entries
            .into_iter()
            .map(|e| match e {
                Entry::Separator(v) | Entry::Item(v) => v,
            })
            .collect()
    }

    #[test]
    fn empty_input_makes_no_calls() {
        let mut calls = 0;
        let out = insert_separators(Vec::<String>::new(), |_, _| {
            calls += 1;
            Some(s("sep"))
        });
        assert!(out.is_empty());
        assert_eq!(calls, 0);
        let out = insert_separators_reversed(Vec::<String>::new(), |_, _| -> Option<String> {
            calls += 1;
            None
        });
        assert!(out.is_empty());
        assert_eq!(calls, 0);
    }

    #[test]
    fn single_item_gets_two_calls_in_both_orders() {
        let mut forward = Vec::new();
        insert_separators(vec![s("x")], |b, a| {
            forward.push((b.cloned(), a.cloned()));
            None::<String>
        });
        assert_eq!(forward, vec![(None, Some(s("x"))), (Some(s("x")), None)]);

        let mut reversed = Vec::new();
        insert_separators_reversed(vec![s("x")], |b, a| {
            reversed.push((b.cloned(), a.cloned()));
            None::<String>
        });
        assert_eq!(reversed, vec![(Some(s("x")), None), (None, Some(s("x")))]);
    }

    /// Header before each new first character, never after the last item.
    fn header(before: Option<&String>, after: Option<&String>) -> Option<String> {
        let after = after?;
        let first = after.chars().next()?;
        match before.and_then(|b| b.chars().next()) {
            Some(prev) if prev == first => None,
            _ => Some(format!("H:{first}")),
        }
    }

    #[test]
    fn grouped_headers_forward_and_reversed() {
        let items = vec![s("a1"), s("a2"), s("b1")];
        let expected = vec![s("H:a"), s("a1"), s("a2"), s("H:b"), s("b1")];

        let mut forward = Vec::new();
        let out = insert_separators(items.clone(), |b, a| {
            forward.push((b.cloned(), a.cloned()));
            header(b, a)
        });
        assert_eq!(flatten(out), expected);
        assert_eq!(
            forward,
            vec![
                (None, Some(s("a1"))),
                (Some(s("a1")), Some(s("a2"))),
                (Some(s("a2")), Some(s("b1"))),
                (Some(s("b1")), None),
            ]
        );

        let mut reversed = Vec::new();
        let out = insert_separators_reversed(items, |b, a| {
            reversed.push((b.cloned(), a.cloned()));
            header(b, a)
        });
        assert_eq!(flatten(out), expected);
        let mut expected_reversed = forward.clone();
        expected_reversed.reverse();
        assert_eq!(reversed, expected_reversed);
    }

    #[test]
    fn always_some_gives_two_n_plus_one_entries() {
        let mut k = 0;
        let out = insert_separators(vec![1, 2], |_, _| {
            k += 1;
            Some(format!("S{}", k - 1))
        });
        let expected = vec![
            Entry::Separator(s("S0")),
            Entry::Item(1),
            Entry::Separator(s("S1")),
            Entry::Item(2),
            Entry::Separator(s("S2")),
        ];
        assert_eq!(out, expected);
    }

    #[test]
    fn always_none_returns_items_unchanged() {
        for items in [vec![], vec![1], vec![3, 1, 4, 1, 5, 9, 2, 6]] {
            let out: Vec<Entry<i32, ()>> = insert_separators(items.clone(), |_, _| None);
            let expected: Vec<Entry<i32, ()>> = items.into_iter().map(Entry::Item).collect();
            assert_eq!(out, expected);
        }
    }

    fn chapter(id: i64, number: f64, scanlator: Option<&str>) -> Chapter {
        Chapter {
            id,
            number,
            scanlator: scanlator.map(str::to_owned),
        }
    }

    #[test]
    fn current_id_wins_over_scanlator_and_first() {
        let list = [
            chapter(1, 1.0, Some("A")),
            chapter(2, 1.0, Some("B")),
            chapter(3, 1.0, Some("A")),
        ];
        let current = chapter(2, 1.0, Some("A"));
        let kept = remove_duplicates(&list, &current);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, 2);
    }

    #[test]
    fn same_scanlator_wins_then_first() {
        let list = [
            chapter(1, 1.0, Some("X")),
            chapter(2, 1.0, Some("Y")),
            chapter(3, 1.0, Some("Y")),
        ];
        let current = chapter(9, 1.0, Some("Y"));
        assert_eq!(remove_duplicates(&list, &current)[0].id, 2);

        let current = chapter(9, 1.0, Some("Z"));
        assert_eq!(remove_duplicates(&list, &current)[0].id, 1);
    }

    #[test]
    fn null_scanlator_matches_null() {
        let list = [chapter(1, 2.0, Some("Y")), chapter(2, 2.0, None)];
        let current = chapter(9, 2.0, None);
        assert_eq!(remove_duplicates(&list, &current)[0].id, 2);
    }

    #[test]
    fn equal_numbers_group_together() {
        let list = [
            chapter(1, 1.0, None),
            chapter(2, 1.00, None),
            chapter(3, 0.0, None),
            chapter(4, -0.0, None),
        ];
        let current = chapter(9, 0.0, None);
        let kept: Vec<i64> = remove_duplicates(&list, &current)
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(kept, vec![1, 3]);
    }

    #[test]
    fn output_follows_first_appearance_of_groups() {
        let list = [
            chapter(10, 3.0, None),
            chapter(11, 1.0, None),
            chapter(12, 3.0, None),
            chapter(13, 2.0, None),
        ];
        let current = chapter(99, 0.0, None);
        let kept: Vec<i64> = remove_duplicates(&list, &current)
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(kept, vec![10, 11, 13]);
    }
}
