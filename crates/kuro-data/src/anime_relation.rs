//! Related-anime groups of one anime (`AnimeRelationRepositoryImpl.kt`, `anime_relations`). A group is a name and the
//! anime linked under it; `sort_order` keeps the order of the rows across all groups.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::statements_anime::{related_groups, relations_last_fetched_at};
use kuro_db::subscribe::Subscription;
use rusqlite::{Connection, named_params};

use crate::anime::{AnimeRelationGroup, map_anime};
use crate::{DataResult, ENTRY_DEBOUNCE};

/// Tables a related-anime query reads.
const RELATION_TABLES: &[&str] = &["anime_relations", "entries"];

/// Deletes the rows of one anime (the `replace_relations` first step).
const RELATION_DELETE: &str = "DELETE FROM anime_relations WHERE entry_id = :animeId";

/// Inserts one relation row. A repeated `(anime_id, related_anime_id)` pair replaces the earlier row.
const RELATION_INSERT: &str = "INSERT OR REPLACE INTO anime_relations(entry_id, related_id, name, sort_order, last_fetched_at) \
     VALUES (:animeId, :relatedAnimeId, :name, :sortOrder, :lastFetchedAt)";

/// Typed access to `anime_relations`. Shares one `Db` with the other anime repositories.
#[derive(Clone)]
pub struct AnimeRelationRepo {
    db: Arc<Db>,
}

impl AnimeRelationRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Replaces every relation of `anime_id` with `groups` in one transaction. Each row gets `fetched_at`, and
    /// `sort_order` counts up from 0 across the groups in the order given.
    pub fn replace_relations(&self, anime_id: i64, groups: &[AnimeRelationGroup], fetched_at: i64) -> DataResult<()> {
        self.db.write(|tx| {
            tx.prepare_cached(RELATION_DELETE)?.execute(named_params! {":animeId": anime_id})?;
            let mut sort_order = 0_i64;
            for group in groups {
                for related in &group.anime {
                    tx.prepare_cached(RELATION_INSERT)?.execute(named_params! {
                        ":animeId": anime_id,
                        ":relatedAnimeId": related.id,
                        ":name": group.name,
                        ":sortOrder": sort_order,
                        ":lastFetchedAt": fetched_at,
                    })?;
                    sort_order += 1;
                }
            }
            Ok(())
        })
    }

    /// `last_fetched_at` of the relations of `anime_id`, or `None` when it has none.
    pub fn last_fetched_at(&self, anime_id: i64) -> DataResult<Option<i64>> {
        self.db.read(|c| Ok(relations_last_fetched_at(c, anime_id)?))
    }

    /// The groups of `anime_id`, published again after each change to the relations or the anime rows they link.
    /// Groups come in the order of their first `sort_order`, and the anime of each group in `sort_order`.
    pub fn subscribe_related(&self, anime_id: i64) -> Subscription<Vec<AnimeRelationGroup>> {
        self.db.subscribe(RELATION_TABLES, ENTRY_DEBOUNCE, move |c| Ok(groups(c, anime_id)?))
    }

    /// The groups of `anime_id` read once.
    pub fn related(&self, anime_id: i64) -> DataResult<Vec<AnimeRelationGroup>> {
        self.db.read(|c| Ok(groups(c, anime_id)?))
    }
}

fn groups(c: &Connection, anime_id: i64) -> rusqlite::Result<Vec<AnimeRelationGroup>> {
    Ok(related_groups(c, anime_id)?
        .into_iter()
        .map(|(name, rows)| AnimeRelationGroup { name, anime: rows.iter().map(map_anime).collect() })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::{Anime, AnimeRepo};
    use crate::support::test_db;

    fn anime(repo: &AnimeRepo, url: &str) -> i64 {
        repo.insert(&Anime { url: url.into(), title: url.into(), source: 1, status: 1, ..Anime::create() }).unwrap()
    }

    fn group(name: &str, ids: &[i64]) -> AnimeRelationGroup {
        let anime = ids.iter().map(|&id| Anime { id, ..Anime::create() }).collect();
        AnimeRelationGroup { name: name.to_owned(), anime }
    }

    fn names(groups: &[AnimeRelationGroup]) -> Vec<(String, Vec<i64>)> {
        groups.iter().map(|g| (g.name.clone(), g.anime.iter().map(|a| a.id).collect())).collect()
    }

    #[test]
    fn no_rows_means_no_last_fetch_and_no_groups() {
        let (_d, db) = test_db();
        let repo = AnimeRelationRepo::new(db.clone());
        let a = anime(&AnimeRepo::new(db), "a");
        assert_eq!(repo.last_fetched_at(a).unwrap(), None);
        assert!(repo.related(a).unwrap().is_empty());
    }

    #[test]
    fn replace_then_read_returns_the_groups_in_order_with_the_fetch_time() {
        let (_d, db) = test_db();
        let animes = AnimeRepo::new(db.clone());
        let repo = AnimeRelationRepo::new(db.clone());
        let (a, b, c, d) = (anime(&animes, "a"), anime(&animes, "b"), anime(&animes, "c"), anime(&animes, "d"));
        repo.replace_relations(a, &[group("Sequels", &[b, c]), group("Spin-offs", &[d])], 1234).unwrap();
        assert_eq!(repo.last_fetched_at(a).unwrap(), Some(1234));
        assert_eq!(
            names(&repo.related(a).unwrap()),
            vec![("Sequels".to_owned(), vec![b, c]), ("Spin-offs".to_owned(), vec![d])]
        );
    }

    #[test]
    fn a_second_replace_drops_the_old_rows() {
        let (_d, db) = test_db();
        let animes = AnimeRepo::new(db.clone());
        let repo = AnimeRelationRepo::new(db.clone());
        let (a, b, c) = (anime(&animes, "a"), anime(&animes, "b"), anime(&animes, "c"));
        repo.replace_relations(a, &[group("Old", &[b])], 1).unwrap();
        repo.replace_relations(a, &[group("New", &[c])], 2).unwrap();
        assert_eq!(names(&repo.related(a).unwrap()), vec![("New".to_owned(), vec![c])]);
        assert_eq!(repo.last_fetched_at(a).unwrap(), Some(2));
    }

    #[test]
    fn the_same_name_in_two_places_is_one_group_in_first_position_order() {
        let (_d, db) = test_db();
        let animes = AnimeRepo::new(db.clone());
        let repo = AnimeRelationRepo::new(db.clone());
        let (a, b, c, d) = (anime(&animes, "a"), anime(&animes, "b"), anime(&animes, "c"), anime(&animes, "d"));
        repo.replace_relations(a, &[group("X", &[b]), group("Y", &[c]), group("X", &[d])], 1).unwrap();
        assert_eq!(
            names(&repo.related(a).unwrap()),
            vec![("X".to_owned(), vec![b, d]), ("Y".to_owned(), vec![c])]
        );
    }

    #[test]
    fn deleting_either_anime_cascades_its_relation_rows() {
        let (_d, db) = test_db();
        let animes = AnimeRepo::new(db.clone());
        let repo = AnimeRelationRepo::new(db.clone());
        let (a, b) = (anime(&animes, "a"), anime(&animes, "b"));
        repo.replace_relations(a, &[group("G", &[b])], 1).unwrap();
        db.write(|tx| Ok(tx.execute("DELETE FROM entries WHERE _id = ?1", [b]).map(drop)?)).unwrap();
        assert!(repo.related(a).unwrap().is_empty(), "the linked anime is gone");

        let c = anime(&animes, "c");
        repo.replace_relations(c, &[group("G", &[a])], 1).unwrap();
        db.write(|tx| Ok(tx.execute("DELETE FROM entries WHERE _id = ?1", [c]).map(drop)?)).unwrap();
        let count: i64 = db
            .read(|conn| Ok(conn.query_row("SELECT count(*) FROM anime_relations WHERE entry_id = ?1", [c], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 0, "the owner's rows go with it");
    }
}
