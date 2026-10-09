//! The chapter list as the widget sees it: one `RowItem` per visible chapter, a diff between two lists that
//! says which rows changed (one `items_changed` per changed row), and the focus rule. Nothing here imports GTK.
//! Upstream: `MangaScreenModel.kt` (`chapterListItems`), `MangaScreen.kt` (`ChapterItem` subtitle).

use kuro_domain::items::missing::{ListRow, Numbered, insert_missing_separators, missing_items_count};
use kuro_downloads::DownloadState;
use kuro_i18n::{Arg, MsgKey, tr_plural, trf};

use crate::entry_core::chapter::ChapterRow;
use crate::entry_core::detail::EntryState;
use crate::entry_core::numfmt::format_chapter_number;
use crate::entry_core::view::view;

/// The alpha of a read row: the disabled alpha of the theme (`ReadItemAlpha`, 0.38).
pub const READ_ALPHA: f64 = 0.38;
/// The row padding at 1x, as (start, top, end, bottom) (`MangaChapterListItem.kt`).
pub const ROW_PADDING: (i32, i32, i32, i32) = (16, 12, 8, 12);
/// The side of the unread dot.
pub const DOT_SIZE: i32 = 8;

/// The opacity of a row: read rows are dimmed.
pub fn row_alpha(read: bool) -> f64 {
    if read { READ_ALPHA } else { 1.0 }
}

/// One row of the list: the chapter with its download state and its selection, built from the models.
#[derive(Clone, Debug, PartialEq)]
pub struct RowItem {
    pub id: i64,
    pub title: String,
    pub subtitle: String,
    pub read: bool,
    pub bookmark: bool,
    pub selected: bool,
    pub download: DownloadState,
    pub progress: u8,
    /// The row has a download indicator. A local manga's chapters are always on disk and have none.
    pub indicator: bool,
    /// `Some(count)` on a missing-chapters marker row: no chapter stands behind it, and its `id` is negative.
    pub missing: Option<i32>,
}

impl RowItem {
    /// The list key of the row: `chapter-{id}`, or `missing-count-{id}` for a marker, where the marker id is the
    /// upstream `"{lower|null}-{higher}"` string (`MangaScreen.kt:776`).
    pub fn key(&self) -> String {
        match &self.missing {
            Some(_) => format!("missing-count-{}", self.title),
            None => format!("chapter-{}", self.id),
        }
    }
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

/// The second line: the upload date, "Page: N" for a partly read chapter, and the scanlator, joined by a bullet.
fn subtitle(row: &ChapterRow) -> String {
    let mut parts: Vec<String> = Vec::new();
    if row.date_upload > 0
        && let Some(date) = chrono::DateTime::from_timestamp_millis(row.date_upload)
    {
        parts.push(date.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string());
    }
    if !row.read && row.last_page_read > 0 {
        parts.push(trf(MsgKey::chapter_progress, &[("arg1", Arg::Int(row.last_page_read + 1))]));
    }
    if let Some(scanlator) = row.scanlator.as_deref().filter(|s| !s.trim().is_empty()) {
        parts.push(scanlator.to_owned());
    }
    parts.join(" \u{2022} ")
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
                id: row.id,
                title: title(row, flags.display_number()),
                subtitle: subtitle(row),
                read: row.read,
                bookmark: row.bookmark,
                selected: page.selection.contains(row.id),
                download: row.display_download(local),
                progress: row.download_progress,
                indicator: !local,
                missing: None,
            },
            ListRow::MissingCount { id, count } => RowItem {
                // the marker has no chapter: a negative id keeps it apart from them, and `title` carries its key id
                id: -marker_number(&id),
                title: id,
                subtitle: tr_plural(MsgKey::missing_chapters, i64::from(count), &[]),
                read: false,
                bookmark: false,
                selected: false,
                download: DownloadState::NotDownloaded,
                progress: 0,
                indicator: false,
                missing: Some(count),
            },
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
    /// Same rows in the same order: these indices changed. One `items_changed(i, 1, 1)` each.
    Rows(Vec<usize>),
    /// Rows were added, removed or reordered: replace the whole list with one `items_changed`.
    Reset,
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
        RowItem {
            id,
            title: format!("c{id}"),
            subtitle: String::new(),
            read: false,
            bookmark: false,
            selected: false,
            download: DownloadState::NotDownloaded,
            progress: 0,
            indicator: true,
            missing: None,
        }
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
            initialized: true,
            favorite: true,
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
    }

    #[test]
    fn display_mode_picks_number_or_name() {
        let rows = vec![ChapterRow::new(1, "Prologue", 12.5)];
        assert_eq!(row_items(&page(0, rows.clone()))[0].title, "Prologue");
        assert_eq!(row_items(&page(0x100000, rows))[0].title, "Chapter 12.5");
    }

    #[test]
    fn subtitle_joins_page_and_scanlator() {
        let mut row = ChapterRow::new(1, "c", 1.0);
        row.last_page_read = 4;
        row.scanlator = Some("Group".into());
        assert_eq!(subtitle(&row), "Page: 5 \u{2022} Group");
        row.read = true;
        assert_eq!(subtitle(&row), "Group");
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
        assert_eq!(ROW_PADDING, (16, 12, 8, 12));
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
    fn focus_stays_on_the_toggled_row_and_never_lands_on_a_hidden_one() {
        assert_eq!(focus_after(&[1, 2, 3], &[1, 2, 3], 2), Some(2));
        assert_eq!(focus_after(&[1, 2, 3], &[1, 3], 2), Some(3), "the next visible row");
        assert_eq!(focus_after(&[1, 2, 3], &[1], 2), Some(1), "then the previous one");
        assert_eq!(focus_after(&[1, 2, 3], &[], 2), None, "an empty list has no focus");
        assert_eq!(focus_after(&[1, 2], &[5], 9), Some(5), "an unknown row falls to the first");
    }
}
