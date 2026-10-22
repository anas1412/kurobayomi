//! Local titles in the library: the folders of the Local folder that are not stored yet, and the folders the user links.
//! Only the entries are written here; their chapters, episodes and details come from the local source (kuro-local) in the
//! library update pass, as any source's do. No GTK, so it runs on a worker thread and its tests need no display.
//!
//! A title is a folder: inside the Local folder its url is the folder's name, a linked one's url is its absolute path
//! (`local_flow::path_of`). What is never done: a title the user took out of the library is not added back by a scan,
//! and a folder that cannot be read adds nothing and removes nothing, so a missing drive never empties the library.

use std::io;
use std::path::Path;
use std::sync::Arc;

use kuro_data::entry::{Entry, EntryRepo};
use kuro_data::{DataResult, LookupOrInsert};
use kuro_db::Db;
use kuro_local::LOCAL_SOURCE_ID;
use kuro_local::format::{format_of, is_anime_episode};
use kuro_local::fs::{FsEntry, list_dir};
use kuro_local::names::is_hidden;
use kuro_platform::kind::Kind;
use kuro_prefs::store::PrefStore;

use crate::entry_core::entry_ops;

/// What one scan or link did: the titles it put in the library, and the folders it passed over, with why.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub added: Vec<String>,
    pub skipped: Vec<(String, String)>,
}

/// Whether a stored title the user took out of the library joins it again: a folder the user links does, the Local
/// folder's own scan does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rejoin {
    Yes,
    No,
}

/// [`discover`] on the Local folder of `kind`, before a library update pass. A Local folder that is not known or cannot
/// be listed adds nothing (and is logged); folders passed over are logged, and one toast says how many.
pub fn discover_local(db: &Arc<Db>, prefs: &PrefStore, kind: Kind) {
    let Some(root) = super::local_root(kind) else { return };
    match discover(db, prefs, kind, &root, chrono::Utc::now().timestamp_millis()) {
        Ok(report) => {
            for (folder, why) in &report.skipped {
                tracing::warn!(%folder, %why, "a local folder was not added");
            }
            if !report.skipped.is_empty() {
                let count = i64::try_from(report.skipped.len()).unwrap_or(i64::MAX);
                crate::toast::notify(kuro_i18n::tr_plural(kuro_i18n::MsgKey::local_folders_skipped, count, &[]));
            }
        }
        Err(error) => tracing::warn!(root = %root.display(), %error, "the Local folder could not be read; nothing was added or removed"),
    }
}

/// Adds to the library each title folder of the Local folder `root` that is not stored yet. `Err` when `root` cannot be
/// listed: nothing is added or removed then.
pub fn discover(db: &Arc<Db>, prefs: &PrefStore, kind: Kind, root: &Path, now_ms: i64) -> io::Result<Report> {
    let mut report = Report::default();
    for folder in list_dir(root)?.into_iter().filter(|e| e.is_dir && !is_hidden(&e.name)) {
        store_folder(db, prefs, kind, &folder, folder.name.clone(), Rejoin::No, now_ms, &mut report);
    }
    Ok(report)
}

/// Links `folder` where it is. A manga folder is one title; an anime folder is one series when it holds episode files,
/// and otherwise each of its folders is a series. `Err` when `folder` cannot be listed.
pub fn link(db: &Arc<Db>, prefs: &PrefStore, kind: Kind, folder: &Path, now_ms: i64) -> io::Result<Report> {
    let name = folder.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let picked = FsEntry { name, path: folder.to_path_buf(), is_dir: true, is_file: false, modified_ms: 0 };
    let children = list_dir(folder)?;
    let mut report = Report::default();
    let series: Vec<FsEntry> = if kind == Kind::Anime && !children.iter().any(|e| e.is_file && is_anime_episode(&e.name)) {
        children.into_iter().filter(|e| e.is_dir && !is_hidden(&e.name)).collect()
    } else {
        vec![picked]
    };
    for entry in &series {
        match entry.path.to_str() {
            Some(url) => store_folder(db, prefs, kind, entry, url.to_owned(), Rejoin::Yes, now_ms, &mut report),
            None => report.skipped.push((entry.name.clone(), "the folder's path is not valid text".to_owned())),
        }
    }
    Ok(report)
}

/// Stores one title folder under `url`, when it holds something of its kind: a folder nothing in it can be read from
/// (empty, unreadable, files of another kind) is passed over until it has.
#[allow(clippy::too_many_arguments)]
fn store_folder(db: &Arc<Db>, prefs: &PrefStore, kind: Kind, folder: &FsEntry, url: String, rejoin: Rejoin, now_ms: i64, report: &mut Report) {
    match has_content(kind, &folder.path) {
        Ok(true) => {}
        Ok(false) => return,
        Err(error) => return report.skipped.push((folder.name.clone(), error.to_string())),
    }
    match store(db, prefs, kind, url, folder.name.clone(), rejoin, now_ms) {
        Ok(true) => report.added.push(folder.name.clone()),
        Ok(false) => {}
        Err(error) => report.skipped.push((folder.name.clone(), error.to_string())),
    }
}

/// Whether a folder holds anything the local source reads: for manga a chapter (a folder, an archive or an EPUB), for
/// anime an episode file or a season folder.
fn has_content(kind: Kind, folder: &Path) -> io::Result<bool> {
    Ok(list_dir(folder)?.iter().filter(|e| !is_hidden(&e.name)).any(|e| match kind {
        Kind::Manga => format_of(&e.path, e.is_dir).is_some(),
        Kind::Anime => e.is_dir || (e.is_file && is_anime_episode(&e.name)),
    }))
}

/// Stores the title and puts it in the library. A new one is not initialized, so its details are read on first open.
/// `true` when it joined the library now.
fn store(db: &Arc<Db>, prefs: &PrefStore, kind: Kind, url: String, title: String, rejoin: Rejoin, now_ms: i64) -> DataResult<bool> {
    let repo = EntryRepo::new(Arc::clone(db));
    let row = Entry { source: LOCAL_SOURCE_ID, url, title, date_added: now_ms, ..Entry::create(kind) };
    let id = match repo.lookup_or_insert(&row)? {
        LookupOrInsert::Inserted(id) => id,
        LookupOrInsert::Found(found) if rejoin == Rejoin::Yes && !found.favorite => found.id,
        LookupOrInsert::Found(_) => return Ok(false),
    };
    entry_ops::add_to_library(&repo, prefs, id, &[], now_ms)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// A database in a temporary folder, opened the way the app opens it.
    fn db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("open db");
        (dir, Arc::new(Db::new(&path, writer).expect("db handle")))
    }

    /// A folder tree with these files (their folders are made).
    fn tree(files: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temp dir");
        for file in files {
            let path = root.path().join(file);
            fs::create_dir_all(path.parent().expect("parent")).expect("folders");
            fs::write(&path, b"x").expect("file");
        }
        root
    }

    fn favorites(db: &Arc<Db>, kind: Kind) -> Vec<(String, String)> {
        let mut rows: Vec<(String, String)> =
            EntryRepo::new(Arc::clone(db)).favorites_by_source(kind, LOCAL_SOURCE_ID).expect("rows").into_iter().map(|e| (e.title, e.url)).collect();
        rows.sort();
        rows
    }

    #[test]
    fn the_local_folder_adds_each_title_with_content_once() {
        let (_d, db) = db();
        let prefs = PrefStore::in_memory();
        let root = tree(&["Frieren/Frieren - 01.mkv", "Dandadan/S1/ep.mkv", "Notes/readme.txt", ".hidden/ep.mkv", "loose.mkv"]);
        fs::create_dir(root.path().join("Empty")).expect("empty");
        let report = discover(&db, &prefs, Kind::Anime, root.path(), 1).expect("scan");
        assert_eq!(report.added, ["Dandadan", "Frieren"], "only folders the source can read: episodes or seasons");
        assert_eq!(favorites(&db, Kind::Anime), [("Dandadan".into(), "Dandadan".into()), ("Frieren".into(), "Frieren".into())]);
        assert!(discover(&db, &prefs, Kind::Anime, root.path(), 2).expect("again").added.is_empty(), "a second scan adds nothing");
    }

    #[test]
    fn a_title_taken_out_of_the_library_stays_out_until_it_is_linked() {
        let (_d, db) = db();
        let prefs = PrefStore::in_memory();
        let root = tree(&["Berserk/Vol 1.cbz"]);
        discover(&db, &prefs, Kind::Manga, root.path(), 1).expect("scan");
        let repo = EntryRepo::new(Arc::clone(&db));
        let id = repo.favorites_by_source(Kind::Manga, LOCAL_SOURCE_ID).expect("rows")[0].id;
        entry_ops::set_favorite(&repo, id, false, 2).expect("remove");
        assert!(discover(&db, &prefs, Kind::Manga, root.path(), 2).expect("scan").added.is_empty());
        assert!(favorites(&db, Kind::Manga).is_empty(), "the scan does not bring it back");
    }

    #[test]
    fn a_missing_local_folder_is_an_error_not_an_empty_library() {
        let (_d, db) = db();
        let gone = tempfile::tempdir().expect("temp dir").path().join("gone");
        assert!(discover(&db, &PrefStore::in_memory(), Kind::Manga, &gone, 1).is_err());
    }

    #[test]
    fn a_linked_folder_is_stored_by_its_absolute_path() {
        let (_d, db) = db();
        let prefs = PrefStore::in_memory();
        let shelf = tree(&["Berserk/Vol 1.cbz", "Series/A/A - 01.mkv", "Series/B/B - 01.mkv", "One/One - 01.mkv"]);
        let manga = link(&db, &prefs, Kind::Manga, &shelf.path().join("Berserk"), 1).expect("link");
        assert_eq!(manga.added, ["Berserk"]);
        let url = shelf.path().join("Berserk").to_string_lossy().into_owned();
        assert_eq!(favorites(&db, Kind::Manga), [("Berserk".into(), url)]);
        let series = link(&db, &prefs, Kind::Anime, &shelf.path().join("Series"), 1).expect("link");
        assert_eq!(series.added, ["A", "B"], "a folder of series adds each one");
        let one = link(&db, &prefs, Kind::Anime, &shelf.path().join("One"), 1).expect("link");
        assert_eq!(one.added, ["One"], "a folder with episode files is one series");
    }
}
