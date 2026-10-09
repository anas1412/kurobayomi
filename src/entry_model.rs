//! The entry screen's model (T-07a-040, T-07a-044). It owns a database holding the entry on screen, applies the
//! default flags on the first open of a non-favourite, follows the live query of the entry and its items, and maps
//! each settings-sheet control to the flag setter named in `p1-rust-design.md` section 6. The list is built from
//! the model's rows (filter, then sort, then the missing-count separators); a write goes through a setter and the
//! list is rebuilt when the query publishes the new word.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gtk::glib;
use kuro_data::DataResult;
use kuro_data::anime::{Anime, AnimeRepo};
use kuro_data::chapter::{Chapter, ChapterRepo};
use kuro_data::episode::{Episode, EpisodeRepo};
use kuro_data::manga::{Manga, MangaRepo};
use kuro_db::handle::Db;
use kuro_db::schema_anime::Memo;
use kuro_db::statements_anime::{self, EpisodeRow};
use kuro_db::statements_manga::{ChapterRow, chapters};
use kuro_domain::entries::anime::{AnimeExt, episode_flags as ep, season_flags as se};
use kuro_domain::entries::defaults::{on_open_anime, on_open_manga};
use kuro_domain::entries::flag_setters::{self as fs, SeasonOverlay};
use kuro_domain::entries::manga::MangaExt;
use kuro_domain::entries::queries::{subscribe_anime_with_episodes_and_seasons, subscribe_manga_with_chapters};
use kuro_domain::entries::season_display::SeasonDisplayMode;
use kuro_domain::flags::{apply_filter, tri};
use kuro_domain::items::missing::{ListRow, insert_missing_separators};
use kuro_domain::items::sorter::{chapter_cmp, episode_cmp};
use kuro_prefs::store::PrefStore;
use kuro_prefs::tristate::TriState;

/// Sheet ids of the entry details screens (`spec/entries.json`).
pub const CHAPTER_SHEET: &str = "chapter_settings";
pub const EPISODE_SHEET: &str = "episode_settings";
pub const SEASON_SHEET: &str = "season_settings";

/// Chapter sort options, in the order of the spec's `Sort` item.
const CHAPTER_SORTS: [i64; 4] = [0x0, 0x100, 0x200, 0x300];
/// Episode sort options.
const EPISODE_SORTS: [i64; 4] = [0x0, 0x200, 0x400, 0x600];
/// Season sort options (seven in the spec).
const SEASON_SORTS: [i64; 7] = [0x0, 0x2000, 0x4000, 0x6000, 0x8000, 0xA000, 0xC000];
/// Display choices of chapters and episodes: source title, then number.
const DISPLAY_CHOICES: [i64; 2] = [0x0, 0x100000];
/// Season `Display` choices (`SEASON_DISPLAY_MODE`): source title, then season number.
const SEASON_DISPLAY_CHOICES: [i64; 2] = [se::DISPLAY_MODE_SOURCE, se::DISPLAY_MODE_NUMBER];

/// The value of one settings-sheet control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// A tri-state filter: disabled, include, exclude.
    Tri(TriState),
    /// The index of the chosen option of a list, sort or chips item.
    Choice(usize),
    /// A switch.
    Toggle(bool),
    /// A slider value.
    Value(i64),
}

/// One line of the item list: an item, or the count of missing items between two neighbours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    Item { name: String, sub: String, read: bool },
    Missing(i32),
}

/// The loaded entry and its items. Filters and sorts read the entry's word; the list reads the items.
enum State {
    Manga { entry: Manga, chapters: Vec<Chapter> },
    Anime { entry: Anime, episodes: Vec<Episode> },
}

/// The entry screen's model. Shared by the screen and its sheets on the GTK main thread.
pub struct EntryModel {
    anime: bool,
    id: i64,
    db: Arc<Db>,
    manga_repo: MangaRepo,
    chapter_repo: ChapterRepo,
    anime_repo: AnimeRepo,
    episode_repo: EpisodeRepo,
    state: RefCell<State>,
    /// The database file lives in this directory and is removed with the model. Declared last, so it drops last.
    _dir: tempfile::TempDir,
}

impl EntryModel {
    /// Opens a model for a new non-favourite entry titled `title`, with the mockup's rows and no stored flags.
    pub fn open(title: &str, anime: bool) -> Result<Rc<Self>, String> {
        Self::open_with(title, anime, false, 0, PrefStore::in_memory())
    }

    /// Opens a model whose entry is seeded with `favorite` and the flag word `word` (chapter and episode word, and
    /// the season word of an anime). On open a non-favourite gets its default flags before the first render
    /// (T-07a-044); a favourite keeps its stored words.
    pub fn open_with(title: &str, anime: bool, favorite: bool, word: i64, store: PrefStore) -> Result<Rc<Self>, String> {
        let err = |e: &dyn std::fmt::Display| e.to_string();
        let dir = tempfile::TempDir::new().map_err(|e| err(&e))?;
        let path = dir.path().join(if anime { "entry.animedb" } else { "entry.db" });
        let db = if anime {
            let writer = kuro_db::schema_anime::open(&path).map_err(|e| err(&e))?;
            Arc::new(Db::new(&path, writer).map_err(|e| err(&e))?)
        } else {
            let writer = kuro_db::schema_manga::open(&path).map_err(|e| err(&e))?;
            Arc::new(Db::new(&path, writer).map_err(|e| err(&e))?)
        };
        let id = if anime {
            let repo = AnimeRepo::new(db.clone());
            let id = seed_anime(&repo, &db, title, favorite, word).map_err(|e| err(&e))?;
            let entry = repo.by_id(id).map_err(|e| err(&e))?;
            on_open_anime(&repo, &store, &entry).map_err(|e| err(&e))?;
            id
        } else {
            let repo = MangaRepo::new(db.clone());
            let id = seed_manga(&repo, &db, title, favorite, word).map_err(|e| err(&e))?;
            let entry = repo.by_id(id).map_err(|e| err(&e))?;
            on_open_manga(&repo, &store, &entry).map_err(|e| err(&e))?;
            id
        };
        let state = read_state(anime, id, &db).map_err(|e| err(&e))?;
        Ok(Rc::new(Self {
            anime,
            id,
            manga_repo: MangaRepo::new(db.clone()),
            chapter_repo: ChapterRepo::new(db.clone()),
            anime_repo: AnimeRepo::new(db.clone()),
            episode_repo: EpisodeRepo::new(db.clone()),
            db,
            state: RefCell::new(state),
            _dir: dir,
        }))
    }

    /// Re-reads the entry and its items. The live query does this on its own; a synchronous re-read is for the
    /// moment right after a write.
    pub fn reload(&self) -> DataResult<()> {
        *self.state.borrow_mut() = read_state(self.anime, self.id, &self.db)?;
        Ok(())
    }

    /// Calls `render` on the GTK main loop after every published change of the entry or its items. The task holds
    /// the model weakly, so closing the screen (dropping the last strong reference) ends it and its database.
    /// Needs the tokio runtime entered on this thread (the live queries are spawned on it).
    pub fn follow(self: &Rc<Self>, render: impl Fn(&EntryModel) + 'static) {
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Some(model) = weak.upgrade() else { return };
            if model.anime {
                let (mut rx, _task) = subscribe_anime_with_episodes_and_seasons(&model.anime_repo, &model.episode_repo, model.id);
                drop(model);
                while rx.changed().await.is_ok() {
                    let Some(model) = weak.upgrade() else { break };
                    let snapshot = rx.borrow().clone();
                    match snapshot {
                        Some(Ok((entry, episodes, _seasons))) => {
                            if let Some(entry) = entry.as_ref().clone() {
                                *model.state.borrow_mut() = State::Anime { entry, episodes: episodes.as_ref().clone() };
                                render(&model);
                            }
                        }
                        Some(Err(e)) => tracing::warn!(error = %e, "entry query failed; keeping the last view"),
                        None => {}
                    }
                }
            } else {
                let (mut rx, _task) = subscribe_manga_with_chapters(&model.manga_repo, &model.chapter_repo, model.id, false);
                drop(model);
                while rx.changed().await.is_ok() {
                    let Some(model) = weak.upgrade() else { break };
                    let snapshot = rx.borrow().clone();
                    match snapshot {
                        Some(Ok((entry, chapters))) => {
                            if let Some(entry) = entry.as_ref().clone() {
                                *model.state.borrow_mut() = State::Manga { entry, chapters: chapters.as_ref().clone() };
                                render(&model);
                            }
                        }
                        Some(Err(e)) => tracing::warn!(error = %e, "entry query failed; keeping the last view"),
                        None => {}
                    }
                }
            }
        });
    }

    /// The list rows: the items that pass the filters, sorted, with the missing-count separators in between.
    pub fn rows(&self) -> Vec<Row> {
        match &*self.state.borrow() {
            State::Manga { entry, chapters } => {
                let (unread, bookmarked) = (entry.unread_filter(), entry.bookmarked_filter());
                let downloaded = downloaded_filter(entry.downloaded_filter_raw());
                let mut list: Vec<Chapter> = chapters
                    .iter()
                    .filter(|c| apply_filter(unread, || !c.read))
                    .filter(|c| apply_filter(bookmarked, || c.bookmark))
                    .filter(|_| apply_filter(downloaded, || false))
                    .cloned()
                    .collect();
                list.sort_by(chapter_cmp(entry.sorting(), entry.sort_descending()));
                let by_number = entry.display_mode() != 0;
                insert_missing_separators(list, entry.sort_descending())
                    .into_iter()
                    .map(|row| match row {
                        ListRow::Item(c) => Row::Item {
                            name: if by_number { format!("Chapter {}", c.chapter_number) } else { c.name.clone() },
                            sub: format!("{}{}", if c.read { "" } else { "● " }, c.scanlator.clone().unwrap_or_default()),
                            read: c.read,
                        },
                        ListRow::MissingCount { count, .. } => Row::Missing(count),
                    })
                    .collect()
            }
            State::Anime { entry, episodes } => {
                let (unseen, bookmarked, fillermarked) = (entry.unseen_filter(), entry.bookmarked_filter(), entry.fillermarked_filter());
                let downloaded = downloaded_filter(entry.downloaded_filter_raw());
                let mut list: Vec<Episode> = episodes
                    .iter()
                    .filter(|e| apply_filter(unseen, || !e.seen))
                    .filter(|e| apply_filter(bookmarked, || e.bookmark))
                    .filter(|e| apply_filter(fillermarked, || e.fillermark))
                    .filter(|_| apply_filter(downloaded, || false))
                    .cloned()
                    .collect();
                list.sort_by(episode_cmp(entry.sorting(), entry.sort_descending()));
                let by_number = entry.display_mode() != 0;
                insert_missing_separators(list, entry.sort_descending())
                    .into_iter()
                    .map(|row| match row {
                        ListRow::Item(e) => Row::Item {
                            name: if by_number { format!("Episode {}", e.episode_number) } else { e.name.clone() },
                            sub: format!("{}{}", if e.seen { "" } else { "● " }, e.scanlator.clone().unwrap_or_default()),
                            read: e.seen,
                        },
                        ListRow::MissingCount { count, .. } => Row::Missing(count),
                    })
                    .collect()
            }
        }
    }

    /// True when any filter of the entry's sheet is not disabled (the filter icon is tinted).
    pub fn filtered(&self) -> bool {
        match &*self.state.borrow() {
            State::Manga { entry, .. } => {
                [entry.unread_filter(), entry.bookmarked_filter(), downloaded_filter(entry.downloaded_filter_raw())]
                    .iter()
                    .any(|t| *t != TriState::DISABLED)
            }
            State::Anime { entry, .. } => [
                entry.unseen_filter(),
                entry.bookmarked_filter(),
                entry.fillermarked_filter(),
                downloaded_filter(entry.downloaded_filter_raw()),
            ]
            .iter()
            .any(|t| *t != TriState::DISABLED),
        }
    }

    /// Whether the sort of a sheet is descending (the season sheet has its own sort word).
    pub fn sort_descending(&self, sheet: &str) -> bool {
        match &*self.state.borrow() {
            State::Manga { entry, .. } => entry.sort_descending(),
            State::Anime { entry, .. } if sheet == SEASON_SHEET => entry.season_sort_descending(),
            State::Anime { entry, .. } => entry.sort_descending(),
        }
    }

    /// The current value of a sheet control, or `None` when the item is not bound to a setter.
    pub fn control(&self, sheet: &str, title: &str) -> Option<Control> {
        let state = self.state.borrow();
        let word = match (&*state, sheet) {
            (State::Manga { entry, .. }, CHAPTER_SHEET) => entry.chapter_flags,
            (State::Anime { entry, .. }, EPISODE_SHEET) => entry.episode_flags,
            (State::Anime { entry, .. }, SEASON_SHEET) => entry.season_flags,
            _ => return None,
        };
        if let Some((is, not)) = tri_states(sheet, title) {
            return Some(Control::Tri(tri(word & (is | not), is, not)));
        }
        let index = |options: &[i64], current: i64| options.iter().position(|o| *o == current).unwrap_or(0);
        Some(match (sheet, title) {
            (CHAPTER_SHEET, "Sort") => Control::Choice(index(&CHAPTER_SORTS, word & 0x300)),
            (CHAPTER_SHEET, "Display") => Control::Choice(index(&DISPLAY_CHOICES, word & 0x100000)),
            (EPISODE_SHEET, "Sort") => Control::Choice(index(&EPISODE_SORTS, word & ep::SORTING_MASK)),
            (EPISODE_SHEET, "Display") => Control::Choice(index(&DISPLAY_CHOICES, word & ep::DISPLAY_MASK)),
            (EPISODE_SHEET, "Episode thumbnails") => Control::Toggle(word & ep::PREVIEWS_MASK == ep::SHOW_PREVIEWS),
            (EPISODE_SHEET, "Episode summaries") => Control::Toggle(word & ep::SUMMARIES_MASK == ep::SHOW_SUMMARIES),
            (SEASON_SHEET, "Sort") => Control::Choice(index(&SEASON_SORTS, word & se::SORT_MASK)),
            (SEASON_SHEET, "Grid display mode") => Control::Choice(index(&SEASON_DISPLAY_CHOICES, word & se::DISPLAY_MODE_MASK)),
            (SEASON_SHEET, "Display mode") => Control::Choice(((word & se::GRID_DISPLAY_MODE_MASK) >> se::GRID_DISPLAY_MODE_BIT_OFFSET) as usize),
            (SEASON_SHEET, "Items per row") => Control::Value((word & se::GRID_DISPLAY_SIZE_MASK) >> se::GRID_DISPLAY_SIZE_BIT_OFFSET),
            (SEASON_SHEET, _) => season_overlay(title).map(|o| Control::Toggle(word & o.mask() != 0))?,
            _ => return None,
        })
    }

    /// Writes a user change of a sheet control to the entry, through the setter named in `p1-rust-design.md`
    /// section 6. Returns false when the control is not bound or the write failed (the failure is logged). The
    /// list follows the stored word through the live query.
    pub fn apply(&self, sheet: &str, title: &str, value: Control) -> bool {
        match self.write(sheet, title, value) {
            Some(Ok(_)) => true,
            Some(Err(e)) => {
                tracing::error!(error = ?e, sheet, title, "entry flag write failed");
                false
            }
            None => false,
        }
    }

    /// The setter call of a control. `None` when the control is not bound or the choice index is out of range.
    fn write(&self, sheet: &str, title: &str, value: Control) -> Option<DataResult<bool>> {
        let (m, a, id) = (&self.manga_repo, &self.anime_repo, self.id);
        let pick = |options: &[i64], i: usize| options.get(i).copied();
        Some(match (sheet, title, value) {
            (CHAPTER_SHEET, "Downloaded" | "Unread" | "Bookmarked", Control::Tri(s)) => {
                let flag = tri_flag(sheet, title, s);
                match title {
                    "Downloaded" => fs::set_chapter_downloaded(m, id, flag),
                    "Unread" => fs::set_chapter_unread(m, id, flag),
                    _ => fs::set_chapter_bookmarked(m, id, flag),
                }
            }
            (CHAPTER_SHEET, "Sort", Control::Choice(i)) => fs::set_chapter_sort_or_flip(m, id, pick(&CHAPTER_SORTS, i)?),
            (CHAPTER_SHEET, "Display", Control::Choice(i)) => fs::set_chapter_display(m, id, pick(&DISPLAY_CHOICES, i)?),
            (EPISODE_SHEET, "Downloaded" | "Unseen" | "Bookmarked" | "Fillermarked", Control::Tri(s)) => {
                let flag = tri_flag(sheet, title, s);
                match title {
                    "Downloaded" => fs::set_episode_downloaded(a, id, flag),
                    "Unseen" => fs::set_episode_unseen(a, id, flag),
                    "Bookmarked" => fs::set_episode_bookmarked(a, id, flag),
                    _ => fs::set_episode_fillermarked(a, id, flag),
                }
            }
            (EPISODE_SHEET, "Sort", Control::Choice(i)) => fs::set_episode_sort_or_flip(a, id, pick(&EPISODE_SORTS, i)?),
            (EPISODE_SHEET, "Display", Control::Choice(i)) => fs::set_episode_display(a, id, pick(&DISPLAY_CHOICES, i)?),
            (EPISODE_SHEET, "Episode thumbnails", Control::Toggle(on)) => {
                fs::set_episode_previews(a, id, if on { ep::SHOW_PREVIEWS } else { ep::SHOW_NOT_PREVIEWS })
            }
            (EPISODE_SHEET, "Episode summaries", Control::Toggle(on)) => {
                fs::set_episode_summaries(a, id, if on { ep::SHOW_SUMMARIES } else { ep::SHOW_NOT_SUMMARIES })
            }
            (SEASON_SHEET, "Downloaded" | "Unseen" | "Started" | "Completed" | "Bookmarked" | "Fillermarked", Control::Tri(s)) => {
                let flag = tri_flag(sheet, title, s);
                match title {
                    "Downloaded" => fs::set_season_downloaded(a, id, flag),
                    "Unseen" => fs::set_season_unseen(a, id, flag),
                    "Started" => fs::set_season_started(a, id, flag),
                    "Completed" => fs::set_season_completed(a, id, flag),
                    "Bookmarked" => fs::set_season_bookmarked(a, id, flag),
                    _ => fs::set_season_fillermarked(a, id, flag),
                }
            }
            (SEASON_SHEET, "Sort", Control::Choice(i)) => fs::set_season_sort_or_flip(a, id, pick(&SEASON_SORTS, i)?),
            (SEASON_SHEET, "Grid display mode", Control::Choice(i)) => fs::set_season_display(a, id, pick(&SEASON_DISPLAY_CHOICES, i)?),
            (SEASON_SHEET, "Display mode", Control::Choice(i)) => fs::set_season_grid_mode(a, id, SeasonDisplayMode::from_i64(i as i64)?),
            (SEASON_SHEET, "Items per row", Control::Value(size)) => fs::set_season_grid_size(a, id, size),
            (SEASON_SHEET, _, Control::Toggle(on)) => fs::set_season_overlay(a, id, season_overlay(title)?, on),
            _ => return None,
        })
    }
}

/// Reads the entry and its items of one database.
fn read_state(anime: bool, id: i64, db: &Arc<Db>) -> DataResult<State> {
    Ok(if anime {
        State::Anime {
            entry: AnimeRepo::new(db.clone()).by_id(id)?,
            episodes: EpisodeRepo::new(db.clone()).by_anime_id(id)?,
        }
    } else {
        State::Manga {
            entry: MangaRepo::new(db.clone()).by_id(id)?,
            chapters: ChapterRepo::new(db.clone()).by_manga_id(id, false)?,
        }
    })
}

/// The downloaded group as a tri-state. This screen has no download state yet, so the downloaded predicate is
/// always false (see `rows`).
fn downloaded_filter(raw: i64) -> TriState {
    tri(raw, 0x8, 0x10)
}

/// The `(is, not)` bits of a tri-state filter of a sheet, from the spec's filter tab (`p1-rust-design.md` 6).
fn tri_states(sheet: &str, title: &str) -> Option<(i64, i64)> {
    Some(match (sheet, title) {
        (CHAPTER_SHEET, "Downloaded") => (0x8, 0x10),
        (CHAPTER_SHEET, "Unread") => (0x2, 0x4),
        (CHAPTER_SHEET, "Bookmarked") => (0x20, 0x40),
        (EPISODE_SHEET, "Downloaded") => (0x8, 0x10),
        (EPISODE_SHEET, "Unseen") => (0x2, 0x4),
        (EPISODE_SHEET, "Bookmarked") => (0x20, 0x40),
        (EPISODE_SHEET, "Fillermarked") => (0x80, 0x100),
        (SEASON_SHEET, "Downloaded") => (0x2, 0x4),
        (SEASON_SHEET, "Unseen") => (0x8, 0x10),
        (SEASON_SHEET, "Started") => (0x20, 0x40),
        (SEASON_SHEET, "Completed") => (0x80, 0x100),
        (SEASON_SHEET, "Bookmarked") => (0x200, 0x400),
        (SEASON_SHEET, "Fillermarked") => (0x800, 0x1000),
        _ => return None,
    })
}

/// The flag a tri-state writes: the `is` bits, the `not` bits, or nothing when disabled.
fn tri_flag(sheet: &str, title: &str, state: TriState) -> i64 {
    let (is, not) = tri_states(sheet, title).unwrap_or((0, 0));
    if state == TriState::ENABLED_IS {
        is
    } else if state == TriState::ENABLED_NOT {
        not
    } else {
        0
    }
}

/// The overlay switch of a season sheet, by its spec title.
fn season_overlay(title: &str) -> Option<SeasonOverlay> {
    Some(match title {
        "Downloaded episodes" => SeasonOverlay::Downloaded,
        "Unseen episodes" => SeasonOverlay::Unseen,
        "Local source" => SeasonOverlay::Local,
        "Language" => SeasonOverlay::Language,
        "Continue watching button" => SeasonOverlay::Continue,
        _ => return None,
    })
}

/// Seeds the mockup's manga: 24 chapters, the first 19 read, all from one scanlator.
fn seed_manga(repo: &MangaRepo, db: &Arc<Db>, title: &str, favorite: bool, word: i64) -> DataResult<i64> {
    let id = repo.insert(&Manga { favorite, source: 1, url: format!("mock/{title}"), title: title.into(), chapter_flags: word, ..Manga::create() })?;
    let rows: Vec<ChapterRow> = (1..=24i64)
        .map(|n| ChapterRow {
            id: 0,
            manga_id: id,
            url: format!("mock/{title}/{n}"),
            name: format!("Chapter {n}"),
            scanlator: Some("Scanlator".into()),
            read: n < 20,
            bookmark: false,
            last_page_read: 0,
            chapter_number: n as f64,
            source_order: 24 - n,
            date_fetch: 0,
            date_upload: 0,
            last_modified_at: 0,
            version: 0,
            is_syncing: false,
        })
        .collect();
    db.write(|tx| Ok(chapters::insert_all(tx, &rows)?))?;
    Ok(id)
}

/// Seeds the mockup's anime: 24 episodes, the first 19 seen.
fn seed_anime(repo: &AnimeRepo, db: &Arc<Db>, title: &str, favorite: bool, word: i64) -> DataResult<i64> {
    let id = repo.insert(&Anime { favorite, source: 1, url: format!("mock/{title}"), title: title.into(), episode_flags: word, season_flags: word, ..Anime::create() })?;
    let rows: Vec<EpisodeRow> = (1..=24i64)
        .map(|n| EpisodeRow {
            id: 0,
            anime_id: id,
            url: format!("mock/{title}/{n}"),
            name: format!("Episode {n}"),
            scanlator: Some("Scanlator".into()),
            seen: n < 20,
            bookmark: false,
            last_second_seen: 0,
            total_seconds: 0,
            episode_number: n as f64,
            source_order: 24 - n,
            date_fetch: 0,
            date_upload: 0,
            last_modified_at: 0,
            version: 0,
            is_syncing: 0,
            summary: None,
            preview_url: None,
            fillermark: false,
            memo: Memo(Default::default()),
        })
        .collect();
    db.write(|tx| {
        for row in &rows {
            statements_anime::insert_episode(tx, row)?;
        }
        Ok(())
    })?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    impl EntryModel {
        /// The chapter (or episode) word and the season word of the stored entry.
        fn words(&self) -> (i64, i64) {
            match &*self.state.borrow() {
                State::Manga { entry, .. } => (entry.chapter_flags, 0),
                State::Anime { entry, .. } => (entry.episode_flags, entry.season_flags),
            }
        }
    }

    fn item_count(m: &EntryModel) -> usize {
        m.rows().iter().filter(|r| matches!(r, Row::Item { .. })).count()
    }

    #[test]
    fn open_shows_the_mockup_rows_in_source_order_with_the_read_state() {
        let m = EntryModel::open("Title", false).unwrap();
        let rows = m.rows();
        assert_eq!(rows.len(), 24);
        assert_eq!(rows[0], Row::Item { name: "Chapter 24".into(), sub: "● Scanlator".into(), read: false });
        assert_eq!(rows[23], Row::Item { name: "Chapter 1".into(), sub: "Scanlator".into(), read: true });
        assert!(!m.filtered());
    }

    #[test]
    fn a_non_favourite_loses_its_stored_word_on_open_and_a_favourite_keeps_it() {
        let m = EntryModel::open_with("T", false, false, 0x101, PrefStore::in_memory()).unwrap();
        assert_eq!(m.words().0, 0, "defaults are zero, so the stored word is replaced");
        let fav = EntryModel::open_with("T", false, true, 0x101, PrefStore::in_memory()).unwrap();
        assert_eq!(fav.words().0, 0x101, "a favourite keeps its word");
    }

    #[test]
    fn an_anime_non_favourite_gets_episode_and_season_defaults_on_open() {
        let m = EntryModel::open_with("T", true, false, 0x101, PrefStore::in_memory()).unwrap();
        assert_eq!(m.words(), (0, 0x5800000));
    }

    #[test]
    fn bookmarked_cycles_disabled_include_exclude_and_each_state_filters_the_list() {
        let m = EntryModel::open("T", false).unwrap();
        let mut state = TriState::DISABLED;
        for (expected, count) in [(TriState::ENABLED_IS, 0), (TriState::ENABLED_NOT, 24), (TriState::DISABLED, 24)] {
            state = state.next();
            assert!(m.apply(CHAPTER_SHEET, "Bookmarked", Control::Tri(state)));
            m.reload().unwrap();
            assert_eq!(m.control(CHAPTER_SHEET, "Bookmarked"), Some(Control::Tri(expected)));
            assert_eq!(item_count(&m), count, "{expected:?}");
            assert_eq!(m.filtered(), expected != TriState::DISABLED, "the filter icon is tinted only when a filter is on");
        }
    }

    #[test]
    fn unseen_include_keeps_the_unseen_episodes_and_exclude_keeps_the_seen_ones() {
        let m = EntryModel::open("T", true).unwrap();
        assert!(m.apply(EPISODE_SHEET, "Unseen", Control::Tri(TriState::ENABLED_IS)));
        m.reload().unwrap();
        assert_eq!(item_count(&m), 5);
        assert!(m.apply(EPISODE_SHEET, "Unseen", Control::Tri(TriState::ENABLED_NOT)));
        m.reload().unwrap();
        assert_eq!(item_count(&m), 19);
    }

    #[test]
    fn choosing_the_sort_in_use_flips_its_direction() {
        let m = EntryModel::open("T", false).unwrap();
        assert_eq!(m.control(CHAPTER_SHEET, "Sort"), Some(Control::Choice(0)));
        assert!(m.apply(CHAPTER_SHEET, "Sort", Control::Choice(2)));
        m.reload().unwrap();
        assert_eq!(m.control(CHAPTER_SHEET, "Sort"), Some(Control::Choice(2)));
        assert!(!m.sort_descending(CHAPTER_SHEET), "a new sort is ascending");
        assert!(m.apply(CHAPTER_SHEET, "Sort", Control::Choice(2)));
        m.reload().unwrap();
        assert!(m.sort_descending(CHAPTER_SHEET), "the sort in use flips to descending");
    }

    #[test]
    fn season_controls_write_their_groups_and_the_defaults_show_through() {
        let m = EntryModel::open_with("T", true, false, 0, PrefStore::in_memory()).unwrap();
        assert!(m.apply(SEASON_SHEET, "Grid display mode", Control::Choice(1)));
        assert!(m.apply(SEASON_SHEET, "Items per row", Control::Value(7)));
        assert!(m.apply(SEASON_SHEET, "Downloaded episodes", Control::Toggle(true)));
        assert!(m.apply(SEASON_SHEET, "Display mode", Control::Choice(3)));
        m.reload().unwrap();
        assert_eq!(m.control(SEASON_SHEET, "Grid display mode"), Some(Control::Choice(1)));
        assert_eq!(m.control(SEASON_SHEET, "Items per row"), Some(Control::Value(7)));
        assert_eq!(m.control(SEASON_SHEET, "Downloaded episodes"), Some(Control::Toggle(true)));
        assert_eq!(m.control(SEASON_SHEET, "Display mode"), Some(Control::Choice(3)));
        assert_eq!(m.control(SEASON_SHEET, "Local source"), Some(Control::Toggle(true)), "default overlay on");
        assert_eq!(m.control(SEASON_SHEET, "Season display unknown"), None);
    }

    #[test]
    fn episode_thumbnail_switch_writes_the_preview_bit() {
        let m = EntryModel::open("T", true).unwrap();
        assert_eq!(m.control(EPISODE_SHEET, "Episode thumbnails"), Some(Control::Toggle(true)));
        assert!(m.apply(EPISODE_SHEET, "Episode thumbnails", Control::Toggle(false)));
        m.reload().unwrap();
        assert_eq!(m.control(EPISODE_SHEET, "Episode thumbnails"), Some(Control::Toggle(false)));
    }

    #[test]
    fn an_out_of_range_choice_is_refused_without_a_write() {
        let m = EntryModel::open("T", false).unwrap();
        let before = m.words();
        assert!(!m.apply(CHAPTER_SHEET, "Sort", Control::Choice(9)));
        m.reload().unwrap();
        assert_eq!(m.words(), before);
    }
}

