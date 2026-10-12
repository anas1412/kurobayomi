//! Local folders of both kinds (plan areas 14, 43 and 45, wire-local-anime). A manga folder is one entry whose
//! subfolders, archives and EPUB files are its chapters; an anime folder holds one folder per series whose video files
//! are its episodes. `scan` reads a folder with kuro-local, `sync` stores it under the built-in local source id, `open`
//! finds the local entry or episode a screen asks for, `reader` opens a local chapter, `cover` draws a local cover and
//! `ui` holds the library menu's add and rescan. Scan and sync take no GTK types, so their tests run without a display.

use std::sync::Arc;

use kuro_data::DataResult;
use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;
use kuro_db::Db;
use kuro_platform::kind::Kind;
use kuro_local::LOCAL_SOURCE_ID;

pub mod cover;
pub mod open;
pub mod reader;
pub mod scan;
pub mod sync;
pub mod ui;

#[cfg(test)]
mod tests;

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
    Ok(match kind {
        Kind::Manga => MangaRepo::new(Arc::clone(db))
            .favorites_by_source(LOCAL_SOURCE_ID)?
            .into_iter()
            .map(|m| LocalRow { id: m.id, title: m.title, url: m.url })
            .collect(),
        Kind::Anime => AnimeRepo::new(Arc::clone(db))
            .favorites_by_source(LOCAL_SOURCE_ID)?
            .into_iter()
            .map(|a| LocalRow { id: a.id, title: a.title, url: a.url })
            .collect(),
    })
}
