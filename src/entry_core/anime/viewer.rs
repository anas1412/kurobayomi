//! Viewer flags of the anime entry (T-39b-009). Upstream: `SetAnimeViewerFlags.kt`. The skip-intro length (bits 0-7),
//! the disable bit (bit 56) and the next-airing groups (bits 8-23 and 24-55) live in one `viewer_flags` word. Each
//! write is one masked update of its own bits (E-AIR-1), so writing one group never reads or clears another. The
//! codec and masks come from `kuro_domain::entries::flags`; this module only builds `(value, mask)` pairs for the
//! store. No GTK here.

/// The length the skip-intro dialog starts at (`AnimeScreen.kt`, `ChangeAnimeSkipIntro`): the stored length, or the
/// library default when the skip is on and no length is stored.
pub fn dialog_start(stored: i32, disabled: bool, default: i32) -> i32 {
    if !disabled && stored == 0 { default } else { stored }
}

#[cfg(test)]
use kuro_domain::entries::flags as viewer_flags;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_starts_at_the_default_only_for_an_unset_enabled_skip() {
        assert_eq!(dialog_start(0, false, 85), 85);
        assert_eq!(dialog_start(0, true, 85), 0, "a disabled skip starts at its stored length");
        assert_eq!(dialog_start(30, false, 85), 30);
    }

    #[test]
    fn the_masks_are_the_kotlin_values() {
        assert_eq!(viewer_flags::INTRO_MASK, 0xFF);
        assert_eq!(viewer_flags::AIRING_EPISODE_MASK, 0x00FF_FF00);
        assert_eq!(viewer_flags::AIRING_TIME_MASK, 0x0FFF_FFFF_F000_000);
        assert_eq!(viewer_flags::INTRO_DISABLE_MASK, 1 << 56);
    }

    #[test]
    fn the_intro_length_airing_episode_and_airing_time_masks_are_disjoint() {
        // T-F1
        let masks = [viewer_flags::INTRO_MASK, viewer_flags::AIRING_EPISODE_MASK, viewer_flags::AIRING_TIME_MASK];
        for (i, a) in masks.iter().enumerate() {
            for b in &masks[i + 1..] {
                assert_eq!(a & b, 0);
            }
        }
    }

}
