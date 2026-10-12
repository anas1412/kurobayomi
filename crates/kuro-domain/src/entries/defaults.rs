//! Default flags of new entries (T-07a-027, T-07a-044). The defaults are the `default_*` keys of the preference
//! registry. Applying them writes the resulting word to the entry; "set as default" writes an entry's raw groups
//! back into the same keys. Two keys are shared between chapter and episode (Q3), and the season downloaded
//! default is written from the downloaded bits (Q2).

use kuro_data::DataResult;
use kuro_data::anime::{Anime, AnimeRepo};
use kuro_data::manga::{Manga, MangaRepo};
use kuro_prefs::store::PrefStore;

use crate::error::DomainError;

use super::anime::{episode_flags as ep, season_flags as se};
use super::flag_setters::{SeasonAll, SeasonOverlay, chapter_all, episode_all, season_all};

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

fn long(store: &PrefStore, key: &'static str) -> i64 {
    store.pref::<i64>(key, 0).get()
}

fn flag(store: &PrefStore, key: &'static str, default: bool) -> bool {
    store.pref::<bool>(key, default).get()
}

fn put_long(store: &PrefStore, key: &'static str, value: i64) {
    store.pref::<i64>(key, 0).set(value);
}

/// The default chapter word, as the preferences describe it.
pub fn chapter_default_word(store: &PrefStore) -> i64 {
    chapter_all(
        long(store, CHAPTER_UNREAD),
        long(store, CHAPTER_DOWNLOADED),
        long(store, CHAPTER_BOOKMARKED),
        long(store, CHAPTER_SORT),
        long(store, SHARED_DIRECTION),
        long(store, CHAPTER_DISPLAY),
    )
}

/// The default episode word, as the preferences describe it.
pub fn episode_default_word(store: &PrefStore) -> i64 {
    episode_all(
        long(store, EPISODE_UNSEEN),
        long(store, EPISODE_DOWNLOADED),
        long(store, EPISODE_BOOKMARKED),
        long(store, EPISODE_FILLERMARKED),
        long(store, EPISODE_SORT),
        long(store, SHARED_DIRECTION),
        long(store, CHAPTER_DISPLAY),
        long(store, EPISODE_PREVIEWS),
        long(store, EPISODE_SUMMARIES),
    )
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

/// Writes the default chapter word to a manga (`applyDefaultChapterFlags`).
pub fn apply_default_chapter_flags(repo: &MangaRepo, store: &PrefStore, manga: &Manga) -> DataResult<bool> {
    let word = chapter_default_word(store);
    repo.update_chapter_flags(manga.id, |_| word)
}

/// Writes the default episode word to an anime.
pub fn apply_default_episode_flags(repo: &AnimeRepo, store: &PrefStore, anime: &Anime) -> DataResult<bool> {
    let word = episode_default_word(store);
    repo.update_episode_flags(anime.id, |_| word)
}

/// Writes the default season word to an anime parent.
pub fn apply_default_season_flags(repo: &AnimeRepo, store: &PrefStore, anime: &Anime) -> DataResult<bool> {
    let word = season_default_word(store);
    repo.update_season_flags(anime.id, |_| word)
}

/// The entry-details first open of a non-favourite manga: its chapter defaults are applied; a favourite keeps its
/// stored word. Returns whether the defaults were written.
pub fn on_open_manga(repo: &MangaRepo, store: &PrefStore, manga: &Manga) -> DataResult<bool> {
    if manga.favorite {
        return Ok(false);
    }
    apply_default_chapter_flags(repo, store, manga)
}

/// The first open of a non-favourite anime: episode and season defaults. A favourite keeps its stored words.
pub fn on_open_anime(repo: &AnimeRepo, store: &PrefStore, anime: &Anime) -> DataResult<bool> {
    if anime.favorite {
        return Ok(false);
    }
    let episodes = apply_default_episode_flags(repo, store, anime)?;
    let seasons = apply_default_season_flags(repo, store, anime)?;
    Ok(episodes || seasons)
}

/// Browse "add to library" for a manga: chapter defaults (no season defaults exist for manga).
pub fn on_add_to_library_manga(repo: &MangaRepo, store: &PrefStore, manga: &Manga) -> DataResult<bool> {
    apply_default_chapter_flags(repo, store, manga)
}

/// Browse "add to library" for an anime: episode defaults only, not the season defaults.
pub fn on_add_to_library_anime(repo: &AnimeRepo, store: &PrefStore, anime: &Anime) -> DataResult<bool> {
    apply_default_episode_flags(repo, store, anime)
}

/// Applies the chapter defaults to every favourite manga. Callers run this on a detached task so closing the dialog
/// does not stop it. Returns how many entries changed.
pub fn apply_chapter_defaults_to_library(repo: &MangaRepo, store: &PrefStore) -> DataResult<usize> {
    let mut changed = 0;
    for manga in repo.favorites()? {
        if apply_default_chapter_flags(repo, store, &manga)? {
            changed += 1;
        }
    }
    Ok(changed)
}

/// Applies the episode and season defaults to every favourite anime.
pub fn apply_anime_defaults_to_library(repo: &AnimeRepo, store: &PrefStore) -> DataResult<usize> {
    let mut changed = 0;
    for anime in repo.favorites()? {
        let episodes = apply_default_episode_flags(repo, store, &anime)?;
        let seasons = apply_default_season_flags(repo, store, &anime)?;
        if episodes || seasons {
            changed += 1;
        }
    }
    Ok(changed)
}

/// [`apply_chapter_defaults_to_library`] on a blocking task the caller does not own: dropping the returned future
/// (the dialog closed) does not stop the run, which continues to every favourite (`withNonCancellableContext`).
pub async fn chapter_defaults_to_library_detached(repo: MangaRepo, store: PrefStore) -> Result<usize, DomainError> {
    tokio::task::spawn_blocking(move || apply_chapter_defaults_to_library(&repo, &store))
        .await
        .map_err(DomainError::Join)?
        .map_err(DomainError::from)
}

/// [`apply_anime_defaults_to_library`] on a detached blocking task, as [`chapter_defaults_to_library_detached`].
pub async fn anime_defaults_to_library_detached(repo: AnimeRepo, store: PrefStore) -> Result<usize, DomainError> {
    tokio::task::spawn_blocking(move || apply_anime_defaults_to_library(&repo, &store))
        .await
        .map_err(DomainError::Join)?
        .map_err(DomainError::from)
}

/// "Set as default" for chapters: writes the groups of `flags` into the chapter keys. Returns the snackbar key.
pub fn set_chapter_settings_default(store: &PrefStore, flags: i64) -> &'static str {
    use kuro_db::flags::chapter as c;
    put_long(store, CHAPTER_UNREAD, flags & c::UNREAD_MASK);
    put_long(store, CHAPTER_DOWNLOADED, flags & c::DOWNLOADED_MASK);
    put_long(store, CHAPTER_BOOKMARKED, flags & c::BOOKMARKED_MASK);
    put_long(store, CHAPTER_SORT, flags & c::SORTING_MASK);
    put_long(store, SHARED_DIRECTION, flags & c::SORT_DIR_MASK);
    put_long(store, CHAPTER_DISPLAY, flags & c::DISPLAY_MASK);
    "updated_default_chapter_settings"
}

/// "Set as default" for episodes. The direction and display keys are the chapter ones (Q3).
pub fn set_episode_settings_default(store: &PrefStore, flags: i64) -> &'static str {
    put_long(store, EPISODE_UNSEEN, flags & ep::UNSEEN_MASK);
    put_long(store, EPISODE_DOWNLOADED, flags & ep::DOWNLOADED_MASK);
    put_long(store, EPISODE_BOOKMARKED, flags & ep::BOOKMARKED_MASK);
    put_long(store, EPISODE_FILLERMARKED, flags & ep::FILLERMARKED_MASK);
    put_long(store, EPISODE_SORT, flags & ep::SORTING_MASK);
    put_long(store, SHARED_DIRECTION, flags & ep::SORT_DIR_MASK);
    put_long(store, CHAPTER_DISPLAY, flags & ep::DISPLAY_MASK);
    put_long(store, EPISODE_PREVIEWS, flags & ep::PREVIEWS_MASK);
    put_long(store, EPISODE_SUMMARIES, flags & ep::SUMMARIES_MASK);
    "updated_default_episode_settings"
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
    put_long(store, SEASON_GRID_MODE, (flags & se::GRID_DISPLAY_MODE_MASK) >> se::GRID_DISPLAY_MODE_BIT_OFFSET);
    store.pref::<i32>(SEASON_GRID_SIZE, 0).set(((flags & se::GRID_DISPLAY_SIZE_MASK) >> se::GRID_DISPLAY_SIZE_BIT_OFFSET) as i32);
    for (key, overlay) in [
        (SEASON_DOWNLOAD_OVERLAY, SeasonOverlay::Downloaded),
        (SEASON_UNSEEN_OVERLAY, SeasonOverlay::Unseen),
        (SEASON_LOCAL_OVERLAY, SeasonOverlay::Local),
        (SEASON_LANG_OVERLAY, SeasonOverlay::Language),
        (SEASON_CONTINUE_OVERLAY, SeasonOverlay::Continue),
    ] {
        store.pref::<bool>(key, false).set(flags & overlay.mask() != 0);
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
    fn all_defaults_give_zero_chapter_and_episode_words_and_the_season_word() {
        let s = store();
        assert_eq!(chapter_default_word(&s), 0);
        assert_eq!(episode_default_word(&s), 0);
        assert_eq!(season_default_word(&s), 0x5800000);
    }

    #[test]
    fn a_non_favourite_opened_with_defaults_zero_loses_its_stored_chapter_word() {
        let (_d, db) = test_db();
        let repo = MangaRepo::new(db);
        let mut manga = Manga { favorite: false, source: 1, url: "u".into(), title: "T".into(), ..kuro_testkit::domain::manga() };
        manga.id = repo.insert(&manga).unwrap();
        repo.update_chapter_flags(manga.id, |_| 0x101).unwrap();
        assert!(on_open_manga(&repo, &store(), &manga).unwrap());
        assert_eq!(repo.by_id(manga.id).unwrap().chapter_flags, 0);
    }

    #[test]
    fn a_favourite_keeps_its_stored_word_when_opened() {
        let (_d, db) = test_db();
        let repo = MangaRepo::new(db);
        let mut manga = Manga { favorite: true, source: 1, url: "u".into(), title: "T".into(), ..kuro_testkit::domain::manga() };
        manga.id = repo.insert(&manga).unwrap();
        repo.update_chapter_flags(manga.id, |_| 0x101).unwrap();
        assert!(!on_open_manga(&repo, &store(), &manga).unwrap());
        assert_eq!(repo.by_id(manga.id).unwrap().chapter_flags, 0x101);
    }

    #[test]
    fn season_defaults_write_the_word_and_browse_add_applies_only_episode_defaults() {
        let (_d, db) = test_db();
        let repo = AnimeRepo::new(db);
        let mut anime = Anime { favorite: false, source: 1, url: "a".into(), title: "A".into(), ..kuro_testkit::domain::anime() };
        anime.id = repo.insert(&anime).unwrap();
        on_add_to_library_anime(&repo, &store(), &anime).unwrap();
        let row = repo.by_id(anime.id).unwrap();
        assert_eq!(row.episode_flags, 0);
        assert_eq!(row.season_flags, 0, "season defaults are not applied on add");
        on_open_anime(&repo, &store(), &anime).unwrap();
        assert_eq!(repo.by_id(anime.id).unwrap().season_flags, 0x5800000);
    }

    #[test]
    fn set_as_default_round_trips_and_shares_the_direction_key() {
        let s = store();
        set_chapter_settings_default(&s, 0x2 | 0x20 | 0x1 | 0x100000);
        assert_eq!(chapter_default_word(&s), 0x2 | 0x20 | 0x1 | 0x100000);
        set_episode_settings_default(&s, 0x0);
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
        assert_eq!(set_chapter_settings_default(&s, 0), "updated_default_chapter_settings");
        assert_eq!(set_episode_settings_default(&s, 0), "updated_default_episode_settings");
        assert_eq!(set_season_settings_default(&s, 0), "updated_default_season_settings");
    }

    #[test]
    fn apply_anime_defaults_to_library_changes_only_favourites_and_counts_them() {
        let (_d, db) = test_db();
        let repo = AnimeRepo::new(db);
        let mut fav = Anime { favorite: true, source: 1, url: "f".into(), title: "F".into(), ..kuro_testkit::domain::anime() };
        fav.id = repo.insert(&fav).unwrap();
        let mut other = Anime { favorite: false, source: 1, url: "o".into(), title: "O".into(), ..kuro_testkit::domain::anime() };
        other.id = repo.insert(&other).unwrap();
        repo.update_episode_flags(fav.id, |_| 0x101).unwrap();
        repo.update_episode_flags(other.id, |_| 0x101).unwrap();
        let s = store();
        set_episode_settings_default(&s, ep::SHOW_UNSEEN);
        let word = episode_default_word(&s);
        assert_eq!(apply_anime_defaults_to_library(&repo, &s).unwrap(), 1, "only the favourite counts");
        assert_eq!(repo.by_id(fav.id).unwrap().episode_flags, word);
        assert_eq!(repo.by_id(other.id).unwrap().episode_flags, 0x101, "non-favourites keep their word");
    }

    #[tokio::test]
    async fn detached_apply_finishes_after_the_caller_is_aborted() {
        let (_d, db) = test_db();
        let repo = MangaRepo::new(db);
        let mut fav = Manga { favorite: true, source: 1, url: "f".into(), title: "F".into(), ..kuro_testkit::domain::manga() };
        fav.id = repo.insert(&fav).unwrap();
        repo.update_chapter_flags(fav.id, |_| 0x101).unwrap();
        let s = store();
        set_chapter_settings_default(&s, 0x2);
        let caller = tokio::spawn(chapter_defaults_to_library_detached(repo.clone(), s.clone()));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        caller.abort();
        let expected = chapter_default_word(&s);
        let finished = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while repo.by_id(fav.id).unwrap().chapter_flags != expected {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(finished.is_ok(), "the detached run kept going after its caller was dropped");
    }

    #[test]
    fn apply_to_library_changes_only_favourites() {
        let (_d, db) = test_db();
        let repo = MangaRepo::new(db);
        let mut fav = Manga { favorite: true, source: 1, url: "f".into(), title: "F".into(), ..kuro_testkit::domain::manga() };
        fav.id = repo.insert(&fav).unwrap();
        repo.update_chapter_flags(fav.id, |_| 0x101).unwrap();
        let s = store();
        set_chapter_settings_default(&s, 0x2);
        assert_eq!(apply_chapter_defaults_to_library(&repo, &s).unwrap(), 1);
        assert_eq!(repo.by_id(fav.id).unwrap().chapter_flags, 0x2);
    }
}
