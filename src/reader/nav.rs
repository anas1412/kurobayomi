//! Adjacent chapter navigation: neighbours by list position, and the N and P keys (`ReaderViewModel.kt:608-619`,
//! `ReaderActivity.kt:308-325`). T-43a-026, and the neighbour rule of T-43a-025.

/// Which neighbour a step asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Next,
    Previous,
}

/// The previous and next positions of `position` in a list of `len` chapters.
///
/// Upstream uses `getOrNull(indexOf - 1)` and `getOrNull(indexOf + 1)`. A chapter that is not in the list has
/// `indexOf == -1`, so its previous is `None` and its next is the first chapter (E-NAV-3, kept on purpose).
pub fn neighbours(len: usize, position: Option<usize>) -> (Option<usize>, Option<usize>) {
    let pos: i64 = position.map_or(-1, |p| p as i64);
    let at = |i: i64| usize::try_from(i).ok().filter(|&i| i < len);
    (at(pos - 1), at(pos + 1))
}

/// The neighbour a step goes to, or `None` at the ends (the button is disabled there).
pub fn target(len: usize, position: Option<usize>, step: Step) -> Option<usize> {
    let (previous, next) = neighbours(len, position);
    match step {
        Step::Next => next,
        Step::Previous => previous,
    }
}

/// N moves to the next chapter and P to the previous one, in either case.
pub fn key_step(key: char) -> Option<Step> {
    match key.to_ascii_lowercase() {
        'n' => Some(Step::Next),
        'p' => Some(Step::Previous),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_are_disabled_at_the_ends_and_enabled_between() {
        assert_eq!(target(3, Some(0), Step::Previous), None);
        assert_eq!(target(3, Some(0), Step::Next), Some(1));
        assert_eq!(target(3, Some(2), Step::Next), None);
        assert_eq!(target(3, Some(2), Step::Previous), Some(1));
    }

    #[test]
    fn an_unknown_position_has_no_previous_and_the_first_chapter_as_next() {
        assert_eq!(neighbours(3, None), (None, Some(0)));
        assert_eq!(neighbours(0, None), (None, None));
    }

    #[test]
    fn n_and_p_keys_map_to_next_and_previous_in_either_case() {
        assert_eq!(key_step('n'), Some(Step::Next));
        assert_eq!(key_step('N'), Some(Step::Next));
        assert_eq!(key_step('p'), Some(Step::Previous));
        assert_eq!(key_step('x'), None);
    }
}
