//! The rows of the chapter and episode list as the widget sees them: one `RowItem` per chapter or episode (and per
//! missing-count marker), the shared second line, a diff between two lists that says which rows changed, and the focus
//! rule. Both kinds build the same `RowItem`, so the list draws them the same way. Nothing here imports GTK.
//! Upstream: `MangaScreenModel.kt` (`chapterListItems`), `MangaScreen.kt` (`ChapterItem` subtitle).

use kuro_domain::items::missing::{ListRow, Numbered, insert_missing_separators, missing_items_count};
use kuro_downloads::DownloadState;
use kuro_i18n::{Arg, MsgKey, tr_plural, trf};

use crate::entry_core::chapter::ChapterRow;
use crate::entry_core::chapter_actions::indicator_enabled;
use crate::entry_core::detail::EntryState;
use crate::entry_core::numfmt::format_chapter_number;
use crate::entry_core::view::view;

/// The alpha of the text of a read or seen row: the disabled alpha of the theme (`ReadItemAlpha`, 0.38).
pub const READ_ALPHA: f64 = 0.38;
/// The side of the unread dot.
pub const DOT_SIZE: i32 = 8;

/// The opacity of a row's text: read rows are dimmed.
pub fn row_alpha(read: bool) -> f64 {
    if read { READ_ALPHA } else { 1.0 }
}

/// One row of the list, a chapter or an episode, built from the models with its download state and its selection.
#[derive(Clone, Debug, PartialEq)]
pub struct RowItem {
    pub id: i64,
    pub title: String,
    /// The dim second line: date, progress and scanlator (`subtitle`).
    pub subtitle: String,
    /// An episode summary under the second line, when the entry shows summaries.
    pub summary: Option<String>,
    /// The summary shows all its lines rather than three.
    pub summary_expanded: bool,
    /// Read or seen: the text is dimmed and the unread dot is off.
    pub read: bool,
    pub bookmark: bool,
    /// An episode marked filler.
    pub filler: bool,
    pub selected: bool,
    pub download: DownloadState,
    pub progress: u8,
    /// The row has a download button. A local entry's items are always on disk and have none.
    pub indicator: bool,
    /// The button answers presses: not while any row is selected (E-27).
    pub indicator_enabled: bool,
    /// `Some(count)` on a missing-count marker row: no chapter or episode stands behind it, and its `id` is negative.
    pub missing: Option<i32>,
}

impl RowItem {
    /// A plain row: unread, unmarked, unselected, not downloaded and with no download button.
    pub fn new(id: i64, title: String) -> Self {
        Self {
            id,
            title,
            subtitle: String::new(),
            summary: None,
            summary_expanded: false,
            read: false,
            bookmark: false,
            filler: false,
            selected: false,
            download: DownloadState::NotDownloaded,
            progress: 0,
            indicator: false,
            indicator_enabled: false,
            missing: None,
        }
    }

    /// The marker of `count` missing items between two neighbours. `key` is the upstream `"{lower|null}-{higher}"` id
    /// and `text` the line it shows ("Missing 2 chapters").
    pub fn marker(key: String, count: i32, text: String) -> Self {
        // the marker has no item: a negative id keeps it apart from them, and `title` carries its key id
        Self { id: -marker_number(&key), subtitle: text, missing: Some(count), ..Self::new(0, key) }
    }

    /// The list key of the row: `chapter-{id}`, or `missing-count-{id}` for a marker, where the marker id is the
    /// upstream `"{lower|null}-{higher}"` string (`MangaScreen.kt:776`).
    pub fn key(&self) -> String {
        match &self.missing {
            Some(_) => format!("missing-count-{}", self.title),
            None => format!("chapter-{}", self.id),
        }
    }
}

/// The upload date as `YYYY-MM-DD`; an unknown date (zero or negative) has no text.
fn date_text(date_upload_ms: i64) -> Option<String> {
    if date_upload_ms <= 0 {
        return None;
    }
    let date = chrono::DateTime::from_timestamp_millis(date_upload_ms)?;
    Some(date.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
}

/// The dim second line of a chapter or an episode: the upload date, the progress and the scanlator or language, joined
/// by a bullet. A missing part leaves no bullet behind, and an unknown scanlator is never shown.
pub fn subtitle(date_upload_ms: i64, progress: Option<String>, scanlator: Option<&str>) -> String {
    [date_text(date_upload_ms), progress, crate::entry_core::text::shown_scanlator(scanlator).map(str::to_owned)]
        .into_iter()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" \u{2022} ")
}

impl Numbered for &ChapterRow {
    fn item_id(&self) -> i64 {
        self.id
    }
    fn number(&self) -> f64 {
        self.number
    }
}

/// The title: "Chapter 12.5" in number mode, the chapter name otherwise (`display_mode_chapter`).
fn title(row: &ChapterRow, number_mode: bool) -> String {
    if number_mode {
        trf(MsgKey::display_mode_chapter, &[("arg1", Arg::Str(&format_chapter_number(row.number)))])
    } else {
        row.name.clone()
    }
}

/// The second line of a chapter: "Page: N" while it is partly read.
fn chapter_line(row: &ChapterRow) -> String {
    let progress = (!row.read && row.last_page_read > 0).then(|| trf(MsgKey::chapter_progress, &[("arg1", Arg::Int(row.last_page_read + 1))]));
    subtitle(row.date_upload, progress, row.scanlator.as_deref())
}

/// The rows the list shows, in view order, each with its download state and selection, with a missing-count marker
/// row wherever the numbers of two neighbours skip chapters (`MangaScreenModel.kt` `insertSeparators`).
pub fn row_items(page: &EntryState) -> Vec<RowItem> {
    let flags = page.manga.flags;
    let local = page.manga.local;
    let shown = view(&page.chapters, flags, page.downloaded_only, local);
    insert_missing_separators(shown, flags.sort_descending())
        .into_iter()
        .map(|row| match row {
            ListRow::Item(row) => RowItem {
                subtitle: chapter_line(row),
                read: row.read,
                bookmark: row.bookmark,
                selected: page.selection.contains(row.id),
                download: row.display_download(local),
                progress: row.download_progress,
                indicator: !local,
                indicator_enabled: indicator_enabled(local, page.selection.len()),
                ..RowItem::new(row.id, title(row, flags.display_number()))
            },
            ListRow::MissingCount { id, count } => RowItem::marker(id, count, tr_plural(MsgKey::missing_chapters, i64::from(count), &[])),
        })
        .collect()
}

/// The higher chapter id of a marker id `"{lower|null}-{higher}"`. Each chapter is the higher side of at most one
/// marker, so the id is unique per marker.
fn marker_number(id: &str) -> i64 {
    id.rsplit('-').next().and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// What the header row over the list shows (`ItemHeader`): the count of the processed list, and the missing warning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListHeader {
    /// "N chapters" over the processed list, "0 chapters" when it is empty (E-02).
    pub count: String,
    /// "Missing N chapters", or `None` at 0 (hidden).
    pub missing: Option<String>,
    /// Clicking the header opens the filter and sort sheet. False while any row is selected.
    pub enabled: bool,
}

/// The header over the processed list. The missing count reads the processed numbers (`missingChaptersCount`).
pub fn list_header(page: &EntryState) -> ListHeader {
    let flags = page.manga.flags;
    let shown = view(&page.chapters, flags, page.downloaded_only, page.manga.local);
    let numbers: Vec<f64> = shown.iter().map(|r| r.number).collect();
    let missing = missing_items_count(&numbers);
    ListHeader {
        count: tr_plural(MsgKey::manga_num_chapters, shown.len() as i64, &[]),
        missing: (missing > 0).then(|| tr_plural(MsgKey::missing_items, i64::from(missing), &[])),
        enabled: page.selection.is_empty(),
    }
}

/// What a model must do to go from one list to the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diff {
    Same,
    /// Same rows in the same order: these indices changed (ascending). One row is one `items_changed(i, 1, 1)`;
    /// several are one splice over their span (`changed_span`), so select all is one signal, not one per row.
    Rows(Vec<usize>),
    /// Rows were added, removed or reordered: replace the whole list with one splice.
    Reset,
}

/// What an item of a row's right-click menu does. Chapters and episodes have the same menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowAction {
    ToggleRead,
    MarkPrevious,
    ToggleBookmark,
    /// Downloads the row, cancels its download while it is queued, or deletes it once it is on disk.
    Download,
    /// Selects the row: the menu's Select item, and what a long press on a row reports.
    Select,
}

/// The right-click menu of `row` as `(label, section, action)`, in section order. Each label says what a press does
/// now (Mark as read or Mark as unread). A marker row has no menu, and Download shows only on a row with a download
/// button. `anime` words read as watched.
pub fn row_menu(row: &RowItem, anime: bool) -> Vec<(MsgKey, u8, RowAction)> {
    if row.missing.is_some() {
        return Vec::new();
    }
    let read = match (anime, row.read) {
        (false, false) => MsgKey::action_mark_as_read,
        (false, true) => MsgKey::action_mark_as_unread,
        (true, false) => MsgKey::action_mark_as_seen,
        (true, true) => MsgKey::action_mark_as_unseen,
    };
    let previous = if anime { MsgKey::action_mark_previous_as_seen } else { MsgKey::action_mark_previous_as_read };
    let bookmark = match (anime, row.bookmark) {
        (false, false) => MsgKey::action_bookmark,
        (false, true) => MsgKey::action_remove_bookmark,
        (true, false) => MsgKey::action_bookmark_episode,
        (true, true) => MsgKey::action_remove_bookmark_episode,
    };
    let mut items = vec![(read, 0, RowAction::ToggleRead), (previous, 0, RowAction::MarkPrevious), (bookmark, 1, RowAction::ToggleBookmark)];
    if row.indicator {
        let download = match row.download {
            DownloadState::Downloaded => MsgKey::action_delete_download,
            DownloadState::Queue | DownloadState::Downloading => MsgKey::action_cancel,
            DownloadState::NotDownloaded | DownloadState::Error => MsgKey::action_download,
        };
        items.push((download, 1, RowAction::Download));
    }
    items.push((MsgKey::action_select, 2, RowAction::Select));
    items
}

/// The span `(start, len)` that covers every index of a `Diff::Rows`. `None` for no index.
pub fn changed_span(indices: &[usize]) -> Option<(usize, usize)> {
    let (first, last) = (indices.iter().min()?, indices.iter().max()?);
    Some((*first, last - first + 1))
}

pub fn diff(old: &[RowItem], new: &[RowItem]) -> Diff {
    if old.len() != new.len() || old.iter().zip(new).any(|(a, b)| a.id != b.id) {
        return Diff::Reset;
    }
    let changed: Vec<usize> = (0..old.len()).filter(|&i| old[i] != new[i]).collect();
    if changed.is_empty() { Diff::Same } else { Diff::Rows(changed) }
}

/// The row that keeps keyboard focus after the list changed from `before` to `after` (both chapter ids in view
/// order). The focused row stays focused while it is visible. When a filter hides it, focus goes to the next
/// visible row, then the previous one, and never to a row that is not in the list.
pub fn focus_after(before: &[i64], after: &[i64], focused: i64) -> Option<i64> {
    if after.contains(&focused) {
        return Some(focused);
    }
    let Some(at) = before.iter().position(|&id| id == focused) else {
        return after.first().copied();
    };
    before[at + 1..]
        .iter()
        .find(|id| after.contains(id))
        .or_else(|| before[..at].iter().rev().find(|id| after.contains(id)))
        .copied()
        .or_else(|| after.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_core::detail::{LoadContext, LoadState, MangaRow, SourceInfo, finish_load};
    use crate::entry_core::flags::ChapterFlags;

    fn item(id: i64) -> RowItem {
        RowItem { indicator: true, indicator_enabled: true, ..RowItem::new(id, format!("c{id}")) }
    }

    fn page(flags: i64, rows: Vec<ChapterRow>) -> Box<EntryState> {
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
        match finish_load(Some(manga), rows, SourceInfo::default(), &LoadContext::default()).state {
            LoadState::Ready(page) => page,
            _ => unreachable!(),
        }
    }

    #[test]
    fn rows_carry_download_state_and_selection_from_the_models() {
        let mut a = ChapterRow::new(1, "One", 1.0);
        a.download = DownloadState::Downloaded;
        let b = ChapterRow::new(2, "Two", 2.0);
        let mut page = page(0x1 | 0x100, vec![a, b]); // number ascending
        page.selection.set_all(&[2], true);
        let rows = row_items(&page);
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(rows[0].download, DownloadState::Downloaded);
        assert!(!rows[0].selected && rows[1].selected);
        assert!(rows.iter().all(|r| r.indicator && !r.indicator_enabled), "the indicators are off while a row is selected");
        page.selection.clear();
        assert!(row_items(&page).iter().all(|r| r.indicator && r.indicator_enabled));
    }

    #[test]
    fn display_mode_picks_number_or_name() {
        let rows = vec![ChapterRow::new(1, "Prologue", 12.5)];
        assert_eq!(row_items(&page(0, rows.clone()))[0].title, "Prologue");
        assert_eq!(row_items(&page(0x100000, rows))[0].title, "Chapter 12.5");
    }

    #[test]
    fn the_chapter_line_joins_page_and_scanlator() {
        let mut row = ChapterRow::new(1, "c", 1.0);
        row.last_page_read = 4;
        row.scanlator = Some("Group".into());
        assert_eq!(chapter_line(&row), "Page: 5 \u{2022} Group");
        row.read = true;
        assert_eq!(chapter_line(&row), "Group");
        row.scanlator = Some("Unknown".into());
        assert_eq!(chapter_line(&row), "");
        row.read = false;
        assert_eq!(chapter_line(&row), "Page: 5", "no dot is left behind");
    }

    #[test]
    fn the_second_line_joins_only_present_parts() {
        assert_eq!(subtitle(0, None, None), "");
        assert_eq!(subtitle(0, Some("12:30 left".into()), Some("grp")), "12:30 left \u{2022} grp");
        assert_eq!(subtitle(-5, None, Some(" ")), "", "an unknown date and a blank scanlator leave nothing");
        assert!(subtitle(86_400_000, None, None).starts_with("1970-01-0"), "the date comes first");
    }

    #[test]
    fn a_gap_in_the_numbers_inserts_a_keyed_marker_and_the_header_counts_the_processed_list() {
        // ascending by number: 1, 2, 5 -> two chapters missing between 2 and 5
        let rows = vec![ChapterRow::new(1, "a", 1.0), ChapterRow::new(2, "b", 2.0), ChapterRow::new(3, "c", 5.0)];
        let page = page(0x1 | 0x100, rows);
        let items = row_items(&page);
        assert_eq!(items.iter().map(|r| r.missing).collect::<Vec<_>>(), [None, None, Some(2), None]);
        let marker = &items[2];
        assert_eq!(marker.key(), "missing-count-2-3", "the id is {{lower}}-{{higher}}");
        assert!(marker.id < 0, "no chapter id collides with a marker");
        assert_eq!(marker.subtitle, "Missing 2 chapters");
        assert_eq!(items[0].key(), "chapter-1");
        let header = list_header(&page);
        assert_eq!((header.count.as_str(), header.missing.as_deref(), header.enabled), ("3 chapters", Some("Missing 2 items"), true));
    }

    #[test]
    fn a_descending_list_puts_the_marker_after_the_higher_chapter() {
        let rows = vec![ChapterRow::new(1, "a", 1.0), ChapterRow::new(2, "b", 4.0)];
        let items = row_items(&page(0x100, rows)); // number, descending
        assert_eq!(items.iter().map(|r| r.id).collect::<Vec<_>>(), [2, -2, 1]);
        assert_eq!(items[1].key(), "missing-count-1-2");
    }

    #[test]
    fn the_header_over_a_filter_that_hides_everything_says_0_chapters_and_a_selection_disables_it() {
        let mut read = ChapterRow::new(1, "a", 1.0);
        read.read = true;
        let one = page(0, vec![ChapterRow::new(1, "a", 1.0)]);
        assert_eq!(list_header(&one).count, "1 chapter");
        let mut page = page(0x1 | 0x100 | kuro_db::flags::chapter::SHOW_UNREAD, vec![read]);
        let header = list_header(&page);
        assert_eq!((header.count.as_str(), header.missing), ("0 chapters", None));
        assert!(row_items(&page).is_empty(), "no rows and no markers (E-02)");
        page.selection.set_all(&[1], true);
        assert!(!list_header(&page).enabled);
    }

    #[test]
    fn row_style_follows_the_upstream_values() {
        assert_eq!(row_alpha(true), 0.38);
        assert_eq!(row_alpha(false), 1.0);
        assert_eq!(DOT_SIZE, 8);
    }

    #[test]
    fn diff_reports_one_row_per_change_and_a_reset_for_structure() {
        let old = vec![item(1), item(2), item(3)];
        assert_eq!(diff(&old, &old), Diff::Same);
        let mut new = old.clone();
        new[1].selected = true;
        new[2].progress = 40;
        assert_eq!(diff(&old, &new), Diff::Rows(vec![1, 2]));
        assert_eq!(diff(&old, &old[..2]), Diff::Reset, "a removed row");
        assert_eq!(diff(&old, &[item(3), item(2), item(1)]), Diff::Reset, "a reorder");
    }

    #[test]
    fn the_row_menu_is_one_menu_for_both_kinds_with_labels_of_what_a_press_does() {
        let actions = |items: Vec<(MsgKey, u8, RowAction)>| items.into_iter().map(|(_, _, a)| a).collect::<Vec<_>>();
        let all = [RowAction::ToggleRead, RowAction::MarkPrevious, RowAction::ToggleBookmark, RowAction::Download, RowAction::Select];
        assert_eq!(actions(row_menu(&item(1), false)), all);
        assert_eq!(actions(row_menu(&item(1), true)), all, "an episode has the same items");
        let sections: Vec<u8> = row_menu(&item(1), false).iter().map(|(_, s, _)| *s).collect();
        assert!(sections.windows(2).all(|w| w[0] <= w[1]), "already in section order, as the menu shows them");

        let chapter = row_menu(&item(1), false);
        assert_eq!((chapter[0].0, chapter[2].0, chapter[3].0), (MsgKey::action_mark_as_read, MsgKey::action_bookmark, MsgKey::action_download));
        let done = RowItem { read: true, bookmark: true, download: DownloadState::Downloaded, ..item(1) };
        let episode = row_menu(&done, true);
        assert_eq!((episode[0].0, episode[1].0, episode[2].0, episode[3].0), (MsgKey::action_mark_as_unseen, MsgKey::action_mark_previous_as_seen, MsgKey::action_remove_bookmark_episode, MsgKey::action_delete_download));
        let queued = RowItem { download: DownloadState::Queue, ..item(1) };
        assert_eq!(row_menu(&queued, false)[3].0, MsgKey::action_cancel);

        let local = RowItem { indicator: false, ..item(1) };
        assert!(!actions(row_menu(&local, false)).contains(&RowAction::Download), "no download button, no Download item");
        assert!(row_menu(&RowItem::marker("1-4".into(), 2, "Missing".into()), false).is_empty(), "a marker has no menu");
    }

    #[test]
    fn the_span_covers_the_first_to_the_last_changed_row() {
        assert_eq!(changed_span(&[]), None);
        assert_eq!(changed_span(&[4]), Some((4, 1)));
        assert_eq!(changed_span(&[2, 3, 9]), Some((2, 8)));
    }

    #[test]
    fn focus_stays_on_the_toggled_row_and_never_lands_on_a_hidden_one() {
        assert_eq!(focus_after(&[1, 2, 3], &[1, 2, 3], 2), Some(2));
        assert_eq!(focus_after(&[1, 2, 3], &[1, 3], 2), Some(3), "the next visible row");
        assert_eq!(focus_after(&[1, 2, 3], &[1], 2), Some(1), "then the previous one");
        assert_eq!(focus_after(&[1, 2, 3], &[], 2), None, "an empty list has no focus");
        assert_eq!(focus_after(&[1, 2], &[5], 9), Some(5), "an unknown row falls to the first");
    }
}
