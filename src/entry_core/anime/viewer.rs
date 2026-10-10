//! Viewer flags of the anime entry (T-39b-009). Upstream: `SetAnimeViewerFlags.kt`. The skip-intro length (bits 0-7),
//! the disable bit (bit 56) and the next-airing groups (bits 8-23 and 24-55) live in one `viewer_flags` word. Each
//! write is one masked update of its own bits (E-AIR-1), so writing one group never reads or clears another. The
//! codec and masks come from `kuro_domain::entries::anime`; this module only builds `(value, mask)` pairs for the
//! store. No GTK here.

use kuro_data::anime::Anime;
use kuro_domain::entries::anime::{AnimeExt, viewer_flags};

/// The bits a skip-intro write owns: the length and the disable bit.
pub const INTRO_WRITE_MASK: i64 = viewer_flags::INTRO_MASK | viewer_flags::INTRO_DISABLE_MASK;

/// The bits a next-airing write owns: the episode number and the airing time.
pub const AIRING_WRITE_MASK: i64 = viewer_flags::AIRING_EPISODE_MASK | viewer_flags::AIRING_TIME_MASK;

/// The `(value, mask)` pair that stores a skip-intro length. The length is stored in its eight bits (300 stores 44,
/// -1 stores 255), and a stored length of 0 sets the disable bit (`setSkipIntroLength`).
pub fn skip_intro_write(length: i32) -> (i64, i64) {
    let mut scratch = Anime::create();
    scratch.set_skip_intro_length(length);
    (scratch.viewer_flags & INTRO_WRITE_MASK, INTRO_WRITE_MASK)
}

/// The `(value, mask)` pair that stores the next episode to air and its airing time, in one change (fixes Q17).
pub fn next_airing_write(episode: i32, airing_at: i64) -> (i64, i64) {
    let mut scratch = Anime::create();
    scratch.set_next_episode_airing(episode, airing_at);
    (scratch.viewer_flags & AIRING_WRITE_MASK, AIRING_WRITE_MASK)
}

/// The word after a masked write: `(old & !mask) | (value & mask)`. Mirrors what the store's UPDATE does.
pub fn apply(word: i64, (value, mask): (i64, i64)) -> i64 {
    (word & !mask) | (value & mask)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_length_round_trips_and_is_clamped_to_eight_bits() {
        for (length, stored) in [(90, 90), (300, 44), (-1, 255)] {
            let word = apply(0, skip_intro_write(length));
            assert_eq!(word & viewer_flags::INTRO_MASK, stored);
        }
        let mut row = Anime::create();
        row.viewer_flags = apply(0, skip_intro_write(90));
        assert_eq!(row.skip_intro_length(), 90);
        assert!(!row.skip_intro_disable());
    }

    #[test]
    fn a_zero_length_sets_the_disable_bit() {
        let mut row = Anime::create();
        row.viewer_flags = apply(0, skip_intro_write(0));
        assert!(row.skip_intro_disable());
        assert_eq!(row.skip_intro_length(), 0);
        row.viewer_flags = apply(row.viewer_flags, skip_intro_write(12));
        assert!(!row.skip_intro_disable(), "a non-zero length clears the disable bit");
        assert_eq!(row.skip_intro_length(), 12);
    }

    #[test]
    fn writes_are_independent_and_leave_the_airing_groups_alone() {
        let airing = (1_700_000_000_i64 << 24) | (7_i64 << 8);
        let word = apply(airing | 10 | viewer_flags::INTRO_DISABLE_MASK, skip_intro_write(90));
        assert_eq!(word & AIRING_WRITE_MASK, airing & AIRING_WRITE_MASK, "the intro write keeps the airing bits");
        assert_eq!(word & viewer_flags::INTRO_MASK, 90);
        assert_eq!(word & viewer_flags::INTRO_DISABLE_MASK, 0);

        let word = apply(word, next_airing_write(12, 1_800_000_000));
        assert_eq!(word & viewer_flags::INTRO_MASK, 90, "the airing write keeps the intro length");
        let mut row = Anime::create();
        row.viewer_flags = word;
        assert_eq!(row.next_episode_to_air(), 12);
        assert_eq!(row.next_episode_airing_at(), 1_800_000_000);
    }

    #[test]
    fn airing_values_are_clamped_like_the_setter() {
        let mut row = Anime::create();
        row.viewer_flags = apply(0, next_airing_write(70_000, 5));
        assert_eq!(row.next_episode_to_air(), 70_000 & 0xFFFF);
        assert_eq!(row.next_episode_airing_at(), 5);
    }
}
