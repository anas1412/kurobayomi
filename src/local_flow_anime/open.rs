//! Which local episode an entry page or the player asks for (wire-local-anime). The screens name an entry by its title
//! and an episode by its name, so these lookups run over the local rows only. A title that is not local gives no id,
//! and the caller keeps its own page.

use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::episode::EpisodeRepo;
use kuro_db::handle::Db;
use kuro_local::LOCAL_SOURCE_ID;

/// One episode ready for the player: its file, its row id and the saved position in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    pub path: String,
    pub episode_id: i64,
    pub start_ms: i64,
}

/// The id of the first local library entry with this title.
pub fn anime_id_with_title(db: &Arc<Db>, title: &str) -> Result<Option<i64>, String> {
    let found = AnimeRepo::new(Arc::clone(db)).favorites_by_source(LOCAL_SOURCE_ID).map_err(|e| e.to_string())?;
    Ok(found.into_iter().find(|a| a.title == title).map(|a| a.id))
}

/// The episode named `episode_name` of the local entry titled `title`, when both exist.
pub fn opening_of(db: &Arc<Db>, title: &str, episode_name: &str) -> Result<Option<Opening>, String> {
    let Some(anime_id) = anime_id_with_title(db, title)? else {
        return Ok(None);
    };
    let episodes = EpisodeRepo::new(Arc::clone(db)).by_anime_id(anime_id).map_err(|e| e.to_string())?;
    Ok(episodes.into_iter().find(|e| e.name == episode_name).map(|e| Opening { path: e.url, episode_id: e.id, start_ms: e.last_second_seen * 1000 }))
}

/// The local entry id for a title on the GTK thread. `None` before the data opens, and for any other title.
pub fn anime_id_for_title(title: &str) -> Option<i64> {
    if !crate::data::is_installed() {
        return None;
    }
    anime_id_with_title(&crate::data::app().anime_db, title).unwrap_or_else(|e| {
        tracing::error!(error = %e, "the local anime lookup failed");
        None
    })
}

/// Opens a local episode in the player. Returns `false` when the title and name match no local episode, so the caller
/// opens its own player instead.
pub fn open_episode(title: &str, episode_name: &str) -> bool {
    if !crate::data::is_installed() {
        return false;
    }
    match opening_of(&crate::data::app().anime_db, title, episode_name) {
        Ok(Some(found)) => {
            crate::player_view::open_episode(std::path::Path::new(&found.path), found.episode_id, found.start_ms);
            true
        }
        Ok(None) => false,
        Err(e) => {
            tracing::error!(error = %e, "the local episode lookup failed");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use kuro_data::episode::EpisodeUpdate;

    use super::super::sync::{scan_root, tests::anime_db};
    use super::*;

    #[test]
    fn a_local_episode_opens_at_its_saved_position_and_other_titles_do_not() {
        let (_db_dir, db) = anime_db();
        let root = tempfile::tempdir().expect("root");
        let dir = root.path().join("Show");
        std::fs::create_dir(&dir).expect("series folder");
        std::fs::write(dir.join("Show - 1.mkv"), b"video").expect("episode 1");
        std::fs::write(dir.join("Show - 2.mkv"), b"video").expect("episode 2");
        scan_root(&db, root.path()).expect("scan");

        let anime_id = anime_id_with_title(&db, "Show").expect("lookup").expect("local Show");
        let episodes = EpisodeRepo::new(Arc::clone(&db)).by_anime_id(anime_id).expect("rows");
        let first = episodes.iter().find(|e| e.name == "Show - 1").expect("episode 1 row");
        EpisodeRepo::new(Arc::clone(&db)).update(&EpisodeUpdate { last_second_seen: Some(90), ..EpisodeUpdate::new(first.id) }).expect("position");

        let opening = opening_of(&db, "Show", "Show - 1").expect("lookup").expect("opening");
        assert_eq!(opening, Opening { path: dir.join("Show - 1.mkv").to_string_lossy().into_owned(), episode_id: first.id, start_ms: 90_000 });
        assert_eq!(opening_of(&db, "Show", "Show - 9").expect("lookup"), None, "an unknown episode name");
        assert_eq!(opening_of(&db, "Berserk", "Show - 1").expect("lookup"), None, "a title with no local entry");
    }

    #[test]
    fn a_non_local_entry_with_the_same_title_is_not_found() {
        let (_db_dir, db) = anime_db();
        AnimeRepo::new(Arc::clone(&db))
            .insert(&kuro_data::anime::Anime { source: 42, url: "remote/1".into(), title: "Remote".into(), favorite: true, ..kuro_data::anime::Anime::create() })
            .expect("remote row");
        assert_eq!(anime_id_with_title(&db, "Remote").expect("lookup"), None);
    }
}
