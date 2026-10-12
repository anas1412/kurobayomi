//! Season (child anime) statistics and the parent/child helpers (T-07a-029, T-07a-030). The stats model is
//! `kuro_data::anime::SeasonAnime`; this file adds the facade names of `AnimeRepository` and the table test of the
//! stats rules. Only `get_seasons_by_parent_id` swallows errors (see `queries.rs`); the others return them.

use kuro_data::DataResult;
use kuro_data::anime::{Anime, AnimeRepo, DeletableEntry, SeasonAnime};
use kuro_db::subscribe::Subscription;

/// `getAnimeSeasonsById`: the children of a parent, with their stats.
pub fn get_anime_seasons_by_id(repo: &AnimeRepo, parent_id: i64) -> DataResult<Vec<SeasonAnime>> {
    repo.seasons_by_parent(parent_id)
}

/// `getAnimeSeasonsByIdAsFlow`: a live list of the children of a parent.
pub fn get_anime_seasons_by_id_as_flow(repo: &AnimeRepo, parent_id: i64) -> Subscription<Vec<SeasonAnime>> {
    repo.subscribe_seasons_by_parent(parent_id)
}

/// `getChildrenByParentId`: the child rows without stats.
pub fn get_children_by_parent_id(repo: &AnimeRepo, parent_id: i64) -> DataResult<Vec<Anime>> {
    repo.children_by_parent(parent_id)
}

/// `removeParentIdByIds`: detaches children. The rows stay; their `parent_id` becomes NULL.
pub fn remove_parent_id_by_ids(repo: &AnimeRepo, ids: &[i64]) -> DataResult<()> {
    repo.remove_parent_ids(ids)
}

/// `getDeletableParentAnime`: non-favourite top-level anime that no library entry needs.
pub fn get_deletable_parent_anime(repo: &AnimeRepo) -> DataResult<Vec<DeletableEntry>> {
    repo.deletable_parents()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::test_db;

    fn season(total: i64, seen: i64, bookmarks: i64, fillers: i64) -> SeasonAnime {
        SeasonAnime {
            anime: kuro_testkit::domain::anime(),
            total_count: total,
            seen_count: seen,
            bookmark_count: bookmarks,
            fillermark_count: fillers,
            latest_upload: 0,
            fetched_at: 77,
            last_seen: 0,
        }
    }

    #[test]
    fn season_stats_follow_the_table_rules() {
        // (total, seen, bookmarks, fillers) -> (seen, unseen, started, bookmarks, fillermarks)
        let rows: [((i64, i64, i64, i64), (bool, i64, bool, bool, bool)); 6] = [
            ((0, 0, 0, 0), (true, 0, false, false, false)),
            ((4, 4, 0, 0), (true, 0, true, false, false)),
            ((4, 1, 2, 0), (false, 3, true, true, false)),
            ((4, 0, 0, 1), (false, 4, false, false, true)),
            ((2, 0, 1, 1), (false, 2, false, true, true)),
            ((3, 3, 0, 2), (true, 0, true, false, true)),
        ];
        for ((total, seen, bm, fm), (seen_flag, unseen, started, bookmarks, fillers)) in rows {
            let s = season(total, seen, bm, fm);
            assert_eq!(s.seen(), seen_flag, "seen {total}/{seen}");
            assert_eq!(s.unseen_count(), unseen, "unseen {total}/{seen}");
            assert_eq!(s.has_started(), started, "started {total}/{seen}");
            assert_eq!(s.has_bookmarks(), bookmarks, "bookmarks {bm}");
            assert_eq!(s.has_fillermarks(), fillers, "fillers {fm}");
        }
    }

    #[test]
    fn to_library_anime_uses_category_minus_one_and_maps_the_fetch_date() {
        let lib = season(5, 2, 0, 0).to_library_anime();
        assert_eq!(lib.category, -1);
        assert_eq!(lib.episode_fetched_at, 77);
        assert_eq!((lib.total_count, lib.seen_count), (5, 2));
    }

    #[test]
    fn detaching_seasons_keeps_the_rows_with_a_null_parent() {
        let (_d, db) = test_db();
        let repo = AnimeRepo::new(db);
        let parent = repo.insert(&Anime { favorite: true, source: 1, url: "p".into(), title: "P".into(), ..kuro_testkit::domain::anime() }).unwrap();
        let child = repo
            .insert(&Anime { favorite: true, source: 1, url: "c".into(), title: "C".into(), parent_id: Some(parent), ..kuro_testkit::domain::anime() })
            .unwrap();
        assert_eq!(get_children_by_parent_id(&repo, parent).unwrap().len(), 1);
        remove_parent_id_by_ids(&repo, &[child]).unwrap();
        assert!(get_children_by_parent_id(&repo, parent).unwrap().is_empty());
        let detached = repo.by_id(child).unwrap();
        assert_eq!(detached.parent_id, None);
    }
}
