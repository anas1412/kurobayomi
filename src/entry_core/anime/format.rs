//! Time and number text of the anime entry screen: `formatTime` (T-39a-022), the episode number (T-39a-048) and the
//! watch progress line (T-39a-013). Upstream: `AnimeScreen.kt` (`formatTime`, lines 1091-1099 and 1153-1182) and
//! `presentation/util/ItemNumberFormatter.kt`. Output is ASCII; the English words come from the message catalogue.

use kuro_i18n::{Arg, MsgKey, trf};

use crate::entry_core::numfmt::format_chapter_number;

/// Above this many milliseconds `formatTime` switches to the `H:MM:SS` form.
const HOUR_MS: i64 = 3_600_000;

/// `formatTime(ms)` in the non-day form: `M:SS` up to and including one hour, `H:MM:SS` above it. Minutes are not
/// wrapped in the `M:SS` form, so `3_600_000` gives `60:00` and `3_600_001` gives `1:00:00`. Negative input is
/// clamped to zero because the player never reports a negative position.
pub fn format_time_ms(ms: i64) -> String {
    let ms = ms.max(0);
    let secs = ms / 1000;
    let mins = secs / 60;
    if ms > HOUR_MS {
        format!("{}:{:02}:{:02}", mins / 60, mins % 60, secs % 60)
    } else {
        format!("{}:{:02}", mins, secs % 60)
    }
}

/// `formatEpisodeNumber`: the `#.###` pattern, so `1.0` gives `1`, `12.5` gives `12.5` and `3.14159` gives `3.142`.
pub fn format_episode_number(number: f64) -> String {
    format_chapter_number(number)
}

/// The watch progress line `"{position}/{total}"` (both in milliseconds). It is shown only for an unseen episode with
/// a saved position. A zero total renders as `0:00` with no special case (behaviour 3.4).
pub fn watch_progress(seen: bool, last_second_seen_ms: i64, total_ms: i64) -> Option<String> {
    if seen || last_second_seen_ms <= 0 {
        return None;
    }
    Some(trf(
        MsgKey::episode_progress,
        &[
            ("arg1", Arg::Str(&format_time_ms(last_second_seen_ms))),
            ("arg2", Arg::Str(&format_time_ms(total_ms))),
        ],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_time_boundaries_from_the_spec() {
        assert_eq!(format_time_ms(0), "0:00");
        assert_eq!(format_time_ms(59_999), "0:59");
        assert_eq!(format_time_ms(3_600_000), "60:00");
        assert_eq!(format_time_ms(3_600_001), "1:00:00");
        assert_eq!(format_time_ms(3_725_000), "1:02:05");
    }

    #[test]
    fn format_time_is_ascii_and_clamps_negative_input() {
        assert!(format_time_ms(5_000_000).is_ascii());
        assert_eq!(format_time_ms(-42), "0:00");
    }

    #[test]
    fn episode_number_vectors() {
        assert_eq!(format_episode_number(1.0), "1");
        assert_eq!(format_episode_number(12.5), "12.5");
        assert_eq!(format_episode_number(3.14159), "3.142");
        assert_eq!(format_episode_number(100.0), "100");
        assert_eq!(format_episode_number(-1.5), "-1.5");
    }

    #[test]
    fn watch_progress_shown_only_for_unseen_with_a_position() {
        assert_eq!(watch_progress(true, 754_000, 1_440_000), None);
        assert_eq!(watch_progress(false, 0, 1_440_000), None);
        assert_eq!(watch_progress(false, 754_000, 1_440_000).as_deref(), Some("12:34/24:00"));
    }

    #[test]
    fn zero_total_renders_without_a_special_case() {
        assert_eq!(watch_progress(false, 65_000, 0).as_deref(), Some("1:05/0:00"));
    }
}
