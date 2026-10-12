//! Writes scanned folders into the database (plan area 43, wire-local-anime). A folder is one library entry under the local
//! source id, and each chapter or episode row is matched by its url, so a rescan never touches read or seen state,
//! bookmarks or saved positions. One path serves both kinds: `sync_entry` stores the entry and puts it in the library
//! (the library defaults are written, as for any add), `sync_items` stores its items.
//!
//! What differs by kind: a manga rescan renames and renumbers only the chapters that changed and keeps the ones removed
//! from disk (ponytail: no removal pass for manga yet); an anime rescan rewrites every episode it finds and removes the
//! rows whose file is gone. A stored manga takes the folder's title again, a stored anime keeps its own. Adding a manga
//! folder adds that one entry; adding an anime folder adds each series folder in it.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use kuro_data::entry::{Entry, EntryRepo, EntryUpdate};
use kuro_data::item::{Item, ItemRepo, ItemUpdate};
use kuro_data::{DataResult, LookupOrInsert};
use kuro_db::Db;
use kuro_local::LOCAL_SOURCE_ID;
use kuro_platform::kind::Kind;
use kuro_prefs::store::PrefStore;

use super::scan::{ScannedFolder, scan, series_in};
use crate::entry_core::entry_ops;

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
pub fn add(db: &Arc<Db>, prefs: &PrefStore, kind: Kind, picked: &Path, now_ms: i64) -> Result<Vec<Outcome>, String> {
    let folders = match kind {
        Kind::Manga => vec![picked.to_path_buf()],
        Kind::Anime => series_in(picked).map_err(|e| e.to_string())?,
    };
    Ok(folders.iter().filter_map(|folder| import(db, prefs, kind, folder, now_ms)).collect())
}

/// Rescans every local entry of `kind` in the library, each from its own folder. An entry the user took out of the
/// library is not added back.
pub fn rescan(db: &Arc<Db>, prefs: &PrefStore, kind: Kind, now_ms: i64) -> Result<Vec<Outcome>, String> {
    let rows = super::local_rows(db, kind).map_err(|e| e.to_string())?;
    Ok(rows.iter().filter_map(|row| import(db, prefs, kind, Path::new(&row.url), now_ms)).collect())
}

/// Scans and stores one folder. `None` for an anime folder with no video file that is not stored yet. A failure is
/// logged as well as returned.
fn import(db: &Arc<Db>, prefs: &PrefStore, kind: Kind, folder: &Path, now_ms: i64) -> Option<Outcome> {
    let outcome = match scan(kind, folder) {
        Err(e) => {
            let title = folder.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            Outcome { title, result: Err(Failure::Read(e.to_string())) }
        }
        Ok(scanned) => {
            // an anime folder with no video file becomes a series once it has one
            let stored = || EntryRepo::new(Arc::clone(db)).by_url_and_source(kind, &url_of(&scanned.path), LOCAL_SOURCE_ID);
            if kind == Kind::Anime && scanned.items.is_empty() && matches!(stored(), Ok(None)) {
                return None;
            }
            let result = sync(db, prefs, kind, &scanned, now_ms).map_err(|e| Failure::Save(e.to_string()));
            Outcome { title: scanned.title.clone(), result }
        }
    };
    if let Err(failure) = &outcome.result {
        tracing::error!(?failure, folder = %outcome.title, "a local folder could not be synced");
    }
    Some(outcome)
}

/// Inserts or updates the entry and its items, and puts the entry in the library when it is not there yet.
pub fn sync(db: &Arc<Db>, prefs: &PrefStore, kind: Kind, scanned: &ScannedFolder, now_ms: i64) -> DataResult<Synced> {
    let entry_id = sync_entry(db, prefs, kind, scanned, now_ms)?;
    sync_items(db, kind, entry_id, scanned, now_ms)
}

/// The entry of a folder. A manga takes the folder's title and cover on every sync; a stored anime keeps its title and a
/// new one is titled by its folder. An entry that is not in the library joins it, with the library defaults.
fn sync_entry(db: &Arc<Db>, prefs: &PrefStore, kind: Kind, scanned: &ScannedFolder, now_ms: i64) -> DataResult<i64> {
    let repo = EntryRepo::new(Arc::clone(db));
    let row = Entry {
        source: LOCAL_SOURCE_ID,
        url: url_of(&scanned.path),
        title: scanned.title.clone(),
        thumbnail_url: scanned.cover.clone(),
        initialized: true,
        date_added: now_ms,
        ..Entry::create(kind)
    };
    let (id, favorite) = match repo.lookup_or_insert(&row)? {
        LookupOrInsert::Inserted(id) => (id, false),
        LookupOrInsert::Found(found) => {
            let update = EntryUpdate {
                title: (kind == Kind::Manga).then(|| scanned.title.clone()),
                thumbnail_url: scanned.cover.clone(),
                ..EntryUpdate::new(found.id)
            };
            repo.update(&update)?;
            (found.id, found.favorite)
        }
    };
    if !favorite {
        entry_ops::add_to_library(&repo, prefs, id, &[], now_ms)?;
    }
    Ok(id)
}

/// The items of a folder, matched to the stored ones by url.
fn sync_items(db: &Arc<Db>, kind: Kind, entry_id: i64, scanned: &ScannedFolder, now_ms: i64) -> DataResult<Synced> {
    let anime = kind.is_anime();
    let items = ItemRepo::new(Arc::clone(db));
    let known = items.by_entry_id(entry_id, false)?;
    let urls = urls_of(scanned);
    let plan = plan(&known.iter().map(|k| k.url.as_str()).collect::<Vec<_>>(), &urls);
    let fresh: Vec<Item> = plan
        .new
        .iter()
        .map(|&i| {
            let found = &scanned.items[i];
            Item {
                entry_id,
                url: urls[i].clone(),
                name: found.name.clone(),
                number: found.number,
                date_upload: found.date_upload,
                date_fetch: now_ms,
                source_order: order(i),
                ..Item::create(kind)
            }
        })
        .collect();
    // only the file's metadata, so done state, bookmarks and the saved position survive. A manga writes the chapters that
    // changed; an anime rewrites every episode it still finds, and the source order with it.
    let renewed: Vec<ItemUpdate> = plan
        .matched
        .iter()
        .filter_map(|&(k, i)| {
            let (known, found) = (&known[k], &scanned.items[i]);
            let changed = anime || known.name != found.name || known.date_upload != found.date_upload || known.number != found.number;
            changed.then(|| ItemUpdate {
                name: Some(found.name.clone()),
                number: Some(found.number),
                date_upload: Some(found.date_upload),
                source_order: anime.then(|| order(i)),
                ..ItemUpdate::new(known.id)
            })
        })
        .collect();
    let removed: Vec<i64> = if anime { plan.gone.iter().map(|&k| known[k].id).collect() } else { Vec::new() };
    if !fresh.is_empty() {
        items.add_all(&fresh)?;
    }
    if !renewed.is_empty() {
        items.update_all(&renewed)?;
    }
    if !removed.is_empty() {
        items.remove_with_ids(&removed)?;
    }
    Ok(Synced { entry_id, added: fresh.len(), updated: renewed.len(), removed: removed.len() })
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
