//! Pure rules of the Download queue page, shared by the anime and manga pages (T-25a-021 to T-25a-025, T-25a-027,
//! T-25a-029, T-25a-030, T-25a-037, T-25a-038). No GTK: the page builds its widgets from these values.

use std::time::{Duration, Instant};

use tokio::sync::broadcast;

use crate::events::DownloadEvent;
use crate::model::DownloadState;
use crate::queue_model::{EntryKind, QueueItem, QueueModel, Section, SortKey};
use crate::throttle::{PROGRESS_INTERVAL, ProgressThrottle};

/// Title of the page, and of the sidebar destination at position [`SIDEBAR_INDEX`].
pub const PAGE_TITLE: &str = "Download queue";
pub const SIDEBAR_INDEX: u32 = 6;

/// Pager order: Anime first, then Manga.
pub const PAGE_ORDER: [EntryKind; 2] = [EntryKind::Anime, EntryKind::Manga];

/// The text of the empty state (both pages).
pub const EMPTY_TEXT: &str = crate::view_model::EMPTY_TEXT;

/// Text of an anime row at 0 percent, and of a manga row that is downloading with no page list yet.
pub const DOWNLOADING_TEXT: &str = "Downloading…";

/// The FAB fades in and out over this time (T-25a-040).
pub const FAB_FADE: Duration = Duration::from_millis(200);
/// The FAB label slides away over this time (proposal, T-25a-040).
pub const FAB_LABEL_SLIDE: Duration = Duration::from_millis(150);
/// Below this content width the FAB is icon-only (T-25a-037).
pub const NARROW_WIDTH: i32 = 480;

/// Manga rows poll for the page list at this interval, and give up after [`PAGES_POLL_TICKS`] ticks (T-25a-029).
pub const PAGES_POLL_INTERVAL: Duration = Duration::from_millis(50);
pub const PAGES_POLL_TICKS: u32 = 600;

pub fn tab_label(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Anime => "Anime",
        EntryKind::Manga => "Manga",
    }
}

/// The title count pill shows the anime count only, and only above zero (E-26, T-E26).
pub fn title_pill(anime_count: usize) -> Option<String> {
    (anime_count > 0).then(|| anime_count.to_string())
}

/// Tab badge: the page's total item count. Hidden at zero (proposal E-14, Kotlin shows "0").
pub fn tab_badge(count: usize) -> Option<String> {
    (count > 0).then(|| count.to_string())
}

/// Accessible name of a tab, `"Anime, N downloads"` (T-25a-038, proposal).
pub fn tab_accessible_name(kind: EntryKind, count: usize) -> String {
    format!("{}, {count} downloads", tab_label(kind))
}

/// The page has a list to show. Empty state, FAB, sort menu and Cancel all all depend on this (E-27, E-35).
pub fn has_rows(list_len: usize) -> bool {
    list_len > 0
}

/// The empty state replaces the list when there are no rows (T-25a-030). Before the first snapshot it is the same.
pub fn show_empty_state(list_len: usize) -> bool {
    !has_rows(list_len)
}

/// What the FAB does when clicked (T-25a-023): anime stops the run, manga pauses it, and a stopped queue starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FabAction {
    Stop,
    PauseAndStop,
    Start,
}

/// The start/pause button of a page. `expanded` is false while the content scrolls up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fab {
    pub kind: EntryKind,
    pub running: bool,
    pub expanded: bool,
}

impl Fab {
    /// Starts expanded (DownloadsTab.kt:111-132).
    pub fn new(kind: EntryKind, running: bool) -> Self {
        Self { kind, running, expanded: true }
    }

    /// Anime running "Stop", anime stopped "Continue", manga running "Pause", manga stopped "Resume" (E-28).
    pub fn label(&self) -> &'static str {
        match (self.kind, self.running) {
            (EntryKind::Anime, true) => "Stop",
            (EntryKind::Anime, false) => "Continue",
            (EntryKind::Manga, true) => "Pause",
            (EntryKind::Manga, false) => "Resume",
        }
    }

    pub fn icon(&self) -> &'static str {
        if self.running { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }
    }

    pub fn click(&self) -> FabAction {
        match (self.running, self.kind) {
            (false, _) => FabAction::Start,
            (true, EntryKind::Anime) => FabAction::Stop,
            (true, EntryKind::Manga) => FabAction::PauseAndStop,
        }
    }

    /// Scroll controller rule: `dy > 0` (content moves up) collapses to icon-only, `dy <= 0` expands.
    pub fn on_scroll(&mut self, dy: f64) {
        self.expanded = dy <= 0.0;
    }

    /// The label is shown when expanded and the content is wide enough (T-25a-037).
    pub fn show_label(&self, content_width: i32) -> bool {
        self.expanded && content_width >= NARROW_WIDTH
    }
}

/// The FAB is visible iff the page's list is not empty (E-27).
pub fn fab_visible(list_len: usize) -> bool {
    has_rows(list_len)
}

/// One leaf of the sort menu. Two submenus, four leaves (T-25a-024).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortLeaf {
    UploadNewest,
    UploadOldest,
    NumberAscending,
    NumberDescending,
}

impl SortLeaf {
    pub const ALL: [SortLeaf; 4] =
        [SortLeaf::UploadNewest, SortLeaf::UploadOldest, SortLeaf::NumberAscending, SortLeaf::NumberDescending];

    /// The submenu this leaf sits in: "By upload date", or "By episode number" / "By chapter number".
    pub fn submenu(self, kind: EntryKind) -> &'static str {
        match self {
            SortLeaf::UploadNewest | SortLeaf::UploadOldest => "By upload date",
            SortLeaf::NumberAscending | SortLeaf::NumberDescending => match kind {
                EntryKind::Anime => "By episode number",
                EntryKind::Manga => "By chapter number",
            },
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortLeaf::UploadNewest => "Newest",
            SortLeaf::UploadOldest => "Oldest",
            SortLeaf::NumberAscending => "Ascending",
            SortLeaf::NumberDescending => "Descending",
        }
    }

    /// The key and `reverse` flag that the queue model sorts with. Newest is upload date descending.
    pub fn order(self) -> (SortKey, bool) {
        match self {
            SortLeaf::UploadNewest => (SortKey::UploadDate, true),
            SortLeaf::UploadOldest => (SortKey::UploadDate, false),
            SortLeaf::NumberAscending => (SortKey::Number, false),
            SortLeaf::NumberDescending => (SortKey::Number, true),
        }
    }

    pub fn action_name(self) -> &'static str {
        match self {
            SortLeaf::UploadNewest => "queue.sort-upload-desc",
            SortLeaf::UploadOldest => "queue.sort-upload-asc",
            SortLeaf::NumberAscending => "queue.sort-number-asc",
            SortLeaf::NumberDescending => "queue.sort-number-desc",
        }
    }
}

/// The sort menu is hidden when the list is empty (DownloadsTab.kt:299, 383; E-35). Empty result means hidden.
pub fn sort_menu(list_len: usize) -> Vec<SortLeaf> {
    if has_rows(list_len) { SortLeaf::ALL.to_vec() } else { Vec::new() }
}

/// "Cancel all" in the overflow menu: shown only with rows. It has no confirmation (matches Kotlin).
pub fn cancel_all_visible(list_len: usize) -> bool {
    has_rows(list_len)
}

/// The row menu entries, in order (T-25a-027).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowAction {
    MoveTop,
    MoveSeriesTop,
    MoveBottom,
    MoveSeriesBottom,
    Cancel,
    CancelSeries,
}

impl RowAction {
    pub fn label(self) -> &'static str {
        match self {
            RowAction::MoveTop => "Move to top",
            RowAction::MoveSeriesTop => "Move series to top",
            RowAction::MoveBottom => "Move to bottom",
            RowAction::MoveSeriesBottom => "Move series to bottom",
            RowAction::Cancel => "Cancel",
            RowAction::CancelSeries => "Cancel all for this series",
        }
    }

    pub fn action_name(self) -> &'static str {
        match self {
            RowAction::MoveTop => "row.move-top",
            RowAction::MoveSeriesTop => "row.move-top-series",
            RowAction::MoveBottom => "row.move-bottom",
            RowAction::MoveSeriesBottom => "row.move-bottom-series",
            RowAction::Cancel => "row.cancel",
            RowAction::CancelSeries => "row.cancel-series",
        }
    }
}

/// The row menu for the row at flat `index` in a queue of `len` rows. "Move to top" needs `index > 0` and "Move to
/// bottom" needs the row not to be last (E-10, T-E10). The menu is built on open, so it follows the current snapshot.
pub fn row_menu(index: usize, len: usize) -> Vec<RowAction> {
    let mut menu = Vec::with_capacity(6);
    if index > 0 {
        menu.push(RowAction::MoveTop);
    }
    menu.push(RowAction::MoveSeriesTop);
    if index + 1 < len {
        menu.push(RowAction::MoveBottom);
    }
    menu.push(RowAction::MoveSeriesBottom);
    menu.push(RowAction::Cancel);
    menu.push(RowAction::CancelSeries);
    menu
}

/// Keyboard on a focused row (proposal, T-25a-026). `alt` with Up or Down moves the row, and Delete cancels it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKey {
    MoveUp,
    MoveDown,
    Cancel,
}

/// `key` is the GDK keyval name. Returns `None` for keys the page does not handle.
pub fn row_key(alt: bool, key: &str) -> Option<RowKey> {
    match (alt, key) {
        (true, "Up") => Some(RowKey::MoveUp),
        (true, "Down") => Some(RowKey::MoveDown),
        (false, "Delete") => Some(RowKey::Cancel),
        _ => None,
    }
}

/// Row text for an anime episode: "Downloading…" at 0 percent, otherwise "NN%" (T-25a-029).
pub fn anime_progress_text(percent: i64) -> String {
    if percent <= 0 { DOWNLOADING_TEXT.to_owned() } else { format!("{percent}%") }
}

/// Row text for a manga chapter: "done/total", empty while the page count is unknown (T-25a-029).
pub fn manga_progress_text(done: u32, total: Option<u32>) -> String {
    match total {
        Some(total) if total > 0 => format!("{done}/{total}"),
        _ => String::new(),
    }
}

/// Progress bar fraction for a row. `None` is indeterminate: anime at 0 percent, and manga with no page list.
pub fn bar_fraction(item: &QueueItem) -> Option<f64> {
    match item.kind {
        EntryKind::Anime => (item.progress > 0).then(|| (item.progress.min(100) as f64) / 100.0),
        EntryKind::Manga => match item.pages_total {
            Some(total) if total > 0 => Some((item.progress as f64) / (f64::from(total) * 100.0)),
            _ => None,
        },
    }
}

/// The row text of one item: the progress text for its kind.
pub fn row_progress_text(item: &QueueItem) -> String {
    match item.kind {
        EntryKind::Anime => anime_progress_text(item.progress),
        EntryKind::Manga => manga_progress_text(item.pages_done, item.pages_total),
    }
}

/// Counts for the tab badges of one page: the sum of the sub-item counts per section, which is the number of rows.
pub fn tab_count(model: &QueueModel) -> usize {
    model.len()
}

/// What a row's progress does for its state (T-25a-029, E-31). A downloading row shows progress live. A downloaded row
/// ends it with one final update. ERROR, queued and not-downloaded rows show no progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressPhase {
    Live,
    FinalUpdate,
    Stopped,
}

pub fn progress_phase(state: DownloadState) -> ProgressPhase {
    match state {
        DownloadState::Downloading => ProgressPhase::Live,
        DownloadState::Downloaded => ProgressPhase::FinalUpdate,
        _ => ProgressPhase::Stopped,
    }
}

/// Decides which progress updates of a page repaint a row (T-25a-029, E-31, E-32). One gate per page: the anime page
/// feeds it episode ids and the manga page chapter ids. A downloading row repaints at most once per
/// [`PROGRESS_INTERVAL`], a downloaded row gets one final update and its gate ends, and any other row is not repainted.
pub struct RowProgressGate {
    throttle: ProgressThrottle,
}

impl Default for RowProgressGate {
    fn default() -> Self {
        Self { throttle: ProgressThrottle::new(PROGRESS_INTERVAL) }
    }
}

impl RowProgressGate {
    /// `true` when the update for row `id` in `state` at `now` should be drawn.
    pub fn allow(&mut self, id: i64, state: DownloadState, now: Instant) -> bool {
        match progress_phase(state) {
            ProgressPhase::Live => self.throttle.allow(id, now),
            ProgressPhase::FinalUpdate => {
                self.throttle.forget(id);
                true
            }
            ProgressPhase::Stopped => {
                self.throttle.forget(id);
                false
            }
        }
    }
}

/// What the row sees on one look for a manga page list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagesCheck {
    /// The page list is known: the row shows `done/total`.
    Known,
    /// Still unknown: poll again.
    Waiting,
    /// The row left the queue or is in ERROR: stop without a warning (E-31).
    Gone,
}

/// The poll loop of a manga row whose page list is not known yet (T-25a-029, E-13). It looks every
/// [`PAGES_POLL_INTERVAL`] and returns `true` once the pages are known. After [`PAGES_POLL_TICKS`] ticks (30 s) it logs a
/// warning and returns `false`, and a row that is gone or in ERROR ends it quietly with `false`.
pub async fn poll_for_pages(chapter_id: i64, mut check: impl FnMut() -> PagesCheck) -> bool {
    let mut poll = PagesPoll::default();
    loop {
        match check() {
            PagesCheck::Known => return true,
            PagesCheck::Gone => return false,
            PagesCheck::Waiting => {}
        }
        if poll.tick() == PollStep::GiveUp {
            tracing::warn!(target: "kuro_downloads", chapter_id, "page list did not appear in 30 s, row polling stopped");
            return false;
        }
        tokio::time::sleep(PAGES_POLL_INTERVAL).await;
    }
}

/// The queue order to send to the engine after a sort (T-25a-024). The sort is per section and the flat order comes back.
pub fn sorted_order(model: &QueueModel, leaf: SortLeaf) -> Vec<i64> {
    let (key, reverse) = leaf.order();
    let mut sorted = model.clone();
    sorted.sort_within_sections(key, reverse);
    sorted.item_ids()
}

/// The header of one source section, `"Source name (count)"` (T-25a-026, proposal).
pub fn section_header(section: &Section) -> String {
    format!("{} ({})", section.source_name, section.items.len())
}

/// Row title and subtitle: the episode or chapter name, then the entry title (T-25a-026).
pub fn row_titles(item: &QueueItem) -> (&str, &str) {
    (&item.item_name, &item.title)
}

/// Counts pages polled for a manga row whose page list is not known yet (T-25a-029, proposal).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PagesPoll {
    ticks: u32,
}

/// What the poll loop does after one tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollStep {
    Continue,
    /// The page list did not appear in time. The caller logs a warning and stops polling.
    GiveUp,
}

impl PagesPoll {
    pub fn tick(&mut self) -> PollStep {
        self.ticks += 1;
        if self.ticks >= PAGES_POLL_TICKS { PollStep::GiveUp } else { PollStep::Continue }
    }
}

/// One kind's pane: its queue model and its single event feed.
#[derive(Default)]
struct Pane {
    model: QueueModel,
    feed: Option<broadcast::Receiver<DownloadEvent>>,
}

/// The Download page owns exactly one [`QueueModel`] per kind and at most one engine subscription per kind (T-25a-032;
/// Kotlin creates two models, E-36). Opening a kind that is already open keeps its model and its subscription.
#[derive(Default)]
pub struct DownloadPage {
    anime: Pane,
    manga: Pane,
}

impl DownloadPage {
    fn pane(&self, kind: EntryKind) -> &Pane {
        match kind {
            EntryKind::Anime => &self.anime,
            EntryKind::Manga => &self.manga,
        }
    }

    fn pane_mut(&mut self, kind: EntryKind) -> &mut Pane {
        match kind {
            EntryKind::Anime => &mut self.anime,
            EntryKind::Manga => &mut self.manga,
        }
    }

    /// Subscribes `kind` to the engine through `subscribe`, once. A second open does not call it again.
    pub fn open(&mut self, kind: EntryKind, subscribe: impl FnOnce() -> broadcast::Receiver<DownloadEvent>) {
        let pane = self.pane_mut(kind);
        if pane.feed.is_none() {
            pane.feed = Some(subscribe());
        }
    }

    /// Drops the subscription of `kind`. The model stays, so the next open shows the last queue at once.
    pub fn close(&mut self, kind: EntryKind) {
        self.pane_mut(kind).feed = None;
    }

    pub fn is_open(&self, kind: EntryKind) -> bool {
        self.pane(kind).feed.is_some()
    }

    pub fn model(&self, kind: EntryKind) -> &QueueModel {
        &self.pane(kind).model
    }

    pub fn model_mut(&mut self, kind: EntryKind) -> &mut QueueModel {
        &mut self.pane_mut(kind).model
    }

    /// The event feed of `kind`, `None` while the page is closed for it.
    pub fn feed(&mut self, kind: EntryKind) -> Option<&mut broadcast::Receiver<DownloadEvent>> {
        self.pane_mut(kind).feed.as_mut()
    }
}

/// The FAB fade and the label slide times (T-25a-040). With the GTK reduced-motion setting on, both are zero.
pub fn fab_motion(reduced_motion: bool) -> (Duration, Duration) {
    if reduced_motion { (Duration::ZERO, Duration::ZERO) } else { (FAB_FADE, FAB_LABEL_SLIDE) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DownloadState;

    #[test]
    fn opening_the_page_twice_keeps_one_subscriber_and_one_model_per_kind() {
        let (anime_tx, _anime_rx) = broadcast::channel::<DownloadEvent>(8);
        let (manga_tx, _manga_rx) = broadcast::channel::<DownloadEvent>(8);
        let base_anime = anime_tx.receiver_count();
        let base_manga = manga_tx.receiver_count();
        let mut page = DownloadPage::default();
        for _ in 0..2 {
            page.open(EntryKind::Anime, || anime_tx.subscribe());
            page.open(EntryKind::Manga, || manga_tx.subscribe());
        }
        assert_eq!(anime_tx.receiver_count(), base_anime + 1, "one anime subscriber");
        assert_eq!(manga_tx.receiver_count(), base_manga + 1, "one manga subscriber");

        // The model survives a second open, and a close drops only the subscription.
        let item = row(EntryKind::Anime, 0, 0, None);
        *page.model_mut(EntryKind::Anime) = QueueModel::from_flat(vec![item]);
        page.open(EntryKind::Anime, || anime_tx.subscribe());
        assert_eq!(page.model(EntryKind::Anime).len(), 1);
        assert!(page.model(EntryKind::Manga).is_empty(), "the kinds do not share a model");
        page.close(EntryKind::Anime);
        assert!(!page.is_open(EntryKind::Anime));
        assert_eq!(anime_tx.receiver_count(), base_anime);
        assert_eq!(page.model(EntryKind::Anime).len(), 1);
        assert_eq!(manga_tx.receiver_count(), base_manga + 1, "the other kind is untouched");
    }

    #[test]
    fn reduced_motion_turns_both_fab_animations_off() {
        assert_eq!(fab_motion(false), (Duration::from_millis(200), Duration::from_millis(150)));
        assert_eq!(fab_motion(true), (Duration::ZERO, Duration::ZERO));
    }

    fn row(kind: EntryKind, progress: i64, pages_done: u32, pages_total: Option<u32>) -> QueueItem {
        QueueItem {
            kind,
            item_id: 1,
            parent_id: 1,
            source_id: 1,
            source_name: "S".into(),
            title: "T".into(),
            item_name: "N".into(),
            date_upload: 0,
            number: 1.0,
            state: DownloadState::Downloading,
            pages_done,
            pages_total,
            progress,
        }
    }

    #[test]
    fn title_pill_is_the_anime_count_above_zero_only() {
        assert_eq!(title_pill(0), None);
        assert_eq!(title_pill(3), Some("3".to_owned()), "E-26: the caller passes the anime count only");
    }

    #[test]
    fn tab_badge_is_hidden_at_zero_and_names_the_tab_for_accessibility() {
        assert_eq!(tab_badge(0), None, "T-E14: hidden at 0");
        assert_eq!(tab_badge(12), Some("12".to_owned()));
        assert_eq!(tab_accessible_name(EntryKind::Anime, 2), "Anime, 2 downloads");
        assert_eq!(tab_accessible_name(EntryKind::Manga, 0), "Manga, 0 downloads");
    }

    #[test]
    fn pages_and_order_match_the_spec() {
        assert_eq!(tab_label(PAGE_ORDER[0]), "Anime");
        assert_eq!(tab_label(PAGE_ORDER[1]), "Manga");
        assert_eq!(SIDEBAR_INDEX, 6);
        assert_eq!(PAGE_TITLE, "Download queue");
    }

    #[test]
    fn empty_page_shows_the_message_and_hides_the_actions() {
        assert!(show_empty_state(0));
        assert_eq!(EMPTY_TEXT, "No downloads");
        assert!(!fab_visible(0), "E-27: no FAB without rows");
        assert!(sort_menu(0).is_empty(), "E-35: no sort menu without rows");
        assert!(!cancel_all_visible(0));
        assert!(fab_visible(1) && cancel_all_visible(1) && !show_empty_state(1));
    }

    #[test]
    fn fab_label_icon_and_click_follow_the_running_state() {
        let anime = Fab::new(EntryKind::Anime, true);
        assert_eq!((anime.label(), anime.icon(), anime.click()), ("Stop", "media-playback-pause-symbolic", FabAction::Stop));
        assert_eq!(Fab::new(EntryKind::Anime, false).label(), "Continue", "E-28");
        assert_eq!(Fab::new(EntryKind::Anime, false).click(), FabAction::Start);
        let manga = Fab::new(EntryKind::Manga, true);
        assert_eq!(manga.label(), "Pause");
        assert_eq!(manga.click(), FabAction::PauseAndStop);
        let stopped = Fab::new(EntryKind::Manga, false);
        assert_eq!((stopped.label(), stopped.icon(), stopped.click()), ("Resume", "media-playback-start-symbolic", FabAction::Start));
    }

    #[test]
    fn fab_collapses_on_scroll_down_and_expands_on_scroll_up() {
        let mut fab = Fab::new(EntryKind::Manga, false);
        assert!(fab.expanded, "starts expanded");
        fab.on_scroll(3.0);
        assert!(!fab.expanded, "dy > 0 collapses");
        fab.on_scroll(0.0);
        assert!(fab.expanded, "dy <= 0 expands");
        assert!(!fab.show_label(NARROW_WIDTH - 1), "narrow content is icon-only");
        assert!(fab.show_label(NARROW_WIDTH));
    }

    #[test]
    fn sort_menu_has_two_submenus_and_four_leaves_with_the_right_orders() {
        assert_eq!(SortLeaf::ALL.len(), 4);
        assert_eq!(SortLeaf::UploadNewest.submenu(EntryKind::Anime), "By upload date");
        assert_eq!(SortLeaf::NumberAscending.submenu(EntryKind::Anime), "By episode number");
        assert_eq!(SortLeaf::NumberDescending.submenu(EntryKind::Manga), "By chapter number");
        assert_eq!(SortLeaf::UploadNewest.order(), (SortKey::UploadDate, true), "Newest is upload date descending");
        assert_eq!(SortLeaf::UploadOldest.order(), (SortKey::UploadDate, false));
        assert_eq!(SortLeaf::NumberAscending.order(), (SortKey::Number, false));
        assert_eq!(SortLeaf::NumberDescending.order(), (SortKey::Number, true));
        let labels: Vec<&str> = SortLeaf::ALL.iter().map(|l| l.label()).collect();
        assert_eq!(labels, ["Newest", "Oldest", "Ascending", "Descending"]);
        assert_eq!(sort_menu(2).len(), 4);
    }

    #[test]
    fn row_menu_has_six_labels_in_order_with_edge_visibility() {
        let labels: Vec<&str> = [
            RowAction::MoveTop,
            RowAction::MoveSeriesTop,
            RowAction::MoveBottom,
            RowAction::MoveSeriesBottom,
            RowAction::Cancel,
            RowAction::CancelSeries,
        ]
        .iter()
        .map(|a| a.label())
        .collect();
        assert_eq!(
            labels,
            [
                "Move to top",
                "Move series to top",
                "Move to bottom",
                "Move series to bottom",
                "Cancel",
                "Cancel all for this series"
            ]
        );
        assert_eq!(row_menu(0, 3), vec![RowAction::MoveSeriesTop, RowAction::MoveBottom, RowAction::MoveSeriesBottom, RowAction::Cancel, RowAction::CancelSeries], "E-10: first row");
        assert_eq!(row_menu(2, 3).len(), 5, "last row has no Move to bottom");
        assert_eq!(row_menu(1, 3), vec![RowAction::MoveTop, RowAction::MoveSeriesTop, RowAction::MoveBottom, RowAction::MoveSeriesBottom, RowAction::Cancel, RowAction::CancelSeries]);
    }

    #[test]
    fn keyboard_moves_with_alt_and_cancels_with_delete() {
        assert_eq!(row_key(true, "Up"), Some(RowKey::MoveUp));
        assert_eq!(row_key(true, "Down"), Some(RowKey::MoveDown));
        assert_eq!(row_key(false, "Delete"), Some(RowKey::Cancel));
        assert_eq!(row_key(false, "Up"), None, "plain arrows scroll the list");
    }

    #[test]
    fn anime_progress_text_and_bar() {
        assert_eq!(anime_progress_text(0), "Downloading…");
        assert_eq!(anime_progress_text(42), "42%");
        assert_eq!(bar_fraction(&row(EntryKind::Anime, 0, 0, None)), None, "indeterminate at 0");
        assert_eq!(bar_fraction(&row(EntryKind::Anime, 50, 0, None)), Some(0.5));
    }

    #[test]
    fn manga_progress_text_and_bar_use_pages() {
        assert_eq!(manga_progress_text(12, Some(30)), "12/30");
        assert_eq!(manga_progress_text(0, None), "", "empty while the page count is unknown");
        // 30 pages, 12 done and the rest at 50 percent: the sum is 12 * 100 + 18 * 50 = 2100 of 3000.
        let item = row(EntryKind::Manga, 2100, 12, Some(30));
        assert_eq!(row_progress_text(&item), "12/30");
        let fraction = bar_fraction(&item).expect("determinate with a page list");
        assert!((fraction - 0.7).abs() < 1e-9, "bar max is pages * 100");
        assert_eq!(bar_fraction(&row(EntryKind::Manga, 0, 0, None)), None);
    }

    #[test]
    fn pages_poll_gives_up_after_600_ticks() {
        let mut poll = PagesPoll::default();
        for _ in 0..PAGES_POLL_TICKS - 1 {
            assert_eq!(poll.tick(), PollStep::Continue);
        }
        assert_eq!(poll.tick(), PollStep::GiveUp, "600 ticks of 50 ms is 30 s");
        assert_eq!(PAGES_POLL_INTERVAL.as_millis() * u128::from(PAGES_POLL_TICKS), 30_000);
    }

    #[test]
    fn a_downloading_row_repaints_once_per_fifty_milliseconds_and_the_final_update_ends_the_gate() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut gate = RowProgressGate::default();
        let live = DownloadState::Downloading;
        let shown: Vec<bool> = [0, 10, 49, 50, 60].into_iter().map(|ms| gate.allow(1, live, at(ms))).collect();
        assert_eq!(shown, vec![true, false, false, true, false], "E-32");
        assert!(gate.allow(2, live, at(1)), "rows are gated separately");
        assert!(gate.allow(1, DownloadState::Downloaded, at(61)), "one final update, even inside the interval");
        assert!(gate.allow(1, live, at(62)), "the gate was released");
    }

    #[test]
    fn an_error_row_is_not_repainted() {
        let mut gate = RowProgressGate::default();
        let now = Instant::now();
        assert!(gate.allow(1, DownloadState::Downloading, now));
        assert!(!gate.allow(1, DownloadState::Error, now), "E-31");
        assert!(!gate.allow(1, DownloadState::Queue, now));
    }

    #[tokio::test(start_paused = true)]
    async fn the_pages_poll_ends_when_the_pages_appear() {
        let mut looks = 0;
        let known = poll_for_pages(7, || {
            looks += 1;
            if looks == 4 { PagesCheck::Known } else { PagesCheck::Waiting }
        })
        .await;
        assert!(known);
        assert_eq!(looks, 4);
    }

    #[tokio::test(start_paused = true)]
    async fn the_pages_poll_gives_up_after_600_ticks_of_50_ms() {
        let started = tokio::time::Instant::now();
        let mut looks = 0u32;
        let known = poll_for_pages(7, || {
            looks += 1;
            PagesCheck::Waiting
        })
        .await;
        assert!(!known);
        assert_eq!(looks, PAGES_POLL_TICKS, "one look per tick");
        assert_eq!(started.elapsed(), PAGES_POLL_INTERVAL * (PAGES_POLL_TICKS - 1), "30 s less the last sleep");
    }

    #[tokio::test(start_paused = true)]
    async fn the_pages_poll_stops_at_once_for_a_row_that_is_gone() {
        let mut looks = 0;
        let known = poll_for_pages(7, || {
            looks += 1;
            if looks == 2 { PagesCheck::Gone } else { PagesCheck::Waiting }
        })
        .await;
        assert!(!known);
        assert_eq!(looks, 2, "E-31");
    }

    #[test]
    fn tab_count_is_the_row_total() {
        let model = QueueModel::from_flat(vec![row(EntryKind::Anime, 0, 0, None), row(EntryKind::Anime, 0, 0, None)]);
        assert_eq!(tab_count(&model), 2);
    }

    #[test]
    fn progress_is_live_only_while_downloading() {
        assert_eq!(progress_phase(DownloadState::Downloading), ProgressPhase::Live);
        assert_eq!(progress_phase(DownloadState::Downloaded), ProgressPhase::FinalUpdate, "one final update");
        assert_eq!(progress_phase(DownloadState::Error), ProgressPhase::Stopped, "E-31: an error stops the updates");
        assert_eq!(progress_phase(DownloadState::Queue), ProgressPhase::Stopped);
    }

    #[test]
    fn a_lookup_miss_is_a_no_op_for_the_progress_update() {
        let model = QueueModel::from_flat(vec![row(EntryKind::Anime, 10, 0, None)]);
        assert_eq!(model.position(999), None, "E-33: an unknown row is skipped");
    }

    #[test]
    fn sorted_order_is_the_per_section_order_sent_to_the_engine() {
        let mut first = row(EntryKind::Anime, 0, 0, None);
        first.item_id = 1;
        first.source_id = 1;
        first.number = 3.0;
        let mut second = row(EntryKind::Anime, 0, 0, None);
        second.item_id = 2;
        second.source_id = 2;
        second.number = 9.0;
        let mut third = row(EntryKind::Anime, 0, 0, None);
        third.item_id = 3;
        third.source_id = 1;
        third.number = 5.0;
        let model = QueueModel::from_flat(vec![first, second, third]);
        assert_eq!(sorted_order(&model, SortLeaf::NumberAscending), vec![1, 3, 2], "the other section stays in place");
        assert_eq!(sorted_order(&model, SortLeaf::NumberDescending), vec![3, 1, 2]);
    }

    #[test]
    fn section_header_counts_its_rows_and_rows_show_their_names() {
        let mut item = row(EntryKind::Manga, 0, 0, None);
        item.source_name = "MangaDex (EN)".into();
        item.item_name = "Chapter 3".into();
        item.title = "Berserk".into();
        let model = QueueModel::from_flat(vec![item.clone(), item.clone()]);
        let sections = model.sections();
        assert_eq!(section_header(&sections[0]), "MangaDex (EN) (2)");
        assert_eq!(row_titles(&item), ("Chapter 3", "Berserk"), "title is the chapter, subtitle is the manga");
    }
}
