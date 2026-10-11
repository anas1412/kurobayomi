//! The page controller: the load state, the banners and every user action, with no widgets. The widget layer
//! feeds it `StoreEvent`s and `UiAction`s and draws what it holds. Actions return `StoreCmd`s to send to the runner.
//! Upstream: `MangaScreenModel.kt` (selection `:971-1057`, mark read `:788-840`, setters `:878-949`).

use kuro_i18n::{MsgKey, tr};
use kuro_prefs::tristate::TriState;

use crate::entry_core::banner::{Banners, Length};
use crate::entry_core::chapter_actions::{DownloadAction, IndicatorAction, acts_on, download_targets, indicator_action, previous_ids, state_after_cancel};
use crate::entry_core::detail::{EntryState, LoadState, StoreCmd, StoreEvent, apply_snapshot};
use crate::entry_core::dialog::{CategoryConfirm, Dialog};
use crate::entry_core::errors::DetailError;
use crate::entry_core::flags::{bookmarked_write, display_write, downloaded_write, unread_write};
use crate::entry_core::list_model::{RowItem, focus_after, row_items};
use crate::entry_core::live::{DownloadEvent, ProgressThrottle, apply_event};
use crate::entry_core::selection::{Command, Key, Mods, Pick, back_clears_selection, click_command, key_command};
use crate::entry_core::view::view;

/// What a row's context menu runs on that one chapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowMenuAction {
    ToggleRead,
    ToggleBookmark,
    Download,
}

/// A user action on the page.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    Refresh,
    /// A click or long press on a row. `long_press_or_right` is a long press or a right click.
    Row {
        id: i64,
        mods: Mods,
        long_press_or_right: bool,
    },
    /// A key press. `focused` is the row that has keyboard focus.
    Key {
        key: Key,
        mods: Mods,
        focused: Option<i64>,
    },
    MarkRead(bool),
    Bookmark(bool),
    /// An item of one row's context menu.
    RowMenu {
        id: i64,
        action: RowMenuAction,
    },
    SetUnread(TriState),
    SetBookmarked(TriState),
    /// The interval dialog's OK or Reset: the wheel value (0 is automatic).
    SetFetchInterval(i32),
    SetDownloaded(TriState),
    SetDisplayNumber(bool),
    SortBy(i64),
    OpenDialog(Dialog),
    ConfirmCategory(CategoryConfirm),
    DismissDialog,
    /// The change-category dialog closed without OK. In the add flow with `auto_open` the tracking sheet follows
    /// (`autoOpenTrack`, E-74); the caller reads `auto_open` at this moment.
    DismissCategory { auto_open: bool },
    /// The Back button or Escape: a selection is cleared first, and the page pops only when nothing is selected.
    Back,
    /// A download action on rows: the indicator of one row, or the Download item of the context menu (`runChapterDownloadActions`).
    Download {
        ids: Vec<i64>,
        action: IndicatorAction,
    },
    /// Marks every chapter before this one, in view order, as read (`markPreviousChapterRead`).
    MarkPrevious(i64),
    /// The library button. `skip_duplicate` is the "Add anyway" answer to the duplicate dialog.
    ToggleFavorite {
        skip_duplicate: bool,
    },
    /// Writes the library's default chapter flags to this manga.
    ResetFlags,
    /// An entry of the Download menu: the next chapters, or every unread one.
    DownloadMenu(DownloadAction),
    /// The scanlator sheet's OK: the exclusions become exactly this set.
    SetExcludedScanlators(std::collections::BTreeSet<String>),
    /// The delete confirmation's OK: the selection is cleared, then the downloads of these chapters are deleted.
    ConfirmDeleteChapters(Vec<i64>),
}

/// What the widget layer does after an action.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub cmds: Vec<StoreCmd>,
    /// A plain click on a row outside selection mode: open this chapter in the reader.
    pub open_chapter: Option<i64>,
    /// The row that should hold keyboard focus.
    pub focus: Option<i64>,
    /// Back or Escape found nothing selected: pop the page.
    pub pop: bool,
    /// Open the tracking sheet: the add flow's category dialog was dismissed with auto-open on (E-74).
    pub open_tracking: bool,
}

#[derive(Debug)]
pub struct Page {
    pub manga_id: i64,
    pub state: LoadState,
    pub banners: Banners<StoreCmd>,
    /// The `removeAfterMarkedAsRead` preference.
    pub remove_after_read: bool,
    /// The stored default category of the library (`-1` when unset), read once at open.
    pub default_category: i64,
    throttle: ProgressThrottle,
    focused: Option<i64>,
    /// The store asked for the category dialog (E-FAV-3); the widget layer takes the request once.
    choose_category: bool,
}

impl Page {
    pub fn new(manga_id: i64, remove_after_read: bool) -> Self {
        Self {
            manga_id,
            state: LoadState::Loading,
            banners: Banners::default(),
            remove_after_read,
            default_category: -1,
            throttle: ProgressThrottle::default(),
            focused: None,
            choose_category: false,
        }
    }

    /// The Retry button of a failed fetch: a manual refresh. None for an uninstalled source or a cancelled fetch, which a
    /// retry cannot change.
    fn retry(&self, error: &DetailError) -> Option<(String, StoreCmd)> {
        let hopeless = matches!(error, DetailError::SourceNotInstalled | DetailError::Cancelled);
        (!hopeless).then(|| (tr(MsgKey::action_retry), StoreCmd::FetchAll { manga_id: self.manga_id, manual: true }))
    }

    /// True once after the store asked for the category dialog.
    pub fn take_category_request(&mut self) -> bool {
        std::mem::take(&mut self.choose_category)
    }

    pub fn ready(&self) -> Option<&EntryState> {
        match &self.state {
            LoadState::Ready(page) => Some(page),
            _ => None,
        }
    }

    fn ready_mut(&mut self) -> Option<&mut EntryState> {
        match &mut self.state {
            LoadState::Ready(page) => Some(page),
            _ => None,
        }
    }

    /// Toolbar actions are enabled only once the page is `Ready` (no actions while `Loading`).
    pub fn actions_enabled(&self) -> bool {
        self.ready().is_some()
    }

    pub fn rows(&self) -> Vec<RowItem> {
        self.ready().map(row_items).unwrap_or_default()
    }

    fn view_ids(&self) -> Vec<i64> {
        self.ready()
            .map(|p| view(&p.chapters, p.manga.flags, p.downloaded_only, p.manga.local).iter().map(|r| r.id).collect())
            .unwrap_or_default()
    }

    /// Applies a store event. Returns the row that should hold focus after it, when focus moved.
    pub fn apply(&mut self, event: StoreEvent, now_ms: u64) -> Option<i64> {
        let before = self.view_ids();
        match event {
            StoreEvent::Loaded(loaded) => self.state = loaded.state,
            StoreEvent::Snapshot { manga, chapters } => {
                if let Some(page) = self.ready_mut() {
                    apply_snapshot(page, *manga, chapters);
                }
            }
            StoreEvent::Refreshing(on) => {
                if let Some(page) = self.ready_mut() {
                    page.is_refreshing = on;
                }
            }
            StoreEvent::Failed(error) => {
                let retry = self.retry(&error);
                self.banners.push(error.user_message(), retry, Length::Short, now_ms);
            }
            StoreEvent::ChapterFetchFailed(error) => {
                let retry = self.retry(&error);
                self.banners.push(error.chapter_fetch_message(), retry, Length::Short, now_ms);
            }
            StoreEvent::Categories { all, membership } => {
                if let Some(page) = self.ready_mut() {
                    page.categories = all;
                    page.category_membership = membership;
                }
            }
            StoreEvent::DownloadsDeleted(_) => {}
            StoreEvent::Duplicate { duplicate_id } => {
                let manga_id = self.manga_id;
                if let Some(page) = self.ready_mut() {
                    page.dialog.open(Dialog::DuplicateManga { manga_id, duplicate_id });
                }
            }
            StoreEvent::ChooseCategory => self.choose_category = true,
            StoreEvent::Removed { ask_delete_downloads } => {
                let manga_id = self.manga_id;
                let downloaded: Vec<i64> = self
                    .ready()
                    .map(|p| p.chapters.iter().filter(|c| c.download == kuro_downloads::DownloadState::Downloaded).map(|c| c.id).collect())
                    .unwrap_or_default();
                if ask_delete_downloads && !downloaded.is_empty() {
                    let action = (tr(MsgKey::action_delete), StoreCmd::DeleteDownloads { manga_id, chapter_ids: downloaded });
                    self.banners.push(tr(MsgKey::delete_downloads_for_manga), Some(action), Length::Short, now_ms);
                }
            }
            StoreEvent::Scanlators { available, excluded } => {
                if let Some(page) = self.ready_mut() {
                    page.available_scanlators = available;
                    page.excluded_scanlators = excluded;
                }
            }
            StoreEvent::Queued => {
                // the add-to-library prompt: once per open, and only for a manga that is not in the library
                let (manga_id, default_category) = (self.manga_id, self.default_category);
                let prompt = self.ready_mut().is_some_and(|p| !p.manga.favorite && !std::mem::replace(&mut p.has_prompted_add, true));
                if prompt {
                    let action = (tr(MsgKey::action_add), StoreCmd::ToggleFavorite { manga_id, default_category, skip_duplicate: false });
                    self.banners.push(tr(MsgKey::snack_add_to_manga_library), Some(action), Length::Short, now_ms);
                }
            }
        }
        let after = self.view_ids();
        let focused = self.focused?;
        let next = focus_after(&before, &after, focused);
        self.focused = next;
        next.filter(|&id| id != focused)
    }

    /// Applies a download event. Returns the raw index of the row that changed.
    pub fn apply_download(&mut self, event: DownloadEvent, now_ms: u64) -> Option<usize> {
        apply_event(&mut self.state, event, &mut self.throttle, now_ms)
    }

    /// Toasts use the banner component with the Short length (T-38b-024).
    pub fn toast(&mut self, text: impl Into<String>, now_ms: u64) {
        self.banners.push(text, None, Length::Toast, now_ms);
    }

    pub fn act(&mut self, action: UiAction) -> Outcome {
        let manga_id = self.manga_id;
        let remove_after_read = self.remove_after_read;
        let default_category = self.default_category;
        let view_ids = if matches!(action, UiAction::MarkPrevious(_)) { self.view_ids() } else { Vec::new() };
        let mut out = Outcome::default();
        let Some(page) = self.ready_mut() else {
            return out;
        };
        let write = |(flag, mask): (i64, i64)| StoreCmd::SetFlagBits { manga_id, flag, mask };
        match action {
            UiAction::Refresh => out.cmds.push(StoreCmd::FetchAll { manga_id, manual: true }),
            UiAction::SetUnread(state) => out.cmds.push(write(unread_write(state))),
            UiAction::SetBookmarked(state) => out.cmds.push(write(bookmarked_write(state))),
            UiAction::SetFetchInterval(wheel) => out.cmds.push(StoreCmd::SetFetchInterval { manga_id, wheel }),
            UiAction::SetExcludedScanlators(excluded) => out.cmds.push(StoreCmd::SetExcludedScanlators { manga_id, excluded }),
            UiAction::SetDownloaded(state) => out.cmds.push(write(downloaded_write(state))),
            UiAction::SetDisplayNumber(number) => out.cmds.push(write(display_write(number))),
            UiAction::SortBy(mode) => out.cmds.push(StoreCmd::SortBy { manga_id, mode }),
            UiAction::MarkRead(read) => {
                if let Some(chapter_ids) = take_selection(page) {
                    out.cmds.push(StoreCmd::MarkRead {
                        manga_id,
                        chapter_ids,
                        read,
                        remove_after_read,
                    });
                }
            }
            UiAction::Bookmark(bookmark) => {
                if let Some(chapter_ids) = take_selection(page) {
                    out.cmds.push(StoreCmd::SetBookmark {
                        manga_id,
                        chapter_ids,
                        bookmark,
                    });
                }
            }
            UiAction::RowMenu { id, action } => {
                if let Some((read, bookmark, download)) = page.chapters.iter().find(|c| c.id == id).map(|c| (c.read, c.bookmark, c.download)) {
                    match action {
                        RowMenuAction::ToggleRead => out.cmds.push(StoreCmd::MarkRead {
                            manga_id,
                            chapter_ids: vec![id],
                            read: !read,
                            remove_after_read,
                        }),
                        RowMenuAction::ToggleBookmark => out.cmds.push(StoreCmd::SetBookmark {
                            manga_id,
                            chapter_ids: vec![id],
                            bookmark: !bookmark,
                        }),
                        RowMenuAction::Download => download_cmd(page, manga_id, vec![id], indicator_action(download), &mut out),
                    }
                }
            }
            UiAction::Download { ids, action } => download_cmd(page, manga_id, ids, action, &mut out),
            UiAction::MarkPrevious(pointer) => {
                let chapter_ids = previous_ids(&view_ids, pointer, page.manga.flags.sort_descending());
                page.selection.clear();
                if !chapter_ids.is_empty() {
                    out.cmds.push(StoreCmd::MarkRead { manga_id, chapter_ids, read: true, remove_after_read });
                }
            }
            UiAction::ToggleFavorite { skip_duplicate } => {
                page.dialog.dismiss();
                out.cmds.push(StoreCmd::ToggleFavorite { manga_id, default_category, skip_duplicate });
            }
            UiAction::ResetFlags => out.cmds.push(StoreCmd::ResetFlags { manga_id }),
            UiAction::DownloadMenu(menu) => {
                let chapter_ids = download_targets(page, menu);
                if !chapter_ids.is_empty() {
                    out.cmds.push(StoreCmd::Download { manga_id, chapter_ids, start_now: false });
                }
            }
            UiAction::Back => {
                if page.dialog.current().is_some() {
                    page.dialog.dismiss();
                } else if back_clears_selection(page.selection.len()) {
                    page.selection.clear();
                } else {
                    out.pop = true;
                }
            }
            UiAction::OpenDialog(dialog) => page.dialog.open(dialog),
            UiAction::DismissDialog => page.dialog.dismiss(),
            UiAction::DismissCategory { auto_open } => {
                let add_flow = matches!(page.dialog.current(), Some(Dialog::ChangeCategory(d)) if d.add_flow);
                page.dialog.dismiss_category(add_flow, auto_open);
                out.open_tracking = page.dialog.take_tracking_intent();
            }
            UiAction::ConfirmCategory(confirm) => {
                page.dialog.dismiss();
                out.cmds.push(confirm.into());
            }
            UiAction::ConfirmDeleteChapters(chapter_ids) => {
                page.dialog.dismiss();
                page.selection.clear();
                out.cmds.push(StoreCmd::DeleteDownloads { manga_id, chapter_ids });
            }
            UiAction::Row {
                id,
                mods,
                long_press_or_right,
            } => {
                let command = click_command(mods, long_press_or_right);
                if command == Command::Tap && !mods.ctrl && page.selection.is_empty() {
                    out.open_chapter = Some(id);
                    return out;
                }
                self.focused = Some(id);
                out.focus = Some(id);
                self.toggle(id, command);
            }
            UiAction::Key { key, mods, focused } => {
                let in_selection = !page.selection.is_empty();
                let dialog_open = page.dialog.current().is_some();
                let Some(command) = key_command(key, mods, in_selection, dialog_open) else {
                    return out;
                };
                match command {
                    Command::Tap | Command::LongPress => {
                        if let Some(id) = focused {
                            self.focused = Some(id);
                            out.focus = Some(id);
                            self.toggle(id, command);
                        }
                    }
                    Command::SelectAll => {
                        let raw: Vec<i64> = page.chapters.iter().map(|c| c.id).collect();
                        page.selection.set_all(&raw, true);
                        out.focus = focused;
                    }
                    Command::Invert => {
                        let raw: Vec<i64> = page.chapters.iter().map(|c| c.id).collect();
                        page.selection.invert(&raw);
                        out.focus = focused;
                    }
                    Command::ClearSelection => {
                        page.selection.clear();
                        out.focus = focused;
                    }
                    Command::CloseDialog => page.dialog.dismiss(),
                }
            }
        }
        out
    }

    /// Toggles one row in the view. A tap flips it. A long press or Shift selects it and the range up to it.
    fn toggle(&mut self, id: i64, command: Command) {
        let ids = self.view_ids();
        let Some(page) = self.ready_mut() else { return };
        let selected = !page.selection.contains(id);
        let pick = if command == Command::LongPress { Pick::LongPress } else { Pick::Tap };
        page.selection.toggle(&ids, id, selected, pick);
    }
}

/// One download action on `ids` (`runChapterDownloadActions`). START_NOW and CANCEL act on exactly one row and do
/// nothing for several (E-DL-1, E-DL-2), so the add prompt is skipped too. A cancel shows `NOT_DOWNLOADED` at once; a
/// later downloader event overrides it (E-CANCEL-1).
fn download_cmd(page: &mut EntryState, manga_id: i64, ids: Vec<i64>, action: IndicatorAction, out: &mut Outcome) {
    if !acts_on(action, ids.len()) {
        return;
    }
    match action {
        IndicatorAction::Start => out.cmds.push(StoreCmd::Download { manga_id, chapter_ids: ids, start_now: false }),
        IndicatorAction::StartNow => out.cmds.push(StoreCmd::Download { manga_id, chapter_ids: ids, start_now: true }),
        IndicatorAction::Cancel => {
            let chapter_id = ids[0];
            if let Some(row) = page.chapters.iter_mut().find(|c| c.id == chapter_id) {
                row.download = state_after_cancel();
                row.download_progress = 0;
            }
            out.cmds.push(StoreCmd::CancelDownload { manga_id, chapter_id });
        }
        IndicatorAction::Delete => out.cmds.push(StoreCmd::DeleteDownloads { manga_id, chapter_ids: ids }),
    }
}

/// The selected ids, after clearing the selection. The selection is cleared even when it is empty (E-SEL-5), and an
/// empty selection gives `None`: no write is sent.
fn take_selection(page: &mut EntryState) -> Option<Vec<i64>> {
    let ids: Vec<i64> = page.selection.ids().collect();
    page.selection.clear();
    if ids.is_empty() { None } else { Some(ids) }
}

#[cfg(test)]
mod tests {
    use kuro_db::flags::chapter as c;
    use kuro_downloads::DownloadState;

    use super::*;
    use crate::entry_core::chapter::ChapterRow;
    use crate::entry_core::detail::{LoadContext, MangaRow, SourceInfo, finish_load};
    use crate::entry_core::errors::DetailError;
    use crate::entry_core::flags::ChapterFlags;

    fn manga(flags: i64) -> MangaRow {
        MangaRow {
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
        }
    }

    /// A ready page with chapters 1..=n, number ascending.
    fn ready(n: i64) -> Page {
        let rows = (1..=n).map(|i| ChapterRow::new(i, &format!("c{i}"), i as f64)).collect();
        let loaded = finish_load(Some(manga(0x101)), rows, SourceInfo::default(), &LoadContext::default());
        let mut page = Page::new(1, false);
        page.apply(StoreEvent::Loaded(Box::new(loaded)), 0);
        page
    }

    fn row(id: i64) -> UiAction {
        UiAction::Row {
            id,
            mods: Mods::default(),
            long_press_or_right: false,
        }
    }

    #[test]
    fn loading_page_has_no_actions_and_ignores_them() {
        let mut page = Page::new(1, false);
        assert!(!page.actions_enabled());
        assert_eq!(page.act(UiAction::Refresh), Outcome::default());
        assert!(page.rows().is_empty());
        page.apply(StoreEvent::Loaded(Box::new(finish_load(None, vec![], SourceInfo::default(), &LoadContext::default()))), 0);
        assert_eq!(page.state, LoadState::NotFound);
        assert!(!page.actions_enabled());
    }

    #[test]
    fn a_plain_click_opens_the_chapter_until_selection_mode_starts() {
        let mut page = ready(3);
        assert_eq!(page.act(row(2)).open_chapter, Some(2));
        let long = UiAction::Row {
            id: 2,
            mods: Mods::default(),
            long_press_or_right: true,
        };
        let out = page.act(long);
        assert_eq!((out.open_chapter, out.focus), (None, Some(2)));
        assert!(page.ready().unwrap().selection.contains(2));
        let out = page.act(row(3));
        assert_eq!(out.open_chapter, None, "in selection mode a click toggles");
        assert!(page.ready().unwrap().selection.contains(3));
    }

    #[test]
    fn shift_click_extends_a_range_and_ctrl_click_toggles() {
        let mut page = ready(5);
        let shift = Mods { shift: true, ctrl: false };
        let ctrl = Mods { shift: false, ctrl: true };
        let click = |id, mods| UiAction::Row { id, mods, long_press_or_right: false };
        page.act(click(2, shift));
        page.act(click(4, shift));
        let ids: Vec<i64> = page.ready().unwrap().selection.ids().collect();
        assert_eq!(ids, [2, 3, 4]);
        page.act(click(3, ctrl));
        let ids: Vec<i64> = page.ready().unwrap().selection.ids().collect();
        assert_eq!(ids, [2, 4]);
    }

    #[test]
    fn keys_select_all_invert_clear_and_space_toggles_the_focused_row() {
        let mut page = ready(3);
        let key = |key, mods, focused| UiAction::Key { key, mods, focused };
        let ctrl = Mods { shift: false, ctrl: true };
        let out = page.act(key(Key::Space, Mods::default(), Some(2)));
        assert_eq!(out.focus, Some(2), "focus stays on the toggled row");
        assert!(page.ready().unwrap().selection.contains(2));
        page.act(key(Key::Letter('i'), ctrl, Some(2)));
        let ids: Vec<i64> = page.ready().unwrap().selection.ids().collect();
        assert_eq!(ids, [1, 3]);
        page.act(key(Key::Letter('a'), ctrl, Some(2)));
        assert_eq!(page.ready().unwrap().selection.len(), 3);
        page.act(key(Key::Escape, Mods::default(), Some(2)));
        assert!(page.ready().unwrap().selection.is_empty());
    }

    #[test]
    fn escape_closes_a_dialog_before_it_clears_the_selection() {
        let mut page = ready(2);
        page.act(UiAction::Key { key: Key::Space, mods: Mods::default(), focused: Some(1) });
        page.act(UiAction::OpenDialog(Dialog::FullCover));
        page.act(UiAction::Key { key: Key::Escape, mods: Mods::default(), focused: Some(1) });
        assert!(page.ready().unwrap().dialog.current().is_none());
        assert_eq!(page.ready().unwrap().selection.len(), 1);
    }

    #[test]
    fn mark_read_clears_the_selection_first_even_when_the_list_is_empty() {
        let mut page = ready(3);
        page.act(UiAction::Key { key: Key::Space, mods: Mods::default(), focused: Some(1) });
        page.act(UiAction::Key { key: Key::Space, mods: Mods::default(), focused: Some(2) });
        let out = page.act(UiAction::MarkRead(true));
        assert!(page.ready().unwrap().selection.is_empty());
        assert_eq!(
            out.cmds,
            vec![StoreCmd::MarkRead { manga_id: 1, chapter_ids: vec![1, 2], read: true, remove_after_read: false }]
        );
        // nothing selected: selection stays clear and no write is sent
        let out = page.act(UiAction::Bookmark(true));
        assert!(out.cmds.is_empty());
        assert!(page.ready().unwrap().selection.is_empty());
    }

    #[test]
    fn remove_after_read_follows_the_preference() {
        let mut page = ready(2);
        page.remove_after_read = true;
        page.act(row(1));
        page.act(UiAction::Key { key: Key::Space, mods: Mods::default(), focused: Some(1) });
        let out = page.act(UiAction::MarkRead(true));
        assert!(matches!(out.cmds[0], StoreCmd::MarkRead { remove_after_read: true, .. }));
    }

    #[test]
    fn setters_become_masked_store_writes_and_never_touch_the_local_state() {
        let mut page = ready(1);
        let before = page.ready().unwrap().manga.flags;
        let out = page.act(UiAction::SetUnread(TriState::ENABLED_IS));
        assert_eq!(out.cmds, vec![StoreCmd::SetFlagBits { manga_id: 1, flag: c::SHOW_UNREAD, mask: c::UNREAD_MASK }]);
        let out = page.act(UiAction::SetDisplayNumber(true));
        assert_eq!(out.cmds, vec![StoreCmd::SetFlagBits { manga_id: 1, flag: c::DISPLAY_NUMBER, mask: c::DISPLAY_MASK }]);
        let out = page.act(UiAction::SortBy(c::SORTING_ALPHABET));
        assert_eq!(out.cmds, vec![StoreCmd::SortBy { manga_id: 1, mode: c::SORTING_ALPHABET }]);
        assert_eq!(page.ready().unwrap().manga.flags, before, "the view changes only when the store snapshot arrives");
        page.apply(
            StoreEvent::Snapshot { manga: Box::new(manga(0x101 | c::SHOW_UNREAD)), chapters: vec![ChapterRow::new(1, "c1", 1.0)] },
            0,
        );
        assert_eq!(page.ready().unwrap().manga.flags.0, 0x101 | c::SHOW_UNREAD);
    }

    #[test]
    fn category_confirm_closes_the_dialog_and_runs_the_matching_command() {
        let mut page = ready(1);
        page.act(UiAction::OpenDialog(Dialog::SettingsSheet));
        let out = page.act(UiAction::ConfirmCategory(CategoryConfirm::AddToLibrary { manga_id: 1, category_ids: vec![4] }));
        assert_eq!(out.cmds, vec![StoreCmd::AddToLibrary { manga_id: 1, category_ids: vec![4] }]);
        assert!(page.ready().unwrap().dialog.current().is_none());
        page.apply(StoreEvent::Categories { all: vec![(4, "A".into())], membership: vec![4] }, 0);
        assert_eq!(page.ready().unwrap().category_membership, vec![4]);
    }

    #[test]
    fn dismissing_the_add_flow_category_dialog_opens_tracking_only_with_auto_open() {
        let dialog = |add_flow| Dialog::ChangeCategory(crate::entry_core::dialog::CategoryDialog::new(1, add_flow, &[(2, "A".into())], &[]));
        let mut page = ready(1);
        page.act(UiAction::OpenDialog(dialog(true)));
        assert!(!page.act(UiAction::DismissCategory { auto_open: false }).open_tracking, "auto-open off");
        page.act(UiAction::OpenDialog(dialog(true)));
        let out = page.act(UiAction::DismissCategory { auto_open: true });
        assert!(out.open_tracking && page.ready().unwrap().dialog.current().is_none(), "add flow with auto-open (E-74)");
        page.act(UiAction::OpenDialog(dialog(false)));
        assert!(!page.act(UiAction::DismissCategory { auto_open: true }).open_tracking, "not the add flow");
        assert!(!page.act(UiAction::DismissCategory { auto_open: true }).open_tracking, "after OK nothing is open, no intent");
    }

    #[test]
    fn refresh_sends_a_manual_fetch_and_errors_become_banners() {
        let mut page = ready(1);
        assert_eq!(page.act(UiAction::Refresh).cmds, vec![StoreCmd::FetchAll { manga_id: 1, manual: true }]);
        page.apply(StoreEvent::Refreshing(true), 0);
        assert!(page.ready().unwrap().is_refreshing);
        page.apply(StoreEvent::Refreshing(false), 0);
        assert!(!page.ready().unwrap().is_refreshing);
        page.apply(StoreEvent::ChapterFetchFailed(DetailError::NoChapters), 10);
        assert_eq!(page.banners.current().unwrap().text, "No chapters found");
        page.apply(StoreEvent::Failed(DetailError::Http(500)), 20);
        assert_eq!(page.banners.queued(), 1);
        // a failed fetch offers Retry, which runs a manual refresh; an uninstalled source offers none
        let retry = page.banners.activate_action(true, 30);
        assert_eq!(retry, Some(StoreCmd::FetchAll { manga_id: 1, manual: true }));
        page.banners.dismiss(40);
        page.apply(StoreEvent::Failed(DetailError::SourceNotInstalled), 50);
        assert_eq!(page.banners.current().unwrap().action, None);
    }

    #[test]
    fn a_toast_is_a_four_second_banner() {
        let mut page = ready(1);
        page.toast("Copied", 100);
        page.banners.tick(4099);
        assert!(page.banners.current().is_some());
        page.banners.tick(4100);
        assert!(page.banners.current().is_none());
    }

    #[test]
    fn the_scanlator_sheet_ok_and_a_scanlator_event_reach_the_page() {
        let mut page = ready(3);
        let excluded: std::collections::BTreeSet<String> = ["B".to_owned()].into();
        assert_eq!(
            page.act(UiAction::SetExcludedScanlators(excluded.clone())).cmds,
            vec![StoreCmd::SetExcludedScanlators { manga_id: 1, excluded: excluded.clone() }]
        );
        page.apply(StoreEvent::Scanlators { available: vec!["A".into(), "B".into()], excluded: excluded.clone() }, 0);
        let ready = page.ready().expect("ready");
        assert_eq!((ready.available_scanlators.clone(), ready.excluded_scanlators.clone()), (vec!["A".to_owned(), "B".to_owned()], excluded));
    }

    #[test]
    fn download_events_patch_one_row() {
        let mut page = ready(3);
        let index = page.apply_download(DownloadEvent::Status { chapter_id: 2, state: DownloadState::Queue }, 0);
        assert_eq!(index, Some(1));
        assert_eq!(page.rows()[1].download, DownloadState::Queue);
    }

    #[test]
    fn focus_moves_off_a_row_that_a_new_filter_hides() {
        let mut page = ready(3);
        page.act(UiAction::Key { key: Key::Space, mods: Mods::default(), focused: Some(2) });
        // the snapshot reads chapter 2, and the new flags hide read rows
        let rows = vec![ChapterRow::new(1, "c1", 1.0), ChapterRow { read: true, ..ChapterRow::new(2, "c2", 2.0) }, ChapterRow::new(3, "c3", 3.0)];
        let moved = page.apply(
            StoreEvent::Snapshot { manga: Box::new(manga(0x101 | c::SHOW_UNREAD)), chapters: rows },
            0,
        );
        assert_eq!(moved, Some(3), "focus goes to a visible neighbour");
    }

    #[test]
    fn a_menu_item_toggles_read_or_bookmark_on_that_one_row() {
        let mut page = ready(3);
        let out = page.act(UiAction::RowMenu { id: 2, action: RowMenuAction::ToggleRead });
        assert_eq!(out.cmds, vec![StoreCmd::MarkRead { manga_id: 1, chapter_ids: vec![2], read: true, remove_after_read: false }]);
        let out = page.act(UiAction::RowMenu { id: 3, action: RowMenuAction::ToggleBookmark });
        assert_eq!(out.cmds, vec![StoreCmd::SetBookmark { manga_id: 1, chapter_ids: vec![3], bookmark: true }]);
        assert!(page.act(UiAction::RowMenu { id: 99, action: RowMenuAction::ToggleRead }).cmds.is_empty(), "unknown row");
    }

    #[test]
    fn back_clears_any_selection_first_even_one_a_filter_hides_and_pops_only_when_nothing_is_selected() {
        let mut page = ready(3);
        assert!(page.act(UiAction::Back).pop, "nothing selected: pop");
        page.act(UiAction::Key { key: Key::Space, mods: Mods::default(), focused: Some(2) });
        let out = page.act(UiAction::Back);
        assert!(!out.pop && page.ready().unwrap().selection.is_empty(), "selection cleared, no pop");
        // chapter 2 is selected, then a filter hides every read row, and 2 is read
        page.act(UiAction::Key { key: Key::Space, mods: Mods::default(), focused: Some(2) });
        let rows = vec![ChapterRow::new(1, "c1", 1.0), ChapterRow { read: true, ..ChapterRow::new(2, "c2", 2.0) }, ChapterRow::new(3, "c3", 3.0)];
        page.apply(StoreEvent::Snapshot { manga: Box::new(manga(0x101 | c::SHOW_UNREAD)), chapters: rows }, 0);
        assert_eq!(page.rows().iter().filter(|r| r.selected).count(), 0, "the selected row is hidden");
        assert!(!page.act(UiAction::Back).pop, "a hidden selection still counts (E-04)");
        assert!(page.ready().unwrap().selection.is_empty());
        // a dialog closes before the selection
        page.act(UiAction::OpenDialog(Dialog::FullCover));
        assert!(!page.act(UiAction::Back).pop);
        assert!(page.ready().unwrap().dialog.current().is_none());
    }

    #[test]
    fn the_download_indicator_maps_each_state_and_several_rows_do_nothing() {
        let mut page = ready(3);
        let dl = |ids: Vec<i64>, action| UiAction::Download { ids, action };
        assert_eq!(page.act(dl(vec![1], IndicatorAction::StartNow)).cmds, vec![StoreCmd::Download { manga_id: 1, chapter_ids: vec![1], start_now: true }]);
        assert_eq!(page.act(dl(vec![1, 2], IndicatorAction::Start)).cmds, vec![StoreCmd::Download { manga_id: 1, chapter_ids: vec![1, 2], start_now: false }]);
        assert!(page.act(dl(vec![1, 2], IndicatorAction::StartNow)).cmds.is_empty(), "E-DL-2");
        assert!(page.act(dl(vec![1, 2], IndicatorAction::Cancel)).cmds.is_empty(), "E-DL-1");
        assert_eq!(page.act(dl(vec![1, 2], IndicatorAction::Delete)).cmds, vec![StoreCmd::DeleteDownloads { manga_id: 1, chapter_ids: vec![1, 2] }]);
        // a cancel shows NOT_DOWNLOADED at once, and a later downloader event overrides it
        page.apply_download(DownloadEvent::Status { chapter_id: 2, state: DownloadState::Downloading }, 0);
        let out = page.act(dl(vec![2], IndicatorAction::Cancel));
        assert_eq!(out.cmds, vec![StoreCmd::CancelDownload { manga_id: 1, chapter_id: 2 }]);
        assert_eq!(page.rows()[1].download, DownloadState::NotDownloaded);
        page.apply_download(DownloadEvent::Status { chapter_id: 2, state: DownloadState::Queue }, 0);
        assert_eq!(page.rows()[1].download, DownloadState::Queue);
    }

    #[test]
    fn the_download_menu_item_follows_the_state_of_the_row() {
        let mut page = ready(2);
        let menu_item = |id| UiAction::RowMenu { id, action: RowMenuAction::Download };
        assert_eq!(page.act(menu_item(1)).cmds, vec![StoreCmd::Download { manga_id: 1, chapter_ids: vec![1], start_now: true }]);
        page.apply_download(DownloadEvent::Status { chapter_id: 2, state: DownloadState::Downloaded }, 0);
        assert_eq!(page.act(menu_item(2)).cmds, vec![StoreCmd::DeleteDownloads { manga_id: 1, chapter_ids: vec![2] }]);
        page.apply_download(DownloadEvent::Status { chapter_id: 1, state: DownloadState::Queue }, 0);
        assert_eq!(page.act(menu_item(1)).cmds, vec![StoreCmd::CancelDownload { manga_id: 1, chapter_id: 1 }]);
    }

    #[test]
    fn mark_previous_follows_the_view_order() {
        // number ascending (0x101): the earlier chapters are the ones above
        let mut page = ready(4);
        let out = page.act(UiAction::MarkPrevious(3));
        assert_eq!(out.cmds, vec![StoreCmd::MarkRead { manga_id: 1, chapter_ids: vec![1, 2], read: true, remove_after_read: false }]);
        assert!(page.act(UiAction::MarkPrevious(1)).cmds.is_empty(), "nothing is before the first");
        assert!(page.act(UiAction::MarkPrevious(99)).cmds.is_empty(), "a pointer outside the view marks nothing (E-PREV-1)");
        // number descending (0x100): the earlier chapters are the ones below, in chronological order
        let rows = (1..=4).map(|i| ChapterRow::new(i, &format!("c{i}"), i as f64)).collect();
        page.apply(StoreEvent::Snapshot { manga: Box::new(manga(0x100)), chapters: rows }, 0);
        let out = page.act(UiAction::MarkPrevious(3));
        assert_eq!(out.cmds, vec![StoreCmd::MarkRead { manga_id: 1, chapter_ids: vec![1, 2], read: true, remove_after_read: false }]);
    }

    #[test]
    fn the_library_button_sends_the_toggle_with_the_stored_default_category() {
        let mut page = ready(1);
        page.default_category = 4;
        assert_eq!(page.act(UiAction::ToggleFavorite { skip_duplicate: false }).cmds, vec![StoreCmd::ToggleFavorite { manga_id: 1, default_category: 4, skip_duplicate: false }]);
        page.act(UiAction::OpenDialog(Dialog::DuplicateManga { manga_id: 1, duplicate_id: 9 }));
        let out = page.act(UiAction::ToggleFavorite { skip_duplicate: true });
        assert_eq!(out.cmds, vec![StoreCmd::ToggleFavorite { manga_id: 1, default_category: 4, skip_duplicate: true }]);
        assert!(page.ready().unwrap().dialog.current().is_none(), "Add anyway closes the duplicate dialog");
    }

    #[test]
    fn the_store_answers_of_the_favorite_toggle_open_the_dialogs() {
        let mut page = ready(1);
        page.apply(StoreEvent::Duplicate { duplicate_id: 9 }, 0);
        assert_eq!(page.ready().unwrap().dialog.current(), Some(&Dialog::DuplicateManga { manga_id: 1, duplicate_id: 9 }));
        assert!(!page.take_category_request());
        page.apply(StoreEvent::ChooseCategory, 0);
        assert!(page.take_category_request());
        assert!(!page.take_category_request(), "the request is taken once");
    }

    #[test]
    fn a_removed_favorite_with_downloads_asks_to_delete_them() {
        let mut page = ready(2);
        page.apply_download(DownloadEvent::Status { chapter_id: 2, state: DownloadState::Downloaded }, 0);
        page.apply(StoreEvent::Removed { ask_delete_downloads: false }, 0);
        assert!(page.banners.current().is_none());
        page.apply(StoreEvent::Removed { ask_delete_downloads: true }, 0);
        let banner = page.banners.current().expect("the prompt");
        assert_eq!(banner.text, "Delete downloaded chapters?");
        assert_eq!(banner.action.as_ref().map(|(label, _)| label.as_str()), Some("Delete"));
        assert_eq!(page.banners.activate_action(true, 1), Some(StoreCmd::DeleteDownloads { manga_id: 1, chapter_ids: vec![2] }));
    }

    #[test]
    fn the_first_queued_download_of_a_non_favorite_offers_to_add_it_once() {
        let mut page = ready(1);
        page.state = finish_load(Some(MangaRow { favorite: false, ..manga(0x101) }), vec![ChapterRow::new(1, "c1", 1.0)], SourceInfo::default(), &LoadContext::default()).state;
        page.apply(StoreEvent::Queued, 0);
        let banner = page.banners.current().expect("the add prompt");
        assert_eq!(banner.text, "Add manga to library?");
        assert_eq!(banner.action.as_ref().map(|(label, _)| label.as_str()), Some("Add"));
        page.banners.dismiss(1);
        page.apply(StoreEvent::Queued, 2);
        assert!(page.banners.current().is_none(), "once per open (E-DL-3)");
        // a favorite is never asked
        let mut favorite = ready(1);
        favorite.apply(StoreEvent::Queued, 0);
        assert!(favorite.banners.current().is_none());
    }

    #[test]
    fn the_download_menu_queues_the_next_unread_chapters_and_nothing_when_there_are_none() {
        let mut page = ready(3);
        let out = page.act(UiAction::DownloadMenu(DownloadAction::Next(2)));
        assert_eq!(out.cmds, vec![StoreCmd::Download { manga_id: 1, chapter_ids: vec![1, 2], start_now: false }]);
        for id in 1..=3 {
            page.apply_download(DownloadEvent::Status { chapter_id: id, state: DownloadState::Downloaded }, 0);
        }
        assert!(page.act(UiAction::DownloadMenu(DownloadAction::Unread)).cmds.is_empty());
    }

    #[test]
    fn reset_sends_the_defaults_write() {
        assert_eq!(ready(1).act(UiAction::ResetFlags).cmds, vec![StoreCmd::ResetFlags { manga_id: 1 }]);
    }
}
