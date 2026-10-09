//! The database side of the anime page: the entry, its episodes and seasons, the open-time defaults, the masked flag
//! writes and the live subscriptions. Every method is synchronous; the runner calls them on the blocking pool.
//! Nothing here imports GTK. Upstream: `AnimeScreenModel.kt:204-283` (init), `SetAnimeEpisodeFlags.kt`.

use std::sync::Arc;

use kuro_data::DataResult;
use kuro_data::anime::{Anime, AnimeRepo, SeasonAnime};
use kuro_data::episode::{Episode, EpisodeRepo};
use kuro_db::handle::Db;
use kuro_db::subscribe::Subscription;

use super::settings::sort_click;

/// Typed access to the tables of one anime page. Cheap to clone: it shares one `Db`.
#[derive(Clone)]
pub struct AnimeStore {
    animes: AnimeRepo,
    episodes: EpisodeRepo,
}

impl AnimeStore {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            animes: AnimeRepo::new(Arc::clone(&db)),
            episodes: EpisodeRepo::new(db),
        }
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

    /// The first open of an entry outside the library: the library's episode and season defaults overwrite its flags
    /// (behaviour 2.3 step 5.5). A favourite keeps its stored words. Returns whether a word was written.
    /// One masked UPDATE of `animes.episode_flags`: `(old & !mask) | (flag & mask)`.
    pub fn set_flag_bits(&self, anime_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.animes.set_episode_flags_masked(anime_id, flag, mask)
    }

    /// Sort by `mode`, or flip the direction when `mode` is the stored key. The word is read and written inside one
    /// writer transaction, so two quick taps cannot both see the same word.
    pub fn sort_by(&self, anime_id: i64, mode: i64) -> DataResult<bool> {
        self.animes.update_episode_flags(anime_id, |word| sort_click(word, mode))
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

    use super::fixture::with_episodes;

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
