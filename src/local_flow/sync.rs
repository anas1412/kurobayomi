//! Writes a scanned folder into the manga database (plan area 43). The folder is one entry under the local source id,
//! and each chapter is matched by its url. A rescan renames and renumbers the chapters it knows and leaves their read
//! state alone; a chapter removed from disk stays in the list (ponytail: no removal pass yet).

use std::sync::Arc;

use kuro_data::chapter::{Chapter, ChapterRepo, ChapterUpdate};
use kuro_data::manga::{Manga, MangaRepo, MangaUpdate};
use kuro_data::{DataResult, LookupOrInsert};
use kuro_db::Db;
use kuro_local::LOCAL_SOURCE_ID;

use super::scan::ScannedManga;
use crate::entry_core::store::DbStore;

/// What one sync did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Synced {
    pub manga_id: i64,
    pub new_chapters: usize,
    pub renamed_chapters: usize,
}

/// Inserts or updates the entry and its chapters, then puts the entry in the library when it is not there yet.
pub fn sync(db: &Arc<Db>, scanned: &ScannedManga, now_ms: i64) -> DataResult<Synced> {
    let url = scanned.path.to_string_lossy().into_owned();
    let manga = MangaRepo::new(Arc::clone(db));
    let row = Manga {
        source: LOCAL_SOURCE_ID,
        url,
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
    let mut fresh = Vec::new();
    let mut renamed = Vec::new();
    for (order, found) in scanned.chapters.iter().enumerate() {
        let url = found.path.to_string_lossy().into_owned();
        match known.iter().find(|k| k.url == url) {
            Some(k) => {
                if k.name != found.name || k.date_upload != found.date_upload || k.chapter_number != found.chapter_number {
                    renamed.push(ChapterUpdate {
                        name: Some(found.name.clone()),
                        date_upload: Some(found.date_upload),
                        chapter_number: Some(found.chapter_number),
                        ..ChapterUpdate::new(k.id)
                    });
                }
            }
            None => fresh.push(Chapter {
                manga_id,
                url,
                name: found.name.clone(),
                date_upload: found.date_upload,
                chapter_number: found.chapter_number,
                date_fetch: now_ms,
                source_order: order as i64,
                ..Chapter::create()
            }),
        }
    }
    if !fresh.is_empty() {
        chapters.add_all(&fresh)?;
    }
    if !renamed.is_empty() {
        chapters.update_all(&renamed)?;
    }
    Ok(Synced { manga_id, new_chapters: fresh.len(), renamed_chapters: renamed.len() })
}
