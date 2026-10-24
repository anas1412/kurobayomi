//! Which local entry, chapter or episode a screen asks for. The title-based routes (see `screens::open_media`) name an
//! entry by its title, which resolves over the local library rows only, so any other title keeps the caller's own page.
//! The id of the manga entry opened last is remembered, so the reader can tell two local folders with one name apart. A
//! local episode plays its file in the player at its saved position.

use std::cell::Cell;
use std::sync::Arc;

use kuro_data::DataResult;
use kuro_data::entry::EntryRepo;
use kuro_data::item::{Item, ItemRepo};
use kuro_db::Db;
use kuro_platform::kind::Kind;
use kuro_local::LOCAL_SOURCE_ID;

thread_local! {
    static CURRENT: Cell<Option<i64>> = const { Cell::new(None) };
}

/// The id of the first local library entry of `kind` with this title.
pub fn local_id_with_title(db: &Arc<Db>, kind: Kind, title: &str) -> DataResult<Option<i64>> {
    Ok(super::local_rows(db, kind)?.into_iter().find(|row| row.title == title).map(|row| row.id))
}

/// Opens the database-backed entry page of a local manga by its id, and remembers the id for the reader.
pub fn entry(manga_id: i64) {
    CURRENT.with(|c| c.set(Some(manga_id)));
    crate::entry_core::details::route::open_kind(Kind::Manga, manga_id);
}

/// The local manga whose page was opened last, when it is still the one the reader is asked about.
pub fn current() -> Option<i64> {
    CURRENT.with(Cell::get)
}

/// One episode ready for the player: its file, its row id and the saved position in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    pub path: String,
    pub episode_id: i64,
    pub start_ms: i64,
}

impl Opening {
    fn of(episode: Item) -> Self {
        // a seen episode starts again from the beginning, like every other open (`player_view::store::start_ms`)
        let start_ms = crate::player_view::start_ms(0, &episode);
        Opening { path: episode.url, episode_id: episode.id, start_ms }
    }
}

/// The episode named `episode_name` of the local anime titled `title`, when both exist.
pub fn opening_of(db: &Arc<Db>, title: &str, episode_name: &str) -> DataResult<Option<Opening>> {
    let Some(anime_id) = local_id_with_title(db, Kind::Anime, title)? else {
        return Ok(None);
    };
    let episodes = ItemRepo::new(Arc::clone(db)).by_entry_id(anime_id, false)?;
    Ok(episodes.into_iter().find(|e| e.name == episode_name).map(Opening::of))
}

/// The opening of episode row `episode_id`, when its entry is local. `None` for a missing row or a remote entry.
pub fn opening_by_id(db: &Arc<Db>, episode_id: i64) -> DataResult<Option<Opening>> {
    let Some(episode) = ItemRepo::new(Arc::clone(db)).by_id(episode_id)? else {
        return Ok(None);
    };
    if EntryRepo::new(Arc::clone(db)).by_id(episode.entry_id)?.source != LOCAL_SOURCE_ID {
        return Ok(None);
    }
    Ok(Some(Opening::of(episode)))
}

/// Plays a local episode by its row id in the player. `false` when the row is not a local episode, so the caller keeps
/// its own player.
pub fn open_local_episode(episode_id: i64) -> bool {
    crate::data::is_installed() && play(opening_by_id(&crate::data::app().db, episode_id))
}

/// Plays the episode named `episode_name` of the local anime titled `title`. `false` when they match no local episode,
/// so the caller opens its own player instead.
pub fn open_episode(title: &str, episode_name: &str) -> bool {
    crate::data::is_installed() && play(opening_of(&crate::data::app().db, title, episode_name))
}

fn play(found: DataResult<Option<Opening>>) -> bool {
    match found {
        Ok(Some(found)) => {
            match super::path_of(Kind::Anime, &found.path) {
                Some(path) => crate::player_view::open_episode(&path, found.episode_id, found.start_ms),
                None => tracing::warn!(url = %found.path, "a local episode url names no file"),
            }
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
    use kuro_data::entry::Entry;
    use kuro_data::item::ItemUpdate;
    use kuro_platform::kind::Kind;

    use super::*;
    use crate::local_flow::tests::{linked, series, test_db};

    #[test]
    fn only_a_local_library_title_resolves() {
        let (_db_dir, db) = test_db();
        let root = tempfile::tempdir().expect("temp dir");
        let id = linked(&db, Kind::Manga, &series(root.path()), &["Ch 1"]);
        assert_eq!(local_id_with_title(&db, Kind::Manga, "Series").expect("lookup"), Some(id));
        assert_eq!(local_id_with_title(&db, Kind::Manga, "Title 1").expect("lookup"), None);
    }

    #[test]
    fn a_local_episode_opens_at_its_saved_position_and_other_titles_do_not() {
        let (_db_dir, db) = test_db();
        let root = tempfile::tempdir().expect("root");
        linked(&db, Kind::Anime, &root.path().join("Show"), &["Show - 1.mkv", "Show - 2.mkv"]);

        let anime_id = local_id_with_title(&db, Kind::Anime, "Show").expect("lookup").expect("local Show");
        let episodes = ItemRepo::new(Arc::clone(&db));
        let first = episodes.by_entry_id(anime_id, false).expect("rows").into_iter().find(|e| e.name == "Show - 1").expect("episode 1 row");
        episodes.update(&ItemUpdate { progress: Some(90), ..ItemUpdate::new(first.id) }).expect("position");

        let opening = opening_of(&db, "Show", "Show - 1").expect("lookup").expect("opening");
        let show = kuro_local::fs::absolute_url(&root.path().join("Show")).expect("an absolute path");
        let path = format!("{show}/Show - 1.mkv");
        assert_eq!(opening, Opening { path, episode_id: first.id, start_ms: 90_000 });
        assert_eq!(opening_of(&db, "Show", "Show - 9").expect("lookup"), None, "an unknown episode name");
        assert_eq!(opening_of(&db, "Berserk", "Show - 1").expect("lookup"), None, "a title with no local entry");
    }

    #[test]
    fn an_episode_id_opens_only_a_local_episode() {
        let (_db_dir, db) = test_db();
        let root = tempfile::tempdir().expect("root");
        let anime_id = linked(&db, Kind::Anime, &root.path().join("Show"), &["Show - 1.mkv"]);
        let episode = ItemRepo::new(Arc::clone(&db)).by_entry_id(anime_id, false).expect("rows").remove(0);
        assert_eq!(opening_by_id(&db, episode.id).expect("lookup").expect("local episode").path, episode.url);

        let remote = EntryRepo::new(Arc::clone(&db))
            .insert(&Entry { source: 42, url: "remote/1".into(), title: "Remote".into(), favorite: true, ..Entry::create(Kind::Anime) })
            .expect("remote row");
        let remote_episode = Item { entry_id: remote, url: "remote/1/ep1".into(), name: "1".into(), ..Item::create(Kind::Anime) };
        ItemRepo::new(Arc::clone(&db)).add_all(&[remote_episode]).expect("remote episode");
        let remote_id = ItemRepo::new(Arc::clone(&db)).by_entry_id(remote, false).expect("rows").remove(0).id;
        assert_eq!(opening_by_id(&db, remote_id).expect("lookup"), None, "a remote episode keeps its own player");
        assert_eq!(opening_by_id(&db, 9_999).expect("lookup"), None, "a missing episode");
        assert_eq!(local_id_with_title(&db, Kind::Anime, "Remote").expect("lookup"), None, "a remote title is not local");
    }
}
