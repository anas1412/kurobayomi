//! Chapter row actions of the manga page that are pure decisions (T-38b-017): the download indicator mapping, which
//! rows a download action may act on, and mark previous as read. Upstream: `MangaScreenModel.kt` (download actions,
//! `markPreviousAsRead`) and `ChapterDownloadIndicator.kt`. Behaviour 6.9.1, 6.9.8; E-DL-1, E-DL-2, E-CANCEL-1, E-PREV-1.

use kuro_downloads::DownloadState;

use crate::entry_core::chapter::{ChapterRow, sort_rows};
use crate::entry_core::detail::EntryState;
use crate::entry_core::view::view;

/// The action the download indicator of a row starts (`ChapterDownloadAction`). `Start` queues the rows; `StartNow` puts
/// one row at the front of the queue and starts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndicatorAction {
    Start,
    StartNow,
    Cancel,
    Delete,
}

/// A press on the indicator of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Click,
    LongPress,
}

/// What a press does (behaviour 7.5): runs an action at once, or opens the menu of the row's state with these items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Run(IndicatorAction),
    Menu(&'static [IndicatorAction]),
}

/// The click and long-press table of the indicator, one entry for each of the five states and two presses. A queued or
/// downloading row opens a menu on a click and cancels on a long press; a downloaded row opens its Delete menu on both;
/// an error row retries on both; a row that is not downloaded starts on a click and starts now on a long press.
pub fn outcome(state: DownloadState, press: Press) -> Outcome {
    match (state, press) {
        (DownloadState::NotDownloaded, Press::Click) => Outcome::Run(IndicatorAction::Start),
        (DownloadState::NotDownloaded, Press::LongPress) => Outcome::Run(IndicatorAction::StartNow),
        (DownloadState::Queue | DownloadState::Downloading, Press::Click) => Outcome::Menu(&[IndicatorAction::StartNow, IndicatorAction::Cancel]),
        (DownloadState::Queue | DownloadState::Downloading, Press::LongPress) => Outcome::Run(IndicatorAction::Cancel),
        (DownloadState::Downloaded, _) => Outcome::Menu(&[IndicatorAction::Delete]),
        (DownloadState::Error, _) => Outcome::Run(IndicatorAction::Start),
    }
}

/// The label of a menu item: "Start downloading now", "Cancel" or "Delete". `Start` is never in a menu.
pub fn menu_label(action: IndicatorAction) -> kuro_i18n::MsgKey {
    match action {
        IndicatorAction::Cancel => kuro_i18n::MsgKey::action_cancel,
        IndicatorAction::Delete => kuro_i18n::MsgKey::action_delete,
        IndicatorAction::Start | IndicatorAction::StartNow => kuro_i18n::MsgKey::action_start_downloading_now,
    }
}

/// The indicator answers presses only while no row is selected, and a local manga has none (E-27).
pub fn indicator_enabled(local: bool, selected: usize) -> bool {
    !local && selected == 0
}

/// How the indicator of a queued or downloading row is drawn (`DownloadingIndicator`). A queued row, and a downloading
/// one at 0 %, spins with no progress. Otherwise the ring fills to `fraction` (the percent over 100, clamped to
/// [0, 1], E-39) and the arrow takes the background colour from half way on, so it stays visible on the filled ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    pub indeterminate: bool,
    pub fraction: f64,
    pub arrow_on_fill: bool,
}

/// The ring of a state, or `None` for the three states that draw an icon.
pub fn ring(state: DownloadState, percent: u8) -> Option<Ring> {
    let indeterminate = match state {
        DownloadState::Queue => true,
        DownloadState::Downloading => percent == 0,
        _ => return None,
    };
    let fraction = (f64::from(percent) / 100.0).clamp(0.0, 1.0);
    Some(Ring { indeterminate, fraction, arrow_on_fill: !indeterminate && fraction >= 0.5 })
}

pub fn indicator_action(state: DownloadState) -> IndicatorAction {
    match state {
        DownloadState::NotDownloaded | DownloadState::Error => IndicatorAction::StartNow,
        DownloadState::Queue | DownloadState::Downloading => IndicatorAction::Cancel,
        DownloadState::Downloaded => IndicatorAction::Delete,
    }
}

/// START_NOW and CANCEL act only on exactly one row (upstream ignores several, E-DL-1, E-DL-2). Start and Delete act on any.
pub fn acts_on(action: IndicatorAction, rows: usize) -> bool {
    match action {
        IndicatorAction::StartNow | IndicatorAction::Cancel => rows == 1,
        IndicatorAction::Start | IndicatorAction::Delete => rows > 0,
    }
}

/// The local state a cancel sets at once. A later downloader event overrides it (E-CANCEL-1).
pub fn state_after_cancel() -> DownloadState {
    DownloadState::NotDownloaded
}

/// The chapters chronologically before `pointer`, in chronological order (6.9.8). `view` is the displayed list. In a
/// descending view the earlier chapters come after the pointer; the list is reversed to chronological order. A pointer
/// that is not in the view marks nothing (E-PREV-1).
pub fn previous_ids(view: &[i64], pointer: i64, descending: bool) -> Vec<i64> {
    let Some(at) = view.iter().position(|id| *id == pointer) else {
        return Vec::new();
    };
    if descending {
        let mut earlier = view[at + 1..].to_vec();
        earlier.reverse();
        earlier
    } else {
        view[..at].to_vec()
    }
}

/// The Resume or Start button: visible when the processed list has an unread chapter and nothing is selected. It
/// reads "Resume" when the unfiltered list has a read chapter, else "Start" (MangaScreen.kt:346-361).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResumeFab {
    pub visible: bool,
    pub resume: bool,
}

impl ResumeFab {
    /// "Resume" when a chapter was read, else "Start".
    pub fn label(self) -> kuro_i18n::MsgKey {
        if self.resume { kuro_i18n::MsgKey::action_resume } else { kuro_i18n::MsgKey::action_start }
    }
}

pub fn resume_fab(processed_has_unread: bool, unfiltered_has_read: bool, selected_total: usize) -> ResumeFab {
    ResumeFab { visible: processed_has_unread && selected_total == 0, resume: unfiltered_has_read }
}

/// The state of the Resume or Start button of a page: it reads the processed list for the unread test and the
/// unfiltered list for the label (`MangaScreen.kt:346-361`).
pub fn fab_state(page: &EntryState) -> ResumeFab {
    let processed = view(&page.chapters, page.manga.flags, page.downloaded_only, page.manga.local);
    resume_fab(processed.iter().any(|r| !r.read), page.chapters.iter().any(|r| r.read), page.selection.len())
}

/// The entries of the Download menu (`DownloadAction`, behaviour 6.9.5): the next 1, 5, 10 or 25 unread chapters, or
/// every unread one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadAction {
    Next(usize),
    Unread,
}

/// The chapters a Download menu entry queues (behaviour 6.9.6): unread and not downloaded, from the filtered list
/// when the reader preference `skipFiltered` is on and from every chapter otherwise. "Next N" takes the oldest first:
/// the stored sort, reversed when it is descending, then the first N. "Unread" keeps the list order.
pub fn download_targets(page: &EntryState, action: DownloadAction) -> Vec<i64> {
    let flags = page.manga.flags;
    let pool: Vec<&ChapterRow> = if page.skip_filtered {
        view(&page.chapters, flags, page.downloaded_only, page.manga.local)
    } else {
        page.chapters.iter().collect()
    };
    let mut unread: Vec<&ChapterRow> = pool.into_iter().filter(|r| !r.read && r.download == DownloadState::NotDownloaded).collect();
    match action {
        DownloadAction::Unread => unread.into_iter().map(|r| r.id).collect(),
        DownloadAction::Next(n) => {
            sort_rows(flags, &mut unread);
            if flags.sort_descending() {
                unread.reverse();
            }
            unread.into_iter().take(n).map(|r| r.id).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_core::detail::{LoadContext, LoadState, MangaRow, SourceInfo, finish_load};
    use crate::entry_core::flags::ChapterFlags;

    #[test]
    fn each_state_and_press_has_the_outcome_of_the_table() {
        use DownloadState::*;
        use IndicatorAction::*;
        use Press::*;
        let menu_now_cancel = Outcome::Menu(&[StartNow, Cancel]);
        let menu_delete = Outcome::Menu(&[Delete]);
        let table = [
            (NotDownloaded, Click, Outcome::Run(Start)),
            (NotDownloaded, LongPress, Outcome::Run(StartNow)),
            (Queue, Click, menu_now_cancel),
            (Queue, LongPress, Outcome::Run(Cancel)),
            (Downloading, Click, menu_now_cancel),
            (Downloading, LongPress, Outcome::Run(Cancel)),
            (Downloaded, Click, menu_delete),
            (Downloaded, LongPress, menu_delete),
            (Error, Click, Outcome::Run(Start)),
            (Error, LongPress, Outcome::Run(Start)),
        ];
        for (state, press, want) in table {
            assert_eq!(outcome(state, press), want, "{state:?} {press:?}");
        }
    }

    #[test]
    fn the_menus_hold_only_labelled_actions_and_a_not_downloaded_or_error_row_has_none() {
        for state in [DownloadState::Queue, DownloadState::Downloading, DownloadState::Downloaded] {
            let Outcome::Menu(items) = outcome(state, Press::Click) else { panic!("{state:?} opens a menu") };
            assert!(!items.contains(&IndicatorAction::Start));
        }
        for state in [DownloadState::NotDownloaded, DownloadState::Error] {
            assert!(matches!(outcome(state, Press::Click), Outcome::Run(_)), "{state:?} has no menu");
        }
        assert_eq!(menu_label(IndicatorAction::Cancel), kuro_i18n::MsgKey::action_cancel);
        assert_eq!(menu_label(IndicatorAction::Delete), kuro_i18n::MsgKey::action_delete);
        assert_eq!(menu_label(IndicatorAction::StartNow), kuro_i18n::MsgKey::action_start_downloading_now);
    }

    #[test]
    fn the_indicator_is_off_for_a_local_manga_and_while_a_row_is_selected() {
        assert!(indicator_enabled(false, 0));
        assert!(!indicator_enabled(false, 1));
        assert!(!indicator_enabled(true, 0));
    }

    #[test]
    fn the_ring_spins_when_queued_or_at_zero_and_switches_the_arrow_at_half() {
        assert_eq!(ring(DownloadState::NotDownloaded, 0), None);
        assert_eq!(ring(DownloadState::Downloaded, 100), None);
        assert_eq!(ring(DownloadState::Error, 40), None);
        assert!(ring(DownloadState::Queue, 60).unwrap().indeterminate, "a queued row never shows progress");
        assert!(ring(DownloadState::Downloading, 0).unwrap().indeterminate);
        let at = |percent| ring(DownloadState::Downloading, percent).unwrap();
        assert_eq!((at(49).indeterminate, at(49).arrow_on_fill), (false, false));
        assert!(at(50).arrow_on_fill, "from 0.5 the arrow takes the background colour");
        assert_eq!(at(25).fraction, 0.25);
        assert_eq!(at(250).fraction, 1.0, "the fraction is clamped to one (E-39)");
        assert!(!ring(DownloadState::Queue, 90).unwrap().arrow_on_fill);
    }

    #[test]
    fn start_acts_on_any_number_of_rows_and_start_now_and_cancel_on_one() {
        assert!(acts_on(IndicatorAction::Start, 3) && !acts_on(IndicatorAction::Start, 0));
        assert!(acts_on(IndicatorAction::StartNow, 1) && !acts_on(IndicatorAction::StartNow, 2));
        assert!(acts_on(IndicatorAction::Cancel, 1) && !acts_on(IndicatorAction::Cancel, 2));
    }

    fn page(flags: i64, skip_filtered: bool, rows: Vec<ChapterRow>) -> Box<EntryState> {
        let manga = MangaRow {
            id: 1,
            source: 1,
            url: String::new(),
            title: "t".into(),
            author: None,
            artist: None,
            description: None,
            genres: vec![],
            status: 0,
            thumbnail_url: None,
            cover_last_modified: 0,
            initialized: true,
            favorite: true,
            next_update: 0,
            fetch_interval: 0,
            local: false,
            flags: ChapterFlags(flags),
        };
        let ctx = LoadContext { skip_filtered, ..LoadContext::default() };
        match finish_load(Some(manga), rows, SourceInfo::default(), &ctx).state {
            LoadState::Ready(page) => page,
            _ => unreachable!(),
        }
    }

    fn rows() -> Vec<ChapterRow> {
        let mut r: Vec<ChapterRow> = (1..=6).map(|i| ChapterRow::new(i, &format!("c{i}"), i as f64)).collect();
        r[0].read = true; // 1 is read
        r[1].download = DownloadState::Downloaded; // 2 is on disk
        r[2].download = DownloadState::Queue; // 3 is queued
        r[5].bookmark = true; // only 6 passes a bookmarked filter
        r
    }

    #[test]
    fn the_resume_button_reads_the_processed_list_for_unread_and_the_whole_list_for_its_label() {
        // chapter 1 is read and the rest are not: the processed list has unread, and the unfiltered one has a read row
        let shown = page(0x1 | 0x100, true, rows());
        assert_eq!(fab_state(&shown), ResumeFab { visible: true, resume: true });
        // a bookmarked filter that keeps only an unread row still shows it; hiding all unread rows hides the button
        let mut all_read = rows();
        all_read.iter_mut().for_each(|r| r.read = true);
        assert!(!fab_state(&page(0x1 | 0x100, true, all_read)).visible);
        let mut selected = page(0x1 | 0x100, true, rows());
        selected.selection.set_all(&[4], true);
        assert!(!fab_state(&selected).visible, "hidden while selecting");
    }

    #[test]
    fn unread_download_targets_skip_read_downloaded_and_queued_rows() {
        let p = page(0x1 | 0x100, true, rows());
        assert_eq!(download_targets(&p, DownloadAction::Unread), vec![4, 5, 6]);
        assert_eq!(download_targets(&p, DownloadAction::Next(2)), vec![4, 5]);
    }

    #[test]
    fn next_n_takes_the_oldest_first_for_a_descending_sort_too() {
        let p = page(0x100, true, rows()); // number descending
        assert_eq!(download_targets(&p, DownloadAction::Next(2)), vec![4, 5], "oldest unread first (6.9.6)");
        assert_eq!(download_targets(&p, DownloadAction::Unread), vec![6, 5, 4], "unread keeps the list order");
    }

    #[test]
    fn skip_filtered_decides_between_the_view_and_every_chapter() {
        // the bookmarked filter shows only chapter 6
        let flags = 0x1 | 0x100 | kuro_db::flags::chapter::SHOW_BOOKMARKED;
        assert_eq!(download_targets(&page(flags, true, rows()), DownloadAction::Unread), vec![6]);
        assert_eq!(download_targets(&page(flags, false, rows()), DownloadAction::Unread), vec![4, 5, 6]);
    }

    #[test]
    fn indicator_maps_each_state() {
        assert_eq!(indicator_action(DownloadState::NotDownloaded), IndicatorAction::StartNow);
        assert_eq!(indicator_action(DownloadState::Error), IndicatorAction::StartNow);
        assert_eq!(indicator_action(DownloadState::Queue), IndicatorAction::Cancel);
        assert_eq!(indicator_action(DownloadState::Downloading), IndicatorAction::Cancel);
        assert_eq!(indicator_action(DownloadState::Downloaded), IndicatorAction::Delete);
    }

    #[test]
    fn start_now_and_cancel_need_one_row() {
        assert!(acts_on(IndicatorAction::StartNow, 1));
        assert!(!acts_on(IndicatorAction::StartNow, 2));
        assert!(acts_on(IndicatorAction::Cancel, 1));
        assert!(!acts_on(IndicatorAction::Cancel, 3));
        assert!(acts_on(IndicatorAction::Delete, 3));
    }

    #[test]
    fn cancel_sets_not_downloaded_locally() {
        assert_eq!(state_after_cancel(), DownloadState::NotDownloaded);
    }

    #[test]
    fn mark_previous_uses_the_view_vector_of_the_spec() {
        assert_eq!(previous_ids(&[9, 8, 7, 6], 7, true), vec![6]);
        assert_eq!(previous_ids(&[9, 8, 7, 6, 5], 7, true), vec![5, 6], "chronological order");
        assert_eq!(previous_ids(&[5, 6, 7, 8], 7, false), vec![5, 6]);
    }

    #[test]
    fn the_button_reads_resume_or_start() {
        assert_eq!(resume_fab(true, true, 0).label(), kuro_i18n::MsgKey::action_resume);
        assert_eq!(resume_fab(true, false, 0).label(), kuro_i18n::MsgKey::action_start);
    }

    #[test]
    fn resume_button_rules() {
        assert_eq!(resume_fab(true, true, 0), ResumeFab { visible: true, resume: true });
        assert_eq!(resume_fab(true, false, 0), ResumeFab { visible: true, resume: false }, "Start");
        assert!(!resume_fab(true, true, 1).visible, "hidden while selecting");
        assert!(!resume_fab(false, true, 0).visible, "nothing unread");
    }

    #[test]
    fn mark_previous_with_a_pointer_outside_the_view_marks_nothing() {
        assert!(previous_ids(&[9, 8, 7], 42, true).is_empty());
        assert!(previous_ids(&[9, 8, 7], 9, true).len() == 2, "descending: everything earlier");
    }
}
