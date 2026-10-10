//! The database side of the anime page: the entry, its episodes and seasons, the open-time defaults, the masked flag
//! writes and the live subscriptions. Every method is synchronous; the runner calls them on the blocking pool.
//! Nothing here imports GTK. Upstream: `AnimeScreenModel.kt:204-283` (init), `SetAnimeEpisodeFlags.kt`.

use std::sync::Arc;

use kuro_data::DataResult;
use kuro_data::category::{CategoryKind, CategoryRepo};
use kuro_data::anime::{Anime, AnimeRepo, AnimeUpdate, SeasonAnime};
use kuro_data::episode::{Episode, EpisodeRepo, EpisodeUpdate};
use kuro_db::handle::Db;
use kuro_db::subscribe::Subscription;
use kuro_data::anime::AnimeRelationGroup;
use kuro_data::anime_relation::AnimeRelationRepo;
use kuro_domain::entries::defaults::{apply_default_episode_flags, apply_default_season_flags};
use kuro_domain::entries::fetch_interval::{get_window, set_anime_fetch_interval};
use kuro_prefs::store::PrefStore;

use super::actions::Mark;
use super::images::{ImageKind, bumped_last_modified};
use super::season_settings;
use super::settings::sort_click;

/// Typed access to the tables of one anime page. Cheap to clone: it shares one `Db`.
#[derive(Clone)]
pub struct AnimeStore {
    animes: AnimeRepo,
    episodes: EpisodeRepo,
    categories: CategoryRepo,
    db: Arc<Db>,
}

impl AnimeStore {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            animes: AnimeRepo::new(Arc::clone(&db)),
            episodes: EpisodeRepo::new(Arc::clone(&db)),
            categories: CategoryRepo::new(Arc::clone(&db), CategoryKind::Anime),
            db,
        }
    }

    /// The database handle, for the pieces that work on it directly (the source refresh).
    pub fn db(&self) -> Arc<Db> {
        Arc::clone(&self.db)
    }

    /// The ids of the categories the entry is in, for the auto-download rule. Empty means uncategorized.
    pub fn category_ids(&self, anime_id: i64) -> DataResult<Vec<i64>> {
        Ok(self.categories.by_entry(anime_id)?.into_iter().map(|c| c.id).collect())
    }

    /// How many trackers the entry is bound to, for the tracking button of the action row.
    pub fn track_count(&self, anime_id: i64) -> DataResult<usize> {
        Ok(kuro_data::track::AnimeTrackRepo::new(Arc::clone(&self.db)).by_entry(anime_id)?.len())
    }

    /// Every category (the default row `0` included, the dialog drops it) and the ids the entry is linked to.
    pub fn categories(&self, anime_id: i64) -> DataResult<(Vec<(i64, String)>, Vec<i64>)> {
        let all = self.categories.all()?.into_iter().map(|cat| (cat.id, cat.name)).collect();
        Ok((all, self.category_ids(anime_id)?))
    }

    /// Replaces the category links of the entry in one transaction.
    pub fn set_categories(&self, anime_id: i64, category_ids: &[i64]) -> DataResult<()> {
        self.animes.set_categories(anime_id, category_ids)
    }

    /// The entry, or `None` when the row does not exist.
    pub fn anime(&self, id: i64) -> DataResult<Option<Anime>> {
        match self.animes.by_id(id) {
            Ok(anime) => Ok(Some(anime)),
            Err(kuro_data::DataError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn episodes(&self, anime_id: i64) -> DataResult<Vec<Episode>> {
        self.episodes.by_anime_id(anime_id)
    }

    pub fn seasons(&self, parent_id: i64) -> DataResult<Vec<SeasonAnime>> {
        self.animes.seasons_by_parent(parent_id)
    }

    /// One masked UPDATE of `animes.episode_flags`: `(old & !mask) | (flag & mask)`.
    pub fn set_flag_bits(&self, anime_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.animes.set_episode_flags_masked(anime_id, flag, mask)
    }

    /// The skip-intro length of the entry: the length bits and the disable bit, in one masked write.
    pub fn set_skip_intro_length(&self, anime_id: i64, length: i32) -> DataResult<bool> {
        kuro_domain::app::entries::set_anime_skip_intro_length(&self.animes, anime_id, i64::from(length))
    }

    /// Sort by `mode`, or flip the direction when `mode` is the stored key. The word is read and written inside one
    /// writer transaction, so two quick taps cannot both see the same word.
    pub fn sort_by(&self, anime_id: i64, mode: i64) -> DataResult<bool> {
        self.animes.update_episode_flags(anime_id, |word| sort_click(word, mode))
    }

    /// One masked UPDATE of `animes.season_flags`: the write of a click in the season settings window.
    pub fn set_season_flag_bits(&self, anime_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.animes.set_season_flags_masked(anime_id, flag, mask)
    }

    /// A click on a Sort row of the season settings: the active key flips its direction, another key starts ascending.
    /// Read and written in one transaction, like the episode sort.
    pub fn season_sort_by(&self, anime_id: i64, code: i64) -> DataResult<bool> {
        self.animes.update_season_flags(anime_id, |word| season_settings::sort_click(word, code))
    }

    /// "Set as default" with the library checkbox on, for the episode settings: the episode default word goes to every
    /// favorite, and nothing else does (`setCurrentSettingsAsDefault` calls `setAnimeDefaultEpisodeFlags` only).
    /// Returns how many entries changed.
    pub fn apply_episode_defaults_to_library(&self, prefs: &PrefStore) -> DataResult<usize> {
        let mut changed = 0;
        for anime in self.animes.favorites()? {
            if apply_default_episode_flags(&self.animes, prefs, &anime)? {
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// The same for the season settings: the season default word goes to every favorite, the episode flags stay.
    pub fn apply_season_defaults_to_library(&self, prefs: &PrefStore) -> DataResult<usize> {
        let mut changed = 0;
        for anime in self.animes.favorites()? {
            if apply_default_season_flags(&self.animes, prefs, &anime)? {
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// The library entries other than this one with the same title, case folded (the duplicate check of an add).
    pub fn duplicates(&self, anime_id: i64) -> DataResult<Vec<Anime>> {
        let Some(anime) = self.anime(anime_id)? else { return Ok(Vec::new()) };
        self.animes.duplicates(anime.id, &anime.title)
    }

    /// The related groups of the entry, published again after a change to the relations or the anime they link.
    pub fn watch_related(&self, anime_id: i64) -> Subscription<Vec<AnimeRelationGroup>> {
        AnimeRelationRepo::new(self.db()).subscribe_related(anime_id)
    }

    /// An edit or a delete of a custom image: the last-modified of that image moves forward, so every cache keyed by
    /// it loads the picture again (T-I12). `Ok(false)` when the row is gone.
    pub fn bump_image(&self, anime_id: i64, kind: ImageKind, now_ms: i64) -> DataResult<bool> {
        let Some(anime) = self.anime(anime_id)? else { return Ok(false) };
        let mut update = AnimeUpdate::new(anime_id);
        match kind {
            ImageKind::Cover => update.cover_last_modified = Some(bumped_last_modified(anime.cover_last_modified, now_ms)),
            ImageKind::Background => update.background_last_modified = Some(bumped_last_modified(anime.background_last_modified, now_ms)),
        }
        self.animes.update(&update)?;
        Ok(true)
    }

    /// One masked UPDATE of `animes.viewer_flags`, so a write never reads back and touches the other groups (E-AIR-1).
    pub fn set_viewer_bits(&self, anime_id: i64, value: i64, mask: i64) -> DataResult<bool> {
        self.animes.set_viewer_bits(anime_id, value, mask)
    }

    /// The favorite write: `favorite` and `date_added` (now when it is added, 0 when it is removed) in one update.
    /// `Ok(false)` when the row is gone.
    pub fn set_favorite(&self, anime_id: i64, favorite: bool, now_ms: i64) -> DataResult<bool> {
        if self.anime(anime_id)?.is_none() {
            return Ok(false);
        }
        self.animes.update(&AnimeUpdate {
            favorite: Some(favorite),
            date_added: Some(if favorite { now_ms } else { 0 }),
            ..AnimeUpdate::new(anime_id)
        })?;
        Ok(true)
    }

    /// Adds the entry to the library: the favorite write, then the library defaults for its episode and season flags
    /// (T-39b-012: defaults are written when it joins the library, not when it is opened). `Ok(false)` when the row is gone.
    pub fn add_to_library(&self, anime_id: i64, prefs: &PrefStore, now_ms: i64) -> DataResult<bool> {
        let Some(anime) = self.anime(anime_id)? else { return Ok(false) };
        self.set_favorite(anime_id, true, now_ms)?;
        apply_default_episode_flags(&self.animes, prefs, &anime)?;
        apply_default_season_flags(&self.animes, prefs, &anime)?;
        Ok(true)
    }

    /// Sets the bookmark or fillermark of exactly these episodes, in one transaction. The caller passes the ids that
    /// differ (`actions::bulk_mark_targets`), so no row is written that does not change.
    pub fn set_marks(&self, ids: &[i64], mark: Mark, on: bool) -> DataResult<()> {
        let updates: Vec<EpisodeUpdate> = ids
            .iter()
            .map(|id| match mark {
                Mark::Bookmark => EpisodeUpdate { bookmark: Some(on), ..EpisodeUpdate::new(*id) },
                Mark::Fillermark => EpisodeUpdate { fillermark: Some(on), ..EpisodeUpdate::new(*id) },
            })
            .collect();
        if updates.is_empty() { Ok(()) } else { self.episodes.update_all(&updates) }
    }

    /// `setFetchInterval` for an anime: stores `-wheel` (0 is automatic) and recomputes the next update (T-39a-026).
    pub fn set_fetch_interval(&self, anime_id: i64, wheel: i32) -> DataResult<()> {
        let anime = self.animes.by_id(anime_id)?;
        let now = chrono::Local::now();
        let window = get_window(&now);
        set_anime_fetch_interval(&self.animes, &self.episodes, &anime, wheel, &now, window, now.timestamp_millis())
    }

    /// Marks these episodes seen or unseen in one transaction. Unseen also resets the position (`SetSeenStatus`).
    pub fn set_seen(&self, ids: &[i64], seen: bool) -> DataResult<()> {
        let updates: Vec<EpisodeUpdate> = ids
            .iter()
            .map(|id| EpisodeUpdate { seen: Some(seen), last_second_seen: (!seen).then_some(0), ..EpisodeUpdate::new(*id) })
            .collect();
        if updates.is_empty() { Ok(()) } else { self.episodes.update_all(&updates) }
    }

    pub fn watch_anime(&self, anime_id: i64) -> Subscription<Option<Anime>> {
        self.animes.subscribe_by_id(anime_id)
    }

    pub fn watch_episodes(&self, anime_id: i64) -> Subscription<Vec<Episode>> {
        self.episodes.subscribe_by_anime_id(anime_id)
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    use kuro_db::schema_anime::FetchType;

    use super::*;

    impl AnimeStore {
        pub fn set_fetch_type_for_test(&self, id: i64, fetch_type: FetchType) {
            let update = kuro_data::anime::AnimeUpdate {
                fetch_type: Some(fetch_type),
                ..kuro_data::anime::AnimeUpdate::new(id)
            };
            self.animes.update(&update).expect("update");
        }
    }

    /// A migrated temp anime database with one entry (`favorite`, `initialized`) and `episodes` episodes, the first
    /// `seen` of them seen. Keep the `TempDir` alive.
    pub fn with_episodes(favorite: bool, initialized: bool, episodes: usize, seen: usize) -> (tempfile::TempDir, AnimeStore, i64) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("anime.db");
        let writer = kuro_db::schema_anime::open(&path).expect("anime schema");
        let db = Arc::new(Db::new(&path, writer).expect("anime handle"));
        let id = AnimeRepo::new(Arc::clone(&db))
            .insert(&Anime {
                source: 7,
                url: "/a".into(),
                title: "Frieren".into(),
                favorite,
                initialized,
                ..Anime::create()
            })
            .expect("insert anime");
        let list: Vec<Episode> = (1..=episodes)
            .map(|n| Episode {
                anime_id: id,
                url: format!("/e{n}"),
                name: format!("Episode {n}"),
                episode_number: n as f64,
                // a source lists the newest episode first, so the oldest has the highest order
                source_order: (episodes - n) as i64,
                seen: n <= seen,
                ..Episode::create()
            })
            .collect();
        EpisodeRepo::new(Arc::clone(&db)).add_all(&list).expect("insert episodes");
        (dir, AnimeStore::new(db), id)
    }
}

#[cfg(test)]
mod tests {
    use kuro_domain::entries::anime::episode_flags as ep;

    use super::super::actions::{Mark, bulk_mark_targets};
    use super::fixture::with_episodes;
    use super::*;

    #[test]
    fn favorite_writes_the_flag_and_the_date_and_a_missing_row_is_false() {
        let (_dir, store, id) = with_episodes(false, true, 0, 0);
        assert!(store.set_favorite(id, true, 5_000).unwrap());
        let row = store.anime(id).unwrap().unwrap();
        assert!(row.favorite && row.date_added == 5_000);
        assert!(store.set_favorite(id, false, 6_000).unwrap());
        let row = store.anime(id).unwrap().unwrap();
        assert!(!row.favorite && row.date_added == 0);
        assert!(!store.set_favorite(4242, true, 1).unwrap());
    }

    #[test]
    fn adding_to_the_library_writes_the_default_flags_and_opening_does_not() {
        let (_dir, store, id) = with_episodes(false, true, 1, 0);
        let prefs = PrefStore::in_memory();
        kuro_domain::entries::defaults::set_episode_settings_default(&prefs, ep::SHOW_UNSEEN | ep::SORT_ASC | ep::SORTING_NUMBER);
        assert_eq!(store.anime(id).unwrap().unwrap().episode_flags, 0, "the stored word is untouched until the add");
        assert!(store.add_to_library(id, &prefs, 9).unwrap());
        let row = store.anime(id).unwrap().unwrap();
        assert!(row.favorite);
        assert_eq!(row.episode_flags & ep::UNSEEN_MASK, ep::SHOW_UNSEEN, "the library default reached the row");
        assert!(!store.add_to_library(4242, &prefs, 9).unwrap());
    }

    #[test]
    fn bulk_marks_write_only_the_episodes_that_differ_and_each_flag_alone() {
        let (_dir, store, id) = with_episodes(true, true, 3, 0);
        let all = store.episodes(id).unwrap();
        store.set_marks(&[all[0].id], Mark::Bookmark, true).unwrap();
        let fresh = store.episodes(id).unwrap();
        let targets = bulk_mark_targets(&fresh, Mark::Bookmark, true);
        assert_eq!(targets.len(), 2, "the bookmarked one is left out");
        let before: Vec<i64> = fresh.iter().map(|e| e.version).collect();
        store.set_marks(&targets, Mark::Bookmark, true).unwrap();
        let after = store.episodes(id).unwrap();
        assert!(after.iter().all(|e| e.bookmark && !e.fillermark), "only the bookmark flag changed");
        assert_eq!(after.iter().find(|e| e.id == all[0].id).unwrap().version, before[fresh.iter().position(|e| e.id == all[0].id).unwrap()], "the row that did not differ was not written");
        store.set_marks(&[all[1].id], Mark::Fillermark, true).unwrap();
        assert!(store.episodes(id).unwrap().iter().find(|e| e.id == all[1].id).unwrap().fillermark);
        store.set_marks(&[], Mark::Bookmark, false).unwrap();
    }

    #[test]
    fn seen_and_unseen_writes_reset_the_position_only_for_unseen() {
        let (_dir, store, id) = with_episodes(true, true, 2, 0);
        let ids: Vec<i64> = store.episodes(id).unwrap().iter().map(|e| e.id).collect();
        store.set_seen(&ids, true).unwrap();
        assert!(store.episodes(id).unwrap().iter().all(|e| e.seen));
        store.set_seen(&ids[..1], false).unwrap();
        let rows = store.episodes(id).unwrap();
        assert_eq!(rows.iter().filter(|e| e.seen).count(), 1);
        assert_eq!(rows.iter().find(|e| e.id == ids[0]).unwrap().last_second_seen, 0);
    }

    #[test]
    fn categories_list_every_row_with_the_links_of_the_entry_and_set_replaces_them() {
        let (_dir, store, id) = with_episodes(true, true, 0, 0);
        let repo = CategoryRepo::new(store.db(), CategoryKind::Anime);
        let a = repo.insert(&kuro_data::category::Category { id: 0, name: "Airing".into(), order: 1, flags: 0, hidden: false }).unwrap();
        let b = repo.insert(&kuro_data::category::Category { id: 0, name: "Later".into(), order: 2, flags: 0, hidden: false }).unwrap();
        store.set_categories(id, &[a]).unwrap();
        let (all, linked) = store.categories(id).unwrap();
        assert!(all.iter().any(|(i, n)| *i == a && n == "Airing") && all.iter().any(|(i, _)| *i == b));
        assert_eq!(linked, vec![a]);
        store.set_categories(id, &[b, b]).unwrap();
        assert_eq!(store.categories(id).unwrap().1, vec![b], "a repeated id is linked once");
        store.set_categories(id, &[]).unwrap();
        assert!(store.categories(id).unwrap().1.is_empty());
    }

    #[test]
    fn the_track_count_is_the_number_of_bound_trackers() {
        let (_dir, store, id) = with_episodes(true, true, 0, 0);
        assert_eq!(store.track_count(id).unwrap(), 0);
        let repo = kuro_data::track::AnimeTrackRepo::new(store.db());
        repo.insert(&kuro_data::track::AnimeTrack { anime_id: id, ..kuro_data::track::AnimeTrack::new(2) }).unwrap();
        assert_eq!(store.track_count(id).unwrap(), 1);
    }

    #[test]
    fn reads_the_entry_and_its_episodes_and_a_missing_row_is_none() {
        let (_dir, store, id) = with_episodes(true, true, 3, 1);
        assert_eq!(store.anime(id).unwrap().unwrap().title, "Frieren");
        assert_eq!(store.episodes(id).unwrap().len(), 3);
        assert!(store.anime(9999).unwrap().is_none());
    }

    #[test]
    fn a_flag_write_changes_only_its_group_and_a_sort_click_flips_the_direction() {
        let (_dir, store, id) = with_episodes(true, true, 0, 0);
        store.set_flag_bits(id, ep::SHOW_UNSEEN, ep::UNSEEN_MASK).unwrap();
        assert_eq!(store.anime(id).unwrap().unwrap().episode_flags & ep::UNSEEN_MASK, ep::SHOW_UNSEEN);
        store.sort_by(id, ep::SORTING_NUMBER).unwrap();
        let word = store.anime(id).unwrap().unwrap().episode_flags;
        assert_eq!((word & ep::SORTING_MASK, word & ep::SORT_DIR_MASK), (ep::SORTING_NUMBER, ep::SORT_ASC));
        assert_eq!(word & ep::UNSEEN_MASK, ep::SHOW_UNSEEN, "the filter group survives a sort");
        store.sort_by(id, ep::SORTING_NUMBER).unwrap();
        assert_eq!(store.anime(id).unwrap().unwrap().episode_flags & ep::SORT_DIR_MASK, ep::SORT_DESC);
    }

    #[test]
    fn the_episode_defaults_reach_every_favorite_and_leave_their_season_flags_alone() {
        use kuro_domain::entries::anime::season_flags as sf;
        let (_dir, store, id) = with_episodes(true, true, 1, 0);
        store.set_season_flag_bits(id, sf::SHOW_UNSEEN, sf::UNSEEN_MASK).unwrap();
        let prefs = PrefStore::in_memory();
        kuro_domain::entries::defaults::set_episode_settings_default(&prefs, ep::SHOW_SEEN | ep::SORT_ASC | ep::SORTING_NUMBER);
        assert_eq!(store.apply_episode_defaults_to_library(&prefs).unwrap(), 1);
        let row = store.anime(id).unwrap().unwrap();
        assert_eq!(row.episode_flags & ep::UNSEEN_MASK, ep::SHOW_SEEN, "the episode default was applied");
        assert_eq!(row.season_flags & sf::UNSEEN_MASK, sf::SHOW_UNSEEN, "the season flags are not the user's request");
    }

    #[test]
    fn the_season_defaults_reach_every_favorite_and_leave_their_episode_flags_alone() {
        use kuro_domain::entries::anime::season_flags as sf;
        let (_dir, store, id) = with_episodes(true, true, 1, 0);
        store.set_flag_bits(id, ep::SHOW_UNSEEN, ep::UNSEEN_MASK).unwrap();
        let prefs = PrefStore::in_memory();
        kuro_domain::entries::defaults::set_season_settings_default(&prefs, sf::SHOW_COMPLETED);
        assert_eq!(store.apply_season_defaults_to_library(&prefs).unwrap(), 1);
        let row = store.anime(id).unwrap().unwrap();
        assert_eq!(row.season_flags & sf::COMPLETED_MASK, sf::SHOW_COMPLETED);
        assert_eq!(row.episode_flags & ep::UNSEEN_MASK, ep::SHOW_UNSEEN);
    }

    #[test]
    fn season_writes_are_masked_and_a_sort_click_flips_the_active_key() {
        use kuro_domain::entries::anime::season_flags as sf;
        let (_dir, store, id) = with_episodes(true, true, 0, 0);
        store.set_season_flag_bits(id, sf::SHOW_STARTED, sf::STARTED_MASK).unwrap();
        store.set_season_flag_bits(id, sf::SHOW_BOOKMARKED, sf::BOOKMARKED_MASK).unwrap();
        let word = store.anime(id).unwrap().unwrap().season_flags;
        assert_eq!((word & sf::STARTED_MASK, word & sf::BOOKMARKED_MASK), (sf::SHOW_STARTED, sf::SHOW_BOOKMARKED));
        store.season_sort_by(id, sf::SORT_UPLOAD).unwrap();
        let word = store.anime(id).unwrap().unwrap().season_flags;
        assert_eq!((word & sf::SORT_MASK, word & sf::SORT_DIR_MASK), (sf::SORT_UPLOAD, sf::SORT_ASC));
        store.season_sort_by(id, sf::SORT_UPLOAD).unwrap();
        assert_eq!(store.anime(id).unwrap().unwrap().season_flags & sf::SORT_DIR_MASK, sf::SORT_DESC, "the second click flips it");
    }

    #[test]
    fn a_library_entry_with_the_same_title_is_a_duplicate_and_the_entry_itself_is_not() {
        let (_dir, store, id) = with_episodes(false, true, 0, 0);
        assert!(store.duplicates(id).unwrap().is_empty());
        let twin = AnimeRepo::new(store.db())
            .insert(&Anime { source: 8, url: "/b".into(), title: "FRIEREN".into(), favorite: true, ..Anime::create() })
            .unwrap();
        let found = store.duplicates(id).unwrap();
        assert_eq!(found.iter().map(|a| a.id).collect::<Vec<_>>(), vec![twin]);
        assert!(store.duplicates(twin).unwrap().is_empty(), "the other entry is not in the library");
    }

    #[test]
    fn an_image_bump_moves_the_stamp_of_that_image_only_and_always_forward() {
        let (_dir, store, id) = with_episodes(true, true, 0, 0);
        assert!(store.bump_image(id, ImageKind::Cover, 100).unwrap());
        let row = store.anime(id).unwrap().unwrap();
        assert_eq!((row.cover_last_modified, row.background_last_modified), (100, 0));
        assert!(store.bump_image(id, ImageKind::Cover, 100).unwrap());
        assert_eq!(store.anime(id).unwrap().unwrap().cover_last_modified, 101, "the same millisecond still moves it");
        assert!(store.bump_image(id, ImageKind::Background, 50).unwrap());
        assert_eq!(store.anime(id).unwrap().unwrap().background_last_modified, 50);
        assert!(!store.bump_image(4242, ImageKind::Cover, 1).unwrap());
    }
}
