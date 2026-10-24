//! The app's side of local titles. Reading the files is the local source's (kuro-local, listed in the source directory
//! like any source): its chapters, episodes, seasons, details and covers reach the library through the library update
//! pass. Here: the Local folders and the one url-to-path rule (`path_of`), `titles` adds the Local folder's new titles and
//! the linked folders to the library, `open` finds the local entry or episode a screen asks for, `reader` opens a local
//! chapter, `cover` draws a local cover and `ui` holds the library menu's actions.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

use kuro_data::DataResult;
use kuro_data::entry::EntryRepo;
use kuro_db::Db;
use kuro_platform::kind::Kind;
use kuro_local::LOCAL_SOURCE_ID;

pub mod cover;
pub mod open;
pub mod reader;
pub mod titles;
pub mod ui;

#[cfg(test)]
mod tests;

/// The Local folders, `[manga, anime]`, as [`load_roots`] last read them. Kept for every thread: the library update and the
/// cover makers run off the GTK thread, where the app data cannot be read.
static ROOTS: RwLock<[Option<PathBuf>; 2]> = RwLock::new([None, None]);

/// Reads the Local folders (`Local/Manga`, `Local/Anime` under the storage folder, created on first use) and keeps them
/// for [`local_root`]. Call on the GTK thread once the app data is open, and again when the storage folder changes. A
/// folder that cannot be made is `None`: nothing local is read from it then.
pub fn load_roots() {
    let storage = crate::data::is_installed().then(|| crate::settings_ui::storage::manager(crate::data::app().base.store())).flatten();
    let roots = match storage {
        Some(storage) => [storage.local_manga_dir().ok(), storage.local_anime_dir().ok()],
        None => [None, None],
    };
    *ROOTS.write().unwrap_or_else(PoisonError::into_inner) = roots;
}

/// The Local folder of `kind`, from any thread. `None` before [`load_roots`] or while the storage folder is missing.
pub fn local_root(kind: Kind) -> Option<PathBuf> {
    ROOTS.read().unwrap_or_else(PoisonError::into_inner)[usize::from(kind == Kind::Anime)].clone()
}

/// The file or folder a local url names: a title (or one of its files) inside the Local folder of `kind` for a relative
/// url, a linked folder for an absolute one. The app's one way from a local url to a path (kuro-local's `resolve`, the
/// same rule the local source follows). `None` for an unsafe url, or a relative one while there is no Local folder.
pub fn path_of(kind: Kind, url: &str) -> Option<PathBuf> {
    // an absolute url (a linked folder) carries its own root
    let root = if Path::new(url).is_absolute() { PathBuf::new() } else { local_root(kind)? };
    kuro_local::fs::resolve(&root, url)
}

/// One library entry that came from a local folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalRow {
    pub id: i64,
    pub title: String,
    /// The absolute path of the folder.
    pub url: String,
}

/// The library entries of `kind` that came from a local folder.
pub fn local_rows(db: &Arc<Db>, kind: Kind) -> DataResult<Vec<LocalRow>> {
    Ok(EntryRepo::new(Arc::clone(db))
        .favorites_by_source(kind, LOCAL_SOURCE_ID)?
        .into_iter()
        .map(|e| LocalRow { id: e.id, title: e.title, url: e.url })
        .collect())
}
