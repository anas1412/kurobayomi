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
use kuro_domain::entries::defaults::{apply_default_episode_flags, apply_default_season_flags};
use kuro_prefs::store::PrefStore;

use super::actions::Mark;
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

    /// Sort by `mode`, or flip the direction when `mode` is the stored key. The word is read and written inside one
    /// writer transaction, so two quick taps cannot both see the same word.
    pub fn sort_by(&self, anime_id: i64, mode: i64) -> DataResult<bool> {
        self.animes.update_episode_flags(anime_id, |word| sort_click(word, mode))
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
}
