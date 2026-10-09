//! Checkbox states for category selection dialogs. These are UI values and are never stored.

/// A checkbox that is either checked or not.
#[derive(Clone, Debug, PartialEq)]
pub enum State<T> {
    Checked(T),
    None(T),
}

impl<T> State<T> {
    /// Toggles between checked and not.
    pub fn next(self) -> Self {
        match self {
            Self::Checked(v) => Self::None(v),
            Self::None(v) => Self::Checked(v),
        }
    }
}

/// A checkbox with include and exclude states.
#[derive(Clone, Debug, PartialEq)]
pub enum TriState<T> {
    Include(T),
    Exclude(T),
    None(T),
}

impl<T> TriState<T> {
    /// The cycle is None, Include, Exclude, None.
    pub fn next(self) -> Self {
        match self {
            Self::None(v) => Self::Include(v),
            Self::Include(v) => Self::Exclude(v),
            Self::Exclude(v) => Self::None(v),
        }
    }
}

/// `Checked` exactly when `condition` holds for the value.
pub fn as_checkbox_state<T>(value: T, condition: impl FnOnce(&T) -> bool) -> State<T> {
    if condition(&value) {
        State::Checked(value)
    } else {
        State::None(value)
    }
}

/// [`as_checkbox_state`] for each value of a list.
pub fn map_as_checkbox_state<T>(values: Vec<T>, condition: impl Fn(&T) -> bool) -> Vec<State<T>> {
    values
        .into_iter()
        .map(|v| as_checkbox_state(v, &condition))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_toggles() {
        assert_eq!(State::Checked(1).next(), State::None(1));
        assert_eq!(State::None(1).next(), State::Checked(1));
    }

    #[test]
    fn tri_state_cycles_none_include_exclude() {
        assert_eq!(TriState::None(1).next(), TriState::Include(1));
        assert_eq!(TriState::Include(1).next(), TriState::Exclude(1));
        assert_eq!(TriState::Exclude(1).next(), TriState::None(1));
    }

    #[test]
    fn checked_exactly_when_the_predicate_holds() {
        assert_eq!(as_checkbox_state(4, |v| v % 2 == 0), State::Checked(4));
        assert_eq!(as_checkbox_state(3, |v| v % 2 == 0), State::None(3));
        assert_eq!(
            map_as_checkbox_state(vec![1, 2, 3, 4], |v| *v > 2),
            vec![
                State::None(1),
                State::None(2),
                State::Checked(3),
                State::Checked(4)
            ]
        );
    }
}
