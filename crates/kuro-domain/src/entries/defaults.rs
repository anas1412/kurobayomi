//! Default flags of new entries (T-07a-027, T-07a-044). The defaults are the `default_*` keys of the preference
//! registry. Applying them writes the resulting word to the entry; "set as default" writes an entry's raw groups
//! back into the same keys. Two keys are shared between chapter and episode (Q3), and the season downloaded
//! default is written from the downloaded bits (Q2).

use kuro_data::DataResult;
use kuro_data::entry::{Entry, EntryRepo};
use kuro_db::flags::{item, season as se};
use kuro_platform::kind::Kind;
use kuro_prefs::store::PrefStore;

use crate::error::DomainError;
use crate::flags::set_flag;

use super::flag_setters::{SeasonAll, SeasonOverlay, item_all, season_all};

const CHAPTER_UNREAD: &str = "default_chapter_filter_by_read";
const CHAPTER_DOWNLOADED: &str = "default_chapter_filter_by_downloaded";
const CHAPTER_BOOKMARKED: &str = "default_chapter_filter_by_bookmarked";
const CHAPTER_SORT: &str = "default_chapter_sort_by_source_or_number";
const CHAPTER_DISPLAY: &str = "default_chapter_display_by_name_or_number";
/// Shared by chapter and episode (Q3).
const SHARED_DIRECTION: &str = "default_chapter_sort_by_ascending_or_descending";

const EPISODE_UNSEEN: &str = "default_episode_filter_by_seen";
const EPISODE_DOWNLOADED: &str = "default_episode_filter_by_downloaded";
const EPISODE_BOOKMARKED: &str = "default_episode_filter_by_bookmarked";
const EPISODE_FILLERMARKED: &str = "default_episode_filter_by_fillermarked";
const EPISODE_SORT: &str = "default_episode_sort_by_source_or_number";
const EPISODE_PREVIEWS: &str = "default_episode_show_thumbnail_previews";
const EPISODE_SUMMARIES: &str = "default_episode_show_summaries";

const SEASON_DOWNLOADED: &str = "default_season_filter_by_downloaded";
const SEASON_UNSEEN: &str = "default_season_filter_by_unseen";
const SEASON_STARTED: &str = "default_season_filter_by_started";
const SEASON_COMPLETED: &str = "default_season_filter_by_completed";
const SEASON_BOOKMARKED: &str = "default_season_filter_by_bookmarked";
const SEASON_FILLERMARKED: &str = "default_season_filter_by_fillermarked";
const SEASON_SORT: &str = "default_season_sort_by_source_or_number";
const SEASON_SORT_DIRECTION: &str = "default_season_sort_by_ascending_or_descending";
const SEASON_GRID_MODE: &str = "default_season_grid_display_mode";
const SEASON_GRID_SIZE: &str = "default_season_grid_display_size";
const SEASON_DOWNLOAD_OVERLAY: &str = "default_season_download_overlay";
const SEASON_UNSEEN_OVERLAY: &str = "default_season_unseen_overlay";
const SEASON_LOCAL_OVERLAY: &str = "default_season_local_overlay";
const SEASON_LANG_OVERLAY: &str = "default_season_lang_overlay";
const SEASON_CONTINUE_OVERLAY: &str = "default_season_continue_overlay";
const SEASON_DISPLAY: &str = "default_season_display_mode";

/// The stored chapter sort preference keeps Aniyomi's values (0, 0x100, 0x200, 0x300), one place below the item word's
/// sorting group (`item::SORTING_MASK`, 0x600). The pref is part of the preference wire, so it is not moved.
const PREF_SORTING_MASK: i64 = 0x300;

fn sorting_from_pref(value: i64) -> i64 {
    (value & PREF_SORTING_MASK) << 1
}

fn sorting_to_pref(word: i64) -> i64 {
    (word & item::SORTING_MASK) >> 1
}

fn long(store: &PrefStore, key: &'static str) -> i64 {
    store.pref::<i64>(key, 0).get()
}

fn flag(store: &PrefStore, key: &'static str, default: bool) -> bool {
    store.pref::<bool>(key, default).get()
}

fn put_long(store: &PrefStore, key: &'static str, value: i64) {
    let _ = store.pref::<i64>(key, 0).set(value);
}

/// The default item word of a kind (unified layout), as the preferences describe it. The two kinds read their own
/// keys (the direction and display keys are shared, Q3). A manga word has only the six manga groups: the anime-only
/// groups are not read for it.
pub fn item_default_word(store: &PrefStore, kind: Kind) -> i64 {
    match kind {
        Kind::Manga => item_all(
            long(store, CHAPTER_UNREAD),
            long(store, CHAPTER_DOWNLOADED),
            long(store, CHAPTER_BOOKMARKED),
            0,
            // a stray bit outside the sorting group of the pref is not a sort
            sorting_from_pref(long(store, CHAPTER_SORT)),
            long(store, SHARED_DIRECTION),
            long(store, CHAPTER_DISPLAY),
            0,
            0,
        ),
        Kind::Anime => item_all(
            long(store, EPISODE_UNSEEN),
            long(store, EPISODE_DOWNLOADED),
            long(store, EPISODE_BOOKMARKED),
            long(store, EPISODE_FILLERMARKED),
            long(store, EPISODE_SORT),
            long(store, SHARED_DIRECTION),
            long(store, CHAPTER_DISPLAY),
            long(store, EPISODE_PREVIEWS),
            long(store, EPISODE_SUMMARIES),
        ),
    }
}

/// The default season word, as the preferences describe it. With the registry defaults this is `0x5800000`.
pub fn season_default_word(store: &PrefStore) -> i64 {
    season_all(SeasonAll {
        download: long(store, SEASON_DOWNLOADED),
        unseen: long(store, SEASON_UNSEEN),
        started: long(store, SEASON_STARTED),
        completed: long(store, SEASON_COMPLETED),
        bookmarked: long(store, SEASON_BOOKMARKED),
        fillermarked: long(store, SEASON_FILLERMARKED),
        sorting: long(store, SEASON_SORT),
        direction: long(store, SEASON_SORT_DIRECTION),
        grid_mode: long(store, SEASON_GRID_MODE),
        grid_size: store.pref::<i32>(SEASON_GRID_SIZE, 0).get().into(),
        overlays: [
            flag(store, SEASON_DOWNLOAD_OVERLAY, false),
            flag(store, SEASON_UNSEEN_OVERLAY, true),
            flag(store, SEASON_LOCAL_OVERLAY, true),
            flag(store, SEASON_LANG_OVERLAY, false),
            flag(store, SEASON_CONTINUE_OVERLAY, true),
        ],
        display: long(store, SEASON_DISPLAY),
    })
}

/// Writes the default item word of the entry's kind to it (on add to library, and "apply to all"). Returns whether the
/// row changed.
pub fn apply_item_defaults(repo: &EntryRepo, store: &PrefStore, e: &Entry) -> DataResult<bool> {
    let word = item_default_word(store, e.kind);
    repo.update_item_flags(e.id, |_| word)
}

/// Writes the default season word to an anime parent.
pub fn apply_season_defaults(repo: &EntryRepo, store: &PrefStore, e: &Entry) -> DataResult<bool> {
    let word = season_default_word(store);
    repo.update_season_flags(e.id, |_| word)
}

/// Applies the item defaults (and, for anime, the season defaults) to every favourite of the kind. Callers run this on a
/// detached task so closing the dialog does not stop it. Returns how many entries changed.
pub fn apply_defaults_to_library(repo: &EntryRepo, store: &PrefStore, kind: Kind) -> DataResult<usize> {
    let mut changed = 0;
    for entry in repo.favorites(kind)? {
        let items = apply_item_defaults(repo, store, &entry)?;
        let seasons = kind.is_anime() && apply_season_defaults(repo, store, &entry)?;
        if items || seasons {
            changed += 1;
        }
    }
    Ok(changed)
}

/// [`apply_defaults_to_library`] on a blocking task the caller does not own: dropping the returned future (the dialog
/// closed) does not stop the run, which continues to every favourite (`withNonCancellableContext`).
pub async fn defaults_to_library_detached(
    repo: EntryRepo,
    store: PrefStore,
    kind: Kind,
) -> Result<usize, DomainError> {
    tokio::task::spawn_blocking(move || apply_defaults_to_library(&repo, &store, kind))
        .await
        .map_err(DomainError::Join)?
        .map_err(DomainError::from)
}

/// The flags a page shows for an entry that is NOT in the library (decision 4): the library defaults, as a copy for this
/// session. The row is never written by showing it; only a user flag change writes the row (and updates the copy, see
/// [`Self::note_item_write`]). An entry in the library has no copy and follows its row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SessionFlags {
    pub item_flags: i64,
    /// Anime only.
    pub season_flags: Option<i64>,
}

impl SessionFlags {
    /// The copy for a non-favourite entry, `None` for a favourite.
    pub fn for_entry(store: &PrefStore, e: &Entry) -> Option<SessionFlags> {
        (!e.favorite).then(|| SessionFlags {
            item_flags: item_default_word(store, e.kind),
            season_flags: e.kind.is_anime().then(|| season_default_word(store)),
        })
    }

    /// Puts the copy over the flags of `e`.
    pub fn overlay(&self, e: &mut Entry) {
        e.item_flags = self.item_flags;
        if self.season_flags.is_some() {
            e.season_flags = self.season_flags;
        }
    }

    /// The copy takes a user's write of one item group, as the row does, or the change would never show.
    pub fn note_item_write(&mut self, flag: i64, mask: i64) {
        self.item_flags = set_flag(self.item_flags, flag, mask);
    }

    /// As [`Self::note_item_write`], for the season word.
    pub fn note_season_write(&mut self, flag: i64, mask: i64) {
        if let Some(word) = &mut self.season_flags {
            *word = set_flag(*word, flag, mask);
        }
    }
}

/// "Set as default" for the items of a kind: writes the groups of the (unified) `word` into the kind's keys. The direction
/// and display keys are the chapter ones for both kinds (Q3). Returns the snackbar key.
pub fn set_item_settings_default(store: &PrefStore, kind: Kind, word: i64) -> &'static str {
    use kuro_db::flags::item as i;
    put_long(store, SHARED_DIRECTION, word & i::SORT_DIR_MASK);
    put_long(store, CHAPTER_DISPLAY, word & i::DISPLAY_MASK);
    match kind {
        Kind::Manga => {
            put_long(store, CHAPTER_UNREAD, word & i::UNDONE_MASK);
            put_long(store, CHAPTER_DOWNLOADED, word & i::DOWNLOADED_MASK);
            put_long(store, CHAPTER_BOOKMARKED, word & i::BOOKMARKED_MASK);
            put_long(store, CHAPTER_SORT, sorting_to_pref(word));
            "updated_default_chapter_settings"
        }
        Kind::Anime => {
            put_long(store, EPISODE_UNSEEN, word & i::UNDONE_MASK);
            put_long(store, EPISODE_DOWNLOADED, word & i::DOWNLOADED_MASK);
            put_long(store, EPISODE_BOOKMARKED, word & i::BOOKMARKED_MASK);
            put_long(store, EPISODE_FILLERMARKED, word & i::FILLERMARKED_MASK);
            put_long(store, EPISODE_SORT, word & i::SORTING_MASK);
            put_long(store, EPISODE_PREVIEWS, word & i::PREVIEWS_MASK);
            put_long(store, EPISODE_SUMMARIES, word & i::SUMMARIES_MASK);
            "updated_default_episode_settings"
        }
    }
}

/// "Set as default" for seasons. The downloaded key takes the downloaded bits (Q2).
pub fn set_season_settings_default(store: &PrefStore, flags: i64) -> &'static str {
    put_long(store, SEASON_DOWNLOADED, flags & se::DOWNLOADED_MASK);
    put_long(store, SEASON_UNSEEN, flags & se::UNSEEN_MASK);
    put_long(store, SEASON_STARTED, flags & se::STARTED_MASK);
    put_long(store, SEASON_COMPLETED, flags & se::COMPLETED_MASK);
    put_long(store, SEASON_BOOKMARKED, flags & se::BOOKMARKED_MASK);
    put_long(store, SEASON_FILLERMARKED, flags & se::FILLERMARKED_MASK);
    put_long(store, SEASON_SORT, flags & se::SORT_MASK);
    put_long(store, SEASON_SORT_DIRECTION, flags & se::SORT_DIR_MASK);
    put_long(store, SEASON_GRID_MODE, (flags & se::GRID_DISPLAY_MODE_MASK) >> se::GRID_DISPLAY_MODE_SHIFT);
    let _ = store.pref::<i32>(SEASON_GRID_SIZE, 0).set(((flags & se::GRID_DISPLAY_SIZE_MASK) >> se::GRID_DISPLAY_SIZE_SHIFT) as i32);
    for (key, overlay) in [
        (SEASON_DOWNLOAD_OVERLAY, SeasonOverlay::Downloaded),
        (SEASON_UNSEEN_OVERLAY, SeasonOverlay::Unseen),
        (SEASON_LOCAL_OVERLAY, SeasonOverlay::Local),
        (SEASON_LANG_OVERLAY, SeasonOverlay::Language),
        (SEASON_CONTINUE_OVERLAY, SeasonOverlay::Continue),
    ] {
        let _ = store.pref::<bool>(key, false).set(flags & overlay.mask() != 0);
    }
    put_long(store, SEASON_DISPLAY, flags & se::DISPLAY_MODE_MASK);
    "updated_default_season_settings"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::test_db;
    fn store() -> PrefStore {
        PrefStore::in_memory()
    }

    #[test]
    fn all_defaults_give_zero_item_words_and_the_season_word() {
        let s = store();
        assert_eq!(item_default_word(&s, Kind::Manga), 0);
        assert_eq!(item_default_word(&s, Kind::Anime), 0);
        assert_eq!(season_default_word(&s), 0x5800000);
    }

    fn entry_repo(kind: Kind, favorite: bool) -> (tempfile::TempDir, EntryRepo, Entry) {
        let (d, db) = test_db();
        let repo = EntryRepo::new(db);
        let mut e = Entry { favorite, source: 1, url: "u".into(), title: "T".into(), ..Entry::create(kind) };
        e.id = repo.insert(&e).unwrap();
        (d, repo, e)
    }

    #[test]
    fn the_default_word_of_a_manga_has_only_the_manga_groups() {
        let s = store();
        set_item_settings_default(&s, Kind::Anime, item::SHOW_NOT_PREVIEWS | item::SHOW_NOT_SUMMARIES | item::SHOW_FILLERMARKED);
        assert_eq!(item_default_word(&s, Kind::Manga) & !item::MANGA_MASK, 0);
        set_item_settings_default(&s, Kind::Manga, item::SORTING_ALPHABET | 0x1);
        assert_eq!(item_default_word(&s, Kind::Manga), item::SORTING_ALPHABET | 0x1);
        assert_eq!(long(&s, CHAPTER_SORT), 0x300, "the sort pref keeps Aniyomi's chapter value");
    }

    #[test]
    fn apply_item_defaults_writes_the_kinds_word_to_the_row() {
        for kind in [Kind::Manga, Kind::Anime] {
            let (_d, repo, e) = entry_repo(kind, false);
            repo.update_item_flags(e.id, |_| 0x201).unwrap();
            let s = store();
            assert!(apply_item_defaults(&repo, &s, &e).unwrap(), "{kind}: the row exists");
            assert_eq!(repo.by_id(e.id).unwrap().item_flags, 0);
            let missing = Entry { id: 9_999, ..e.clone() };
            assert!(!apply_item_defaults(&repo, &s, &missing).unwrap(), "{kind}: a missing row gives false");
        }
    }

    #[test]
    fn season_defaults_write_the_word_and_add_to_library_applies_only_the_item_defaults() {
        let (_d, repo, e) = entry_repo(Kind::Anime, false);
        apply_item_defaults(&repo, &store(), &e).unwrap();
        assert_eq!(repo.by_id(e.id).unwrap().season_flags, Some(0), "season defaults are not applied with the items");
        apply_season_defaults(&repo, &store(), &e).unwrap();
        assert_eq!(repo.by_id(e.id).unwrap().season_flags, Some(0x5800000));
    }

    /// Decision 4: a non-library entry shows the defaults as a copy and the row is never written by showing it.
    #[test]
    fn session_flags_overlay_never_writes_the_row() {
        for kind in [Kind::Manga, Kind::Anime] {
            let (_d, repo, e) = entry_repo(kind, false);
            repo.update_item_flags(e.id, |_| 0x201).unwrap();
            let s = store();
            let mut shown = repo.by_id(e.id).unwrap();
            let mut copy = SessionFlags::for_entry(&s, &shown).expect("a non-favourite has a copy");
            copy.overlay(&mut shown);
            assert_eq!(shown.item_flags, 0, "{kind}: the page shows the defaults");
            assert_eq!(repo.by_id(e.id).unwrap().item_flags, 0x201, "{kind}: the row is untouched");
            // a user write goes to the row (one masked statement) and to the copy
            crate::entries::flag_setters::set_item_flag(&repo, e.id, crate::entries::flag_setters::ItemFlagGroup::Undone, item::SHOW_UNDONE).unwrap();
            copy.note_item_write(item::SHOW_UNDONE, item::UNDONE_MASK);
            assert_eq!(repo.by_id(e.id).unwrap().item_flags, 0x201 | item::SHOW_UNDONE);
            assert_eq!(copy.item_flags & item::UNDONE_MASK, item::SHOW_UNDONE);
            // joining the library drops the copy
            repo.update(&{ let mut u = kuro_data::entry::EntryUpdate::new(e.id); u.favorite = Some(true); u }).unwrap();
            assert_eq!(SessionFlags::for_entry(&s, &repo.by_id(e.id).unwrap()), None, "{kind}: a favourite follows its row");
        }
        let (_d, _repo, anime) = entry_repo(Kind::Anime, false);
        let copy = SessionFlags::for_entry(&store(), &anime).unwrap();
        assert_eq!(copy.season_flags, Some(0x5800000));
        let (_d, _repo, manga) = entry_repo(Kind::Manga, false);
        assert_eq!(SessionFlags::for_entry(&store(), &manga).unwrap().season_flags, None);
    }

    #[test]
    fn apply_defaults_to_library_changes_only_favourites_and_counts_them() {
        for kind in [Kind::Manga, Kind::Anime] {
            let (_d, db) = test_db();
            let repo = EntryRepo::new(db);
            let fav = repo.insert(&Entry { favorite: true, source: 1, url: "f".into(), title: "F".into(), ..Entry::create(kind) }).unwrap();
            let other = repo.insert(&Entry { favorite: false, source: 1, url: "o".into(), title: "O".into(), ..Entry::create(kind) }).unwrap();
            repo.update_item_flags(fav, |_| 0x201).unwrap();
            repo.update_item_flags(other, |_| 0x201).unwrap();
            let s = store();
            set_item_settings_default(&s, kind, item::SHOW_UNDONE);
            let word = item_default_word(&s, kind);
            assert_eq!(apply_defaults_to_library(&repo, &s, kind).unwrap(), 1, "{kind}: only the favourite counts");
            assert_eq!(repo.by_id(fav).unwrap().item_flags, word);
            assert_eq!(repo.by_id(other).unwrap().item_flags, 0x201, "non-favourites keep their word");
        }
    }

    #[tokio::test]
    async fn detached_apply_finishes_after_the_caller_is_aborted() {
        let (_d, repo, e) = entry_repo(Kind::Manga, true);
        repo.update_item_flags(e.id, |_| 0x201).unwrap();
        let s = store();
        set_item_settings_default(&s, Kind::Manga, item::SHOW_UNDONE);
        let caller = tokio::spawn(defaults_to_library_detached(repo.clone(), s.clone(), Kind::Manga));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        caller.abort();
        let expected = item_default_word(&s, Kind::Manga);
        let finished = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while repo.by_id(e.id).unwrap().item_flags != expected {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(finished.is_ok(), "the detached run kept going after its caller was dropped");
    }

    #[test]
    fn set_as_default_round_trips_and_shares_the_direction_key() {
        let s = store();
        let word = item::SHOW_UNDONE | item::SHOW_BOOKMARKED | item::SORT_ASC | item::DISPLAY_NUMBER;
        set_item_settings_default(&s, Kind::Manga, word);
        assert_eq!(item_default_word(&s, Kind::Manga), word);
        set_item_settings_default(&s, Kind::Anime, 0x0);
        assert_eq!(long(&s, SHARED_DIRECTION), 0, "episode direction overwrote the shared key (Q3)");
    }

    #[test]
    fn season_downloaded_default_comes_from_the_downloaded_bits() {
        let s = store();
        set_season_settings_default(&s, se::SHOW_DOWNLOADED | se::SHOW_NOT_STARTED | se::SHOW_UNSEEN);
        assert_eq!(long(&s, SEASON_DOWNLOADED), se::SHOW_DOWNLOADED, "Q2: the downloaded key holds the downloaded group");
        assert_eq!(long(&s, SEASON_STARTED), se::SHOW_NOT_STARTED);
        assert_eq!(long(&s, SEASON_UNSEEN), se::SHOW_UNSEEN);
        assert_eq!(long(&s, SEASON_STARTED) & se::STARTED_MASK, se::SHOW_NOT_STARTED);
    }

    #[test]
    fn snackbar_keys_name_the_three_default_kinds() {
        let s = store();
        assert_eq!(set_item_settings_default(&s, Kind::Manga, 0), "updated_default_chapter_settings");
        assert_eq!(set_item_settings_default(&s, Kind::Anime, 0), "updated_default_episode_settings");
        assert_eq!(set_season_settings_default(&s, 0), "updated_default_season_settings");
    }
}
