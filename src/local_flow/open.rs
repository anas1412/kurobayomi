//! Which local manga entry a screen asks for. The entry screens name an entry by its title (see `screens::open_entry`),
//! so the title is resolved over the local library rows only; any other title keeps the caller's own page. The id of
//! the entry that was opened is remembered, so the reader can tell two local folders with the same name apart.

use std::cell::Cell;
use std::sync::Arc;

use kuro_data::manga::MangaRepo;
use kuro_db::Db;
use kuro_local::LOCAL_SOURCE_ID;

thread_local! {
    static CURRENT: Cell<Option<i64>> = const { Cell::new(None) };
}

/// The id of the first local library entry with this title.
pub fn local_id_with_title(db: &Arc<Db>, title: &str) -> Option<i64> {
    let found = MangaRepo::new(Arc::clone(db)).favorites_by_source(LOCAL_SOURCE_ID).ok()?;
    found.into_iter().find(|m| m.title == title).map(|m| m.id)
}

/// Opens the database-backed entry page of a local entry by its id, and remembers the id for the reader.
pub fn entry(manga_id: i64) {
    CURRENT.with(|c| c.set(Some(manga_id)));
    crate::entry_core::route::open(manga_id);
}

/// Opens the entry page of the local entry titled `title`. `false` when no local entry has that title.
pub fn entry_for_title(title: &str) -> bool {
    if !crate::data::is_installed() {
        return false;
    }
    match local_id_with_title(&crate::data::app().db(kuro_domain::kind::EntryKind::Manga), title) {
        Some(id) => {
            entry(id);
            true
        }
        None => false,
    }
}

/// The local entry whose page was opened last, when it is still the one the reader is asked about.
pub fn current() -> Option<i64> {
    CURRENT.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_flow::tests::{manga_db, series};
    use crate::local_flow::{scan::scan, sync::sync};

    #[test]
    fn only_a_local_library_title_resolves() {
        let root = tempfile::tempdir().expect("temp dir");
        let db = manga_db(root.path());
        let done = sync(&db, &scan(&series(root.path())).expect("scan"), 1_000).expect("sync");
        assert_eq!(local_id_with_title(&db, "Series"), Some(done.manga_id));
        assert_eq!(local_id_with_title(&db, "Title 1"), None);
    }
}
