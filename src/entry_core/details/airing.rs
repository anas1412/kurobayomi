//! The next-episode airing row (T-39a-020, T-39a-021): when it shows and its countdown text. The remaining time is
//! always derived from the absolute airing time, so a refreshed airing time shows on the next tick (behaviour 3.8).
//! The text "Airing in ..." is hard-coded English upstream and here (behaviour 3.9).

use kuro_data::entry::Entry;
use kuro_domain::entries::flags::EntryFlags;
use kuro_i18n::{Arg, MsgKey, trf};

use super::format::format_episode_number;

/// `SAnime.COMPLETED`: a completed entry shows no countdown.
const COMPLETED: i64 = 2;

/// The next airing episode of an entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Airing {
    pub episode: i32,
    /// The absolute airing time in epoch milliseconds.
    pub at_ms: i64,
}

impl Airing {
    /// `None` when the entry has no airing time (`airingTime > 0`).
    pub fn of(entry: &Entry) -> Option<Self> {
        let at = entry.viewer().next_episode_airing_at();
        (at > 0).then(|| Self {
            episode: entry.viewer().next_episode_to_air(),
            at_ms: at.saturating_mul(1000),
        })
    }

    /// Milliseconds left at `now_ms`. Not clamped: it goes negative once the episode aired.
    pub fn remaining(self, now_ms: i64) -> i64 {
        self.at_ms - now_ms
    }

    /// The row title: "Episode {n}".
    pub fn title(self) -> String {
        trf(MsgKey::display_mode_episode, &[("arg1", Arg::Str(&format_episode_number(f64::from(self.episode))))])
    }
}

/// The row shows only for a positive remaining time, with the preference on, for an entry that is not completed.
pub fn visible(remaining_ms: i64, show_pref: bool, status: i64) -> bool {
    remaining_ms > 0 && show_pref && status != COMPLETED
}

/// `formatTime(ms, useDayFormat = true)`: `Airing in 01d 01h 01m 01s`. Zero and negative input show zeros.
pub fn format_airing(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    let two = |n: i64| format!("{n:02}");
    let (days, hours, minutes, seconds) = (two(secs / 86_400), two(secs % 86_400 / 3600), two(secs % 3600 / 60), two(secs % 60));
    trf(MsgKey::airing_in_countdown, &[("days", Arg::Str(&days)), ("hours", Arg::Str(&hours)), ("minutes", Arg::Str(&minutes)), ("seconds", Arg::Str(&seconds))])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_text_vectors() {
        assert_eq!(format_airing(90_061_000), "Airing in 01d 01h 01m 01s");
        assert_eq!(format_airing(0), "Airing in 00d 00h 00m 00s");
        assert_eq!(format_airing(-5), "Airing in 00d 00h 00m 00s");
        assert_eq!(format_airing(59_999), "Airing in 00d 00h 00m 59s");
    }

    #[test]
    fn the_row_needs_time_left_the_preference_and_an_unfinished_entry() {
        assert!(visible(1, true, 1));
        assert!(!visible(0, true, 1));
        assert!(!visible(-1000, true, 1));
        assert!(!visible(5000, false, 1));
        assert!(!visible(5000, true, 2));
    }

    #[test]
    fn airing_reads_the_viewer_flags_and_the_remaining_time_follows_the_absolute_target() {
        let mut entry = Entry::create(kuro_platform::kind::Kind::Anime);
        assert_eq!(Airing::of(&entry), None);
        entry.set_next_episode_airing(7, 1_700_000_000);
        let airing = Airing::of(&entry).unwrap();
        assert_eq!(airing, Airing { episode: 7, at_ms: 1_700_000_000_000 });
        assert_eq!(airing.title(), "Episode 7");
        assert_eq!(airing.remaining(1_699_999_999_000), 1000);
        // a refreshed airing time moves the target: the next tick shows the new remaining time
        entry.set_next_episode_airing(8, 1_700_000_100);
        assert_eq!(Airing::of(&entry).unwrap().remaining(1_699_999_999_000), 101_000);
        assert_eq!(airing.remaining(1_700_000_001_000), -1000);
    }
}
