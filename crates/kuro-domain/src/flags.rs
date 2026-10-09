//! Flag primitives shared by every bit-flag word (`Flag.kt`, and the private `setFlag` copies in the
//! `Set*Flags` interactors). `set_flag` and `TriState` are re-exported, not redefined: the `kuro-db`
//! and `kuro-prefs` crates own them.

pub use kuro_db::flags::set_flag;
pub use kuro_prefs::tristate::TriState;

/// Maps a masked two-bit filter group to its state. `raw == is` is `EnabledIs`, `raw == not` is
/// `EnabledNot`, and everything else (including `0` and the invalid value `is | not`) is `Disabled`.
pub fn tri(raw: i64, is: i64, not: i64) -> TriState {
    if raw == 0 {
        TriState::DISABLED
    } else if raw == is {
        TriState::ENABLED_IS
    } else if raw == not {
        TriState::ENABLED_NOT
    } else {
        TriState::DISABLED
    }
}

/// `applyFilter`: `Disabled` passes without calling `predicate`, `EnabledIs` returns the predicate,
/// `EnabledNot` returns its negation.
pub fn apply_filter(filter: TriState, predicate: impl FnOnce() -> bool) -> bool {
    filter.apply_filter(predicate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn set_flag_replaces_only_masked_bits() {
        let vectors: [(i64, i64, i64, i64); 8] = [
            (0, 0x6, 0x6, 0x6),
            (0x7, 0x0, 0x6, 0x1),
            (0x7, 0x8, 0x6, 0x1),
            (0x0, 0xFF, 0x18, 0x18),
            (-1, 0, 0x6, -7),
            (-1, 0x2, 0x6, -5),
            (0x100_0000, 0x4, 0x6, 0x100_0004),
            (0x1234, -1, 0x60, 0x1274),
        ];
        for (value, flag, mask, expected) in vectors {
            assert_eq!(
                set_flag(value, flag, mask),
                expected,
                "set_flag({value:#x}, {flag:#x}, {mask:#x})"
            );
        }
    }

    #[test]
    fn tri_state_next_cycles_in_order() {
        assert_eq!(TriState::DISABLED.next(), TriState::ENABLED_IS);
        assert_eq!(TriState::ENABLED_IS.next(), TriState::ENABLED_NOT);
        assert_eq!(TriState::ENABLED_NOT.next(), TriState::DISABLED);
    }

    #[test]
    fn apply_filter_does_not_call_predicate_when_disabled() {
        let calls = Cell::new(0);
        let pass = apply_filter(TriState::DISABLED, || {
            calls.set(calls.get() + 1);
            false
        });
        assert!(pass);
        assert_eq!(calls.get(), 0);

        let is = apply_filter(TriState::ENABLED_IS, || {
            calls.set(calls.get() + 1);
            true
        });
        assert!(is);
        assert_eq!(calls.get(), 1);

        assert!(!apply_filter(TriState::ENABLED_NOT, || true));
        assert!(apply_filter(TriState::ENABLED_NOT, || false));
    }

    #[test]
    fn tri_maps_zero_and_invalid_group_to_disabled() {
        assert_eq!(tri(0, 0x2, 0x4), TriState::DISABLED);
        assert_eq!(tri(0x2, 0x2, 0x4), TriState::ENABLED_IS);
        assert_eq!(tri(0x4, 0x2, 0x4), TriState::ENABLED_NOT);
        assert_eq!(tri(0x6, 0x2, 0x4), TriState::DISABLED);
    }
}
