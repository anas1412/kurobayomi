//! The page controller: the load state, the banners and every user action, with no widgets. The widget layer
//! feeds it `StoreEvent`s and `UiAction`s and draws what it holds. Actions return `StoreCmd`s to send to the runner.
//! Upstream: `MangaScreenModel.kt` (selection `:971-1057`, mark read `:788-840`, setters `:878-949`).

use kuro_prefs::tristate::TriState;

use crate::entry_core::banner::{Banners, Length};
use crate::entry_core::detail::{EntryState, LoadState, StoreCmd, StoreEvent, apply_snapshot};
use crate::entry_core::dialog::{CategoryConfirm, Dialog};
use crate::entry_core::flags::{bookmarked_write, display_write, downloaded_write, unread_write};
use crate::entry_core::list_model::{RowItem, focus_after, row_items};
use crate::entry_core::live::{DownloadEvent, ProgressThrottle, apply_event};
use crate::entry_core::selection::{Command, Key, Mods, Pick, click_command, key_command};
use crate::entry_core::view::view;

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
    SetUnread(TriState),
    SetBookmarked(TriState),
    SetDownloaded(TriState),
    SetDisplayNumber(bool),
    SortBy(i64),
    OpenDialog(Dialog),
    ConfirmCategory(CategoryConfirm),
    DismissDialog,
}

/// What the widget layer does after an action.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub cmds: Vec<StoreCmd>,
    /// A plain click on a row outside selection mode: open this chapter in the reader.
    pub open_chapter: Option<i64>,
    /// The row that should hold keyboard focus.
    pub focus: Option<i64>,
}

#[derive(Debug)]
pub struct Page {
    pub manga_id: i64,
    pub state: LoadState,
    pub banners: Banners<StoreCmd>,
    /// The `removeAfterMarkedAsRead` preference.
    pub remove_after_read: bool,
    throttle: ProgressThrottle,
    focused: Option<i64>,
}

impl Page {
    pub fn new(manga_id: i64, remove_after_read: bool) -> Self {
        Self {
            manga_id,
            state: LoadState::Loading,
            banners: Banners::default(),
            remove_after_read,
            throttle: ProgressThrottle::default(),
            focused: None,
        }
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
                self.banners.push(error.user_message(), None, Length::Short, now_ms);
            }
            StoreEvent::ChapterFetchFailed(error) => {
                self.banners.push(error.chapter_fetch_message(), None, Length::Short, now_ms);
            }
            StoreEvent::Categories { all, membership } => {
                if let Some(page) = self.ready_mut() {
                    page.categories = all;
                    page.category_membership = membership;
                }
            }
            StoreEvent::DownloadsDeleted(_) => {}
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
        let mut out = Outcome::default();
        let Some(page) = self.ready_mut() else {
            return out;
        };
        let write = |(flag, mask): (i64, i64)| StoreCmd::SetFlagBits { manga_id, flag, mask };
        match action {
            UiAction::Refresh => out.cmds.push(StoreCmd::FetchAll { manga_id, manual: true }),
            UiAction::SetUnread(state) => out.cmds.push(write(unread_write(state))),
            UiAction::SetBookmarked(state) => out.cmds.push(write(bookmarked_write(state))),
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
            UiAction::OpenDialog(dialog) => page.dialog.open(dialog),
            UiAction::DismissDialog => page.dialog.dismiss(),
            UiAction::ConfirmCategory(confirm) => {
                page.dialog.dismiss();
                out.cmds.push(confirm.into());
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
            initialized: true,
            favorite: true,
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
}
