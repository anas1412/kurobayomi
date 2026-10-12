//! Writes scanned folders into the database of their kind (plan area 43, wire-local-anime). A folder is one library entry
//! under the local source id, and each chapter or episode row is matched by its url, so a rescan never touches read or
//! seen state, bookmarks or saved positions.
//!
//! What differs by kind: a manga rescan renames and renumbers only the chapters that changed and keeps the ones removed
//! from disk (ponytail: no removal pass for manga yet); an anime rescan rewrites every episode it finds and removes the
//! rows whose file is gone. Adding a manga folder adds that one entry; adding an anime folder adds each series folder in
//! it.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use kuro_data::anime::{Anime, AnimeRepo, AnimeUpdate};
use kuro_data::chapter::{Chapter, ChapterRepo, ChapterUpdate};
use kuro_data::episode::{Episode, EpisodeRepo, EpisodeUpdate};
use kuro_data::manga::{Manga, MangaRepo, MangaUpdate};
use kuro_data::{DataResult, LookupOrInsert};
use kuro_db::Db;
use kuro_domain::kind::EntryKind;
use kuro_local::LOCAL_SOURCE_ID;

use super::scan::{ScannedFolder, scan, series_in};
use crate::entry_core::store::DbStore;

/// What one sync did to one folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Synced {
    pub entry_id: i64,
    pub added: usize,
    /// Rows rewritten: the manga chapters that were renamed or renumbered, or every anime episode still on disk.
    pub updated: usize,
    pub removed: usize,
}

/// Why a folder was not stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The folder could not be read.
    Read(String),
    /// The database write failed.
    Save(String),
}

/// What an add or a rescan did to one folder, named by its title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub title: String,
    pub result: Result<Synced, Failure>,
}

/// Adds the folder the user picked. A manga folder is one entry; an anime folder holds one folder per series, and a
/// series folder with no video file is skipped until it has one. Picking a folder again is a rescan that also puts its
/// entries back in the library. `Err` when the anime folder cannot be listed.
pub fn add(db: &Arc<Db>, kind: EntryKind, picked: &Path, now_ms: i64) -> Result<Vec<Outcome>, String> {
    let folders = match kind {
        EntryKind::Manga => vec![picked.to_path_buf()],
        EntryKind::Anime => series_in(picked).map_err(|e| e.to_string())?,
    };
    Ok(folders.iter().filter_map(|folder| import(db, kind, folder, now_ms)).collect())
}

/// Rescans every local entry of `kind` in the library, each from its own folder. An entry the user took out of the
/// library is not added back.
pub fn rescan(db: &Arc<Db>, kind: EntryKind, now_ms: i64) -> Result<Vec<Outcome>, String> {
    let rows = super::local_rows(db, kind).map_err(|e| e.to_string())?;
    Ok(rows.iter().filter_map(|row| import(db, kind, Path::new(&row.url), now_ms)).collect())
}

/// Scans and stores one folder. `None` for an anime folder with no video file that is not stored yet. A failure is
/// logged as well as returned.
fn import(db: &Arc<Db>, kind: EntryKind, folder: &Path, now_ms: i64) -> Option<Outcome> {
    let outcome = match scan(kind, folder) {
        Err(e) => {
            let title = folder.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            Outcome { title, result: Err(Failure::Read(e.to_string())) }
        }
        Ok(scanned) => {
            // an anime folder with no video file becomes a series once it has one
            let stored = || AnimeRepo::new(Arc::clone(db)).by_url_and_source(&url_of(&scanned.path), LOCAL_SOURCE_ID);
            if kind == EntryKind::Anime && scanned.items.is_empty() && matches!(stored(), Ok(None)) {
                return None;
            }
            let result = sync(db, kind, &scanned, now_ms).map_err(|e| Failure::Save(e.to_string()));
            Outcome { title: scanned.title.clone(), result }
        }
    };
    if let Err(failure) = &outcome.result {
        tracing::error!(?failure, folder = %outcome.title, "a local folder could not be synced");
    }
    Some(outcome)
}

/// Inserts or updates the entry and its items, and puts the entry in the library when it is not there yet.
pub fn sync(db: &Arc<Db>, kind: EntryKind, scanned: &ScannedFolder, now_ms: i64) -> DataResult<Synced> {
    match kind {
        EntryKind::Manga => sync_manga(db, scanned, now_ms),
        EntryKind::Anime => sync_anime(db, scanned, now_ms),
    }
}

/// The manga entry takes the folder's title and cover on every sync.
fn sync_manga(db: &Arc<Db>, scanned: &ScannedFolder, now_ms: i64) -> DataResult<Synced> {
    let manga = MangaRepo::new(Arc::clone(db));
    let row = Manga {
        source: LOCAL_SOURCE_ID,
        url: url_of(&scanned.path),
        title: scanned.title.clone(),
        thumbnail_url: scanned.cover.clone(),
        initialized: true,
        date_added: now_ms,
        ..Manga::create()
    };
    let (manga_id, favorite) = match manga.lookup_or_insert(&row)? {
        LookupOrInsert::Inserted(id) => (id, false),
        LookupOrInsert::Found(found) => {
            let update = MangaUpdate {
                title: Some(scanned.title.clone()),
                thumbnail_url: scanned.cover.clone(),
                ..MangaUpdate::new(found.id)
            };
            manga.update(&update)?;
            (found.id, found.favorite)
        }
    };
    if !favorite {
        DbStore::new(Arc::clone(db)).add_to_library(manga_id, &[], now_ms)?;
    }

    let chapters = ChapterRepo::new(Arc::clone(db));
    let known = chapters.by_manga_id(manga_id, false)?;
    let urls = urls_of(scanned);
    let plan = plan(&known.iter().map(|k| k.url.as_str()).collect::<Vec<_>>(), &urls);
    let fresh: Vec<Chapter> = plan
        .new
        .iter()
        .map(|&i| {
            let found = &scanned.items[i];
            Chapter {
                manga_id,
                url: urls[i].clone(),
                name: found.name.clone(),
                date_upload: found.date_upload,
                chapter_number: found.number,
                date_fetch: now_ms,
                source_order: order(i),
                ..Chapter::create()
            }
        })
        .collect();
    let renamed: Vec<ChapterUpdate> = plan
        .matched
        .iter()
        .filter_map(|&(k, i)| {
            let (known, found) = (&known[k], &scanned.items[i]);
            let changed = known.name != found.name || known.date_upload != found.date_upload || known.chapter_number != found.number;
            changed.then(|| ChapterUpdate {
                name: Some(found.name.clone()),
                date_upload: Some(found.date_upload),
                chapter_number: Some(found.number),
                ..ChapterUpdate::new(known.id)
            })
        })
        .collect();
    if !fresh.is_empty() {
        chapters.add_all(&fresh)?;
    }
    if !renamed.is_empty() {
        chapters.update_all(&renamed)?;
    }
    Ok(Synced { entry_id: manga_id, added: fresh.len(), updated: renamed.len(), removed: 0 })
}

/// A new anime entry is titled by its folder; a stored one keeps its title.
fn sync_anime(db: &Arc<Db>, scanned: &ScannedFolder, now_ms: i64) -> DataResult<Synced> {
    let anime = AnimeRepo::new(Arc::clone(db));
    let url = url_of(&scanned.path);
    let anime_id = match anime.by_url_and_source(&url, LOCAL_SOURCE_ID)? {
        Some(found) => {
            // the user chose this folder again, so the series belongs in the library
            if !found.favorite {
                anime.update(&AnimeUpdate { favorite: Some(true), ..AnimeUpdate::new(found.id) })?;
            }
            found.id
        }
        None => {
            let row = Anime {
                source: LOCAL_SOURCE_ID,
                url,
                title: scanned.title.clone(),
                favorite: true,
                initialized: true,
                date_added: now_ms,
                ..Anime::create()
            };
            match anime.lookup_or_insert(&row)? {
                LookupOrInsert::Found(found) => found.id,
                LookupOrInsert::Inserted(id) => id,
            }
        }
    };

    let episodes = EpisodeRepo::new(Arc::clone(db));
    let known = episodes.by_anime_id(anime_id)?;
    let urls = urls_of(scanned);
    let plan = plan(&known.iter().map(|k| k.url.as_str()).collect::<Vec<_>>(), &urls);
    let add: Vec<Episode> = plan
        .new
        .iter()
        .map(|&i| {
            let found = &scanned.items[i];
            Episode {
                anime_id,
                url: urls[i].clone(),
                name: found.name.clone(),
                episode_number: found.number,
                date_upload: found.date_upload,
                source_order: order(i),
                date_fetch: now_ms,
                ..Episode::create()
            }
        })
        .collect();
    // only the file's metadata, so seen state, bookmarks and the saved position survive
    let update: Vec<EpisodeUpdate> = plan
        .matched
        .iter()
        .map(|&(k, i)| {
            let found = &scanned.items[i];
            EpisodeUpdate {
                name: Some(found.name.clone()),
                episode_number: Some(found.number),
                date_upload: Some(found.date_upload),
                source_order: Some(order(i)),
                ..EpisodeUpdate::new(known[k].id)
            }
        })
        .collect();
    let remove: Vec<i64> = plan.gone.iter().map(|&k| known[k].id).collect();
    episodes.add_all(&add)?;
    episodes.update_all(&update)?;
    episodes.remove_with_ids(&remove)?;
    Ok(Synced { entry_id: anime_id, added: add.len(), updated: update.len(), removed: remove.len() })
}

/// Which found items are new, which match a stored row by url, and which stored rows have no item any more. Indexes
/// into `known` and `found`.
#[derive(Debug, Default, PartialEq, Eq)]
struct Plan {
    new: Vec<usize>,
    /// `(known, found)` pairs.
    matched: Vec<(usize, usize)>,
    gone: Vec<usize>,
}

fn plan(known: &[&str], found: &[String]) -> Plan {
    let mut by_url: HashMap<&str, usize> = HashMap::new();
    for (k, url) in known.iter().enumerate() {
        by_url.entry(*url).or_insert(k);
    }
    let found_urls: HashSet<&str> = found.iter().map(String::as_str).collect();
    let mut plan = Plan::default();
    for (i, url) in found.iter().enumerate() {
        match by_url.get(url.as_str()) {
            Some(&k) => plan.matched.push((k, i)),
            None => plan.new.push(i),
        }
    }
    plan.gone = (0..known.len()).filter(|&k| !found_urls.contains(known[k])).collect();
    plan
}

fn url_of(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn urls_of(scanned: &ScannedFolder) -> Vec<String> {
    scanned.items.iter().map(|item| url_of(&item.path)).collect()
}

/// The source order of the item at `index`: its place in the scanned list.
fn order(index: usize) -> i64 {
    i64::try_from(index).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(list: &[&str]) -> Vec<String> {
        list.iter().map(|u| (*u).to_owned()).collect()
    }

    #[test]
    fn a_new_url_is_added_a_known_one_matched_and_a_missing_one_gone() {
        let plan = plan(&["/show/01.mkv", "/show/02.mkv"], &urls(&["/show/03.mkv", "/show/01.mkv"]));
        assert_eq!(plan, Plan { new: vec![0], matched: vec![(0, 1)], gone: vec![1] });
    }

    #[test]
    fn an_empty_folder_leaves_every_row_gone() {
        assert_eq!(plan(&["/show/01.mkv"], &[]), Plan { new: vec![], matched: vec![], gone: vec![0] });
    }
}
