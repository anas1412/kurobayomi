//! Episode settings window rules (T-39a-007, T-39a-004, T-39a-005): the Filter, Sort and Display rows, the state each row
//! shows, and the value a click sends. Upstream: `EpisodeSettingsDialog.kt` and behaviour section 15.
//! The rules take the episode flag word, so they run without a database. A click sends a masked write `(flag, mask)`
//! that applies as `(word & !mask) | flag`. Persistence is in `kuro_domain::entries::flag_setters`.

use kuro_domain::entries::anime::episode_flags as ep;
use kuro_domain::entries::flag_setters::sort_or_flip;
use kuro_domain::flags::{TriState, set_flag, tri};
use kuro_i18n::MsgKey;

use crate::entry_core::flags::{Write, filter_write};

/// Applies one masked write to an episode flag word.
pub fn apply(word: i64, write: Write) -> i64 {
    set_flag(word, write.0, write.1)
}

/// Which Filter row a click came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    Downloaded,
    Unseen,
    Bookmarked,
    Fillermarked,
}

/// One Filter row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterRow {
    pub kind: FilterKind,
    pub label: MsgKey,
    pub state: TriState,
    /// False when the row cannot be changed: the Downloaded row while the global downloaded-only mode is on.
    pub sensitive: bool,
}

/// T-39a-004: the stored downloaded group, or `EnabledIs` while the global downloaded-only mode is on. The stored bits
/// are not changed by the override.
pub fn downloaded_state(word: i64, downloaded_only: bool) -> TriState {
    if downloaded_only {
        TriState::ENABLED_IS
    } else {
        tri(word & ep::DOWNLOADED_MASK, ep::SHOW_DOWNLOADED, ep::SHOW_NOT_DOWNLOADED)
    }
}

/// The Filter rows in order: Downloaded, Unseen, Bookmarked, Fillermarked (behaviour 15.1).
pub fn filter_rows(word: i64, downloaded_only: bool) -> [FilterRow; 4] {
    [
        FilterRow {
            kind: FilterKind::Downloaded,
            label: MsgKey::label_downloaded,
            state: downloaded_state(word, downloaded_only),
            sensitive: !downloaded_only,
        },
        FilterRow {
            kind: FilterKind::Unseen,
            label: MsgKey::action_filter_unseen,
            state: tri(word & ep::UNSEEN_MASK, ep::SHOW_UNSEEN, ep::SHOW_SEEN),
            sensitive: true,
        },
        FilterRow {
            kind: FilterKind::Bookmarked,
            label: MsgKey::action_filter_bookmarked,
            state: tri(word & ep::BOOKMARKED_MASK, ep::SHOW_BOOKMARKED, ep::SHOW_NOT_BOOKMARKED),
            sensitive: true,
        },
        FilterRow {
            kind: FilterKind::Fillermarked,
            label: MsgKey::action_filter_fillermarked,
            state: tri(word & ep::FILLERMARKED_MASK, ep::SHOW_FILLERMARKED, ep::SHOW_NOT_FILLERMARKED),
            sensitive: true,
        },
    ]
}

/// The write for a Downloaded click. `None` while downloaded-only is on: that row sends nothing.
pub fn downloaded_write(state: TriState, downloaded_only: bool) -> Option<Write> {
    (!downloaded_only).then(|| {
        filter_write(state, ep::SHOW_DOWNLOADED, ep::SHOW_NOT_DOWNLOADED, ep::DOWNLOADED_MASK)
    })
}

/// The write for an Unseen click.
pub fn unseen_write(state: TriState) -> Write {
    filter_write(state, ep::SHOW_UNSEEN, ep::SHOW_SEEN, ep::UNSEEN_MASK)
}

/// The write for a Bookmarked click.
pub fn bookmarked_write(state: TriState) -> Write {
    filter_write(state, ep::SHOW_BOOKMARKED, ep::SHOW_NOT_BOOKMARKED, ep::BOOKMARKED_MASK)
}

/// The write for a Fillermarked click.
pub fn fillermarked_write(state: TriState) -> Write {
    filter_write(state, ep::SHOW_FILLERMARKED, ep::SHOW_NOT_FILLERMARKED, ep::FILLERMARKED_MASK)
}

/// The write a click on `row` sends: the state moves to the next one of the cycle. `None` for the Downloaded row while
/// the global downloaded-only mode is on.
pub fn filter_click(row: &FilterRow, downloaded_only: bool) -> Option<Write> {
    let next = row.state.next();
    match row.kind {
        FilterKind::Downloaded => downloaded_write(next, downloaded_only),
        FilterKind::Unseen => Some(unseen_write(next)),
        FilterKind::Bookmarked => Some(bookmarked_write(next)),
        FilterKind::Fillermarked => Some(fillermarked_write(next)),
    }
}

/// One Sort row. `descending` is set only on the active row (behaviour 15.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortRow {
    pub label: MsgKey,
    pub mode: i64,
    pub descending: Option<bool>,
}

/// The Sort rows in order: source, episode number, upload date, alphabetical.
pub fn sort_rows(word: i64) -> [SortRow; 4] {
    let active = word & ep::SORTING_MASK;
    let descending = word & ep::SORT_DIR_MASK == ep::SORT_DESC;
    let row = |label, mode| SortRow {
        label,
        mode,
        descending: (active == mode).then_some(descending),
    };
    [
        row(MsgKey::sort_by_source, ep::SORTING_SOURCE),
        row(MsgKey::sort_by_episode_number, ep::SORTING_NUMBER),
        row(MsgKey::sort_by_upload_date, ep::SORTING_UPLOAD_DATE),
        row(MsgKey::action_sort_alpha, ep::SORTING_ALPHABET),
    ]
}

/// A click on a Sort row. The key in use flips its direction; another key becomes the sort, ascending. The domain
/// setter does this, so the rule has one copy.
pub fn sort_click(word: i64, mode: i64) -> i64 {
    sort_or_flip(word, mode, ep::SORTING_MASK, ep::SORT_DIR_MASK, ep::SORT_ASC)
}

/// The Display rows: two radios (source title or episode number) and two checkboxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayRows {
    pub source_title: bool,
    pub episode_number: bool,
    pub previews: bool,
    pub summaries: bool,
}

pub fn display_rows(word: i64) -> DisplayRows {
    DisplayRows {
        source_title: word & ep::DISPLAY_MASK == ep::DISPLAY_NAME,
        episode_number: word & ep::DISPLAY_MASK == ep::DISPLAY_NUMBER,
        previews: word & ep::PREVIEWS_MASK == ep::SHOW_PREVIEWS,
        summaries: word & ep::SUMMARIES_MASK == ep::SHOW_SUMMARIES,
    }
}

/// A radio click: `true` for the episode-number title, `false` for the source title.
pub fn display_mode_write(episode_number: bool) -> Write {
    let value = if episode_number { ep::DISPLAY_NUMBER } else { ep::DISPLAY_NAME };
    (value, ep::DISPLAY_MASK)
}

/// A thumbnails click. `currently_shown` is the state before the click: shown sends the hide value.
pub fn previews_write(currently_shown: bool) -> Write {
    let value = if currently_shown { ep::SHOW_NOT_PREVIEWS } else { ep::SHOW_PREVIEWS };
    (value, ep::PREVIEWS_MASK)
}

/// A summaries click. `currently_shown` is the state before the click.
pub fn summaries_write(currently_shown: bool) -> Write {
    let value = if currently_shown { ep::SHOW_NOT_SUMMARIES } else { ep::SHOW_SUMMARIES };
    (value, ep::SUMMARIES_MASK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downloaded_only_forces_enabled_is_without_touching_the_bits() {
        let stored_not = ep::SHOW_NOT_DOWNLOADED;
        assert_eq!(downloaded_state(stored_not, true), TriState::ENABLED_IS);
        assert_eq!(downloaded_state(stored_not, false), TriState::ENABLED_NOT);
        assert_eq!(downloaded_state(0, false), TriState::DISABLED);
        assert!(downloaded_write(TriState::DISABLED, true).is_none());
        assert_eq!(
            downloaded_write(TriState::ENABLED_IS, false),
            Some((ep::SHOW_DOWNLOADED, ep::DOWNLOADED_MASK))
        );
    }

    #[test]
    fn filter_rows_in_order_with_downloaded_insensitive_under_the_override() {
        let word = ep::SHOW_UNSEEN | ep::SHOW_NOT_BOOKMARKED | ep::SHOW_FILLERMARKED;
        let rows = filter_rows(word, true);
        assert_eq!(rows[0].label, MsgKey::label_downloaded);
        assert_eq!(rows[0].state, TriState::ENABLED_IS);
        assert!(!rows[0].sensitive);
        assert_eq!(rows[1].state, TriState::ENABLED_IS);
        assert_eq!(rows[2].state, TriState::ENABLED_NOT);
        assert_eq!(rows[3].state, TriState::ENABLED_IS);
        assert!(rows[1..].iter().all(|r| r.sensitive));
    }

    #[test]
    fn a_filter_click_sends_the_next_state_of_the_cycle() {
        let rows = filter_rows(0, false);
        assert_eq!(filter_click(&rows[1], false), Some((ep::SHOW_UNSEEN, ep::UNSEEN_MASK)));
        assert_eq!(filter_click(&rows[0], false), Some((ep::SHOW_DOWNLOADED, ep::DOWNLOADED_MASK)));
        let on = filter_rows(ep::SHOW_NOT_FILLERMARKED, false);
        assert_eq!(filter_click(&on[3], false), Some((0, ep::FILLERMARKED_MASK)), "Not cycles back to off");
        let word = apply(0, filter_click(&rows[2], false).unwrap());
        assert_eq!(filter_rows(word, false)[2].state, TriState::ENABLED_IS);
        assert_eq!(filter_click(&filter_rows(0, true)[0], true), None, "downloaded-only locks the row");
    }

    #[test]
    fn a_filter_write_changes_only_its_own_group() {
        // Bit 27 (display) and bit 56 are not covered by the filter groups and must survive.
        let other = ep::DISPLAY_NUMBER | (1i64 << 56) | ep::SORTING_NUMBER;
        let word = apply(other, unseen_write(TriState::ENABLED_NOT));
        assert_eq!(word & ep::UNSEEN_MASK, ep::SHOW_SEEN);
        assert_eq!(word & !ep::UNSEEN_MASK, other & !ep::UNSEEN_MASK);
        assert_eq!(apply(word, unseen_write(TriState::DISABLED)) & ep::UNSEEN_MASK, 0);
    }

    #[test]
    fn sort_rows_show_direction_only_on_the_active_row() {
        let rows = sort_rows(ep::SORTING_NUMBER | ep::SORT_ASC);
        assert_eq!(rows[1].descending, Some(false));
        assert!(rows[0].descending.is_none() && rows[2].descending.is_none() && rows[3].descending.is_none());
        let default = sort_rows(0);
        assert_eq!(default[0].descending, Some(true));
    }

    #[test]
    fn sort_click_flips_the_active_key_and_starts_a_new_key_ascending() {
        assert_eq!(sort_click(0, ep::SORTING_SOURCE), ep::SORT_ASC);
        assert_eq!(sort_click(ep::SORT_ASC, ep::SORTING_SOURCE), 0);
        assert_eq!(sort_click(0, ep::SORTING_NUMBER), ep::SORTING_NUMBER | ep::SORT_ASC);
        assert_eq!(sort_click(ep::SORTING_ALPHABET, ep::SORTING_NUMBER), ep::SORTING_NUMBER | ep::SORT_ASC);
    }

    #[test]
    fn display_radios_and_checkboxes_send_the_spec_values() {
        assert_eq!(display_mode_write(true), (ep::DISPLAY_NUMBER, ep::DISPLAY_MASK));
        assert_eq!(display_mode_write(false), (ep::DISPLAY_NAME, ep::DISPLAY_MASK));
        assert_eq!(previews_write(true), (ep::SHOW_NOT_PREVIEWS, ep::PREVIEWS_MASK));
        assert_eq!(previews_write(false), (ep::SHOW_PREVIEWS, ep::PREVIEWS_MASK));
        assert_eq!(summaries_write(true), (ep::SHOW_NOT_SUMMARIES, ep::SUMMARIES_MASK));
        let rows = display_rows(apply(0, previews_write(false)));
        assert!(rows.previews && rows.source_title && !rows.episode_number);
        let rows = display_rows(apply(0, display_mode_write(true)));
        assert!(rows.episode_number && !rows.source_title);
    }
}
