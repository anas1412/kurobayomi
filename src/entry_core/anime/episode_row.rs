//! Episode rows (T-39a-010, 014, 019): the title and the shared list row of one episode. The row is the same
//! `list_model::RowItem` a chapter has, so the list draws both kinds alike. Upstream: `AnimeEpisodeListItem.kt` and
//! `AnimeScreen.kt:1082-1099`.

use kuro_data::episode::Episode;
use kuro_i18n::{Arg, MsgKey, trf};

use super::format::{format_episode_number, watch_progress};
use crate::entry_core::list_model::{RowItem, subtitle};

/// The title: `Episode {n}` in episode-number mode, otherwise the episode name.
pub fn title(ep: &Episode, display_number: bool) -> String {
    if display_number {
        trf(MsgKey::display_mode_episode, &[("arg1", Arg::Str(&format_episode_number(ep.episode_number)))])
    } else {
        ep.name.clone()
    }
}

/// The row of one episode. The second line is the date, the time left of a partly watched episode and the scanlator;
/// the summary shows only when summaries are on and it is not blank. `last_second_seen` and `total_seconds` hold
/// milliseconds, as the player stores them (behaviour 3.4). The download state and the button are the page's to set.
pub fn row(ep: &Episode, display_number: bool, show_summaries: bool, selected: bool) -> RowItem {
    RowItem {
        subtitle: subtitle(ep.date_upload, watch_progress(ep.seen, ep.last_second_seen, ep.total_seconds), ep.scanlator.as_deref()),
        summary: ep.summary.clone().filter(|t| show_summaries && !t.trim().is_empty()),
        read: ep.seen,
        bookmark: ep.bookmark,
        filler: ep.fillermark,
        selected,
        ..RowItem::new(ep.id, title(ep, display_number))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_follows_the_display_mode() {
        let mut ep = Episode::create();
        ep.name = "The Start".into();
        ep.episode_number = 12.5;
        assert_eq!(title(&ep, false), "The Start");
        assert_eq!(title(&ep, true), "Episode 12.5");
    }

    #[test]
    fn a_row_resolves_title_second_line_marks_and_summary() {
        let mut ep = Episode::create();
        ep.id = 4;
        ep.name = "Pilot".into();
        ep.scanlator = Some("grp".into());
        ep.last_second_seen = 690_000;
        ep.total_seconds = 1_440_000;
        ep.summary = Some("plot".into());
        ep.bookmark = true;
        ep.fillermark = true;
        let shown = row(&ep, false, true, false);
        assert_eq!(shown.title, "Pilot");
        assert_eq!(shown.subtitle, "12:30 left \u{2022} grp");
        assert_eq!(shown.summary.as_deref(), Some("plot"));
        assert!(shown.bookmark && shown.filler && !shown.read);
        let hidden = row(&ep, true, false, true);
        assert_eq!(hidden.title, "Episode -1");
        assert_eq!(hidden.summary, None, "summaries are off");
        assert!(hidden.selected);
    }

    #[test]
    fn a_seen_episode_is_read_with_no_progress_and_an_unknown_scanlator_is_hidden() {
        let mut ep = Episode::create();
        ep.seen = true;
        ep.last_second_seen = 5_000;
        ep.total_seconds = 9_000;
        ep.scanlator = Some("Unknown".into());
        ep.summary = Some("  ".into());
        let shown = row(&ep, false, true, false);
        assert!(shown.read);
        assert_eq!(shown.subtitle, "");
        assert_eq!(shown.summary, None, "a blank summary is no summary");
    }
}
