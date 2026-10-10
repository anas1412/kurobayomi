//! Per-anime skip-intro length, stored in the anime's viewer flags (area 45 `PlayerViewModel`
//! `getAnimeSkipIntroLength` and `setAnimeSkipIntroLength`, `SetAnimeViewerFlags`, T-45b-041).
//! Bits 0-7 hold the length in seconds. Bit 56 disables the skip. Pure: the caller reads and writes
//! the viewer flags.

/// Length used when the anime has none (`defaultIntroLength`).
pub const DEFAULT_LENGTH: i64 = 85;
/// Largest length the 8-bit field can hold.
pub const MAX_LENGTH: i64 = 255;
const LENGTH_MASK: i64 = 0xFF;
/// `ANIME_INTRO_DISABLE_MASK`: bit 56, the same as `1 << 56`.
const DISABLE_BIT: i64 = 1 << 56;

/// The stored length (bits 0-7), without the default or the disable bit applied.
pub fn stored_length(viewer_flags: i64) -> i64 {
    viewer_flags & LENGTH_MASK
}

/// True when bit 56 is set.
pub fn is_disabled(viewer_flags: i64) -> bool {
    viewer_flags & DISABLE_BIT == DISABLE_BIT
}

/// `getAnimeSkipIntroLength`: the default when there is no anime, 0 when disabled, the default when
/// the stored length is zero, and the stored length otherwise.
pub fn effective_length(viewer_flags: Option<i64>, default: i64) -> i64 {
    match viewer_flags {
        None => default,
        Some(flags) if is_disabled(flags) => 0,
        Some(flags) => match stored_length(flags) {
            0 => default,
            length => length,
        },
    }
}

/// The viewer flags after writing `length`. The value is clamped to 0..=255 first (E-27), so a
/// larger value is never truncated to a wrong one. Bits outside 0-7 and 56 are kept. Zero sets the
/// disable bit.
pub fn with_length(viewer_flags: i64, length: i64) -> i64 {
    let length = length.clamp(0, MAX_LENGTH);
    let kept = viewer_flags & !LENGTH_MASK & !DISABLE_BIT;
    let disable = if length == 0 { DISABLE_BIT } else { 0 };
    kept | length | disable
}

/// What a length change does, following the Kotlin order of checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// No anime: nothing to write.
    Nothing,
    /// Not in the library: the length applies to this session only (E-24). Holds the clamped value.
    SessionOnly(i64),
    /// Write these viewer flags.
    Store(i64),
    /// Same as the current effective length: nothing to write.
    Unchanged,
}

/// Decides what a request for `length` does for an anime with `viewer_flags` (`None` when there is
/// no anime). `favorite` says whether the anime is in the library.
pub fn plan_set(viewer_flags: Option<i64>, favorite: bool, length: i64) -> Plan {
    let Some(flags) = viewer_flags else {
        return Plan::Nothing;
    };
    let clamped = length.clamp(0, MAX_LENGTH);
    if !favorite {
        return Plan::SessionOnly(clamped);
    }
    if clamped == effective_length(Some(flags), DEFAULT_LENGTH) {
        return Plan::Unchanged;
    }
    Plan::Store(with_length(flags, clamped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_round_trips_through_the_low_byte() {
        let flags = with_length(0, 85);
        assert_eq!(stored_length(flags), 85);
        assert!(!is_disabled(flags));
        assert_eq!(effective_length(Some(flags), DEFAULT_LENGTH), 85);
    }

    #[test]
    fn zero_sets_the_disable_bit_and_reads_back_as_zero() {
        let flags = with_length(0, 0);
        assert!(is_disabled(flags));
        assert_eq!(stored_length(flags), 0);
        assert_eq!(effective_length(Some(flags), DEFAULT_LENGTH), 0);
        let enabled = with_length(flags, 30);
        assert!(!is_disabled(enabled));
    }

    #[test]
    fn t78_oversized_values_are_clamped_not_truncated() {
        let flags = with_length(0, 300);
        assert_eq!(stored_length(flags), 255);
        assert_eq!(with_length(0, -4), with_length(0, 0));
    }

    #[test]
    fn other_viewer_bits_are_kept() {
        let other = 0x0000_0000_0000_0100_i64;
        assert_eq!(with_length(other, 10) & other, other);
    }

    #[test]
    fn no_anime_uses_the_default_and_a_zero_length_falls_back() {
        assert_eq!(effective_length(None, DEFAULT_LENGTH), 85);
        assert_eq!(effective_length(Some(0), DEFAULT_LENGTH), 85);
    }

    #[test]
    fn non_favourites_only_change_the_session() {
        assert_eq!(plan_set(Some(0), false, 300), Plan::SessionOnly(255));
        assert_eq!(plan_set(None, true, 30), Plan::Nothing);
        assert_eq!(plan_set(Some(0), true, 85), Plan::Unchanged);
        assert_eq!(plan_set(Some(0), true, 30), Plan::Store(30));
    }
}
