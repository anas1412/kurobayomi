//! Season settings window rules (T-39a-008): the Filter, Sort and Display rows of a `Seasons` entry, the state each row
//! shows, and the masked write a click sends. Upstream: `SeasonSettingsDialog.kt` and behaviour section 16. The rules take
//! the season flag word, so they run without a database. A write is `(flag, mask)` applied as `(word & !mask) | flag`.
//! The stored bits are the only source: an existing entry never falls back to dialog defaults (T-39a-006, D-6).

use kuro_domain::entries::anime::season_flags as sf;
use kuro_domain::entries::flag_setters::sort_or_flip;
use kuro_domain::entries::season_display::SeasonDisplayMode;
use kuro_domain::flags::{TriState, tri};
use kuro_i18n::MsgKey;

use crate::entry_core::flags::Write;

/// The Filter rows, in order (p1-ui-parity 9.2): Downloaded, Unseen, Started, Completed, Bookmarked, Fillermarked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterKind {
    Downloaded,
    Unseen,
    Started,
    Completed,
    Bookmarked,
    Fillermarked,
}

pub const FILTER_ORDER: [FilterKind; 6] = [
    FilterKind::Downloaded,
    FilterKind::Unseen,
    FilterKind::Started,
    FilterKind::Completed,
    FilterKind::Bookmarked,
    FilterKind::Fillermarked,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterRow {
    pub kind: FilterKind,
    pub label: MsgKey,
    pub state: TriState,
    /// False for the Downloaded row while the global downloaded-only mode is on (T-39a-004 applies to seasons too).
    pub sensitive: bool,
}

/// The two stored bits of a filter group: the value that means "include" and the value that means "exclude".
fn group(kind: FilterKind) -> (i64, i64, i64) {
    match kind {
        FilterKind::Downloaded => (sf::DOWNLOADED_MASK, sf::SHOW_DOWNLOADED, sf::SHOW_NOT_DOWNLOADED),
        FilterKind::Unseen => (sf::UNSEEN_MASK, sf::SHOW_UNSEEN, sf::SHOW_SEEN),
        FilterKind::Started => (sf::STARTED_MASK, sf::SHOW_STARTED, sf::SHOW_NOT_STARTED),
        FilterKind::Completed => (sf::COMPLETED_MASK, sf::SHOW_COMPLETED, sf::SHOW_NOT_COMPLETED),
        FilterKind::Bookmarked => (sf::BOOKMARKED_MASK, sf::SHOW_BOOKMARKED, sf::SHOW_NOT_BOOKMARKED),
        FilterKind::Fillermarked => (sf::FILLERMARKED_MASK, sf::SHOW_FILLERMARKED, sf::SHOW_NOT_FILLERMARKED),
    }
}

fn label(kind: FilterKind) -> MsgKey {
    match kind {
        FilterKind::Downloaded => MsgKey::label_downloaded,
        FilterKind::Unseen => MsgKey::action_filter_unseen,
        FilterKind::Started => MsgKey::action_filter_started,
        FilterKind::Completed => MsgKey::completed,
        FilterKind::Bookmarked => MsgKey::action_filter_bookmarked,
        FilterKind::Fillermarked => MsgKey::action_filter_fillermarked,
    }
}

/// The state a filter group shows. The Downloaded group reads `EnabledIs` while the downloaded-only mode is on.
pub fn filter_state(word: i64, kind: FilterKind, downloaded_only: bool) -> TriState {
    if kind == FilterKind::Downloaded && downloaded_only {
        return TriState::ENABLED_IS;
    }
    let (mask, is, not) = group(kind);
    tri(word & mask, is, not)
}

/// The six Filter rows with their state.
pub fn filter_rows(word: i64, downloaded_only: bool) -> [FilterRow; 6] {
    FILTER_ORDER.map(|kind| FilterRow {
        kind,
        label: label(kind),
        state: filter_state(word, kind, downloaded_only),
        sensitive: !(kind == FilterKind::Downloaded && downloaded_only),
    })
}

/// The masked write of a click on a Filter row, cycling Disabled, Enabled-is, Enabled-not (the same cycle as the episode
/// window). `None` for a row that cannot change.
pub fn filter_click(kind: FilterKind, current: TriState, downloaded_only: bool) -> Option<Write> {
    if kind == FilterKind::Downloaded && downloaded_only {
        return None;
    }
    let (mask, is, not) = group(kind);
    let value = if current == TriState::DISABLED {
        is
    } else if current == TriState::ENABLED_IS {
        not
    } else {
        0
    };
    Some((value, mask))
}

/// The Sort rows, in order (behaviour 16.2): the stored sort code of each, with its label.
pub const SORT_ROWS: [(MsgKey, i64); 7] = [
    (MsgKey::sort_by_source, sf::SORT_SOURCE),
    (MsgKey::sort_by_season_number, sf::SORT_SEASON),
    (MsgKey::sort_by_upload_date, sf::SORT_UPLOAD),
    (MsgKey::action_sort_alpha, sf::SORT_ALPHABET),
    (MsgKey::action_sort_unseen_count, sf::SORT_COUNT),
    (MsgKey::action_sort_last_seen, sf::SORT_LAST_SEEN),
    (MsgKey::action_sort_episode_fetch_date, sf::SORT_FETCHED),
];

/// One Sort row. The direction arrow shows only on the active row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortRow {
    pub label: MsgKey,
    pub code: i64,
    pub descending: Option<bool>,
}

pub fn sort_rows(word: i64) -> [SortRow; 7] {
    let active = word & sf::SORT_MASK;
    let descending = word & sf::SORT_DIR_MASK == sf::SORT_DESC;
    SORT_ROWS.map(|(label, code)| SortRow { label, code, descending: (active == code).then_some(descending) })
}

/// A click on a Sort row: the active row flips its direction, another row becomes the sort, ascending.
pub fn sort_click(word: i64, code: i64) -> i64 {
    sort_or_flip(word, code, sf::SORT_MASK, sf::SORT_DIR_MASK, sf::SORT_ASC)
}

/// The Display tab: four grid-mode chips, one size slider, five overlay checkboxes and two grid-title radios.
pub const GRID_MODES: [(MsgKey, SeasonDisplayMode); 4] = [
    (MsgKey::action_display_grid, SeasonDisplayMode::CompactGrid),
    (MsgKey::action_display_comfortable_grid, SeasonDisplayMode::ComfortableGrid),
    (MsgKey::action_display_cover_only_grid, SeasonDisplayMode::CoverOnlyGrid),
    (MsgKey::action_display_list, SeasonDisplayMode::List),
];

/// The overlay checkboxes, in order. Each reads its own stored bit (set means on, T-F5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Downloaded,
    Unseen,
    Local,
    Language,
    Continue,
}

pub const OVERLAY_ORDER: [Overlay; 5] = [Overlay::Downloaded, Overlay::Unseen, Overlay::Local, Overlay::Language, Overlay::Continue];

impl Overlay {
    pub fn mask(self) -> i64 {
        match self {
            Self::Downloaded => sf::OVERLAY_DOWNLOADED_MASK,
            Self::Unseen => sf::OVERLAY_UNSEEN_MASK,
            Self::Local => sf::OVERLAY_LOCAL_MASK,
            Self::Language => sf::OVERLAY_LANG_MASK,
            Self::Continue => sf::OVERLAY_CONT_MASK,
        }
    }

    pub fn label(self) -> MsgKey {
        match self {
            Self::Downloaded => MsgKey::action_display_download_badge_anime,
            Self::Unseen => MsgKey::action_display_unseen_badge,
            Self::Local => MsgKey::action_display_local_badge_anime,
            Self::Language => MsgKey::action_display_language_badge,
            Self::Continue => MsgKey::action_display_show_continue_watching_button,
        }
    }
}

/// Whether an overlay is on in the stored word.
pub fn overlay_on(word: i64, overlay: Overlay) -> bool {
    word & overlay.mask() != 0
}

/// The write of an overlay checkbox: `on` sets its bit, otherwise clears it.
pub fn overlay_write(overlay: Overlay, on: bool) -> Write {
    (if on { overlay.mask() } else { 0 }, overlay.mask())
}

/// The grid-title radio: the source title (`false`) or "Season {n}" (`true`).
pub fn title_write(number: bool) -> Write {
    (if number { sf::DISPLAY_MODE_NUMBER } else { sf::DISPLAY_MODE_SOURCE }, sf::DISPLAY_MODE_MASK)
}

/// The grid-mode chip write.
pub fn grid_mode_write(mode: SeasonDisplayMode) -> Write {
    (mode.to_i64() << sf::GRID_DISPLAY_MODE_BIT_OFFSET, sf::GRID_DISPLAY_MODE_MASK)
}

/// The size slider write (0 is automatic, 1..=10 are the column counts).
pub fn grid_size_write(size: i64) -> Write {
    let size = size.clamp(0, 10);
    (size << sf::GRID_DISPLAY_SIZE_BIT_OFFSET, sf::GRID_DISPLAY_SIZE_MASK)
}

/// The stored size of the slider, 0 to 10.
pub fn grid_size(word: i64) -> i64 {
    (word & sf::GRID_DISPLAY_SIZE_MASK) >> sf::GRID_DISPLAY_SIZE_BIT_OFFSET
}

/// The grid mode the stored word shows (a value without a mode shows the compact grid, fix Q11).
pub fn grid_mode(word: i64) -> SeasonDisplayMode {
    SeasonDisplayMode::from_stored((word & sf::GRID_DISPLAY_MODE_MASK) >> sf::GRID_DISPLAY_MODE_BIT_OFFSET)
}

/// Whether the grid title radio is on "Season {n}" (the number) rather than the source title.
pub fn title_is_number(word: i64) -> bool {
    word & sf::DISPLAY_MODE_MASK == sf::DISPLAY_MODE_NUMBER
}

/// The slider label: "List size" in list mode, "Items per row" otherwise.
pub fn size_label(mode: SeasonDisplayMode) -> MsgKey {
    if mode == SeasonDisplayMode::List { MsgKey::pref_library_rows } else { MsgKey::pref_library_columns }
}

/// The slider value text: "Auto" at 0, else the number.
pub fn size_value(size: i64) -> String {
    if size == 0 { "Auto".to_owned() } else { size.to_string() }
}

#[cfg(test)]
use kuro_domain::flags::set_flag;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_rows_are_six_in_order_with_their_bits() {
        let rows = filter_rows(0, false);
        let kinds: Vec<FilterKind> = rows.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, FILTER_ORDER.to_vec());
        assert_eq!(rows[1].state, TriState::DISABLED);
        let word = sf::SHOW_STARTED | sf::SHOW_NOT_COMPLETED;
        assert_eq!(filter_state(word, FilterKind::Started, false), TriState::ENABLED_IS);
        assert_eq!(filter_state(word, FilterKind::Completed, false), TriState::ENABLED_NOT);
    }

    #[test]
    fn the_downloaded_row_is_forced_while_downloaded_only_is_on() {
        assert_eq!(filter_state(0, FilterKind::Downloaded, true), TriState::ENABLED_IS);
        assert!(!filter_rows(0, true)[0].sensitive);
        assert_eq!(filter_click(FilterKind::Downloaded, TriState::DISABLED, true), None);
    }

    #[test]
    fn a_filter_click_cycles_and_writes_only_its_group() {
        let (value, mask) = filter_click(FilterKind::Bookmarked, TriState::DISABLED, false).expect("a write");
        assert_eq!((value, mask), (sf::SHOW_BOOKMARKED, sf::BOOKMARKED_MASK));
        let word = set_flag(-1, value, mask);
        assert_eq!(word & sf::BOOKMARKED_MASK, sf::SHOW_BOOKMARKED);
        assert_eq!(filter_click(FilterKind::Bookmarked, TriState::ENABLED_IS, false), Some((sf::SHOW_NOT_BOOKMARKED, sf::BOOKMARKED_MASK)));
        assert_eq!(filter_click(FilterKind::Bookmarked, TriState::ENABLED_NOT, false), Some((0, sf::BOOKMARKED_MASK)));
    }

    #[test]
    fn sort_rows_are_seven_and_the_direction_shows_on_the_active_one_only() {
        let rows = sort_rows(sf::SORT_SEASON | sf::SORT_ASC);
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[1].descending, Some(false));
        assert!(rows.iter().enumerate().all(|(i, r)| i == 1 || r.descending.is_none()));
        assert_eq!(rows[4].code, sf::SORT_COUNT, "unseen count is the 0x8000 code");
    }

    #[test]
    fn sort_click_flips_the_active_row_and_starts_others_ascending() {
        let word = sort_click(sf::SORT_SEASON, sf::SORT_SEASON);
        assert_eq!(word & sf::SORT_DIR_MASK, sf::SORT_ASC, "the active row flips to ascending");
        let other = sort_click(word, sf::SORT_UPLOAD);
        assert_eq!(other & sf::SORT_MASK, sf::SORT_UPLOAD);
        assert_eq!(other & sf::SORT_DIR_MASK, sf::SORT_ASC);
    }

}
