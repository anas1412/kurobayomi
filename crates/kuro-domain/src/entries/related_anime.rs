//! Related anime groups (T-07a-031): the facade names of the Kotlin `AnimeRelationRepository` and `GetRelatedAnime`.
//! The group model is `kuro_data::anime::AnimeRelationGroup`; the rows live in `kuro-data`.

use kuro_data::DataResult;
use kuro_data::anime::AnimeRelationGroup;
use kuro_data::anime_relation::AnimeRelationRepo;
use kuro_db::subscribe::Subscription;

/// `subscribeRelatedAnime`: the groups of one anime, live. Groups keep the repository order.
pub fn subscribe_related_anime(repo: &AnimeRelationRepo, anime_id: i64) -> Subscription<Vec<AnimeRelationGroup>> {
    repo.subscribe_related(anime_id)
}

/// `getLastFetchedAt`: when the relations of this anime were last replaced, or `None` when never.
pub fn get_last_fetched_at(repo: &AnimeRelationRepo, anime_id: i64) -> DataResult<Option<i64>> {
    repo.last_fetched_at(anime_id)
}

/// `replaceRelations`: swaps the groups of one anime for `groups`, all stamped `fetched_at`.
pub fn replace_relations(
    repo: &AnimeRelationRepo,
    anime_id: i64,
    groups: &[AnimeRelationGroup],
    fetched_at: i64,
) -> DataResult<()> {
    repo.replace_relations(anime_id, groups, fetched_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::anime_db;
    use kuro_data::anime::{Anime, AnimeRepo};

    fn anime(repo: &AnimeRepo, url: &str) -> i64 {
        repo.insert(&Anime { url: url.into(), title: url.into(), source: 1, status: 1, ..Anime::create() }).unwrap()
    }

    fn group(name: &str, ids: &[i64]) -> AnimeRelationGroup {
        AnimeRelationGroup { name: name.into(), anime: ids.iter().map(|&id| Anime { id, ..Anime::create() }).collect() }
    }

    #[tokio::test]
    async fn replace_then_subscribe_yields_the_new_groups() {
        let (_d, db) = anime_db();
        let animes = AnimeRepo::new(db.clone());
        let repo = AnimeRelationRepo::new(db);
        let (a, b, c) = (anime(&animes, "a"), anime(&animes, "b"), anime(&animes, "c"));
        let mut sub = subscribe_related_anime(&repo, a);
        let first = tokio::time::timeout(std::time::Duration::from_millis(500), sub.changed()).await.unwrap().unwrap().unwrap();
        assert!(first.is_empty());

        replace_relations(&repo, a, &[group("Sequels", &[b]), group("Specials", &[c])], 77).unwrap();
        let published = tokio::time::timeout(std::time::Duration::from_millis(500), sub.changed()).await.unwrap().unwrap().unwrap();
        let names: Vec<(&str, Vec<i64>)> =
            published.iter().map(|g| (g.name.as_str(), g.anime.iter().map(|x| x.id).collect())).collect();
        assert_eq!(names, vec![("Sequels", vec![b]), ("Specials", vec![c])]);
        assert_eq!(get_last_fetched_at(&repo, a).unwrap(), Some(77));
    }

    #[test]
    fn the_last_fetch_is_none_until_the_first_replace() {
        let (_d, db) = anime_db();
        let animes = AnimeRepo::new(db.clone());
        let repo = AnimeRelationRepo::new(db);
        let a = anime(&animes, "a");
        assert_eq!(get_last_fetched_at(&repo, a).unwrap(), None);
    }
}
