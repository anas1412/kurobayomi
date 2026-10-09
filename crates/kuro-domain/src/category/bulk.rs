//! Bulk change of categories for a selection (behaviour 1.10, Q26). Pure: the screen reads the entries' category
//! ids (hidden ones included), shows the dialog states, and writes the result with [`apply`].

use std::collections::BTreeSet;

use crate::SYSTEM_CATEGORY_ID;

/// How a category looks before the user clicks: every entry has it, some have it, or none does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initial {
    Checked,
    Mixed,
    Unchecked,
}

/// The value of one category row in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// A plain row that every entry has.
    Checked,
    /// A plain row that is off: the category is removed from every entry.
    Unchecked,
    /// A mixed row left alone: the entries keep whatever they have.
    Untouched,
    /// A mixed row set to add the category to every entry.
    Include,
    /// A mixed row set to remove the category from every entry.
    Exclude,
}

/// `bulk_dialog_state`: for each non-system category, `Checked` if every entry is in it, `Mixed` if some are, and
/// `Unchecked` otherwise. An empty selection has nothing checked.
pub fn initial_states(entries: &[Vec<i64>], categories: &[i64]) -> Vec<(i64, Initial)> {
    categories
        .iter()
        .copied()
        .filter(|c| *c != SYSTEM_CATEGORY_ID)
        .map(|c| {
            let count = entries.iter().filter(|cats| cats.contains(&c)).count();
            let state = if !entries.is_empty() && count == entries.len() {
                Initial::Checked
            } else if count > 0 {
                Initial::Mixed
            } else {
                Initial::Unchecked
            };
            (c, state)
        })
        .collect()
}

/// The starting value of a row: a mixed row starts `Untouched`, a plain row keeps its checked state.
pub fn start_value(initial: Initial) -> Choice {
    match initial {
        Initial::Checked => Choice::Checked,
        Initial::Mixed => Choice::Untouched,
        Initial::Unchecked => Choice::Unchecked,
    }
}

/// One click. Plain rows toggle `Checked` and `Unchecked`. Mixed rows follow the Android tri-state order
/// (CategoryDialogs.kt): `Untouched` -> `Exclude` (remove from all) -> `Include` (add to all) -> `Untouched`.
pub fn click(choice: Choice) -> Choice {
    match choice {
        Choice::Checked => Choice::Unchecked,
        Choice::Unchecked => Choice::Checked,
        Choice::Untouched => Choice::Exclude,
        Choice::Exclude => Choice::Include,
        Choice::Include => Choice::Untouched,
    }
}

/// The two id lists of a confirm: `(include, exclude)`.
pub fn confirm(choices: &[(i64, Choice)]) -> (Vec<i64>, Vec<i64>) {
    let mut include = Vec::new();
    let mut exclude = Vec::new();
    for (id, choice) in choices {
        match choice {
            Choice::Checked | Choice::Include => include.push(*id),
            Choice::Unchecked | Choice::Exclude => exclude.push(*id),
            Choice::Untouched => {}
        }
    }
    (include, exclude)
}

/// The new category set of one entry: its current set (every category, hidden ones included) minus `exclude`, plus
/// `include`. The result is sorted and distinct, and the system category is never added.
pub fn apply(current: &[i64], include: &[i64], exclude: &[i64]) -> Vec<i64> {
    let mut out: BTreeSet<i64> = current.iter().copied().filter(|c| !exclude.contains(c)).collect();
    out.extend(include.iter().copied().filter(|c| *c != SYSTEM_CATEGORY_ID));
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_and_mixed_states() {
        // E1 in {A=1, B=2}, E2 in {B=2, C=3}: common {B}, mixed {A, C}.
        let entries = vec![vec![1, 2], vec![2, 3]];
        let states = initial_states(&entries, &[0, 1, 2, 3]);
        assert_eq!(states, vec![(1, Initial::Mixed), (2, Initial::Checked), (3, Initial::Mixed)]);
    }

    #[test]
    fn plain_rows_toggle_and_mixed_rows_cycle() {
        assert_eq!(click(Choice::Checked), Choice::Unchecked);
        assert_eq!(click(Choice::Unchecked), Choice::Checked);
        let mixed = start_value(Initial::Mixed);
        assert_eq!(mixed, Choice::Untouched);
        assert_eq!(click(mixed), Choice::Exclude, "first click on a mixed row removes it from every entry");
        assert_eq!(click(click(mixed)), Choice::Include, "second click adds it to every entry");
        assert_eq!(click(click(click(mixed))), Choice::Untouched, "third click leaves the entries alone again");
    }

    #[test]
    fn apply_adds_and_removes_and_keeps_hidden_memberships() {
        // E1 in {A=1, B=2} plus hidden H=9; add D=4, remove B=2 -> {1, 4, 9}; E2 in {2, 3} -> {3, 4}.
        assert_eq!(apply(&[1, 2, 9], &[4], &[2]), vec![1, 4, 9]);
        assert_eq!(apply(&[2, 3], &[4], &[2]), vec![3, 4]);
        assert_eq!(apply(&[1], &[0], &[]), vec![1], "the system category is never added");
    }

    #[test]
    fn confirm_splits_the_choices_into_include_and_exclude() {
        let (inc, exc) = confirm(&[
            (1, Choice::Checked),
            (2, Choice::Unchecked),
            (3, Choice::Include),
            (4, Choice::Exclude),
            (5, Choice::Untouched),
        ]);
        assert_eq!(inc, vec![1, 3]);
        assert_eq!(exc, vec![2, 4]);
    }
}
