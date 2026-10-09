//! Property tests of the anime flag words and of the number recognisers (T-07a-043). Each word setter must write only its
//! own bits, every accessor must accept any 64-bit word without panicking, and every recogniser must return `-1.0` or a
//! finite non-negative number for any name.

use kuro_data::anime::Anime;
use kuro_domain::entries::anime::{AnimeExt, episode_flags, season_flags, viewer_flags};
use kuro_domain::items::recognition::{parse_chapter_number, parse_episode_number, parse_season_number};
use kuro_domain::library::flags::merge;
use proptest::prelude::*;

/// `word` with the group under `mask` set to `value` (the value is shifted into the mask's position first).
fn set_group(word: i64, mask: i64, value: i64) -> i64 {
    let shift = mask.trailing_zeros();
    merge(word, (value << shift) & mask, mask)
}

/// Every episode group: its mask and the raw accessor that reads it back.
fn episode_groups() -> Vec<(i64, fn(&Anime) -> i64)> {
    vec![
        (episode_flags::SORTING_MASK, |a| a.sorting()),
        (episode_flags::DISPLAY_MASK, |a| a.display_mode()),
        (episode_flags::UNSEEN_MASK, |a| a.unseen_filter_raw()),
        (episode_flags::DOWNLOADED_MASK, |a| a.downloaded_filter_raw()),
        (episode_flags::BOOKMARKED_MASK, |a| a.bookmarked_filter_raw()),
        (episode_flags::FILLERMARKED_MASK, |a| a.fillermarked_filter_raw()),
        (episode_flags::PREVIEWS_MASK, |a| a.episode_flags & episode_flags::PREVIEWS_MASK),
        (episode_flags::SUMMARIES_MASK, |a| a.episode_flags & episode_flags::SUMMARIES_MASK),
    ]
}

/// Every season group: its mask and the raw accessor that reads it back.
fn season_groups() -> Vec<(i64, fn(&Anime) -> i64)> {
    vec![
        (season_flags::DOWNLOADED_MASK, |a| a.season_downloaded_filter_raw()),
        (season_flags::UNSEEN_MASK, |a| a.season_unseen_filter_raw()),
        (season_flags::STARTED_MASK, |a| a.season_started_filter_raw()),
        (season_flags::COMPLETED_MASK, |a| a.season_completed_filter_raw()),
        (season_flags::BOOKMARKED_MASK, |a| a.season_bookmarked_filter_raw()),
        (season_flags::FILLERMARKED_MASK, |a| a.season_fillermarked_filter_raw()),
        (season_flags::SORT_MASK, |a| a.season_sorting()),
        (season_flags::DISPLAY_MODE_MASK, |a| a.season_display_mode()),
    ]
}

fn anime_with(episode: i64, season: i64, viewer: i64) -> Anime {
    Anime { episode_flags: episode, season_flags: season, viewer_flags: viewer, ..Anime::create() }
}

/// Reads every accessor of the trait, so a panic in any of them fails the property.
fn read_all(a: &Anime) {
    let _ = (
        a.sorting(),
        a.display_mode(),
        a.unseen_filter(),
        a.bookmarked_filter(),
        a.fillermarked_filter(),
        a.sort_descending(),
        a.show_previews(),
        a.show_summaries(),
        a.season_downloaded_filter(),
        a.season_unseen_filter(),
        a.season_started_filter(),
        a.season_completed_filter(),
        a.season_bookmarked_filter(),
        a.season_fillermarked_filter(),
        a.season_sorting(),
        a.season_sort_descending(),
        a.season_display_grid_mode(),
        a.season_display_grid_size(),
        a.season_downloaded_overlay(),
        a.season_unseen_overlay(),
        a.season_local_overlay(),
        a.season_lang_overlay(),
        a.season_continue_overlay(),
        a.season_display_mode(),
        a.skip_intro_length(),
        a.skip_intro_disable(),
        a.next_episode_to_air(),
        a.next_episode_airing_at(),
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Set then get returns the value for every episode group, and no other episode bit moves.
    #[test]
    fn every_episode_group_round_trips_and_leaves_the_other_bits_alone(word: i64, group in 0usize..8, value in 0i64..=0xFFFF) {
        let (mask, read) = episode_groups()[group];
        let value = value & (mask >> mask.trailing_zeros());
        let written = set_group(word, mask, value);
        let a = anime_with(written, 0, 0);
        prop_assert_eq!(read(&a), value << mask.trailing_zeros());
        prop_assert_eq!(written & !mask, word & !mask);
    }

    /// Set then get returns the value for every season group, and no other season bit moves.
    #[test]
    fn every_season_group_round_trips_and_leaves_the_other_bits_alone(word: i64, group in 0usize..8, value in 0i64..=0xFFFF) {
        let (mask, read) = season_groups()[group];
        let value = value & (mask >> mask.trailing_zeros());
        let written = set_group(word, mask, value);
        let a = anime_with(0, written, 0);
        prop_assert_eq!(read(&a), value << mask.trailing_zeros());
        prop_assert_eq!(written & !mask, word & !mask);
    }

    /// The skip-intro length keeps its 8 bits and the airing data stays put. A zero length also sets the disable bit
    /// (upstream rule), so that bit is excluded when the length is zero.
    #[test]
    fn skip_intro_length_round_trips_and_keeps_the_other_viewer_bits(word: i64, length in 0i32..=255) {
        let mut a = anime_with(0, 0, word);
        a.set_skip_intro_length(length);
        prop_assert_eq!(a.skip_intro_length(), length);
        let airing = viewer_flags::AIRING_EPISODE_MASK | viewer_flags::AIRING_TIME_MASK;
        prop_assert_eq!(a.viewer_flags & airing, word & airing);
        if length != 0 {
            let intro = viewer_flags::INTRO_MASK | viewer_flags::INTRO_DISABLE_MASK;
            prop_assert_eq!(a.viewer_flags & !intro, word & !intro);
        }
    }

    /// The next-episode airing data round trips inside its masks, and the intro bits stay put.
    #[test]
    fn next_airing_round_trips_and_keeps_the_intro_bits(word: i64, episode in 0i32..=0xFFFF, airing in 0i64..=0xFFFF_FFFF) {
        let mut a = anime_with(0, 0, word);
        a.set_next_episode_airing(episode, airing);
        prop_assert_eq!(a.next_episode_to_air(), episode);
        prop_assert_eq!(a.next_episode_airing_at(), airing);
        let intro = viewer_flags::INTRO_MASK | viewer_flags::INTRO_DISABLE_MASK;
        prop_assert_eq!(a.viewer_flags & intro, word & intro);
    }

    /// Any 64-bit word in any of the three words is safe to read through every accessor.
    #[test]
    fn random_words_never_panic_any_accessor(episode: i64, season: i64, viewer: i64) {
        read_all(&anime_with(episode, season, viewer));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// 10 000 random names through each recogniser: the result is `-1.0` (not found) or a finite non-negative number.
    #[test]
    fn recognition_is_minus_one_or_a_finite_non_negative_number(title in "\\PC{0,60}", name in "\\PC{0,60}") {
        for value in [
            parse_chapter_number(&title, &name, None),
            parse_episode_number(&title, &name, None),
            parse_season_number(&title, &name, None),
        ] {
            prop_assert!(value == -1.0 || (value.is_finite() && value >= 0.0), "got {value}");
        }
    }
}
