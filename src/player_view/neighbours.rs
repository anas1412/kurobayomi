//! The episode before or after another one in its entry, for the page's previous and next buttons and for autoplay.
//! Episodes are ordered by their number (ties by row id), whatever order the entry page lists them in. Database reads
//! only.

use std::sync::Arc;

use kuro_data::episode::EpisodeRepo;
use kuro_db::handle::Db;

/// Which neighbour to find.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Previous,
    Next,
}

/// The row id of the episode before or after `episode_id` in its entry. `None` at either end, and for a row that does
/// not exist.
pub fn adjacent(db: &Arc<Db>, episode_id: i64, step: Step) -> Result<Option<i64>, String> {
    let repo = EpisodeRepo::new(Arc::clone(db));
    let Some(current) = repo.by_id(episode_id).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let mut episodes = repo.by_anime_id(current.anime_id).map_err(|e| e.to_string())?;
    episodes.sort_by(|a, b| a.episode_number.total_cmp(&b.episode_number).then(a.id.cmp(&b.id)));
    let Some(at) = episodes.iter().position(|e| e.id == episode_id) else {
        return Ok(None);
    };
    Ok(match step {
        Step::Previous => at.checked_sub(1).map(|i| episodes[i].id),
        Step::Next => episodes.get(at + 1).map(|e| e.id),
    })
}

#[cfg(test)]
mod tests {
    use kuro_data::anime::{Anime, AnimeRepo};
    use kuro_data::episode::Episode;
    use kuro_testkit::{TempDir, TestDb};

    use super::*;

    #[test]
    fn the_neighbours_follow_the_episode_numbers_and_stop_at_the_ends() {
        let dir = TempDir::new().unwrap();
        let db = TestDb::handle(&dir);
        let anime_id = AnimeRepo::new(Arc::clone(&db)).insert(&Anime::create()).expect("anime row");
        let episodes: Vec<Episode> = [3.0, 1.0, 2.0]
            .iter()
            .map(|&n| Episode { anime_id, url: format!("/{n}"), name: format!("E{n}"), episode_number: n, ..Episode::create() })
            .collect();
        let rows = EpisodeRepo::new(Arc::clone(&db)).add_all(&episodes).expect("rows");
        let id_of = |n: f64| rows.iter().find(|e| e.episode_number == n).expect("row").id;

        assert_eq!(adjacent(&db, id_of(1.0), Step::Next).unwrap(), Some(id_of(2.0)));
        assert_eq!(adjacent(&db, id_of(2.0), Step::Previous).unwrap(), Some(id_of(1.0)));
        assert_eq!(adjacent(&db, id_of(3.0), Step::Next).unwrap(), None, "the last episode has no next");
        assert_eq!(adjacent(&db, id_of(1.0), Step::Previous).unwrap(), None, "the first episode has no previous");
        assert_eq!(adjacent(&db, 9_999, Step::Next).unwrap(), None, "a missing episode has no neighbour");
    }
}
