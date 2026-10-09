//! Anime download queue page rules (T-24a-040, T-24a-041, T-24a-042, T-24a-043; ui-parity 1.3 to 1.7).
//!
//! Pure data and rules, with no GTK. The page renders the [`Group`] and [`RowView`] values, asks [`menu_for`] which
//! entries to show, and sends the [`Plan`] of a chosen entry to the queue as `Reorder` or `Cancel`. Rows are keyed by
//! episode id.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::anime::lifecycle::RunnerState;
use crate::anime::model::AnimeDownload;
use crate::model::DownloadState;

/// The queue page rebuilds at most this often (T-24a-046).
pub const REBUILD_INTERVAL: Duration = Duration::from_millis(200);

/// The sort keys of the queue dropdown (T-24a-045, ui 1.10): `date_upload` or `episode_number`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    UploadDate,
    EpisodeNumber,
}

/// The episode ids in the order the sort gives, sorted inside each source group. `reverse` gives descending order
/// (Newest first for upload date, Descending for episode number). Groups keep their order, as in `reorderQueue`.
pub fn sorted_order(items: &[Arc<AnimeDownload>], key: SortKey, reverse: bool) -> Vec<i64> {
    let by_id: HashMap<i64, &AnimeDownload> = items.iter().map(|d| (d.episode.id, d.as_ref())).collect();
    let mut groups = group_ids(items);
    for (_, ids) in &mut groups {
        ids.sort_by(|a, b| {
            let (x, y) = (&by_id[a].episode, &by_id[b].episode);
            let order = match key {
                SortKey::UploadDate => x.upload_date.cmp(&y.upload_date),
                SortKey::EpisodeNumber => x.episode_number.total_cmp(&y.episode_number),
            };
            if reverse { order.reverse() } else { order }
        });
    }
    flatten(&groups)
}

/// The Stop / Continue button of the queue footer (T-24a-044, ui 1.11 and section 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterAction {
    Stop,
    Continue,
}

impl FooterAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stop => "Stop",
            Self::Continue => "Continue",
        }
    }
}

/// `None` when the queue is empty, so the footer action is hidden. Otherwise Stop while the runner runs, Continue when
/// it is stopped or paused.
pub fn footer_action(state: RunnerState, queued: usize) -> Option<FooterAction> {
    if queued == 0 {
        return None;
    }
    Some(if state == RunnerState::Running { FooterAction::Stop } else { FooterAction::Continue })
}

/// Lets a rebuild through at most once per [`REBUILD_INTERVAL`] (T-24a-046). A call that returns `false` is not lost:
/// the caller asks again on its next tick, and the latest state is drawn then.
#[derive(Debug, Default)]
pub struct RebuildGate {
    last: Option<Instant>,
}

impl RebuildGate {
    pub fn ready(&mut self, now: Instant) -> bool {
        match self.last {
            Some(last) if now.saturating_duration_since(last) < REBUILD_INTERVAL => false,
            _ => {
                self.last = Some(now);
                true
            }
        }
    }
}

/// Shown on an empty queue (`information_no_downloads`).
pub const EMPTY_TEXT: &str = "No downloads";
/// Shown while hoster resolution runs, before a video is known (E-U05, proposal).
pub const LOOKING_FOR_VIDEO_TEXT: &str = "Looking for video…";
/// Shown at 0 percent, when the bar is indeterminate.
pub const DOWNLOADING_TEXT: &str = "Downloading…";

/// One queue row, as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowView {
    pub episode_id: i64,
    pub anime_id: i64,
    pub anime_title: String,
    pub episode_name: String,
    pub status: DownloadState,
    pub progress: i32,
    pub has_video: bool,
}

/// Progress bar state: `value` of `max`, indeterminate at 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bar {
    pub value: i32,
    pub max: i32,
    pub indeterminate: bool,
}

impl RowView {
    pub fn from_item(item: &AnimeDownload) -> Self {
        Self {
            episode_id: item.episode.id,
            anime_id: item.anime.id,
            anime_title: item.anime.title.clone(),
            episode_name: item.episode.name.clone(),
            status: item.status(),
            progress: item.progress.get(),
            has_video: item.video().is_some(),
        }
    }

    /// Resolution is running while the item is DOWNLOADING and no video is known yet.
    pub fn resolving(&self) -> bool {
        self.status == DownloadState::Downloading && !self.has_video
    }

    /// The bar: empty with no video, else the percent out of 100 and indeterminate at 0.
    pub fn bar(&self) -> Bar {
        if !self.has_video {
            return Bar { value: 0, max: 1, indeterminate: false };
        }
        Bar { value: self.progress, max: 100, indeterminate: self.progress == 0 }
    }

    /// The text beside the bar: "Looking for video…" while resolving, "" before a video, "Downloading…" at 0, else N%.
    pub fn text(&self) -> String {
        if self.resolving() {
            LOOKING_FOR_VIDEO_TEXT.to_owned()
        } else if !self.has_video {
            String::new()
        } else if self.progress == 0 {
            DOWNLOADING_TEXT.to_owned()
        } else {
            format!("{}%", self.progress)
        }
    }
}

/// One source group: the header and its rows, in queue order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub source_id: i64,
    pub name: String,
    pub rows: Vec<RowView>,
}

impl Group {
    /// `"<source name> (<count>)"`.
    pub fn header(&self) -> String {
        format!("{} ({})", self.name, self.rows.len())
    }
}

/// The groups in order of their first row. Items keep queue order inside a group.
pub fn groups(items: &[Arc<AnimeDownload>]) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for item in items {
        let row = RowView::from_item(item);
        match out.iter_mut().find(|g| g.source_id == item.source.id) {
            Some(group) => group.rows.push(row),
            None => out.push(Group { source_id: item.source.id, name: item.source.name.clone(), rows: vec![row] }),
        }
    }
    out
}

/// The count on the tab: every queued item, whatever its state.
pub fn tab_count(items: &[Arc<AnimeDownload>]) -> usize {
    items.len()
}

/// The entries of the item menu, in their order (ui-parity 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    MoveToTop,
    MoveToTopSeries,
    MoveToBottom,
    MoveToBottomSeries,
    Cancel,
    CancelSeries,
}

impl MenuAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::MoveToTop => "Move to top",
            Self::MoveToTopSeries => "Move series to top",
            Self::MoveToBottom => "Move to bottom",
            Self::MoveToBottomSeries => "Move series to bottom",
            Self::Cancel => "Cancel",
            Self::CancelSeries => "Cancel all for this series",
        }
    }
}

/// The menu for one item. "Move to top" is hidden for the first item of its own group, and "Move to bottom" for the
/// last (E-U07). The series entries and the cancel entries are always shown. An unknown id has no menu.
pub fn menu_for(items: &[Arc<AnimeDownload>], episode_id: i64) -> Vec<MenuAction> {
    let Some(item) = items.iter().find(|d| d.episode.id == episode_id) else {
        return Vec::new();
    };
    let group: Vec<i64> = items.iter().filter(|d| d.source.id == item.source.id).map(|d| d.episode.id).collect();
    let first = group.first() == Some(&episode_id);
    let last = group.last() == Some(&episode_id);
    let mut menu = Vec::new();
    if !first {
        menu.push(MenuAction::MoveToTop);
    }
    menu.push(MenuAction::MoveToTopSeries);
    if !last {
        menu.push(MenuAction::MoveToBottom);
    }
    menu.push(MenuAction::MoveToBottomSeries);
    menu.push(MenuAction::Cancel);
    menu.push(MenuAction::CancelSeries);
    menu
}

/// What the queue must do for a menu entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Send `Reorder` with these episode ids. Every item is listed here; the queue appends unlisted ones anyway (E-15).
    Reorder(Vec<i64>),
    /// Send `Cancel` with these episode ids.
    Cancel(Vec<i64>),
    Nothing,
}

/// Source groups as `(source id, episode ids in order)`, the order the page shows them in.
fn group_ids(items: &[Arc<AnimeDownload>]) -> Vec<(i64, Vec<i64>)> {
    let mut out: Vec<(i64, Vec<i64>)> = Vec::new();
    for item in items {
        match out.iter_mut().find(|(source, _)| *source == item.source.id) {
            Some((_, ids)) => ids.push(item.episode.id),
            None => out.push((item.source.id, vec![item.episode.id])),
        }
    }
    out
}

/// The flat list of the groups, in header order.
fn flatten(groups: &[(i64, Vec<i64>)]) -> Vec<i64> {
    groups.iter().flat_map(|(_, ids)| ids.iter().copied()).collect()
}

/// The ids of every item of the same anime as `episode_id`, in queue order.
fn series_ids(items: &[Arc<AnimeDownload>], episode_id: i64) -> Vec<i64> {
    let Some(anime) = items.iter().find(|d| d.episode.id == episode_id).map(|d| d.anime.id) else {
        return Vec::new();
    };
    items.iter().filter(|d| d.anime.id == anime).map(|d| d.episode.id).collect()
}

/// The plan for a menu entry. Single moves stay inside the group. Series moves cross the groups: the series is
/// pulled out of them, as in the Kotlin code (ui-parity 1.6).
pub fn plan(items: &[Arc<AnimeDownload>], episode_id: i64, action: MenuAction) -> Plan {
    match action {
        MenuAction::Cancel => Plan::Cancel(vec![episode_id]),
        MenuAction::CancelSeries => {
            let ids = series_ids(items, episode_id);
            if ids.is_empty() { Plan::Nothing } else { Plan::Cancel(ids) }
        }
        MenuAction::MoveToTop | MenuAction::MoveToBottom => {
            let mut groups = group_ids(items);
            let Some((_, ids)) = groups.iter_mut().find(|(_, ids)| ids.contains(&episode_id)) else {
                return Plan::Nothing;
            };
            ids.retain(|id| *id != episode_id);
            if action == MenuAction::MoveToTop {
                ids.insert(0, episode_id);
            } else {
                ids.push(episode_id);
            }
            Plan::Reorder(flatten(&groups))
        }
        MenuAction::MoveToTopSeries | MenuAction::MoveToBottomSeries => {
            let selected = series_ids(items, episode_id);
            if selected.is_empty() {
                return Plan::Nothing;
            }
            let others: Vec<i64> = items.iter().map(|d| d.episode.id).filter(|id| !selected.contains(id)).collect();
            let order = if action == MenuAction::MoveToTopSeries {
                [selected, others].concat()
            } else {
                [others, selected].concat()
            };
            Plan::Reorder(order)
        }
    }
}

/// `shouldMove`: a drag may only land inside the source group of the dragged item.
pub fn same_group(items: &[Arc<AnimeDownload>], a: i64, b: i64) -> bool {
    let source_of = |id: i64| items.iter().find(|d| d.episode.id == id).map(|d| d.source.id);
    matches!((source_of(a), source_of(b)), (Some(x), Some(y)) if x == y)
}

/// A row dragged onto `target` inside its own group. The dragged row takes the target's place in that group, and the
/// result is the flat list across groups. `None` when the drop is rejected (the target is in another group) or lands on
/// the row's own position: then no reorder is sent (T-25a-028).
pub fn drag_within_group(items: &[Arc<AnimeDownload>], dragged: i64, target: i64) -> Option<Vec<i64>> {
    if !same_group(items, dragged, target) {
        return None;
    }
    let mut groups = group_ids(items);
    let (_, ids) = groups.iter_mut().find(|(_, ids)| ids.contains(&dragged))?;
    let from = ids.iter().position(|id| *id == dragged)?;
    let to = ids.iter().position(|id| *id == target)?;
    if from == to {
        return None;
    }
    let moved = ids.remove(from);
    ids.insert(to, moved);
    Some(flatten(&groups))
}

/// A whole group dragged to header position `to_index`. Returns the flat list across groups in the new header order.
pub fn drag_group(items: &[Arc<AnimeDownload>], source_id: i64, to_index: usize) -> Vec<i64> {
    let mut groups = group_ids(items);
    let Some(from) = groups.iter().position(|(source, _)| *source == source_id) else {
        return flatten(&groups);
    };
    let moved = groups.remove(from);
    let at = to_index.min(groups.len());
    groups.insert(at, moved);
    flatten(&groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::model::{AnimeRef, AnimeSource, EpisodeRef, Video};

    fn source(id: i64, name: &str) -> AnimeSource {
        AnimeSource { id, name: name.into(), lang: "en".into(), is_http: true, unmetered: false }
    }

    /// `(episode id, source id, anime id)` triples to queue items.
    fn queue(spec: &[(i64, i64, i64)]) -> Vec<Arc<AnimeDownload>> {
        spec.iter()
            .map(|&(episode, source_id, anime_id)| {
                Arc::new(AnimeDownload::new(
                    source(source_id, &format!("Source {source_id}")),
                    AnimeRef { id: anime_id, source_id, url: format!("/a/{anime_id}"), title: format!("Anime {anime_id}") },
                    EpisodeRef {
                        id: episode,
                        anime_id,
                        url: String::new(),
                        name: format!("Episode {episode}"),
                        scanlator: None,
                        seen: false,
                        bookmark: false,
                        fillermark: false,
                        source_order: episode,
                        upload_date: 0,
                        episode_number: 0.0,
                    },
                    false,
                    None,
                ))
            })
            .collect()
    }

    #[test]
    fn groups_follow_first_appearance_and_keep_queue_order() {
        // Source 1 appears first, source 2 second, then source 1 again.
        let items = queue(&[(1, 1, 10), (2, 2, 20), (3, 1, 10)]);
        let grouped = groups(&items);
        assert_eq!(grouped.iter().map(|g| g.source_id).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(grouped[0].header(), "Source 1 (2)");
        assert_eq!(grouped[0].rows.iter().map(|r| r.episode_id).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(grouped[1].header(), "Source 2 (1)");
        assert_eq!(tab_count(&items), 3, "the tab counts every queued item");
        assert_eq!(EMPTY_TEXT, "No downloads");
    }

    #[test]
    fn row_text_and_bar_follow_the_table_in_ui_parity_1_5() {
        let items = queue(&[(1, 1, 10)]);
        let row = |status, progress, video: bool| RowView {
            status,
            progress,
            has_video: video,
            ..RowView::from_item(&items[0])
        };
        assert_eq!(row(DownloadState::Queue, 0, false).text(), "");
        assert_eq!(row(DownloadState::Queue, 0, false).bar(), Bar { value: 0, max: 1, indeterminate: false });
        assert_eq!(row(DownloadState::Downloading, 0, false).text(), LOOKING_FOR_VIDEO_TEXT);
        assert_eq!(row(DownloadState::Downloading, 0, true).text(), "Downloading…");
        assert_eq!(row(DownloadState::Downloading, 0, true).bar(), Bar { value: 0, max: 100, indeterminate: true });
        assert_eq!(row(DownloadState::Downloading, 42, true).text(), "42%");
        assert_eq!(row(DownloadState::Downloading, 42, true).bar(), Bar { value: 42, max: 100, indeterminate: false });
    }

    #[test]
    fn a_row_reads_its_progress_from_the_live_cell() {
        let items = queue(&[(1, 1, 10)]);
        items[0].set_video(Video { url: "https://x".into(), ..Default::default() });
        items[0].progress.set(65);
        let row = RowView::from_item(&items[0]);
        assert!(row.has_video);
        assert_eq!(row.text(), "65%");
    }

    #[test]
    fn move_to_top_is_hidden_for_the_first_item_of_its_own_group() {
        // Episode 1 is first in source 1, even though episode 3 is the first overall row of the list.
        let items = queue(&[(3, 2, 20), (1, 1, 10), (2, 1, 10)]);
        assert!(!menu_for(&items, 1).contains(&MenuAction::MoveToTop), "first in its group");
        assert!(menu_for(&items, 2).contains(&MenuAction::MoveToTop));
        assert!(!menu_for(&items, 2).contains(&MenuAction::MoveToBottom), "last in its group");
        assert!(!menu_for(&items, 3).contains(&MenuAction::MoveToBottom), "the only item of its group");
        assert!(menu_for(&items, 3).contains(&MenuAction::MoveToTopSeries), "series entries always show");
        assert_eq!(menu_for(&items, 99), Vec::new());
    }

    #[test]
    fn the_menu_order_matches_the_page() {
        let items = queue(&[(1, 1, 10), (2, 1, 10), (3, 1, 10)]);
        assert_eq!(
            menu_for(&items, 2),
            vec![
                MenuAction::MoveToTop,
                MenuAction::MoveToTopSeries,
                MenuAction::MoveToBottom,
                MenuAction::MoveToBottomSeries,
                MenuAction::Cancel,
                MenuAction::CancelSeries,
            ]
        );
        assert_eq!(MenuAction::CancelSeries.label(), "Cancel all for this series");
    }

    #[test]
    fn single_moves_stay_in_the_group_and_the_list_is_flattened_by_group() {
        // Queue: A1, B1, A2. Moving A2 to the top of its group gives A2, A1 inside group A.
        let items = queue(&[(1, 1, 10), (2, 2, 20), (3, 1, 10)]);
        assert_eq!(plan(&items, 3, MenuAction::MoveToTop), Plan::Reorder(vec![3, 1, 2]));
        assert_eq!(plan(&items, 1, MenuAction::MoveToBottom), Plan::Reorder(vec![3, 1, 2]));
        assert_eq!(plan(&items, 1, MenuAction::Cancel), Plan::Cancel(vec![1]));
    }

    #[test]
    fn series_moves_pull_the_series_out_of_the_groups() {
        // Anime 10 owns episodes 1 and 3. Anime 20 owns episode 2.
        let items = queue(&[(1, 1, 10), (2, 2, 20), (3, 1, 10)]);
        assert_eq!(plan(&items, 3, MenuAction::MoveToTopSeries), Plan::Reorder(vec![1, 3, 2]));
        assert_eq!(plan(&items, 1, MenuAction::MoveToBottomSeries), Plan::Reorder(vec![2, 1, 3]));
        assert_eq!(plan(&items, 2, MenuAction::CancelSeries), Plan::Cancel(vec![2]));
        assert_eq!(plan(&items, 1, MenuAction::CancelSeries), Plan::Cancel(vec![1, 3]));
        assert_eq!(plan(&items, 404, MenuAction::MoveToTop), Plan::Nothing);
    }

    #[test]
    fn a_drag_is_rejected_across_groups() {
        let items = queue(&[(1, 1, 10), (2, 2, 20), (3, 1, 10)]);
        assert!(same_group(&items, 1, 3));
        assert!(!same_group(&items, 1, 2));
        assert_eq!(drag_within_group(&items, 1, 2), None);
    }

    #[test]
    fn a_drag_inside_a_group_moves_the_row_to_the_target_place() {
        let items = queue(&[(1, 1, 10), (2, 2, 20), (3, 1, 10), (4, 1, 10)]);
        // Group 1 is [1, 3, 4]. Drag 4 onto 1: group becomes [4, 1, 3]. Flat: 4, 1, 3, 2.
        assert_eq!(drag_within_group(&items, 4, 1), Some(vec![4, 1, 3, 2]));
    }

    #[test]
    fn a_group_drag_reorders_the_headers() {
        let items = queue(&[(1, 1, 10), (2, 2, 20), (3, 1, 10)]);
        assert_eq!(drag_group(&items, 2, 0), vec![2, 1, 3]);
        assert_eq!(drag_group(&items, 1, 9), vec![2, 1, 3], "an index past the end is the last place");
    }

    fn sortable(id: i64, source_id: i64, date: i64, number: f64) -> Arc<AnimeDownload> {
        let mut d = queue(&[(id, source_id, 1)]).remove(0);
        let inner = Arc::get_mut(&mut d).unwrap();
        inner.episode.upload_date = date;
        inner.episode.episode_number = number;
        d
    }

    #[test]
    fn sort_keeps_groups_and_sorts_inside_each_group() {
        let items = vec![
            sortable(1, 10, 300, 2.0),
            sortable(2, 20, 100, 9.0),
            sortable(3, 10, 200, 1.0),
            sortable(4, 20, 500, 3.0),
        ];
        assert_eq!(sorted_order(&items, SortKey::UploadDate, false), vec![3, 1, 2, 4], "oldest first inside groups");
        assert_eq!(sorted_order(&items, SortKey::UploadDate, true), vec![1, 3, 4, 2], "newest first inside groups");
        assert_eq!(sorted_order(&items, SortKey::EpisodeNumber, false), vec![3, 1, 4, 2]);
        assert_eq!(sorted_order(&items, SortKey::EpisodeNumber, true), vec![1, 3, 2, 4]);
        assert!(sorted_order(&[], SortKey::UploadDate, false).is_empty());
    }

    #[test]
    fn footer_action_shows_stop_while_running_and_continue_when_stopped() {
        assert_eq!(footer_action(RunnerState::Running, 2), Some(FooterAction::Stop));
        assert_eq!(footer_action(RunnerState::Paused, 2), Some(FooterAction::Continue));
        assert_eq!(footer_action(RunnerState::Stopped, 1), Some(FooterAction::Continue));
        assert_eq!(footer_action(RunnerState::Running, 0), None, "hidden on an empty queue");
        assert_eq!(FooterAction::Stop.label(), "Stop");
        assert_eq!(FooterAction::Continue.label(), "Continue");
        assert_eq!(EMPTY_TEXT, "No downloads");
    }

    #[test]
    fn rebuilds_are_limited_to_one_per_200_ms() {
        let base = Instant::now();
        let mut gate = RebuildGate::default();
        assert!(gate.ready(base));
        assert!(!gate.ready(base + Duration::from_millis(199)));
        assert!(gate.ready(base + Duration::from_millis(200)));
        assert!(!gate.ready(base + Duration::from_millis(250)));
    }

    #[test]
    fn a_status_subscriber_sees_changes_but_not_the_initial_value() {
        let item = queue(&[(1, 10, 1)]).remove(0);
        let mut rx = item.subscribe_status();
        assert!(rx.has_changed().is_ok_and(|changed| !changed), "the first value is not a change");
        item.set_status(DownloadState::Downloading);
        assert!(rx.has_changed().unwrap());
        rx.mark_unchanged();
        item.set_status(DownloadState::Downloading);
        assert!(!rx.has_changed().unwrap(), "setting the same status is not a change");
    }
}
