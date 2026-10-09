//! Episode list dialog model (area 46 `EpisodeListDialog.kt`, T-46a-024). Pure: row titles, tints,
//! dimming, the scroll target and the subtitle line. The GTK dialog draws what these return.

use crate::enums::Dialogs;
use crate::episodes::begin_switch;
use crate::hosts::open_dialog;
use crate::state::PlayerState;
use crate::time::format_episode_number;

/// Title of the dialog.
pub const DIALOG_TITLE: &str = "Episodes";
/// The dialog is this fraction of the window in both directions.
pub const DIALOG_FRACTION: f32 = 0.8;

/// Size of the dialog for a window of `width` by `height`.
pub fn dialog_size(width: i32, height: i32) -> (i32, i32) {
    let scale = |v: i32| (v as f32 * DIALOG_FRACTION).round() as i32;
    (scale(width), scale(height))
}

/// The display bit of `Anime.displayMode` (`EPISODE_DISPLAY_MASK`). Other bits in the same value
/// (not-previews, not-summaries, season flags) do not change the title, so the test is by mask.
pub const EPISODE_DISPLAY_MASK: i64 = 0x0010_0000;
/// Opacity of dimmed items (watched rows, disabled icons).
pub const DISABLED_ALPHA: f32 = 0.38;

#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeRow {
    pub id: i64,
    pub number: f64,
    pub name: String,
    pub seen: bool,
    pub bookmarked: bool,
    pub fillermarked: bool,
    /// Upload date already formatted, empty when the episode has none.
    pub date: String,
    pub scanlator: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Normal,
    /// Primary colour: bookmarks.
    Primary,
    /// Tertiary colour: fillermarks.
    Tertiary,
}

/// Tint and opacity of one icon or text run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub tint: Tint,
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowLook {
    pub title: String,
    pub text: Paint,
    /// Bold and italic for the current episode.
    pub bold_italic: bool,
    pub bookmark: Paint,
    pub filler: Paint,
}

/// Indices of the playlist in display order: newest first, the reverse of the playlist.
pub fn display_order(len: usize) -> Vec<usize> {
    (0..len).rev().collect()
}

/// The first visible row in display order, so that the current episode is at the top
/// (`(size - index) - 1`). `None` when the current episode is not in the list, which shows the top.
pub fn scroll_index(len: usize, current_index: Option<usize>) -> usize {
    match current_index {
        Some(index) if index < len => len - index - 1,
        _ => 0,
    }
}

/// Current is matched by id, not by index, so a missing episode does not crash (E-46-017).
pub fn is_current(row: &EpisodeRow, current_id: i64) -> bool {
    row.id == current_id
}

/// Row title: `Episode N` when the display bit is set, else the episode name.
pub fn title(row: &EpisodeRow, display_mode: i64) -> String {
    if display_mode & EPISODE_DISPLAY_MASK == EPISODE_DISPLAY_MASK {
        format!("Episode {}", format_episode_number(row.number))
    } else {
        row.name.clone()
    }
}

/// Subtitle line: the date, then the scanlator after a dot only when both are present.
pub fn subtitle(date: &str, scanlator: Option<&str>) -> String {
    match (date.is_empty(), scanlator) {
        (false, Some(s)) => format!("{date} \u{00b7} {s}"),
        (false, None) => date.to_owned(),
        (true, Some(s)) => s.to_owned(),
        (true, None) => String::new(),
    }
}

/// The date label rule of `toRelativeString`. `days` is the days from the upload date to today
/// (positive for the past). The caller formats the text for each case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateLabel {
    /// Print the date with the short format.
    Formatted,
    /// Upcoming by this many days.
    Upcoming(i64),
    Today,
    /// This many days ago, under a week.
    DaysAgo(i64),
}

/// Picks the relative label for a date. With relative dates off, the caller always prints the date.
pub fn date_label(days: i64) -> DateLabel {
    if days < -7 {
        DateLabel::Formatted
    } else if days < 0 {
        DateLabel::Upcoming(-days)
    } else if days < 1 {
        DateLabel::Today
    } else if days < 7 {
        DateLabel::DaysAgo(days)
    } else {
        DateLabel::Formatted
    }
}

/// Everything the row draws for one episode.
pub fn look(row: &EpisodeRow, display_mode: i64, current_id: i64) -> RowLook {
    let text_tint = if row.bookmarked {
        Tint::Primary
    } else if row.fillermarked {
        Tint::Tertiary
    } else {
        Tint::Normal
    };
    let marked = |on: bool, tint: Tint| Paint {
        tint: if on { tint } else { Tint::Normal },
        alpha: if on { 1.0 } else { DISABLED_ALPHA },
    };
    RowLook {
        title: title(row, display_mode),
        text: Paint {
            tint: text_tint,
            alpha: if row.seen { DISABLED_ALPHA } else { 1.0 },
        },
        bold_italic: is_current(row, current_id),
        bookmark: marked(row.bookmarked, Tint::Primary),
        filler: marked(row.fillermarked, Tint::Tertiary),
    }
}

/// The value a mark shows after a write. A failed write reverts the row to the previous value.
pub fn mark_after_write(previous: bool, requested: bool, write_ok: bool) -> bool {
    if write_ok { requested } else { previous }
}

/// A click on a row: the dialog closes and the episode switch starts (`onEpisodeClicked`). Returns
/// the episode id the caller loads after pausing mpv. Clicking the current row switches again, as
/// in Aniyomi.
pub fn select(state: &mut PlayerState, episode_id: i64) -> i64 {
    open_dialog(state, Dialogs::None);
    begin_switch(state);
    episode_id
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Bookmark,
    Fillermark,
}

/// Where mark changes are saved. The app layer implements it over the episode repository.
pub trait EpisodeMarkStore {
    fn set_mark(&self, episode_id: i64, mark: Mark, value: bool) -> Result<(), String>;
}

/// Flips one mark of a row and saves it. A failed write leaves the row at its previous value and is
/// logged. Returns whether the write succeeded.
pub fn toggle_mark(store: &impl EpisodeMarkStore, row: &mut EpisodeRow, mark: Mark) -> bool {
    let slot = match mark {
        Mark::Bookmark => &mut row.bookmarked,
        Mark::Fillermark => &mut row.fillermarked,
    };
    let requested = !*slot;
    let written = store.set_mark(row.id, mark, requested);
    if let Err(error) = &written {
        tracing::error!(error, "saving the episode mark failed");
    }
    *slot = mark_after_write(*slot, requested, written.is_ok());
    written.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::Sheets;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Store {
        fail: bool,
        writes: RefCell<Vec<(i64, Mark, bool)>>,
    }

    impl EpisodeMarkStore for Store {
        fn set_mark(&self, id: i64, mark: Mark, value: bool) -> Result<(), String> {
            if self.fail {
                return Err("database is locked".to_owned());
            }
            self.writes.borrow_mut().push((id, mark, value));
            Ok(())
        }
    }

    #[test]
    fn clicking_a_row_closes_the_dialog_and_starts_the_switch() {
        let mut state = PlayerState {
            dialog: Dialogs::EpisodeList,
            sheet: Sheets::More,
            ..PlayerState::default()
        };
        assert_eq!(select(&mut state, 7), 7);
        assert_eq!(state.dialog, Dialogs::None);
        assert_eq!(state.sheet, Sheets::None);
        assert!(state.is_loading_episode);
    }

    #[test]
    fn marks_are_saved_and_a_failed_write_reverts() {
        let mut r = row(3, 1.0);
        let store = Store::default();
        assert!(toggle_mark(&store, &mut r, Mark::Bookmark));
        assert!(r.bookmarked);
        assert_eq!(*store.writes.borrow(), vec![(3, Mark::Bookmark, true)]);
        assert!(toggle_mark(&store, &mut r, Mark::Bookmark));
        assert!(!r.bookmarked);

        let bad = Store {
            fail: true,
            ..Store::default()
        };
        assert!(!toggle_mark(&bad, &mut r, Mark::Fillermark));
        assert!(!r.fillermarked);
        r.bookmarked = true;
        assert!(!toggle_mark(&bad, &mut r, Mark::Bookmark));
        assert!(r.bookmarked);
    }

    #[test]
    fn dialog_is_eighty_percent_of_the_window() {
        assert_eq!(DIALOG_TITLE, "Episodes");
        assert_eq!(dialog_size(1000, 500), (800, 400));
    }

    fn row(id: i64, number: f64) -> EpisodeRow {
        EpisodeRow {
            id,
            number,
            name: format!("Name {id}"),
            seen: false,
            bookmarked: false,
            fillermarked: false,
            date: String::new(),
            scanlator: None,
        }
    }

    #[test]
    fn display_is_reversed_and_scrolls_to_the_current_row() {
        assert_eq!(display_order(3), vec![2, 1, 0]);
        assert_eq!(scroll_index(10, Some(0)), 9);
        assert_eq!(scroll_index(10, Some(9)), 0);
        assert_eq!(scroll_index(10, None), 0);
    }

    #[test]
    fn title_uses_the_mask_bit() {
        let r = row(1, 12.5);
        assert_eq!(title(&r, EPISODE_DISPLAY_MASK), "Episode 12.5");
        assert_eq!(title(&r, EPISODE_DISPLAY_MASK | 0x800), "Episode 12.5");
        assert_eq!(title(&r, 0x800), "Name 1");
    }

    #[test]
    fn subtitle_separator_needs_both_parts() {
        assert_eq!(
            subtitle("2026-01-02", Some("Group")),
            "2026-01-02 \u{00b7} Group"
        );
        assert_eq!(subtitle("", Some("Group")), "Group");
        assert_eq!(subtitle("2026-01-02", None), "2026-01-02");
        assert_eq!(subtitle("", None), "");
    }

    #[test]
    fn date_label_thresholds() {
        assert_eq!(date_label(-8), DateLabel::Formatted);
        assert_eq!(date_label(-7), DateLabel::Upcoming(7));
        assert_eq!(date_label(-1), DateLabel::Upcoming(1));
        assert_eq!(date_label(0), DateLabel::Today);
        assert_eq!(date_label(6), DateLabel::DaysAgo(6));
        assert_eq!(date_label(7), DateLabel::Formatted);
    }

    #[test]
    fn bookmark_wins_over_filler_and_seen_dims() {
        let mut r = row(1, 1.0);
        r.bookmarked = true;
        r.fillermarked = true;
        r.seen = true;
        let l = look(&r, 0, 1);
        assert_eq!(l.text.tint, Tint::Primary);
        assert_eq!(l.text.alpha, DISABLED_ALPHA);
        assert!(l.bold_italic);
        assert_eq!(l.bookmark.alpha, 1.0);
        assert_eq!(l.filler.tint, Tint::Tertiary);
    }

    #[test]
    fn filler_tints_text_when_not_bookmarked() {
        let mut r = row(2, 1.0);
        r.fillermarked = true;
        let l = look(&r, 0, 1);
        assert_eq!(l.text.tint, Tint::Tertiary);
        assert!(!l.bold_italic);
    }

    #[test]
    fn missing_current_episode_is_not_an_error() {
        let rows = [row(1, 1.0), row(2, 2.0)];
        let current_index = rows.iter().position(|r| r.id == 99);
        assert_eq!(scroll_index(rows.len(), current_index), 0);
        assert!(rows.iter().all(|r| !is_current(r, 99)));
    }

    #[test]
    fn failed_write_reverts_the_mark() {
        assert!(mark_after_write(false, true, true));
        assert!(!mark_after_write(false, true, false));
    }
}
