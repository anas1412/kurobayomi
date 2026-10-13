//! The live sources as the download queue asks for them: the name of a source for a saved row (`SourceDirectory`), the
//! manga page source of a running chapter (`ChapterSources`) and the enqueue request of an entry's units. The name comes
//! from the live source when it is installed, else from the stub table, which keeps the names of sources that were listed
//! before. A source with neither has no name, and its rows wait for it (docs/design/unified-downloads.md D5).

use std::sync::Arc;

use kuro_data::entry::EntryRepo;
use kuro_data::item::ItemRepo;
use kuro_data::source::StubSourceRepo;
use kuro_db::handle::Db;
use kuro_downloads::next::queue::{EnqueueRequest, Place};
use kuro_downloads::next::runner::{ChapterSources, SourceDirectory};
use kuro_downloads::next::unit::{Availability, EntryRef, SourceKey, UnitRef};
use kuro_downloads::source::ChapterSource;
use kuro_platform::kind::Kind;
use kuro_source::local::LOCAL_SOURCE_ID;

use crate::downloads::manga_io::LiveChapterSource;

/// Cheap to clone; holds only the database for the stub rows.
#[derive(Clone)]
pub struct AppDirectory {
    db: Arc<Db>,
}

impl AppDirectory {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The entry and the name of its source: `None` when the entry is gone, is of the other kind or its source has no name.
    pub fn entry_context(&self, kind: Kind, entry_id: i64) -> Option<(EntryRef, SourceKey)> {
        let entry = EntryRepo::new(Arc::clone(&self.db)).by_id(entry_id).ok().filter(|e| e.kind == kind)?;
        let source = self.source_key(kind, entry.source)?;
        Some((EntryRef::from(&entry), source))
    }

    /// The listed units of an entry whose ids are in `unit_ids`, in the order of `unit_ids`.
    pub fn units(&self, entry_id: i64, unit_ids: &[i64]) -> Vec<UnitRef> {
        let items = ItemRepo::new(Arc::clone(&self.db)).by_entry_id(entry_id, false).unwrap_or_default();
        // in the caller's order: it is the order the units queue in
        unit_ids.iter().filter_map(|id| items.iter().find(|item| item.id == *id)).map(UnitRef::from).collect()
    }

    /// The request that queues `unit_ids` of entry `entry_id`: `None` when the entry or the source name is unknown, or no
    /// listed unit matches. `skip_fillermarked` is set only by the automatic download after a refresh (D6).
    pub fn enqueue_request(
        &self,
        kind: Kind,
        entry_id: i64,
        unit_ids: &[i64],
        place: Place,
        auto_start: bool,
        skip_fillermarked: bool,
    ) -> Option<EnqueueRequest> {
        let (entry, source) = self.entry_context(kind, entry_id)?;
        let units = self.units(entry_id, unit_ids);
        (!units.is_empty()).then_some(EnqueueRequest { kind, entry, source, units, place, auto_start, skip_fillermarked })
    }
}

impl SourceDirectory for AppDirectory {
    fn source_key(&self, kind: Kind, source_id: i64) -> Option<SourceKey> {
        if source_id == LOCAL_SOURCE_ID {
            return Some(SourceKey { id: source_id, name: "Local source".to_owned(), lang: "other".to_owned(), unmetered: true, availability: Availability::Local });
        }
        let catalogue = crate::browse::directory::current(kind).catalogue(source_id);
        let live = match kind {
            Kind::Manga => catalogue.and_then(|c| c.manga()).map(|s| (s.name().to_owned(), s.lang().to_owned(), s.is_unmetered())),
            Kind::Anime => catalogue.and_then(|c| c.anime()).map(|s| (s.name().to_owned(), s.lang().to_owned(), s.is_unmetered())),
        };
        if let Some((name, lang, unmetered)) = live {
            return Some(SourceKey { id: source_id, name, lang, unmetered, availability: Availability::Live });
        }
        let stub = StubSourceRepo::new(Arc::clone(&self.db), kind).get(source_id).ok()??;
        (!stub.is_invalid()).then_some(SourceKey { id: source_id, name: stub.name, lang: stub.lang, unmetered: false, availability: Availability::Stub })
    }
}

impl ChapterSources for AppDirectory {
    fn chapter_source(&self, source_id: i64) -> Option<Arc<dyn ChapterSource>> {
        let source = crate::browse::directory::current(Kind::Manga).catalogue(source_id)?.manga()?;
        Some(Arc::new(LiveChapterSource::new(source)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::entry::Entry;
    use kuro_data::item::Item;
    use kuro_db::schema_unified;

    fn test_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let path = schema_unified::db_path(dir.path());
        let writer = schema_unified::open_file(&path).unwrap();
        (dir, Arc::new(Db::new(&path, writer).unwrap()))
    }

    #[test]
    fn a_stub_row_names_a_source_that_is_not_installed_for_both_kinds() {
        let (_dir, db) = test_db();
        for kind in Kind::ALL {
            StubSourceRepo::new(Arc::clone(&db), kind).upsert(77, "en", "Site").unwrap();
            let key = AppDirectory::new(Arc::clone(&db)).source_key(kind, 77).expect("named by the stub");
            assert_eq!((key.name.as_str(), key.lang.as_str(), key.availability), ("Site", "en", Availability::Stub));
        }
    }

    #[test]
    fn a_source_with_no_live_source_and_no_stub_name_has_no_key() {
        let (_dir, db) = test_db();
        for kind in Kind::ALL {
            assert_eq!(AppDirectory::new(Arc::clone(&db)).source_key(kind, 88), None);
        }
        StubSourceRepo::new(Arc::clone(&db), Kind::Anime).upsert(89, "", "").unwrap();
        assert_eq!(AppDirectory::new(db).source_key(Kind::Anime, 89), None, "a stub with no name is no name");
    }

    #[test]
    fn the_local_source_is_local_and_unmetered() {
        let (_dir, db) = test_db();
        let key = AppDirectory::new(db).source_key(Kind::Manga, LOCAL_SOURCE_ID).unwrap();
        assert_eq!((key.availability, key.unmetered), (Availability::Local, true));
    }

    #[test]
    fn the_request_carries_the_listed_units_of_the_entry() {
        let (_dir, db) = test_db();
        StubSourceRepo::new(Arc::clone(&db), Kind::Anime).upsert(77, "en", "Site").unwrap();
        let anime = Entry { url: "a".into(), title: "A".into(), source: 77, status: 1, ..Entry::create(Kind::Anime) };
        let entry_id = EntryRepo::new(Arc::clone(&db)).insert(&anime).unwrap();
        let episodes: Vec<Item> = (1..=3).map(|n| Item { entry_id, url: format!("e{n}"), name: format!("Episode {n}"), ..Item::create(Kind::Anime) }).collect();
        let added = ItemRepo::new(Arc::clone(&db)).add_all(&episodes).unwrap();
        let directory = AppDirectory::new(db);
        let ids = [added[0].id, added[2].id, 9999];
        let request = directory.enqueue_request(Kind::Anime, entry_id, &ids, Place::Back, false, true).expect("a request");
        assert_eq!(request.units.iter().map(|u| u.id).collect::<Vec<_>>(), vec![added[0].id, added[2].id]);
        assert_eq!((request.entry.id, request.source.id, request.skip_fillermarked), (entry_id, 77, true));
        assert!(directory.enqueue_request(Kind::Manga, entry_id, &ids, Place::Back, false, false).is_none(), "the other kind");
        assert!(directory.enqueue_request(Kind::Anime, entry_id, &[9999], Place::Back, false, false).is_none(), "no unit matches");
    }
}
