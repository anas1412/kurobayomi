//! Library read path: one `LibraryRow` per (favourite entry, category link). The SQL lives in `kuro-db`
//! (`libraryView`, `animelibView`) and the row mapping in `kuro-data`; this file only joins them.

use kuro_platform::kind::Kind;
use std::sync::Arc;

use kuro_db::handle::Db;

use super::model::LibraryRow;
use crate::error::DomainError;

/// Reads the library rows of either kind.
pub struct LibraryRepo {
    db: Arc<Db>,
}

impl LibraryRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Every favourite entry, once per category it is in (an entry in no category has `category = 0`).
    /// A database error is returned as `Err`; NULL aggregates are already coalesced in the view.
    pub fn all(&self, kind: Kind) -> Result<Vec<LibraryRow>, DomainError> {
        Ok(match kind {
            Kind::Manga => kuro_data::manga::MangaRepo::new(self.db.clone())
                .library()?
                .iter()
                .map(LibraryRow::from_manga)
                .collect(),
            Kind::Anime => kuro_data::anime::AnimeRepo::new(self.db.clone())
                .library()?
                .iter()
                .map(LibraryRow::from_anime)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use kuro_data::anime::Anime;
    use kuro_data::manga::{Manga, MangaRepo};

    use super::*;
    use crate::testdb::{anime_db, manga_db};

    fn favourite(title: &str) -> Manga {
        Manga { favorite: true, source: 7, url: format!("u/{title}"), title: title.into(), ..Manga::create() }
    }

    /// One chapter row, inserted through the writer. `scanlator` may be excluded.
    fn chapter(db: &Db, manga: i64, scanlator: Option<&str>, read: bool) {
        db.write(|tx| {
            tx.execute(
                "INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload) \
                 VALUES (?1, ?2, 'c', ?3, ?4, 0, 0, 1, 0, 10, 20)",
                (manga, format!("c{manga}{scanlator:?}{read}"), scanlator, read),
            )?;
            Ok(())
        })
        .unwrap();
    }

    fn exclude(db: &Db, manga: i64, scanlator: &str) {
        db.write(|tx| {
            tx.execute("INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (?1, ?2)", (manga, scanlator))?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn an_entry_in_three_categories_gives_three_rows_and_no_category_gives_zero() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let in_three = repo.insert(&favourite("three")).unwrap();
        let none = repo.insert(&favourite("none")).unwrap();
        let cats: Vec<i64> = (0..3)
            .map(|n| kuro_data::category::create_manga_category(&db, &format!("c{n}"), 0).unwrap())
            .collect();
        repo.set_categories(in_three, &cats).unwrap();
        let rows = LibraryRepo::new(db).all(Kind::Manga).unwrap();
        assert_eq!(rows.iter().filter(|r| r.id == in_three).count(), 3);
        let lone: Vec<&LibraryRow> = rows.iter().filter(|r| r.id == none).collect();
        assert_eq!((lone.len(), lone[0].category), (1, 0));
    }

    #[test]
    fn counts_exclude_chapters_of_excluded_scanlators_and_derive_pending() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&favourite("counted")).unwrap();
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
    fn an_empty_library_gives_no_rows_and_anime_rows_follow_the_same_rule() {
        let (_d, db) = manga_db();
        assert!(LibraryRepo::new(db).all(Kind::Manga).unwrap().is_empty());
        let (_d2, adb) = anime_db();
        let anime = kuro_data::anime::AnimeRepo::new(adb.clone());
        let id = anime
            .insert(&Anime { favorite: true, source: 3, url: "a".into(), title: "A".into(), ..Anime::create() })
            .unwrap();
        let c1 = kuro_data::category::create_anime_category(&adb, "one", 0).unwrap();
        let c2 = kuro_data::category::create_anime_category(&adb, "two", 0).unwrap();
        anime.set_categories(id, &[c1, c2]).unwrap();
        let rows = LibraryRepo::new(adb).all(Kind::Anime).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.id == id && r.done == 0 && r.total == 0));
    }
}
