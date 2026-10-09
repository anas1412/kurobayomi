//! `UpdateStrategy` and its stored ordinal (`UpdateStrategy.kt`, T-15a-007).

use serde::{Deserialize, Serialize};

/// How the library update job treats a manga. The stored value is the ordinal.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "i64", into = "i64")]
pub enum UpdateStrategy {
    #[default]
    AlwaysUpdate = 0,
    OnlyFetchOnce = 1,
}

impl UpdateStrategy {
    /// Whether the library update job skips the entry. Only `OnlyFetchOnce` is skipped; the decision is taken by the
    /// update job, not by the sources (behaviour 4.8).
    pub fn skips_library_update(self) -> bool {
        self == Self::OnlyFetchOnce
    }
}

/// Any stored value other than `1` reads as `AlwaysUpdate` (EC-MD-05).
impl From<i64> for UpdateStrategy {
    fn from(stored: i64) -> Self {
        match stored {
            1 => Self::OnlyFetchOnce,
            _ => Self::AlwaysUpdate,
        }
    }
}

impl From<UpdateStrategy> for i64 {
    fn from(strategy: UpdateStrategy) -> Self {
        strategy as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ordinals_are_zero_and_one() {
        assert_eq!(i64::from(UpdateStrategy::AlwaysUpdate), 0);
        assert_eq!(i64::from(UpdateStrategy::OnlyFetchOnce), 1);
    }

    #[test]
    fn both_variants_round_trip_through_the_stored_value() {
        for s in [UpdateStrategy::AlwaysUpdate, UpdateStrategy::OnlyFetchOnce] {
            assert_eq!(UpdateStrategy::from(i64::from(s)), s);
        }
    }

    #[test]
    fn only_fetch_once_is_skipped_by_the_library_update() {
        assert!(UpdateStrategy::OnlyFetchOnce.skips_library_update());
        assert!(!UpdateStrategy::AlwaysUpdate.skips_library_update());
        assert!(!UpdateStrategy::default().skips_library_update());
    }

    #[test]
    fn an_unknown_stored_value_is_always_update() {
        for stored in [2, -1, 999] {
            assert_eq!(UpdateStrategy::from(stored), UpdateStrategy::AlwaysUpdate);
        }
    }
}
