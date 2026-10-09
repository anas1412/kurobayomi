//! The details page model (GTK-free): load state, the load pipeline, snapshot application, and the store protocol.
//! Upstream: `MangaScreenModel.kt:173-256` (init and initial load), `:1144-1155` (State.Success).
//! Store commands run on tokio (`runner.rs`). Snapshots reach the GTK thread through `LatestWins`, which keeps only the newest.

use std::collections::BTreeSet;
use std::sync::Mutex;

use crate::entry_core::chapter::{ChapterRow, sort_rows};
use crate::entry_core::dialog::DialogSlot;
use crate::entry_core::errors::DetailError;
use crate::entry_core::flags::ChapterFlags;
use crate::entry_core::selection::Selection;
use crate::entry_core::view::{next_unread, view};

/// The manga row fields the details page reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MangaRow {
    pub id: i64,
    pub source: i64,
    pub url: String,
    pub title: String,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    /// `SManga` status, 0..6 (`0` is unknown).
    pub status: i64,
    pub thumbnail_url: Option<String>,
    /// `initialized`: details were fetched from the source at least once.
    pub initialized: bool,
    pub favorite: bool,
    pub local: bool,
    pub flags: ChapterFlags,
}

/// The state of the details page. `Loading` is the only state before the row is read (E-LOAD-1: no endless Loading).
#[derive(Clone, Debug, PartialEq)]
pub enum LoadState {
    Loading,
    Ready(Box<EntryState>),
    NotFound,
}

/// The loaded page (`State.Success`).
#[derive(Clone, Debug, PartialEq)]
pub struct EntryState {
    pub manga: MangaRow,
    /// `fromSource`: the page was opened from a source list. Used for the small-layout description default.
    pub from_source: bool,
    pub chapters: Vec<ChapterRow>,
    pub selection: Selection,
    pub available_scanlators: Vec<String>,
    pub excluded_scanlators: BTreeSet<String>,
    pub tracking_count: i64,
    pub has_logged_in_trackers: bool,
    pub has_prompted_add: bool,
    pub is_refreshing: bool,
    pub dialog: DialogSlot,
    pub downloaded_only: bool,
    /// The source of the manga: its display name and whether it is a stub (extension missing).
    pub source: SourceInfo,
    /// The reader preference `skipFiltered`: "next chapter" looks only at the filtered rows when true.
    pub skip_filtered: bool,
    /// The user categories (`id`, name) and the ids the manga belongs to. Reloaded after a category write.
    pub categories: Vec<(i64, String)>,
    pub category_membership: Vec<i64>,
}

impl EntryState {
    /// `getNextUnread` input: the filtered view when `skip_filtered`, else every row in the stored sort.
    pub fn next_unread(&self) -> Option<i64> {
        let flags = self.manga.flags;
        let desc = flags.sort_descending();
        if self.skip_filtered {
            let rows = view(&self.chapters, flags, self.downloaded_only, self.manga.local);
            next_unread(&rows, desc)
        } else {
            let mut rows: Vec<&ChapterRow> = self.chapters.iter().collect();
            sort_rows(flags, &mut rows);
            next_unread(&rows, desc)
        }
    }
}

/// The source line of the info header (`SourceName` and the stub warning).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceInfo {
    pub name: String,
    /// True when the extension is not installed: the header shows a warning icon in the error color.
    pub stub: bool,
}

/// What the loader needs besides the rows: preferences read once at load time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadContext {
    pub library_defaults: ChapterFlags,
    pub from_source: bool,
    pub downloaded_only: bool,
    pub skip_filtered: bool,
}

/// The result of the load pipeline, and which fetches the caller must start.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub state: LoadState,
    /// Start a details fetch (`manual = false`). Read once, at load time.
    pub needs_info: bool,
    /// Start a chapter-list fetch (`manual = false`). Read once, at load time.
    pub needs_chapters: bool,
}

/// The load pipeline (T-38b-003). A missing row is `NotFound`. A non-favorite gets the library's default chapter
/// flags before the first paint, so the page never shows stale filters. Refreshing starts when info or chapters are missing.
pub fn finish_load(
    manga: Option<MangaRow>,
    chapters: Vec<ChapterRow>,
    source: SourceInfo,
    ctx: &LoadContext,
) -> Loaded {
    let Some(mut manga) = manga else {
        return Loaded {
            state: LoadState::NotFound,
            needs_info: false,
            needs_chapters: false,
        };
    };
    if !manga.favorite {
        manga.flags = ctx.library_defaults;
    }
    let needs_info = !manga.initialized;
    let needs_chapters = chapters.is_empty();
    let state = EntryState {
        manga,
        from_source: ctx.from_source,
        chapters,
        selection: Selection::default(),
        available_scanlators: Vec::new(),
        excluded_scanlators: BTreeSet::new(),
        tracking_count: 0,
        has_logged_in_trackers: false,
        has_prompted_add: false,
        is_refreshing: needs_info || needs_chapters,
        dialog: DialogSlot::default(),
        downloaded_only: ctx.downloaded_only,
        source,
        skip_filtered: ctx.skip_filtered,
        categories: Vec::new(),
        category_membership: Vec::new(),
    };
    Loaded {
        state: LoadState::Ready(Box::new(state)),
        needs_info,
        needs_chapters,
    }
}

/// Applies a store snapshot. Selection is owned by the page and survives snapshots. Only ids that still exist are kept.
pub fn apply_snapshot(state: &mut EntryState, manga: MangaRow, chapters: Vec<ChapterRow>) {
    let ids: Vec<i64> = chapters.iter().map(|c| c.id).collect();
    state.selection.retain_existing(&ids);
    state.manga = manga;
    state.chapters = chapters;
}

/// Commands from the page to the store. Each runs on tokio (`runner.rs`). Flag, read, bookmark and category writes
/// run detached, so closing the page does not cancel them (`launchNonCancellable`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreCmd {
    /// Reads the page, starts the live subscriptions, and starts the first fetch when info or chapters are missing.
    Load { manga_id: i64 },
    /// `fetchAllFromSource`. One refresh per manga runs at a time.
    FetchAll { manga_id: i64, manual: bool },
    /// One masked UPDATE of `mangas.chapter_flags`.
    SetFlagBits { manga_id: i64, flag: i64, mask: i64 },
    /// Sort by `mode`, or flip the direction when `mode` is the stored key. Decided inside the write transaction.
    SortBy { manga_id: i64, mode: i64 },
    MarkRead {
        manga_id: i64,
        chapter_ids: Vec<i64>,
        read: bool,
        remove_after_read: bool,
    },
    SetBookmark {
        manga_id: i64,
        chapter_ids: Vec<i64>,
        bookmark: bool,
    },
    /// `CategoryConfirm::AddToLibrary`: favorite and categories in one transaction. Categories reload after.
    AddToLibrary { manga_id: i64, category_ids: Vec<i64> },
    /// `CategoryConfirm::SetCategories`: the links in one transaction. Categories reload after.
    SetCategories { manga_id: i64, category_ids: Vec<i64> },
    /// The library button: removes a favorite, or adds a manga that is not one, after the duplicate check and the
    /// default-category routing (`toggleFavorite`, behaviour 4.2). `default_category` is the stored preference, and
    /// `skip_duplicate` is the "Add anyway" answer to the duplicate dialog.
    ToggleFavorite { manga_id: i64, default_category: i64, skip_duplicate: bool },
    /// Queues chapters for download. The page decided which rows may start (`chapter_actions::acts_on`).
    Download { manga_id: i64, chapter_ids: Vec<i64>, start_now: bool },
    /// Takes one chapter out of the download queue.
    CancelDownload { manga_id: i64, chapter_id: i64 },
    /// Deletes the downloads of these chapters.
    DeleteDownloads { manga_id: i64, chapter_ids: Vec<i64> },
    /// Writes the library's default chapter flags to this manga (the Reset item of the settings sheet).
    ResetFlags { manga_id: i64 },
    /// Drops the subscriptions and stops the runner. Detached writes keep running.
    Shutdown,
}

impl From<crate::entry_core::dialog::CategoryConfirm> for StoreCmd {
    fn from(confirm: crate::entry_core::dialog::CategoryConfirm) -> Self {
        use crate::entry_core::dialog::CategoryConfirm as C;
        match confirm {
            C::AddToLibrary { manga_id, category_ids } => Self::AddToLibrary { manga_id, category_ids },
            C::SetCategories { manga_id, category_ids } => Self::SetCategories { manga_id, category_ids },
        }
    }
}

/// Events from the store to the page.
#[derive(Clone, Debug, PartialEq)]
pub enum StoreEvent {
    /// The first read: the state, and which fetches the runner already started.
    Loaded(Box<Loaded>),
    Snapshot {
        manga: Box<MangaRow>,
        chapters: Vec<ChapterRow>,
    },
    Refreshing(bool),
    /// A details fetch failed (`manga fetch` banner text is `DetailError::user_message`).
    Failed(DetailError),
    /// A chapter fetch failed (banner text is `DetailError::chapter_fetch_message`).
    ChapterFetchFailed(DetailError),
    /// Categories reloaded after a confirm (or at load).
    Categories {
        all: Vec<(i64, String)>,
        membership: Vec<i64>,
    },
    /// A mark-read asked for these downloads to go, and the backend removed them.
    DownloadsDeleted(Vec<i64>),
    /// The library button found another favorite with the same title. Nothing was written (E-FAV-2).
    Duplicate { duplicate_id: i64 },
    /// The default category is unset or deleted: the page opens the category dialog. Nothing was written (E-FAV-3).
    ChooseCategory,
    /// The manga left the library. `ask_delete_downloads` is true when downloads exist (the delete prompt).
    Removed { ask_delete_downloads: bool },
    /// A download call queued chapters. The page may offer to add the manga to the library.
    Queued,
}

/// A single slot that keeps only the newest value. A producer `put`s, the consumer `take`s, and an older value is
/// overwritten, so a burst of snapshots becomes one redraw.
#[derive(Debug, Default)]
pub struct LatestWins<T> {
    slot: Mutex<Option<T>>,
}

impl<T> LatestWins<T> {
    pub fn new() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }

    pub fn put(&self, value: T) {
        *self.slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
    }

    pub fn take(&self) -> Option<T> {
        self.slot.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_core::dialog::Dialog;

    fn load(
        manga: Option<MangaRow>,
        chapters: Vec<ChapterRow>,
        library_defaults: ChapterFlags,
        from_source: bool,
        downloaded_only: bool,
    ) -> Loaded {
        let ctx = LoadContext {
            library_defaults,
            from_source,
            downloaded_only,
            skip_filtered: false,
        };
        finish_load(manga, chapters, SourceInfo::default(), &ctx)
    }

    fn manga(initialized: bool, favorite: bool) -> MangaRow {
        MangaRow {
            id: 1,
            source: 9,
            url: "/m/1".into(),
            title: "Title".into(),
            author: None,
            artist: None,
            description: None,
            genres: vec![],
            status: 0,
            thumbnail_url: None,
            initialized,
            favorite,
            local: false,
            flags: ChapterFlags(0x100),
        }
    }

    fn chapter(id: i64) -> ChapterRow {
        ChapterRow::new(id, "c", id as f64)
    }

    #[test]
    fn missing_row_is_not_found_not_loading() {
        let loaded = load(None, vec![], ChapterFlags::default(), false, false);
        assert_eq!(loaded.state, LoadState::NotFound);
        assert!(!loaded.needs_info && !loaded.needs_chapters);
    }

    #[test]
    fn non_favorite_gets_library_defaults_before_first_paint() {
        let defaults = ChapterFlags(0x301);
        let loaded = load(
            Some(manga(true, false)),
            vec![chapter(1)],
            defaults,
            false,
            false,
        );
        let LoadState::Ready(state) = loaded.state else {
            panic!("expected Ready")
        };
        assert_eq!(state.manga.flags, defaults);
    }

    #[test]
    fn favorite_keeps_its_own_flags() {
        let loaded = load(
            Some(manga(true, true)),
            vec![chapter(1)],
            ChapterFlags(0x301),
            false,
            false,
        );
        let LoadState::Ready(state) = loaded.state else {
            panic!("expected Ready")
        };
        assert_eq!(state.manga.flags, ChapterFlags(0x100));
    }

    #[test]
    fn needs_flags_are_read_once_and_drive_refreshing() {
        let loaded = load(
            Some(manga(false, true)),
            vec![],
            ChapterFlags::default(),
            true,
            false,
        );
        assert!(loaded.needs_info && loaded.needs_chapters);
        let LoadState::Ready(state) = loaded.state else {
            panic!("expected Ready")
        };
        assert!(state.is_refreshing);
        assert!(state.from_source);
        assert!(state.dialog.current().is_none());
        assert!(!state.has_prompted_add);

        let idle = load(
            Some(manga(true, true)),
            vec![chapter(1)],
            ChapterFlags::default(),
            false,
            false,
        );
        assert!(!idle.needs_info && !idle.needs_chapters);
        let LoadState::Ready(state) = idle.state else {
            panic!("expected Ready")
        };
        assert!(!state.is_refreshing);
    }

    #[test]
    fn two_snapshots_keep_selection_for_chapters_that_still_exist() {
        let loaded = load(
            Some(manga(true, true)),
            vec![chapter(1), chapter(2), chapter(3)],
            ChapterFlags::default(),
            false,
            false,
        );
        let LoadState::Ready(mut state) = loaded.state else {
            panic!("expected Ready")
        };
        state.selection.set_all(&[1, 2, 3], true);
        apply_snapshot(
            &mut state,
            manga(true, true),
            vec![chapter(1), chapter(2), chapter(3)],
        );
        apply_snapshot(
            &mut state,
            manga(true, true),
            vec![chapter(1), chapter(3), chapter(4)],
        );
        assert!(state.selection.contains(1));
        assert!(state.selection.contains(3));
        assert!(
            !state.selection.contains(2),
            "a removed chapter leaves the selection"
        );
        assert_eq!(state.chapters.len(), 3);
    }

    #[test]
    fn latest_wins_keeps_only_the_newest_value() {
        let slot = LatestWins::new();
        slot.put(1);
        slot.put(2);
        assert_eq!(slot.take(), Some(2));
        assert_eq!(slot.take(), None);
    }

    #[test]
    fn dialog_slot_lives_in_the_state() {
        let loaded = load(
            Some(manga(true, true)),
            vec![chapter(1)],
            ChapterFlags::default(),
            false,
            false,
        );
        let LoadState::Ready(mut state) = loaded.state else {
            panic!("expected Ready")
        };
        state.dialog.open(Dialog::FullCover);
        assert_eq!(state.dialog.current(), Some(&Dialog::FullCover));
    }

    #[test]
    fn source_info_and_skip_filtered_reach_the_state() {
        let ctx = LoadContext {
            skip_filtered: true,
            ..LoadContext::default()
        };
        let source = SourceInfo {
            name: "Source A (EN)".into(),
            stub: true,
        };
        let loaded = finish_load(Some(manga(true, true)), vec![chapter(1)], source.clone(), &ctx);
        let LoadState::Ready(state) = loaded.state else {
            panic!("expected Ready")
        };
        assert_eq!(state.source, source);
        assert!(state.skip_filtered);
    }

    #[test]
    fn next_unread_follows_skip_filtered() {
        let mut rows = vec![chapter(1), chapter(2), chapter(3)];
        rows[0].read = true;
        rows[1].bookmark = true;
        let mut m = manga(true, true);
        m.flags = ChapterFlags(0x1 | 0x20 | 0x100); // number ascending, bookmarked only
        let ctx = |skip_filtered| LoadContext {
            skip_filtered,
            ..LoadContext::default()
        };
        let next = |rows: &[ChapterRow], skip| {
            let loaded = finish_load(Some(m.clone()), rows.to_vec(), SourceInfo::default(), &ctx(skip));
            let LoadState::Ready(state) = loaded.state else {
                panic!("expected Ready")
            };
            state.next_unread()
        };
        assert_eq!(next(&rows, true), Some(2), "only the bookmarked rows count");
        assert_eq!(next(&rows, false), Some(2), "all rows: 1 is read, so 2 comes first");
        rows[1].read = true;
        assert_eq!(next(&rows, true), None, "the filtered view has no unread row");
        assert_eq!(next(&rows, false), Some(3));
    }
}
