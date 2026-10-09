//! The filter state of a library filter: off, must match, or must not match.

use crate::pref_enum;

pref_enum! {
    /// Stored as the Kotlin constant name.
    pub enum TriState { DISABLED, ENABLED_IS, ENABLED_NOT }
}

impl TriState {
    /// The next state when the user clicks: DISABLED, ENABLED_IS, ENABLED_NOT, DISABLED.
    pub fn next(self) -> Self {
        match self {
            Self::DISABLED => Self::ENABLED_IS,
            Self::ENABLED_IS => Self::ENABLED_NOT,
            Self::ENABLED_NOT => Self::DISABLED,
        }
    }

    /// Whether an item passes this filter. `predicate` runs only when the filter is on.
    pub fn apply_filter(self, predicate: impl FnOnce() -> bool) -> bool {
        match self {
            Self::DISABLED => true,
            Self::ENABLED_IS => predicate(),
            Self::ENABLED_NOT => !predicate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::{Decoded, PrefType, PrefValue};

    #[test]
    fn next_cycles_in_order() {
        assert_eq!(TriState::DISABLED.next(), TriState::ENABLED_IS);
        assert_eq!(TriState::ENABLED_IS.next(), TriState::ENABLED_NOT);
        assert_eq!(TriState::ENABLED_NOT.next(), TriState::DISABLED);
    }

    #[test]
    fn apply_filter_matches_the_kotlin_rules() {
        assert!(TriState::DISABLED.apply_filter(|| false));
        assert!(TriState::ENABLED_IS.apply_filter(|| true));
        assert!(!TriState::ENABLED_IS.apply_filter(|| false));
        assert!(TriState::ENABLED_NOT.apply_filter(|| false));
        assert!(!TriState::ENABLED_NOT.apply_filter(|| true));
    }

    #[test]
    fn stored_as_the_constant_name() {
        assert_eq!(
            TriState::ENABLED_NOT.encode(),
            PrefValue::Str("ENABLED_NOT".into())
        );
        assert_eq!(
            TriState::decode(&PrefValue::Str("nope".into())),
            Decoded::Invalid
        );
    }
}
