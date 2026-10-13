//! Library read path: one `LibraryRow` per (favourite entry, category link). The SQL is `kuro_db::tables::library` and the
//! row mapping is `kuro-data`'s `EntryRepo::library`; this file only turns the entries into the page-builder row.

use kuro_platform::kind::Kind;
use std::sync::Arc;

use kuro_data::entry::EntryRepo;
use kuro_db::handle::Db;

use super::model::LibraryRow;
use crate::error::DomainError;

/// Reads the library rows of either kind.
pub struct LibraryRepo {
    entries: EntryRepo,
}

impl LibraryRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { entries: EntryRepo::new(db) }
    }

    /// Every favourite entry, once per category it is in (an entry in no category has `category = 0`).
    /// A database error is returned as `Err`; NULL aggregates are already coalesced in the view.
    pub fn all(&self, kind: Kind) -> Result<Vec<LibraryRow>, DomainError> {
        Ok(self.entries.library(kind)?.iter().map(LibraryRow::from_entry).collect())
    }
}

#[cfg(test)]
mod tests {
    use kuro_data::entry::Entry;

    use super::*;
    use crate::testdb::test_db;
    use kuro_testkit::insert_category;

    fn favourite(kind: Kind, title: &str) -> Entry {
        Entry { favorite: true, source: 7, url: format!("u/{title}"), title: title.into(), ..Entry::create(kind) }
    }

    /// One item row, inserted through the writer. `scanlator` may be excluded.
    fn chapter(db: &Db, manga: i64, scanlator: Option<&str>, read: bool) {
        db.write(|tx| {
            tx.execute(
                "INSERT INTO items(entry_id, url, name, scanlator, done, bookmark, progress, number, source_order, date_fetch, date_upload) \
                 VALUES (?1, ?2, 'c', ?3, ?4, 0, 0, 1, 0, 10, 20)",
                (manga, format!("c{manga}{scanlator:?}{read}"), scanlator, read),
            )?;
            Ok(())
        })
        .unwrap();
    }

    fn exclude(db: &Db, manga: i64, scanlator: &str) {
        db.write(|tx| {
            tx.execute("INSERT INTO excluded_scanlators(entry_id, scanlator) VALUES (?1, ?2)", (manga, scanlator))?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn an_entry_in_three_categories_gives_three_rows_and_no_category_gives_zero() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let in_three = repo.insert(&favourite(kind, "three")).unwrap();
            let none = repo.insert(&favourite(kind, "none")).unwrap();
            let cats: Vec<i64> = (0..3).map(|n| insert_category(&db, kind, &format!("c{n}"))).collect();
            repo.set_categories(in_three, &cats).unwrap();
            let rows = LibraryRepo::new(db.clone()).all(kind).unwrap();
            assert_eq!(rows.iter().filter(|r| r.id == in_three).count(), 3, "{kind}");
            let lone: Vec<&LibraryRow> = rows.iter().filter(|r| r.id == none).collect();
            assert_eq!((lone.len(), lone[0].category), (1, 0));
        }
    }

    #[test]
    fn counts_exclude_chapters_of_excluded_scanlators_and_derive_pending() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let id = repo.insert(&favourite(Kind::Manga, "counted")).unwrap();
        chapter(&db, id, Some("A"), true);
        chapter(&db, id, Some("A"), false);
        chapter(&db, id, Some("B"), true);
        chapter(&db, id, None, false);
        exclude(&db, id, "B");
        let row = LibraryRepo::new(db).all(Kind::Manga).unwrap().remove(0);
        assert_eq!((row.total, row.done, row.pending()), (3, 1, 2));
        assert!(row.has_started());
        assert_eq!(row.fillermarks, 0, "manga has no fillermarks");
    }

    #[test]
    fn an_empty_library_gives_no_rows() {
        let (_d, db) = test_db();
        for kind in Kind::ALL {
            assert!(LibraryRepo::new(db.clone()).all(kind).unwrap().is_empty());
        }
    }
}
