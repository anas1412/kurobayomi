//! Episode row rules (T-39a-010, 014, 019): the layout choice, the title, the information line, the indicators and the
//! alpha of one row. The widget reads these values and draws them. Upstream: `AnimeEpisodeListItem.kt` and
//! `AnimeScreen.kt:1082-1099`.

use kuro_data::episode::Episode;
use kuro_i18n::{Arg, MsgKey, trf};

use super::format::format_episode_number;

/// Row padding in dp: start 16, top 12, end 8, bottom 12 (behaviour 3.2).
pub const PADDING: [i32; 4] = [16, 12, 8, 12];
/// Alpha of a seen row's title and of its information line.
pub const SEEN_ALPHA: f64 = 0.38;
/// Alpha of the information line of an unseen row.
pub const SECONDARY_ALPHA: f64 = 0.78;
/// Gap under the title in the simple layout, 0 when the episode is marked filler.
pub const TITLE_GAP: i32 = 6;

/// The layout of one row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowLayout {
    /// No preview and no summary: one title line and the information line below it.
    Simple,
    /// A preview or a summary exists. The title takes two lines when a preview exists.
    Rich { title_lines: u8 },
}

/// `previewUrl.isNullOrBlank() && summary.isNullOrBlank()` gives the simple layout; anything else is rich. The
/// values that decide are the filtered ones (behaviour 3.2 and 3.7): a preview hidden by the thumbnails setting, or a
/// summary hidden by the summaries setting, counts as absent, like a blank one.
pub fn layout(ep: &Episode, show_previews: bool, show_summaries: bool) -> RowLayout {
    let present = |value: &Option<String>, shown: bool| shown && value.as_deref().is_some_and(|t| !t.trim().is_empty());
    let preview = present(&ep.preview_url, show_previews);
    if preview || present(&ep.summary, show_summaries) {
        RowLayout::Rich {
            title_lines: if preview { 2 } else { 1 },
        }
    } else {
        RowLayout::Simple
    }
}

/// The title: `Episode {n}` in episode-number mode, otherwise the episode name.
pub fn title(ep: &Episode, display_number: bool) -> String {
    if display_number {
        trf(
            MsgKey::display_mode_episode,
            &[("arg1", Arg::Str(&format_episode_number(ep.episode_number)))],
        )
    } else {
        ep.name.clone()
    }
}

/// Title alpha: 0.38 for a seen row, full for an unseen one.
pub fn title_alpha(seen: bool) -> f64 {
    if seen { SEEN_ALPHA } else { 1.0 }
}

/// The gap under the title in the simple layout (T-39a-014).
pub fn title_gap(filler: bool) -> i32 {
    if filler { 0 } else { TITLE_GAP }
}

/// Information-line alpha: 0.38 when seen, 0.78 otherwise.
pub fn info_alpha(seen: bool) -> f64 {
    if seen { SEEN_ALPHA } else { SECONDARY_ALPHA }
}

/// The indicators of a row: the bookmark icon shows only when bookmarked, the filler label only when fillermarked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Indicators {
    pub bookmark: bool,
    pub filler: bool,
}

impl Indicators {
    pub fn of(ep: &Episode) -> Self {
        Self {
            bookmark: ep.bookmark,
            filler: ep.fillermark,
        }
    }
}

/// The information line after the optional filler label: date, watch progress and scanlator, joined by ` • `.
/// Empty parts are dropped, so the dot separator appears only between parts that exist.
pub fn info_text(date: Option<String>, progress: Option<String>, scanlator: Option<&str>) -> String {
    [date, progress, scanlator.map(str::to_owned)]
        .into_iter()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" • ")
}

/// Everything the row widget draws, resolved from the episode and the entry's display settings. Two equal values
/// draw the same row, so the list can tell which rows changed.
#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeRow {
    pub id: i64,
    pub title: String,
    /// Date, watch progress and scanlator, without the filler label.
    pub info: String,
    pub seen: bool,
    pub layout: RowLayout,
    pub indicators: Indicators,
    /// Present only when thumbnails are shown and the url is not blank.
    pub preview_url: Option<String>,
    /// Present only when summaries are shown and the text is not blank.
    pub summary: Option<String>,
    /// The summary is expanded (all its lines) rather than three lines (T-39a-012). Kept per episode by the model.
    pub summary_expanded: bool,
    pub selected: bool,
}

/// One line of the episode list: an episode row, or the missing-count separator after or before episodes (T-39a-015).
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Episode(EpisodeRow),
    /// `key` is `missing-count-{lower|null}-{higher}`; `count` is the number of episodes missing there.
    Missing { key: String, count: i32 },
}

impl Line {
    /// The identity of a line for the diff: the episode id, or the separator key.
    pub fn slot(&self) -> String {
        match self {
            Self::Episode(row) => row.id.to_string(),
            Self::Missing { key, .. } => key.clone(),
        }
    }
}

/// The upload date as `YYYY-MM-DD`; an unknown date (zero or negative) has no text.
fn date_text(date_upload: i64) -> Option<String> {
    if date_upload <= 0 {
        return None;
    }
    let date = chrono::DateTime::from_timestamp_millis(date_upload)?;
    Some(date.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
}

/// `last_second_seen` and `total_seconds` hold milliseconds, as the player stores them (behaviour 3.4).
pub fn row(ep: &Episode, display_number: bool, show_previews: bool, show_summaries: bool, selected: bool) -> EpisodeRow {
    let shown = |value: &Option<String>, on: bool| value.clone().filter(|t| on && !t.trim().is_empty());
    EpisodeRow {
        id: ep.id,
        title: title(ep, display_number),
        info: info_text(
            date_text(ep.date_upload),
            super::format::watch_progress(ep.seen, ep.last_second_seen, ep.total_seconds),
            ep.scanlator.as_deref(),
        ),
        seen: ep.seen,
        layout: layout(ep, show_previews, show_summaries),
        indicators: Indicators::of(ep),
        preview_url: shown(&ep.preview_url, show_previews),
        summary: shown(&ep.summary, show_summaries),
        summary_expanded: false,
        selected,
    }
}

/// What the list model must do to go from one list of rows to the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diff {
    Same,
    /// Same rows in the same order: these indices changed (ascending). One row is one `items_changed(i, 1, 1)`; several
    /// are one splice over their span (`changed_span`).
    Rows(Vec<usize>),
    /// Rows were added, removed or reordered: one splice replaces the list.
    Reset,
}

/// The span `(start, len)` that covers every index of a `Diff::Rows`, so a bulk change (select all, mark many) is one
/// splice instead of one signal per row. `None` for no index.
pub fn changed_span(indices: &[usize]) -> Option<(usize, usize)> {
    let (first, last) = (indices.iter().min()?, indices.iter().max()?);
    Some((*first, last - first + 1))
}

/// The diff of list lines: a line keeps its place when it has the same slot (an episode id or a separator key).
pub fn diff_lines(old: &[Line], new: &[Line]) -> Diff {
    diff_by(old, new, Line::slot)
}

fn diff_by<T: PartialEq>(old: &[T], new: &[T], slot: impl Fn(&T) -> String) -> Diff {
    if old.len() != new.len() || old.iter().zip(new).any(|(a, b)| slot(a) != slot(b)) {
        return Diff::Reset;
    }
    let changed: Vec<usize> = (0..old.len()).filter(|&i| old[i] != new[i]).collect();
    if changed.is_empty() { Diff::Same } else { Diff::Rows(changed) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode() -> Episode {
        Episode::create()
    }

    #[test]
    fn the_span_covers_the_first_to_the_last_changed_row() {
        assert_eq!(changed_span(&[]), None);
        assert_eq!(changed_span(&[4]), Some((4, 1)));
        assert_eq!(changed_span(&[2, 3, 9]), Some((2, 8)));
    }

    #[test]
    fn plain_episode_uses_the_simple_layout() {
        assert_eq!(layout(&episode(), true, true), RowLayout::Simple);
    }

    #[test]
    fn blank_preview_and_summary_stay_simple() {
        let mut ep = episode();
        ep.preview_url = Some("   ".into());
        ep.summary = Some(String::new());
        assert_eq!(layout(&ep, true, true), RowLayout::Simple);
    }

    #[test]
    fn summary_alone_is_rich_with_one_title_line() {
        let mut ep = episode();
        ep.summary = Some("plot".into());
        assert_eq!(layout(&ep, true, true), RowLayout::Rich { title_lines: 1 });
    }

    #[test]
    fn preview_makes_the_title_two_lines() {
        let mut ep = episode();
        ep.preview_url = Some("https://example.invalid/t.jpg".into());
        assert_eq!(layout(&ep, true, true), RowLayout::Rich { title_lines: 2 });
    }

    #[test]
    fn a_hidden_preview_or_summary_counts_as_absent() {
        let mut ep = episode();
        ep.preview_url = Some("https://example.invalid/t.jpg".into());
        assert_eq!(layout(&ep, false, true), RowLayout::Simple, "thumbnails off, no summary");
        ep.summary = Some("plot".into());
        assert_eq!(layout(&ep, false, true), RowLayout::Rich { title_lines: 1 }, "the summary alone keeps it rich");
        assert_eq!(layout(&ep, false, false), RowLayout::Simple);
        assert_eq!(layout(&ep, true, false), RowLayout::Rich { title_lines: 2 });
    }

    #[test]
    fn title_follows_the_display_mode() {
        let mut ep = episode();
        ep.name = "The Start".into();
        ep.episode_number = 12.5;
        assert_eq!(title(&ep, false), "The Start");
        assert_eq!(title(&ep, true), "Episode 12.5");
    }

    #[test]
    fn seen_rows_are_dimmed_and_filler_drops_the_gap() {
        assert_eq!(title_alpha(true), SEEN_ALPHA);
        assert_eq!(title_alpha(false), 1.0);
        assert_eq!(info_alpha(true), SEEN_ALPHA);
        assert_eq!(info_alpha(false), SECONDARY_ALPHA);
        assert_eq!(title_gap(true), 0);
        assert_eq!(title_gap(false), TITLE_GAP);
    }

    #[test]
    fn indicators_read_the_stored_flags() {
        let mut ep = episode();
        ep.bookmark = true;
        assert_eq!(Indicators::of(&ep), Indicators { bookmark: true, filler: false });
    }

    #[test]
    fn info_line_joins_only_present_parts() {
        assert_eq!(info_text(None, None, None), "");
        assert_eq!(info_text(Some("Today".into()), None, Some("grp")), "Today • grp");
        assert_eq!(info_text(None, Some("1:00/2:00".into()), None), "1:00/2:00");
        assert_eq!(info_text(Some("Today".into()), Some("1:00/2:00".into()), Some(" ")), "Today • 1:00/2:00");
    }

    #[test]
    fn a_row_resolves_title_info_and_hidden_values() {
        let mut ep = episode();
        ep.id = 4;
        ep.name = "Pilot".into();
        ep.scanlator = Some("grp".into());
        ep.last_second_seen = 754_000;
        ep.total_seconds = 1_440_000;
        ep.summary = Some("plot".into());
        ep.preview_url = Some("https://example.invalid/t.jpg".into());
        let shown = row(&ep, false, true, true, false);
        assert_eq!(shown.title, "Pilot");
        assert_eq!(shown.info, "12:34/24:00 \u{2022} grp");
        assert_eq!(shown.layout, RowLayout::Rich { title_lines: 2 });
        assert_eq!(shown.summary.as_deref(), Some("plot"));
        let hidden = row(&ep, true, false, false, true);
        assert_eq!(hidden.title, "Episode -1");
        assert_eq!((hidden.preview_url, hidden.summary), (None, None));
        assert_eq!(hidden.layout, RowLayout::Simple);
        assert!(hidden.selected);
    }

}
